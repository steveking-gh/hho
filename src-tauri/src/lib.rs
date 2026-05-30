// Tauri application entry point.
// Responsibilities: native menu, recent-file persistence, CSV reading, IPC commands.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri::menu::{IsMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};

// ── Constants ─────────────────────────────────────────────────────────────────

const MAX_RECENTS: usize = 5;
const RECENTS_FILE: &str = "recents.json";
const MENU_EVENT:   &str = "hho-menu";

// ── Managed state ─────────────────────────────────────────────────────────────

/// MRU file list shared between the menu builder and Tauri commands.
pub struct RecentsState {
    pub files:      Mutex<Vec<PathBuf>>,
    pub config_dir: PathBuf,
}

// ── IPC types ─────────────────────────────────────────────────────────────────

/// Data returned to the frontend after a successful CSV open.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CsvFile {
    pub path: String,
    pub rows: Vec<String>,
}

/// Payload emitted on MENU_EVENT; drives the frontend's response.
#[derive(Serialize, Clone, Debug)]
#[serde(tag = "action", rename_all = "kebab-case")]
enum MenuAction {
    Open,
    OpenRecent { path: String },
}

// ── CSV helpers ───────────────────────────────────────────────────────────────

/// Read every non-empty row from `path` and format it as a display label.
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

// ── Recents helpers ───────────────────────────────────────────────────────────

fn recents_path(config_dir: &Path) -> PathBuf {
    config_dir.join(RECENTS_FILE)
}

/// Load the persisted MRU list; skip entries whose files no longer exist.
fn load_recents(config_dir: &Path) -> Vec<PathBuf> {
    std::fs::read_to_string(recents_path(config_dir))
        .ok()
        .and_then(|s| serde_json::from_str::<Vec<String>>(&s).ok())
        .unwrap_or_default()
        .into_iter()
        .map(PathBuf::from)
        .filter(|p| p.exists())
        .take(MAX_RECENTS)
        .collect()
}

/// Persist `files` to disk as a JSON array of path strings.
fn save_recents(config_dir: &Path, files: &[PathBuf]) {
    let _ = std::fs::create_dir_all(config_dir);
    let strings: Vec<_> = files
        .iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect();
    if let Ok(json) = serde_json::to_string_pretty(&strings) {
        let _ = std::fs::write(recents_path(config_dir), json);
    }
}

/// Prepend `new_path` to `files`, dedup by path, cap to MAX_RECENTS.
pub fn push_recent(files: &mut Vec<PathBuf>, new_path: PathBuf) {
    files.retain(|p| p != &new_path);
    files.insert(0, new_path);
    files.truncate(MAX_RECENTS);
}

// ── Menu builder ──────────────────────────────────────────────────────────────

/// Construct the File menu with Open, Open Recent submenu, and Quit.
/// Rebuilds the full menu on every call so recents are always current.
fn build_menu(app: &AppHandle, recents: &[PathBuf]) -> tauri::Result<Menu<tauri::Wry>> {
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
    recents: &[PathBuf],
) -> tauri::Result<Submenu<tauri::Wry>> {
    if recents.is_empty() {
        let none = MenuItem::with_id(
            app, "no-recents", "No Recent Files", false, None::<&str>,
        )?;
        return Submenu::with_items(app, "Open Recent", true, &[&none]);
    }

    // Build items in a Vec first; borrow them as trait objects for with_items.
    let items: Vec<MenuItem<tauri::Wry>> = recents
        .iter()
        .enumerate()
        .filter_map(|(i, path)| {
            let name = path.file_name()?.to_string_lossy().into_owned();
            MenuItem::with_id(app, format!("recent-{i}"), name, true, None::<&str>).ok()
        })
        .collect();

    let refs: Vec<&dyn IsMenuItem<tauri::Wry>> =
        items.iter().map(|m| m as &dyn IsMenuItem<tauri::Wry>).collect();

    Submenu::with_items(app, "Open Recent", true, refs.as_slice())
}

// ── Shared open logic ─────────────────────────────────────────────────────────

/// Read CSV, record in recents, persist, rebuild menu, return result.
fn finalize_open(
    app:   &AppHandle,
    state: &RecentsState,
    path:  PathBuf,
) -> Result<CsvFile, String> {
    let rows     = read_csv_rows(&path)?;
    let path_str = path.to_string_lossy().to_string();

    let mut files = state.files.lock().unwrap();
    push_recent(&mut files, path);
    save_recents(&state.config_dir, &files);

    let menu = build_menu(app, &files).map_err(|e| e.to_string())?;
    app.set_menu(menu).map_err(|e| e.to_string())?;

    Ok(CsvFile { path: path_str, rows })
}

// ── Tauri commands ────────────────────────────────────────────────────────────

/// Show a native file-picker, read the selected CSV, update recents.
/// Returns None when the user cancels the dialog.
#[tauri::command]
async fn pick_csv(
    app:   AppHandle,
    state: State<'_, RecentsState>,
) -> Result<Option<CsvFile>, String> {
    use tauri_plugin_dialog::DialogExt;

    let picked = app
        .dialog()
        .file()
        .add_filter("CSV files", &["csv", "CSV"])
        .blocking_pick_file();

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

/// Read a CSV at a known path (Open Recent path), update recents.
#[tauri::command]
async fn open_csv(
    path:  String,
    app:   AppHandle,
    state: State<'_, RecentsState>,
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
            let config_dir = app
                .path()
                .app_config_dir()
                .unwrap_or_else(|_| PathBuf::from("."));

            let recents = load_recents(&config_dir);

            app.manage(RecentsState {
                files:      Mutex::new(recents.clone()),
                config_dir,
            });

            let menu = build_menu(app.handle(), &recents)?;
            app.set_menu(menu)?;

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
                            let state = handle.state::<RecentsState>();
                            let files = state.files.lock().unwrap();
                            if let Some(path) = files.get(idx) {
                                let _ = handle.emit(
                                    MENU_EVENT,
                                    MenuAction::OpenRecent {
                                        path: path.to_string_lossy().to_string(),
                                    },
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

    fn paths(names: &[&str]) -> Vec<PathBuf> {
        names.iter().map(PathBuf::from).collect()
    }

    #[test]
    fn push_recent_prepends_new_entry() {
        let mut files = paths(&["b.csv", "c.csv"]);
        push_recent(&mut files, PathBuf::from("a.csv"));
        assert_eq!(files[0], PathBuf::from("a.csv"));
    }

    #[test]
    fn push_recent_deduplicates_existing_entry() {
        let mut files = paths(&["a.csv", "b.csv", "c.csv"]);
        push_recent(&mut files, PathBuf::from("b.csv"));
        assert_eq!(files, paths(&["b.csv", "a.csv", "c.csv"]));
    }

    #[test]
    fn push_recent_caps_at_max() {
        let mut files: Vec<PathBuf> = (0..MAX_RECENTS)
            .map(|i| PathBuf::from(format!("{i}.csv")))
            .collect();
        push_recent(&mut files, PathBuf::from("new.csv"));
        assert_eq!(files.len(), MAX_RECENTS);
        assert_eq!(files[0], PathBuf::from("new.csv"));
    }

    #[test]
    fn push_recent_on_empty_list_adds_entry() {
        let mut files = vec![];
        push_recent(&mut files, PathBuf::from("only.csv"));
        assert_eq!(files, paths(&["only.csv"]));
    }

    #[test]
    fn csv_rows_join_columns_with_separator() {
        // Write a temp CSV and verify row formatting.
        let dir = std::env::temp_dir();
        let path = dir.join("hho_test.csv");
        std::fs::write(&path, "Alice,30,Engineer\nBob,,Manager\n\n").unwrap();
        let rows = read_csv_rows(&path).unwrap();
        assert_eq!(rows[0], "Alice │ 30 │ Engineer");
        assert_eq!(rows[1], "Bob │ Manager");  // empty column skipped
        assert_eq!(rows.len(), 2);             // blank row skipped
        std::fs::remove_file(&path).unwrap();
    }
}
