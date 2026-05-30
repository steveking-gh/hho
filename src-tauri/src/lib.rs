// Tauri application entry point.
// Responsibilities: native menu, user-config persistence (TOML in home dir),
// CSV reading, and IPC commands.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri::menu::{IsMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};

// ── Constants ─────────────────────────────────────────────────────────────────

const MAX_RECENTS:   usize = 5;
const CONFIG_FILE:   &str  = "hho_user_config.toml";
const MENU_EVENT:    &str  = "hho-menu";

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
}

// ── Config file helpers ───────────────────────────────────────────────────────

/// Resolve ~/hho_user_config.toml on Windows and Linux.
fn config_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(CONFIG_FILE)
}

/// Load config from disk; auto-create a default file if absent.
/// Drops entries for files that no longer exist on the filesystem.
fn load_config() -> UserConfig {
    let path = config_path();

    if !path.exists() {
        let default = UserConfig::default();
        save_config(&default); // Create the file immediately so users can inspect it.
        return default;
    }

    let raw = match std::fs::read_to_string(&path) {
        Ok(s)  => s,
        Err(_) => return UserConfig::default(),
    };

    let mut cfg: UserConfig = toml::from_str(&raw).unwrap_or_default();

    // Filter out paths that no longer exist so the menu stays clean.
    cfg.recent_files.retain(|p| Path::new(p).exists());
    cfg.recent_files.truncate(MAX_RECENTS);

    cfg
}

/// Serialize `config` to TOML and overwrite ~/hho_user_config.toml.
fn save_config(config: &UserConfig) {
    let path = config_path();
    match toml::to_string_pretty(config) {
        Ok(toml_str) => { let _ = std::fs::write(&path, toml_str); }
        Err(e)       => eprintln!("hho: failed to serialize config: {e}"),
    }
}

// ── Managed state ─────────────────────────────────────────────────────────────

/// Runtime wrapper around UserConfig shared across Tauri commands.
struct ConfigState {
    config: Mutex<UserConfig>,
}

// ── IPC types ─────────────────────────────────────────────────────────────────

/// Data returned to the frontend after a successful CSV open.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CsvFile {
    pub path: String,
    pub rows: Vec<String>,
}

/// Payload emitted on MENU_EVENT to drive the frontend.
#[derive(Serialize, Clone, Debug)]
#[serde(tag = "action", rename_all = "kebab-case")]
enum MenuAction {
    Open,
    OpenRecent { path: String },
}

// ── CSV helpers ───────────────────────────────────────────────────────────────

/// Parse every non-empty row in `path` into a display label.
/// Joins non-blank columns with " │ "; skips fully-blank rows.
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
            record
                .iter()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join(" │ ")
        })
        .filter(|label| !label.is_empty())
        .collect();

    Ok(rows)
}

// ── Recent-file helpers ───────────────────────────────────────────────────────

/// Prepend `new_path` to `files`, dedup, cap to MAX_RECENTS.
pub fn push_recent(files: &mut Vec<String>, new_path: String) {
    files.retain(|p| p != &new_path);
    files.insert(0, new_path);
    files.truncate(MAX_RECENTS);
}

// ── Menu builder ──────────────────────────────────────────────────────────────

/// Construct the full File menu with Open, Open Recent, and Quit.
/// Rebuilds on every call so the recent-files submenu is always current.
fn build_menu(app: &AppHandle, recents: &[String]) -> tauri::Result<Menu<tauri::Wry>> {
    let open = MenuItem::with_id(app, "open", "Open...", true, Some("CmdOrCtrl+O"))?;
    let sep  = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, "quit", "Quit",    true, Some("CmdOrCtrl+Q"))?;

    let recent_sub = build_recent_submenu(app, recents)?;

    let file_menu = Submenu::with_items(
        app, "File", true,
        &[&open, &sep, &recent_sub, &PredefinedMenuItem::separator(app)?, &quit],
    )?;
    Menu::with_items(app, &[&file_menu])
}

fn build_recent_submenu(
    app:     &AppHandle,
    recents: &[String],
) -> tauri::Result<Submenu<tauri::Wry>> {
    if recents.is_empty() {
        let none = MenuItem::with_id(
            app, "no-recents", "No Recent Files", false, None::<&str>,
        )?;
        return Submenu::with_items(app, "Open Recent", true, &[&none]);
    }

    let items: Vec<MenuItem<tauri::Wry>> = recents
        .iter()
        .enumerate()
        .filter_map(|(i, path_str)| {
            let name = Path::new(path_str)
                .file_name()?
                .to_string_lossy()
                .into_owned();
            MenuItem::with_id(app, format!("recent-{i}"), name, true, None::<&str>).ok()
        })
        .collect();

    let refs: Vec<&dyn IsMenuItem<tauri::Wry>> =
        items.iter().map(|m| m as &dyn IsMenuItem<tauri::Wry>).collect();

    Submenu::with_items(app, "Open Recent", true, refs.as_slice())
}

// ── Shared open logic ─────────────────────────────────────────────────────────

/// Read CSV, update config (recents + last_opened_dir), persist, rebuild menu.
fn finalize_open(
    app:   &AppHandle,
    state: &ConfigState,
    path:  PathBuf,
) -> Result<CsvFile, String> {
    let rows     = read_csv_rows(&path)?;
    let path_str = path.to_string_lossy().to_string();
    let dir_str  = path
        .parent()
        .map(|p| p.to_string_lossy().to_string());

    let mut cfg = state.config.lock().unwrap();
    push_recent(&mut cfg.recent_files, path_str.clone());
    cfg.last_opened_dir = dir_str;
    save_config(&cfg);

    let menu = build_menu(app, &cfg.recent_files).map_err(|e| e.to_string())?;
    app.set_menu(menu).map_err(|e| e.to_string())?;

    Ok(CsvFile { path: path_str, rows })
}

// ── Tauri commands ────────────────────────────────────────────────────────────

/// Open a native file-picker, defaulting to the last-used directory.
/// Returns None when the user cancels.
#[tauri::command]
async fn pick_csv(
    app:   AppHandle,
    state: State<'_, ConfigState>,
) -> Result<Option<CsvFile>, String> {
    use tauri_plugin_dialog::DialogExt;

    // Seed the dialog with the last-used directory (or home dir as fallback).
    let start_dir: Option<PathBuf> = {
        let cfg = state.config.lock().unwrap();
        cfg.last_opened_dir
            .as_deref()
            .map(PathBuf::from)
            .filter(|p| p.exists())
            .or_else(dirs::home_dir)
    };

    let mut builder = app
        .dialog()
        .file()
        .add_filter("CSV files", &["csv", "CSV"]);

    if let Some(dir) = start_dir {
        builder = builder.set_directory(dir);
    }

    let picked = builder.blocking_pick_file();

    let Some(file_path) = picked else {
        return Ok(None);
    };

    // FilePath is an enum (Path | Url); desktop dialogs always produce Path.
    let path = file_path
        .as_path()
        .ok_or_else(|| "dialog returned a URL, not a file path".to_string())?
        .to_path_buf();

    finalize_open(&app, &state, path).map(Some)
}

/// Read a CSV at a known path (Open Recent flow), update config.
#[tauri::command]
async fn open_csv(
    path:  String,
    app:   AppHandle,
    state: State<'_, ConfigState>,
) -> Result<Option<CsvFile>, String> {
    let path_buf = PathBuf::from(&path);
    if !path_buf.exists() {
        return Err(format!("file not found: {path}"));
    }
    finalize_open(&app, &state, path_buf).map(Some)
}

// ── App entry point ───────────────────────────────────────────────────────────

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let cfg = load_config();

            let menu = build_menu(app.handle(), &cfg.recent_files)?;
            app.set_menu(menu)?;

            app.manage(ConfigState {
                config: Mutex::new(cfg),
            });

            let handle = app.handle().clone();
            app.on_menu_event(move |_app, event| {
                let id = event.id().as_ref();
                match id {
                    "open" => {
                        let _ = handle.emit(MENU_EVENT, MenuAction::Open);
                    }
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
        .invoke_handler(tauri::generate_handler![pick_csv, open_csv])
        .run(tauri::generate_context!())
        .expect("error while running tauri application")
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // push_recent ─────────────────────────────────────────────────────────────

    #[test]
    fn push_recent_prepends_new_entry() {
        let mut files = vec!["b.csv".to_string(), "c.csv".to_string()];
        push_recent(&mut files, "a.csv".to_string());
        assert_eq!(files[0], "a.csv");
    }

    #[test]
    fn push_recent_deduplicates_existing_entry() {
        let mut files = vec!["a.csv", "b.csv", "c.csv"]
            .into_iter().map(String::from).collect();
        push_recent(&mut files, "b.csv".to_string());
        assert_eq!(files, vec!["b.csv", "a.csv", "c.csv"]);
    }

    #[test]
    fn push_recent_caps_at_max_recents() {
        let mut files: Vec<String> = (0..MAX_RECENTS)
            .map(|i| format!("{i}.csv"))
            .collect();
        push_recent(&mut files, "new.csv".to_string());
        assert_eq!(files.len(), MAX_RECENTS);
        assert_eq!(files[0], "new.csv");
    }

    #[test]
    fn push_recent_on_empty_list_adds_single_entry() {
        let mut files = vec![];
        push_recent(&mut files, "only.csv".to_string());
        assert_eq!(files, vec!["only.csv"]);
    }

    // UserConfig serialization ─────────────────────────────────────────────────

    #[test]
    fn config_roundtrips_through_toml() {
        let original = UserConfig {
            recent_files:    vec!["a.csv".into(), "b.csv".into()],
            last_opened_dir: Some("/home/user/docs".into()),
        };
        let toml_str  = toml::to_string_pretty(&original).unwrap();
        let recovered: UserConfig = toml::from_str(&toml_str).unwrap();
        assert_eq!(recovered.recent_files,    original.recent_files);
        assert_eq!(recovered.last_opened_dir, original.last_opened_dir);
    }

    #[test]
    fn config_with_no_dir_omits_last_opened_dir_key() {
        let cfg = UserConfig { recent_files: vec![], last_opened_dir: None };
        let toml_str = toml::to_string_pretty(&cfg).unwrap();
        assert!(!toml_str.contains("last_opened_dir"));
    }

    #[test]
    fn default_config_is_empty() {
        let cfg = UserConfig::default();
        assert!(cfg.recent_files.is_empty());
        assert!(cfg.last_opened_dir.is_none());
    }

    // CSV parsing ─────────────────────────────────────────────────────────────

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
