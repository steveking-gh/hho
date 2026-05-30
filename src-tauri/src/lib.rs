// Tauri application entry point.
// Responsibilities: native menu, user-config persistence (TOML in home dir),
// CSV reading, layout persistence, window-size persistence, and IPC commands.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri::menu::{IsMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};

// ── Constants ─────────────────────────────────────────────────────────────────

const MAX_RECENTS:    usize = 5;
const CONFIG_FILE:    &str  = "hho_user_config.toml";
const MENU_EVENT:     &str  = "hho-menu";

// Default layout dimensions (px, logical).
const DEFAULT_LEFT_W:  f32 = 200.0;
const DEFAULT_RIGHT_W: f32 = 200.0;
const DEFAULT_BOT_H:   f32 = 200.0;
const DEFAULT_DBG_H:   f32 = 150.0;

// Default window dimensions (logical px).
const DEFAULT_WIN_W: f64 = 1024.0;
const DEFAULT_WIN_H: f64 =  700.0;

// ── User configuration ────────────────────────────────────────────────────────

/// Persisted user preferences written to ~/hho_user_config.toml.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
struct UserConfig {
    /// MRU list of CSV file paths (most-recent first, max MAX_RECENTS entries).
    #[serde(default)]
    recent_files: Vec<String>,

    /// Directory last used in a file-open dialog; seeds the next dialog.
    #[serde(skip_serializing_if = "Option::is_none")]
    last_opened_dir: Option<String>,

    // ── Pane layout ───────────────────────────────────────────────────────────
    #[serde(skip_serializing_if = "Option::is_none")]
    left_width: Option<f32>,   // Joint pane width (px)

    #[serde(skip_serializing_if = "Option::is_none")]
    right_width: Option<f32>,  // Mine pane width (px)

    #[serde(skip_serializing_if = "Option::is_none")]
    bottom_h: Option<f32>,     // Ignored pane height (px)

    #[serde(skip_serializing_if = "Option::is_none")]
    debug_h: Option<f32>,      // Debug panel height (px)

    // ── Window geometry ───────────────────────────────────────────────────────
    #[serde(skip_serializing_if = "Option::is_none")]
    window_width: Option<f64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    window_height: Option<f64>,
}

// ── Config file helpers ───────────────────────────────────────────────────────

fn config_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(CONFIG_FILE)
}

/// Load config; auto-create a default file if absent.
/// Drops recent-file entries whose paths no longer exist.
fn load_config() -> UserConfig {
    let path = config_path();
    if !path.exists() {
        let default = UserConfig::default();
        save_config(&default);
        return default;
    }
    let raw = match std::fs::read_to_string(&path) {
        Ok(s)  => s,
        Err(_) => return UserConfig::default(),
    };
    let mut cfg: UserConfig = toml::from_str(&raw).unwrap_or_default();
    cfg.recent_files.retain(|p| Path::new(p).exists());
    cfg.recent_files.truncate(MAX_RECENTS);
    cfg
}

fn save_config(config: &UserConfig) {
    let path = config_path();
    match toml::to_string_pretty(config) {
        Ok(s)  => { let _ = std::fs::write(&path, s); }
        Err(e) => eprintln!("hho: config serialize error: {e}"),
    }
}

// ── Managed state ─────────────────────────────────────────────────────────────

struct ConfigState {
    config: Mutex<UserConfig>,
}

// ── IPC types ─────────────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CsvFile {
    pub path: String,
    pub rows: Vec<String>,
}

/// Layout dimensions sent to the frontend on startup.
#[derive(Serialize)]
struct LayoutConfig {
    left_width:  f32,
    right_width: f32,
    bottom_h:    f32,
    debug_h:     f32,
}

#[derive(Serialize, Clone, Debug)]
#[serde(tag = "action", rename_all = "kebab-case")]
enum MenuAction {
    Open,
    OpenRecent { path: String },
}

// ── CSV helpers ───────────────────────────────────────────────────────────────

fn read_csv_rows(path: &Path) -> Result<Vec<String>, String> {
    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .from_path(path)
        .map_err(|e| format!("csv open: {e}"))?;

    let rows = rdr
        .records()
        .filter_map(|r| r.ok())
        .map(|record| {
            record.iter()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join(" │ ")
        })
        .filter(|l| !l.is_empty())
        .collect();
    Ok(rows)
}

// ── Recent-file helpers ───────────────────────────────────────────────────────

/// Prepend `new_path`, dedup, cap to MAX_RECENTS.
pub fn push_recent(files: &mut Vec<String>, new_path: String) {
    files.retain(|p| p != &new_path);
    files.insert(0, new_path);
    files.truncate(MAX_RECENTS);
}

// ── Menu builder ──────────────────────────────────────────────────────────────

fn build_menu(app: &AppHandle, recents: &[String]) -> tauri::Result<Menu<tauri::Wry>> {
    let open = MenuItem::with_id(app, "open", "Open...", true, Some("CmdOrCtrl+O"))?;
    let sep  = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, "quit", "Quit",    true, Some("CmdOrCtrl+Q"))?;
    let sub  = build_recent_submenu(app, recents)?;
    let file = Submenu::with_items(
        app, "File", true,
        &[&open, &sep, &sub, &PredefinedMenuItem::separator(app)?, &quit],
    )?;
    Menu::with_items(app, &[&file])
}

fn build_recent_submenu(
    app:     &AppHandle,
    recents: &[String],
) -> tauri::Result<Submenu<tauri::Wry>> {
    if recents.is_empty() {
        let none = MenuItem::with_id(app, "no-recents", "No Recent Files", false, None::<&str>)?;
        return Submenu::with_items(app, "Open Recent", true, &[&none]);
    }
    let items: Vec<MenuItem<tauri::Wry>> = recents.iter().enumerate()
        .filter_map(|(i, p)| {
            let name = Path::new(p).file_name()?.to_string_lossy().into_owned();
            MenuItem::with_id(app, format!("recent-{i}"), name, true, None::<&str>).ok()
        })
        .collect();
    let refs: Vec<&dyn IsMenuItem<tauri::Wry>> =
        items.iter().map(|m| m as &dyn IsMenuItem<tauri::Wry>).collect();
    Submenu::with_items(app, "Open Recent", true, refs.as_slice())
}

// ── Shared open logic ─────────────────────────────────────────────────────────

fn finalize_open(
    app:   &AppHandle,
    state: &ConfigState,
    path:  PathBuf,
) -> Result<CsvFile, String> {
    let rows     = read_csv_rows(&path)?;
    let path_str = path.to_string_lossy().to_string();
    let dir_str  = path.parent().map(|p| p.to_string_lossy().to_string());

    let mut cfg = state.config.lock().unwrap();
    push_recent(&mut cfg.recent_files, path_str.clone());
    cfg.last_opened_dir = dir_str;
    save_config(&cfg);

    let menu = build_menu(app, &cfg.recent_files).map_err(|e| e.to_string())?;
    app.set_menu(menu).map_err(|e| e.to_string())?;
    Ok(CsvFile { path: path_str, rows })
}

// ── Tauri commands ────────────────────────────────────────────────────────────

#[tauri::command]
async fn pick_csv(
    app:   AppHandle,
    state: State<'_, ConfigState>,
) -> Result<Option<CsvFile>, String> {
    use tauri_plugin_dialog::DialogExt;

    let start_dir: Option<PathBuf> = {
        let cfg = state.config.lock().unwrap();
        cfg.last_opened_dir
            .as_deref()
            .map(PathBuf::from)
            .filter(|p| p.exists())
            .or_else(dirs::home_dir)
    };

    let mut builder = app.dialog().file().add_filter("CSV files", &["csv", "CSV"]);
    if let Some(dir) = start_dir {
        builder = builder.set_directory(dir);
    }

    let Some(fp) = builder.blocking_pick_file() else { return Ok(None) };
    let path = fp.as_path()
        .ok_or_else(|| "dialog returned a URL, not a file path".to_string())?
        .to_path_buf();
    finalize_open(&app, &state, path).map(Some)
}

#[tauri::command]
async fn open_csv(
    path:  String,
    app:   AppHandle,
    state: State<'_, ConfigState>,
) -> Result<Option<CsvFile>, String> {
    let pb = PathBuf::from(&path);
    if !pb.exists() { return Err(format!("file not found: {path}")); }
    finalize_open(&app, &state, pb).map(Some)
}

/// Return persisted layout dimensions; frontend applies them to size signals.
#[tauri::command]
fn get_layout(state: State<'_, ConfigState>) -> LayoutConfig {
    let cfg = state.config.lock().unwrap();
    LayoutConfig {
        left_width:  cfg.left_width.unwrap_or(DEFAULT_LEFT_W),
        right_width: cfg.right_width.unwrap_or(DEFAULT_RIGHT_W),
        bottom_h:    cfg.bottom_h.unwrap_or(DEFAULT_BOT_H),
        debug_h:     cfg.debug_h.unwrap_or(DEFAULT_DBG_H),
    }
}

/// Persist pane layout after a drag gesture ends.
#[tauri::command]
fn save_layout(
    left_width:  f32,
    right_width: f32,
    bottom_h:    f32,
    debug_h:     f32,
    state:       State<'_, ConfigState>,
) {
    let mut cfg = state.config.lock().unwrap();
    cfg.left_width  = Some(left_width);
    cfg.right_width = Some(right_width);
    cfg.bottom_h    = Some(bottom_h);
    cfg.debug_h     = Some(debug_h);
    save_config(&cfg);
}

/// Persist window dimensions after the OS window is resized.
#[tauri::command]
fn save_window_size(
    width:  f64,
    height: f64,
    state:  State<'_, ConfigState>,
) {
    let mut cfg = state.config.lock().unwrap();
    cfg.window_width  = Some(width);
    cfg.window_height = Some(height);
    save_config(&cfg);
}

// ── App entry point ───────────────────────────────────────────────────────────

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let cfg = load_config();

            // Restore window geometry before the window becomes visible.
            let win_w = cfg.window_width.unwrap_or(DEFAULT_WIN_W);
            let win_h = cfg.window_height.unwrap_or(DEFAULT_WIN_H);
            if let Some(win) = app.get_webview_window("main") {
                let _ = win.set_size(tauri::LogicalSize::new(win_w, win_h));
            }

            let menu = build_menu(app.handle(), &cfg.recent_files)?;
            app.set_menu(menu)?;
            app.manage(ConfigState { config: Mutex::new(cfg) });

            let handle = app.handle().clone();
            app.on_menu_event(move |_app, event| {
                let id = event.id().as_ref();
                match id {
                    "open" => { let _ = handle.emit(MENU_EVENT, MenuAction::Open); }
                    "quit" => handle.exit(0),
                    id if id.starts_with("recent-") => {
                        if let Ok(idx) = id["recent-".len()..].parse::<usize>() {
                            let state = handle.state::<ConfigState>();
                            let cfg   = state.config.lock().unwrap();
                            if let Some(path) = cfg.recent_files.get(idx) {
                                let _ = handle.emit(
                                    MENU_EVENT,
                                    MenuAction::OpenRecent { path: path.clone() },
                                );
                            }
                        }
                    }
                    _ => {}
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            pick_csv, open_csv,
            get_layout, save_layout, save_window_size,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application")
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_recent_prepends_new_entry() {
        let mut v = vec!["b.csv".to_string(), "c.csv".to_string()];
        push_recent(&mut v, "a.csv".to_string());
        assert_eq!(v[0], "a.csv");
    }

    #[test]
    fn push_recent_deduplicates_existing_entry() {
        let mut v: Vec<String> = ["a","b","c"].iter().map(|s| s.to_string()).collect();
        push_recent(&mut v, "b.csv".to_string());
        assert_eq!(v, vec!["b.csv", "a", "b", "c"]);
    }

    #[test]
    fn push_recent_caps_at_max_recents() {
        let mut v: Vec<String> = (0..MAX_RECENTS).map(|i| format!("{i}.csv")).collect();
        push_recent(&mut v, "new.csv".to_string());
        assert_eq!(v.len(), MAX_RECENTS);
        assert_eq!(v[0], "new.csv");
    }

    #[test]
    fn push_recent_on_empty_list() {
        let mut v = vec![];
        push_recent(&mut v, "only.csv".to_string());
        assert_eq!(v, vec!["only.csv"]);
    }

    #[test]
    fn config_roundtrips_through_toml() {
        let original = UserConfig {
            recent_files:    vec!["a.csv".into(), "b.csv".into()],
            last_opened_dir: Some("/home/user/docs".into()),
            left_width:      Some(250.0),
            right_width:     Some(180.0),
            bottom_h:        Some(220.0),
            debug_h:         Some(130.0),
            window_width:    Some(1200.0),
            window_height:   Some(800.0),
        };
        let toml_str  = toml::to_string_pretty(&original).unwrap();
        let recovered: UserConfig = toml::from_str(&toml_str).unwrap();
        assert_eq!(recovered.recent_files,    original.recent_files);
        assert_eq!(recovered.last_opened_dir, original.last_opened_dir);
        assert_eq!(recovered.left_width,      original.left_width);
        assert_eq!(recovered.window_width,    original.window_width);
    }

    #[test]
    fn config_none_fields_omitted_from_toml() {
        let cfg = UserConfig::default();
        let toml_str = toml::to_string_pretty(&cfg).unwrap();
        assert!(!toml_str.contains("last_opened_dir"));
        assert!(!toml_str.contains("left_width"));
        assert!(!toml_str.contains("window_width"));
    }

    #[test]
    fn csv_rows_join_columns_with_separator() {
        let dir  = std::env::temp_dir();
        let path = dir.join("hho_test.csv");
        std::fs::write(&path, "Alice,30,Engineer\nBob,,Manager\n\n").unwrap();
        let rows = read_csv_rows(&path).unwrap();
        assert_eq!(rows[0], "Alice │ 30 │ Engineer");
        assert_eq!(rows[1], "Bob │ Manager");
        assert_eq!(rows.len(), 2);
        std::fs::remove_file(&path).unwrap();
    }
}
