// Tauri application entry point.
// Responsibilities: native menu, user-config persistence (TOML in home dir),
// CSV reading + transaction mapping, layout/window persistence, IPC commands.

mod mapping;

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, State};

// IPC payload types are shared with the frontend via the hho-types crate.
use hho_types::{Institution, LayoutConfig, OpenResult, Transaction};

// ── Constants ─────────────────────────────────────────────────────────────────

const MAX_RECENTS:    usize = 5;
const CONFIG_FILE:    &str  = "hho_user_config.toml";

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

    // ── Saved column mappings ───────────────────────────────────────────────────
    // Declared LAST: TOML requires array-of-tables to follow all scalar fields.
    #[serde(default)]
    institutions: Vec<Institution>,
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

// OpenResult and LayoutConfig are defined in hho-types (shared with the frontend).



// ── CSV helpers ───────────────────────────────────────────────────────────────

/// Read a CSV into its header row and data rows (each a Vec of cell strings).
/// `flexible` tolerates ragged rows; the first record is treated as the header.
fn read_csv_table(path: &Path) -> Result<(Vec<String>, Vec<Vec<String>>), String> {
    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(true)
        .from_path(path)
        .map_err(|e| format!("csv open: {e}"))?;

    let headers = rdr
        .headers()
        .map_err(|e| format!("csv header: {e}"))?
        .iter()
        .map(|s| s.to_string())
        .collect();

    let rows = rdr
        .records()
        .filter_map(|r| r.ok())
        .map(|record| record.iter().map(|s| s.to_string()).collect())
        .collect();

    Ok((headers, rows))
}

// ── Recent-file helpers ───────────────────────────────────────────────────────

/// Prepend `new_path`, dedup, cap to MAX_RECENTS.
pub fn push_recent(files: &mut Vec<String>, new_path: String) {
    files.retain(|p| p != &new_path);
    files.insert(0, new_path);
    files.truncate(MAX_RECENTS);
}


// ── Shared open logic ─────────────────────────────────────────────────────────

/// Read the CSV, record it in recents, rebuild the menu, then either parse it
/// with a saved institution (Mapped) or request a new mapping (NeedsMapping).
fn finalize_open(
    state: &ConfigState,
    path:  PathBuf,
) -> Result<OpenResult, String> {
    let (headers, rows) = read_csv_table(&path)?;
    let fp       = mapping::fingerprint(&headers);
    let path_str = path.to_string_lossy().to_string();
    let dir_str  = path.parent().map(|p| p.to_string_lossy().to_string());

    let mut cfg = state.config.lock().unwrap();
    push_recent(&mut cfg.recent_files, path_str.clone());
    cfg.last_opened_dir = dir_str;
    save_config(&cfg);



    // Known institution → parse rows; unknown → ask the frontend for a mapping.
    let result = match mapping::find_institution(&fp, &cfg.institutions) {
        Some(inst) => {
            let transactions = rows
                .iter()
                .filter_map(|r| mapping::parse_row(inst, r))
                .collect();
            OpenResult::Mapped {
                institution: inst.name.clone(),
                transactions,
            }
        }
        None => OpenResult::NeedsMapping {
            fingerprint:  fp,
            sample_rows:  rows.iter().take(3).cloned().collect(),
            suggested:    mapping::suggest_mapping(&headers),
            headers,
            pending_path: path_str,
        },
    };
    Ok(result)
}

// ── Tauri commands ────────────────────────────────────────────────────────────

#[tauri::command]
async fn pick_csv(
    window: tauri::Window,
    state: State<'_, ConfigState>,
) -> Result<OpenResult, String> {
    use tauri_plugin_dialog::DialogExt;

    let start_dir: Option<PathBuf> = {
        let cfg = state.config.lock().unwrap();
        cfg.last_opened_dir
            .as_deref()
            .map(PathBuf::from)
            .filter(|p| p.exists())
            .or_else(dirs::home_dir)
    };

    // Associates file dialog with calling window as parent owner.
    // Prevents duplicate dialog instances and keeps dialog in focus.
    let mut builder = window.dialog().file()
        .set_parent(&window)
        .add_filter("CSV files", &["csv", "CSV"]);
    if let Some(dir) = start_dir {
        builder = builder.set_directory(dir);
    }

    let Some(fp) = builder.blocking_pick_file() else {
        return Ok(OpenResult::Cancelled);
    };
    let path = fp.as_path()
        .ok_or_else(|| "dialog returned a URL, not a file path".to_string())?
        .to_path_buf();
    finalize_open(&state, path)
}

#[tauri::command]
async fn open_csv(
    path:  String,
    _app:  AppHandle,
    state: State<'_, ConfigState>,
) -> Result<OpenResult, String> {
    let pb = PathBuf::from(&path);
    if !pb.exists() { return Err(format!("file not found: {path}")); }
    finalize_open(&state, pb)
}

/// Persist a user-defined column mapping, then parse the pending file with it.
/// Replaces any existing mapping sharing the same header fingerprint.
#[tauri::command]
fn save_mapping(
    institution:  Institution,
    pending_path: String,
    state:        State<'_, ConfigState>,
) -> Result<Vec<Transaction>, String> {
    let (headers, rows) = read_csv_table(Path::new(&pending_path))?;

    // Trust the backend-computed fingerprint over whatever the frontend sent.
    let mut inst = institution;
    inst.fingerprint = mapping::fingerprint(&headers);

    let mut cfg = state.config.lock().unwrap();
    cfg.institutions.retain(|i| i.fingerprint != inst.fingerprint);
    cfg.institutions.push(inst.clone());
    save_config(&cfg);

    let transactions = rows
        .iter()
        .filter_map(|r| mapping::parse_row(&inst, r))
        .collect();
    Ok(transactions)
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

/// Returns the current MRU list of recent files.
#[tauri::command]
fn get_recent_files(state: State<'_, ConfigState>) -> Vec<String> {
    let cfg = state.config.lock().unwrap();
    cfg.recent_files.clone()
}

/// Exits the application cleanly by closing all open windows.
/// Posts window closure to main thread event loop.
/// Prevents webview destruction while IPC handler is active.
#[tauri::command]
fn exit_app(app: AppHandle) {
    let app_clone = app.clone();
    let _ = app.run_on_main_thread(move || {
        for window in app_clone.webview_windows().values() {
            let _ = window.close();
        }
    });
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

            app.manage(ConfigState { config: Mutex::new(cfg) });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            pick_csv, open_csv, save_mapping,
            get_layout, save_layout, save_window_size,
            get_recent_files, exit_app,
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
            institutions:    vec![],
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
    fn read_csv_table_splits_header_and_rows() {
        let dir  = std::env::temp_dir();
        let path = dir.join("hho_table_test.csv");
        std::fs::write(&path, "Date,Description,Amount\n05/18/2026,STARBUCKS,-5.40\n").unwrap();
        let (headers, rows) = read_csv_table(&path).unwrap();
        assert_eq!(headers, vec!["Date", "Description", "Amount"]);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0], vec!["05/18/2026", "STARBUCKS", "-5.40"]);
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn config_with_institution_roundtrips_through_toml() {
        let cfg = UserConfig {
            recent_files:    vec![],
            last_opened_dir: None,
            left_width:      None,
            right_width:     None,
            bottom_h:        None,
            debug_h:         None,
            window_width:    None,
            window_height:   None,
            institutions:    vec![hho_types::Institution {
                name: "Chase".into(),
                fingerprint: "date,description,amount".into(),
                date_col: 0,
                vendor_col: 1,
                ignore_cols: vec![],
                amount: hho_types::AmountScheme::SingleSigned {
                    amount_col: 2,
                    debit_is_negative: true,
                },
            }],
        };
        let toml_str  = toml::to_string_pretty(&cfg).unwrap();
        let recovered: UserConfig = toml::from_str(&toml_str).unwrap();
        assert_eq!(recovered.institutions, cfg.institutions);
    }

    #[test]
    fn test_frontend_ipc_commands_are_registered_in_backend() {
        use std::collections::HashSet;
        use std::path::{Path, PathBuf};

        fn get_rs_files(dir: &Path) -> Vec<PathBuf> {
            let mut files = Vec::new();
            let mut dirs_to_visit = vec![dir.to_path_buf()];
            while let Some(current_dir) = dirs_to_visit.pop() {
                if let Ok(entries) = std::fs::read_dir(current_dir) {
                    for entry in entries.filter_map(Result::ok) {
                        let path = entry.path();
                        if path.is_dir() {
                            dirs_to_visit.push(path);
                        } else if path.extension().and_then(|s| s.to_str()) == Some("rs") {
                            files.push(path);
                        }
                    }
                }
            }
            files
        }

        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set");
        let manifest_path = Path::new(&manifest_dir);
        let frontend_dir = manifest_path.join("../src");
        let backend_dir = manifest_path.join("src");

        // 1. Gather all frontend commands
        let frontend_files = get_rs_files(&frontend_dir);
        let mut frontend_cmds = HashSet::new();

        let call_re = regex::Regex::new(r#"\b(call|call_unit|invoke_raw)\s*\(\s*"([^"]+)"#).unwrap();
        let re_line_comment = regex::Regex::new(r"//.*").unwrap();
        let re_block_comment = regex::Regex::new(r"(?s)/\*.*?\*/").unwrap();

        for file_path in frontend_files {
            if let Ok(content) = std::fs::read_to_string(&file_path) {
                // Strip comments to avoid matching commented-out code
                let content_no_line = re_line_comment.replace_all(&content, "");
                let content_clean = re_block_comment.replace_all(&content_no_line, "");

                for cap in call_re.captures_iter(&content_clean) {
                    frontend_cmds.insert(cap[2].to_string());
                }
            }
        }

        // 2. Gather all backend registered commands from generate_handler!
        let backend_files = get_rs_files(&backend_dir);
        let mut backend_cmds = HashSet::new();
        let handler_re = regex::Regex::new(r"(?s)generate_handler!\[(.*?)\]").unwrap();

        for file_path in backend_files {
            if let Ok(content) = std::fs::read_to_string(&file_path) {
                for cap in handler_re.captures_iter(&content) {
                    let inside = &cap[1];
                    for part in inside.split(',') {
                        let trimmed = part.trim();
                        if !trimmed.is_empty() {
                            backend_cmds.insert(trimmed.to_string());
                        }
                    }
                }
            }
        }

        // 3. Find any frontend commands that are not in backend_cmds
        let mut missing = Vec::new();
        for cmd in &frontend_cmds {
            if !backend_cmds.contains(cmd) {
                missing.push(cmd.clone());
            }
        }

        assert!(
            missing.is_empty(),
            "Found frontend IPC commands with no matching registered backend handler in generate_handler!: {:?}",
            missing
        );
    }
}

