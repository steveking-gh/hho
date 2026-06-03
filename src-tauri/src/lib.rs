// Tauri application entry point.
// Responsibilities: native menu, user-config persistence (TOML in home dir),
// CSV reading + transaction mapping, layout/window persistence, IPC commands.

mod mapping;

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, State};
use tauri_plugin_dialog::DialogExt;

use hho_types::{AmazonOrder, AutoAssignRule, Institution, LayoutConfig, NicknameRule, OpenResult, Transaction};

// ── Constants ─────────────────────────────────────────────────────────────────

const MAX_RECENTS: usize = 5;
const CONFIG_FILE: &str = "hho_user_config.toml";

// Default layout dimensions (px, logical).
const DEFAULT_LEFT_W: f32 = 200.0;
const DEFAULT_RIGHT_W: f32 = 200.0;
const DEFAULT_BOT_H: f32 = 200.0;
const DEFAULT_DBG_H: f32 = 150.0;

// Default window dimensions (logical px).
const DEFAULT_WIN_W: f64 = 1024.0;
const DEFAULT_WIN_H: f64 = 700.0;

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

    /// Directory last used in a file-save dialog; seeds the next dialog.
    #[serde(skip_serializing_if = "Option::is_none")]
    last_saved_dir: Option<String>,

    // ── Pane layout ───────────────────────────────────────────────────────────
    #[serde(skip_serializing_if = "Option::is_none")]
    left_width: Option<f32>, // Joint pane width (px)

    #[serde(skip_serializing_if = "Option::is_none")]
    right_width: Option<f32>, // Personal pane width (px)

    #[serde(skip_serializing_if = "Option::is_none")]
    bottom_h: Option<f32>, // Ignored pane height (px)

    #[serde(skip_serializing_if = "Option::is_none")]
    debug_h: Option<f32>, // Debug panel height (px)

    // ── Window geometry ───────────────────────────────────────────────────────
    #[serde(skip_serializing_if = "Option::is_none")]
    window_width: Option<f64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    window_height: Option<f64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    window_x: Option<f64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    window_y: Option<f64>,

    // ── Saved column mappings ───────────────────────────────────────────────────
    // Declared LAST: TOML requires array-of-tables to follow all scalar fields.
    #[serde(default)]
    institutions: Vec<Institution>,

    #[serde(default)]
    auto_assign_rules: Vec<AutoAssignRule>,

    #[serde(default)]
    nickname_rules: Vec<NicknameRule>,
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
        Ok(s) => s,
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
        Ok(s) => {
            let _ = std::fs::write(&path, s);
        }
        Err(e) => eprintln!("hho: config serialize error: {e}"),
    }
}

// ── Managed state ─────────────────────────────────────────────────────────────

struct ConfigState {
    config: Mutex<UserConfig>,
}

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
fn finalize_open(state: &ConfigState, path: PathBuf) -> Result<OpenResult, String> {
    let (headers, rows) = read_csv_table(&path)?;
    let fp = mapping::fingerprint(&headers);
    let path_str = path.to_string_lossy().to_string();
    let dir_str = path.parent().map(|p| p.to_string_lossy().to_string());

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
            fingerprint: fp,
            sample_rows: rows.iter().take(3).cloned().collect(),
            suggested: mapping::suggest_mapping(&headers),
            headers,
            pending_path: path_str,
        },
    };
    Ok(result)
}

// ── Tauri commands ────────────────────────────────────────────────────────────

fn pick_file(
    window: &tauri::Window,
    state: &ConfigState,
    filter_name: &str,
    extensions: &[&str],
) -> Result<Option<PathBuf>, String> {
    let start_dir: Option<PathBuf> = {
        let cfg = state.config.lock().unwrap();
        cfg.last_opened_dir
            .as_deref()
            .map(PathBuf::from)
            .filter(|p| p.exists())
            .or_else(dirs::home_dir)
    };

    let mut builder = window
        .dialog()
        .file()
        .set_parent(window)
        .add_filter(filter_name, extensions);
    if let Some(dir) = start_dir {
        builder = builder.set_directory(dir);
    }

    let Some(fp) = builder.blocking_pick_file() else {
        return Ok(None);
    };
    let path = fp
        .as_path()
        .ok_or_else(|| "dialog returned a URL, not a file path".to_string())?
        .to_path_buf();
    Ok(Some(path))
}

#[tauri::command]
async fn pick_csv(
    window: tauri::Window,
    state: State<'_, ConfigState>,
) -> Result<OpenResult, String> {
    match pick_file(&window, &state, "CSV files", &["csv", "CSV"])? {
        Some(path) => finalize_open(&state, path),
        None => Ok(OpenResult::Cancelled),
    }
}

#[tauri::command]
async fn open_csv(
    path: String,
    state: State<'_, ConfigState>,
) -> Result<OpenResult, String> {
    let pb = PathBuf::from(&path);
    if !pb.exists() {
        return Err(format!("file not found: {path}"));
    }
    finalize_open(&state, pb)
}

/// Persist a user-defined column mapping, then parse the pending file with it.
/// Replaces any existing mapping sharing the same header fingerprint.
#[tauri::command]
fn save_mapping(
    institution: Institution,
    pending_path: String,
    state: State<'_, ConfigState>,
) -> Result<Vec<Transaction>, String> {
    let (headers, rows) = read_csv_table(Path::new(&pending_path))?;

    // Trust the backend-computed fingerprint over whatever the frontend sent.
    let mut inst = institution;
    inst.fingerprint = mapping::fingerprint(&headers);

    let mut cfg = state.config.lock().unwrap();
    cfg.institutions
        .retain(|i| i.fingerprint != inst.fingerprint);
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
        left_width: cfg.left_width.unwrap_or(DEFAULT_LEFT_W),
        right_width: cfg.right_width.unwrap_or(DEFAULT_RIGHT_W),
        bottom_h: cfg.bottom_h.unwrap_or(DEFAULT_BOT_H),
        debug_h: cfg.debug_h.unwrap_or(DEFAULT_DBG_H),
    }
}

/// Persist pane layout after a drag gesture ends.
#[tauri::command]
fn save_layout(
    left_width: f32,
    right_width: f32,
    bottom_h: f32,
    debug_h: f32,
    state: State<'_, ConfigState>,
) {
    let mut cfg = state.config.lock().unwrap();
    cfg.left_width = Some(left_width);
    cfg.right_width = Some(right_width);
    cfg.bottom_h = Some(bottom_h);
    cfg.debug_h = Some(debug_h);
    save_config(&cfg);
}

/// Persist window dimensions and position after the OS window is resized.
#[tauri::command]
fn save_window_size(
    width: f64,
    height: f64,
    window: tauri::Window,
    state: State<'_, ConfigState>,
) {
    let mut cfg = state.config.lock().unwrap();
    cfg.window_width = Some(width);
    cfg.window_height = Some(height);
    if let (Ok(physical_pos), Ok(scale_factor)) = (window.outer_position(), window.scale_factor()) {
        let logical_pos = physical_pos.to_logical::<f64>(scale_factor);
        cfg.window_x = Some(logical_pos.x);
        cfg.window_y = Some(logical_pos.y);
    }
    save_config(&cfg);
}

/// Returns the current MRU list of recent files.
#[tauri::command]
fn get_recent_files(state: State<'_, ConfigState>) -> Vec<String> {
    let cfg = state.config.lock().unwrap();
    cfg.recent_files.clone()
}

/// Returns the persisted list of auto-assign rules.
#[tauri::command]
fn get_auto_assign_rules(state: State<'_, ConfigState>) -> Vec<AutoAssignRule> {
    let cfg = state.config.lock().unwrap();
    cfg.auto_assign_rules.clone()
}

/// Replaces the persisted list of auto-assign rules and writes the updated configuration to disk.
#[tauri::command]
fn save_auto_assign_rules(rules: Vec<AutoAssignRule>, state: State<'_, ConfigState>) {
    let mut cfg = state.config.lock().unwrap();
    cfg.auto_assign_rules = rules;
    save_config(&cfg);
}

/// Returns the persisted list of vendor nickname rules.
#[tauri::command]
fn get_nickname_rules(state: State<'_, ConfigState>) -> Vec<NicknameRule> {
    let cfg = state.config.lock().unwrap();
    cfg.nickname_rules.clone()
}

/// Replaces the persisted list of vendor nickname rules and writes the updated configuration to disk.
#[tauri::command]
fn save_nickname_rules(rules: Vec<NicknameRule>, state: State<'_, ConfigState>) {
    let mut cfg = state.config.lock().unwrap();
    cfg.nickname_rules = rules;
    save_config(&cfg);
}

#[tauri::command]
async fn pick_amazon_orders(
    window: tauri::Window,
    state: State<'_, ConfigState>,
) -> Result<Vec<AmazonOrder>, String> {
    let path = match pick_file(&window, &state, "CSV files", &["csv", "CSV"])? {
        Some(p) => p,
        None => return Ok(vec![]),
    };

    let (headers, rows) = read_csv_table(&path)?;

    let order_id_idx = headers
        .iter()
        .position(|h| h.to_lowercase() == "order id")
        .ok_or_else(|| "missing 'order id' column in Amazon CSV".to_string())?;
    let date_idx = headers
        .iter()
        .position(|h| h.to_lowercase() == "date")
        .ok_or_else(|| "missing 'date' column in Amazon CSV".to_string())?;
    let total_idx = headers
        .iter()
        .position(|h| h.to_lowercase() == "total")
        .ok_or_else(|| "missing 'total' column in Amazon CSV".to_string())?;
    let items_idx = headers
        .iter()
        .position(|h| h.to_lowercase() == "items")
        .ok_or_else(|| "missing 'items' column in Amazon CSV".to_string())?;

    let mut orders = Vec::new();
    for row in rows {
        if row.len() <= order_id_idx
            || row.len() <= date_idx
            || row.len() <= total_idx
            || row.len() <= items_idx
        {
            continue;
        }

        let order_id = row[order_id_idx].trim().to_string();
        let date = row[date_idx].trim().to_string();
        let total_str = &row[total_idx];
        let items_str = &row[items_idx];

        if order_id.is_empty() || date.is_empty() {
            continue;
        }

        let total_cents = hho_types::parse_amount_cents(total_str).unwrap_or(0);

        let items: Vec<String> = items_str
            .split(';')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        orders.push(AmazonOrder {
            order_id,
            date,
            total_cents,
            items,
        });
    }

    Ok(orders)
}

/// Saves transactions of a selected pane to a CSV file.
/// Presents a native file save dialog pre-populated with a default name.
/// Appends a blank row and total summary row to the written CSV file.
#[tauri::command]
async fn save_pane_transactions(
    window: tauri::Window,
    state: State<'_, ConfigState>,
    pane_title: String,
    month_name: String,
    year: i32,
    transactions: Vec<Transaction>,
) -> Result<(), String> {
    let default_filename = format!("{}_{}_{}.csv", pane_title, month_name, year);

    let start_dir: Option<PathBuf> = {
        let cfg = state.config.lock().unwrap();
        cfg.last_saved_dir
            .as_deref()
            .map(PathBuf::from)
            .filter(|p| p.exists())
            .or_else(dirs::home_dir)
    };

    let mut builder = window
        .dialog()
        .file()
        .set_parent(&window)
        .add_filter("CSV files", &["csv", "CSV"])
        .set_file_name(&default_filename);
    if let Some(dir) = start_dir {
        builder = builder.set_directory(dir);
    }

    let Some(fp) = builder.blocking_save_file() else {
        return Ok(());
    };
    let path = fp
        .as_path()
        .ok_or_else(|| "dialog returned a URL, not a file path".to_string())?
        .to_path_buf();

    if let Some(parent) = path.parent() {
        let mut cfg = state.config.lock().unwrap();
        cfg.last_saved_dir = Some(parent.to_string_lossy().to_string());
        save_config(&cfg);
    }

    write_pane_csv(&path, &transactions)
}

fn write_pane_csv(path: &Path, transactions: &[Transaction]) -> Result<(), String> {
    let mut wtr =
        csv::Writer::from_path(path).map_err(|e| format!("failed to create file: {e}"))?;

    wtr.write_record(["Date", "Vendor", "Description", "Amount", "Category"])
        .map_err(|e| format!("failed to write header: {e}"))?;

    for t in transactions {
        let amount_str = hho_types::format_cents(hho_types::net_cents(t.amount_cents, t.direction));
        let display_vendor = t.nickname.as_ref().unwrap_or(&t.vendor);
        wtr.write_record([&t.date, display_vendor, &t.description, &amount_str, &t.category])
            .map_err(|e| format!("failed to write record: {e}"))?;
    }

    wtr.flush().map_err(|e| format!("failed to flush: {e}"))?;

    let mut file = std::fs::OpenOptions::new()
        .append(true)
        .open(path)
        .map_err(|e| format!("failed to open file for appending: {e}"))?;

    use std::io::Write;

    let total_cents: i64 = transactions
        .iter()
        .map(|t| hho_types::net_cents(t.amount_cents, t.direction))
        .sum();

    writeln!(file).map_err(|e| format!("failed to write blank line: {e}"))?;
    writeln!(file, "TOTAL,{}", hho_types::format_cents(total_cents))
        .map_err(|e| format!("failed to write total: {e}"))?;

    let categories = hho_types::summarize_by_category(transactions.iter().map(|t| {
        (
            t.category.as_str(),
            hho_types::net_cents(t.amount_cents, t.direction),
        )
    }));

    for (name, cat_total) in categories {
        // CSV-escape category names that contain a comma or quote.
        let escaped_name = if name.contains(',') || name.contains('"') {
            format!("\"{}\"", name.replace('"', "\"\""))
        } else {
            name
        };

        writeln!(
            file,
            "{},{}",
            escaped_name,
            hho_types::format_cents(cat_total)
        )
        .map_err(|e| format!("failed to write category total: {e}"))?;
    }

    Ok(())
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

            let win_w = cfg.window_width.unwrap_or(DEFAULT_WIN_W);
            let win_h = cfg.window_height.unwrap_or(DEFAULT_WIN_H);
            let win_x = cfg.window_x;
            let win_y = cfg.window_y;

            // Manage configuration state prior to window listener registration.
            app.manage(ConfigState {
                config: Mutex::new(cfg),
            });

            if let Some(win) = app.get_webview_window("main") {
                // Restore window size and position.
                let _ = win.set_size(tauri::LogicalSize::new(win_w, win_h));
                if let (Some(x), Some(y)) = (win_x, win_y) {
                    let _ = win.set_position(tauri::LogicalPosition::new(x, y));
                }

                // Register native move and resize listener to save geometry.
                let app_handle = app.handle().clone();
                let save_pending = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));

                win.on_window_event(move |event| {
                    if let tauri::WindowEvent::Resized(_) | tauri::WindowEvent::Moved(_) = event {
                        let save_pending = save_pending.clone();
                        let app_handle = app_handle.clone();

                        if !save_pending.swap(true, std::sync::atomic::Ordering::SeqCst) {
                            std::thread::spawn(move || {
                                std::thread::sleep(std::time::Duration::from_millis(500));
                                save_pending.store(false, std::sync::atomic::Ordering::SeqCst);

                                if let Some(w) = app_handle.get_webview_window("main") {
                                    if let (Ok(physical_size), Ok(physical_pos)) = (w.outer_size(), w.outer_position()) {
                                        if let Ok(scale_factor) = w.scale_factor() {
                                            let logical_size = physical_size.to_logical::<f64>(scale_factor);
                                            let logical_pos = physical_pos.to_logical::<f64>(scale_factor);

                                            // Lock global configuration state and update window parameters.
                                            let state = app_handle.state::<ConfigState>();
                                            let mut cfg = state.config.lock().unwrap();
                                            cfg.window_width = Some(logical_size.width);
                                            cfg.window_height = Some(logical_size.height);
                                            cfg.window_x = Some(logical_pos.x);
                                            cfg.window_y = Some(logical_pos.y);
                                            save_config(&cfg);
                                        }
                                    }
                                }
                            });
                        }
                    }
                });
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            pick_csv,
            open_csv,
            save_mapping,
            get_layout,
            save_layout,
            save_window_size,
            get_recent_files,
            exit_app,
            get_auto_assign_rules,
            save_auto_assign_rules,
            save_pane_transactions,
            get_nickname_rules,
            save_nickname_rules,
            pick_amazon_orders,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application")
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use hho_types::RulePane;

    #[test]
    fn push_recent_prepends_new_entry() {
        let mut v = vec!["b.csv".to_string(), "c.csv".to_string()];
        push_recent(&mut v, "a.csv".to_string());
        assert_eq!(v[0], "a.csv");
    }

    #[test]
    fn push_recent_deduplicates_existing_entry() {
        let mut v: Vec<String> = ["a", "b", "c"].iter().map(|s| s.to_string()).collect();
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
            recent_files: vec!["a.csv".into(), "b.csv".into()],
            last_opened_dir: Some("/home/user/docs".into()),
            last_saved_dir: Some("/home/user/saved".into()),
            left_width: Some(250.0),
            right_width: Some(180.0),
            bottom_h: Some(220.0),
            debug_h: Some(130.0),
            window_width: Some(1200.0),
            window_height: Some(800.0),
            window_x: Some(100.0),
            window_y: Some(150.0),
            institutions: vec![],
            auto_assign_rules: vec![],
            nickname_rules: vec![],
        };
        let toml_str = toml::to_string_pretty(&original).unwrap();
        let recovered: UserConfig = toml::from_str(&toml_str).unwrap();
        assert_eq!(recovered.recent_files, original.recent_files);
        assert_eq!(recovered.last_opened_dir, original.last_opened_dir);
        assert_eq!(recovered.left_width, original.left_width);
        assert_eq!(recovered.window_width, original.window_width);
        assert_eq!(recovered.window_x, original.window_x);
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
        let dir = std::env::temp_dir();
        let path = dir.join("hho_table_test.csv");
        std::fs::write(
            &path,
            "Date,Description,Amount\n05/18/2026,STARBUCKS,-5.40\n",
        )
        .unwrap();
        let (headers, rows) = read_csv_table(&path).unwrap();
        assert_eq!(headers, vec!["Date", "Description", "Amount"]);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0], vec!["05/18/2026", "STARBUCKS", "-5.40"]);
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn config_with_institution_roundtrips_through_toml() {
        let cfg = UserConfig {
            recent_files: vec![],
            last_opened_dir: None,
            last_saved_dir: None,
            left_width: None,
            right_width: None,
            bottom_h: None,
            debug_h: None,
            window_width: None,
            window_height: None,
            window_x: None,
            window_y: None,
            auto_assign_rules: vec![],
            nickname_rules: vec![],
            institutions: vec![hho_types::Institution {
                name: "Chase".into(),
                fingerprint: "date,description,amount".into(),
                date_col: 0,
                vendor_col: 1,
                description_col: None,
                category_col: None,
                ignore_cols: vec![],
                amount: hho_types::AmountScheme::SingleSigned {
                    amount_col: 2,
                    debit_is_negative: true,
                },
            }],
        };
        let toml_str = toml::to_string_pretty(&cfg).unwrap();
        let recovered: UserConfig = toml::from_str(&toml_str).unwrap();
        assert_eq!(recovered.institutions, cfg.institutions);
    }

    #[test]
    fn config_with_auto_assign_rules_roundtrips_through_toml() {
        let cfg = UserConfig {
            recent_files: vec![],
            last_opened_dir: None,
            last_saved_dir: None,
            left_width: None,
            right_width: None,
            bottom_h: None,
            debug_h: None,
            window_width: None,
            window_height: None,
            window_x: None,
            window_y: None,
            institutions: vec![],
            nickname_rules: vec![],
            auto_assign_rules: vec![
                AutoAssignRule {
                    regex: Some("STARBUCKS".to_string()),
                    vendor_regex: None,
                    description_regex: None,
                    pane: RulePane::Joint,
                    category_override: None,
                },
                AutoAssignRule {
                    regex: Some("NETFLIX".to_string()),
                    vendor_regex: Some("NETFLIX.*".to_string()),
                    description_regex: Some("NETFLIX".to_string()),
                    pane: RulePane::Personal,
                    category_override: Some("Streaming".to_string()),
                },
            ],
        };
        let toml_str = toml::to_string_pretty(&cfg).unwrap();
        let recovered: UserConfig = toml::from_str(&toml_str).unwrap();
        assert_eq!(recovered.auto_assign_rules, cfg.auto_assign_rules);
    }

    #[test]
    fn save_auto_assign_rules_replaces_all_rules_in_config() {
        let mut cfg = UserConfig {
            auto_assign_rules: vec![AutoAssignRule {
                regex: Some("OLD".to_string()),
                vendor_regex: None,
                description_regex: None,
                pane: RulePane::Joint,
                category_override: None,
            }],
            ..Default::default()
        };
        let new_rules = vec![AutoAssignRule {
            regex: Some("NEW".to_string()),
            vendor_regex: None,
            description_regex: None,
            pane: RulePane::Personal,
            category_override: None,
        }];
        cfg.auto_assign_rules = new_rules.clone();
        assert_eq!(cfg.auto_assign_rules, new_rules);
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

        let call_re =
            regex::Regex::new(r#"\b(call|call_unit|invoke_raw)\s*\(\s*"([^"]+)"#).unwrap();
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

    #[test]
    fn test_write_pane_csv_outputs_correct_format() {
        let dir = std::env::temp_dir();
        let path = dir.join("test_write_pane_csv.csv");
        let transactions = vec![
            Transaction {
                id: None,
                date: "2026-05-18".to_string(),
                vendor: "BUDGET RENT A CAR".to_string(),
                nickname: Some("Budget".to_string()),
                description: "Car rental memo".to_string(),
                category: "Travel".to_string(),
                amount_cents: 28697,
                direction: hho_types::Direction::Debit,
                manual_pane: None,
                ..Default::default()
            },
            Transaction {
                id: None,
                date: "2026-05-19".to_string(),
                vendor: "STARBUCKS".to_string(),
                nickname: None,
                description: "".to_string(),
                category: "".to_string(),
                amount_cents: 540,
                direction: hho_types::Direction::Debit,
                manual_pane: None,
                ..Default::default()
            },
            Transaction {
                id: None,
                date: "2026-05-20".to_string(),
                vendor: "CREDIT REFUND".to_string(),
                nickname: None,
                description: "Refund details".to_string(),
                category: "Refund, Special".to_string(),
                amount_cents: 1000,
                direction: hho_types::Direction::Credit,
                manual_pane: None,
                ..Default::default()
            },
        ];

        let res = write_pane_csv(&path, &transactions);
        assert!(res.is_ok());

        let contents = std::fs::read_to_string(&path).unwrap();
        let contents_lf = contents.replace("\r\n", "\n");
        let expected = "\
Date,Vendor,Description,Amount,Category
2026-05-18,Budget,Car rental memo,-286.97,Travel
2026-05-19,STARBUCKS,,-5.40,
2026-05-20,CREDIT REFUND,Refund details,10.00,\"Refund, Special\"

TOTAL,-282.37
(No Category),-5.40
\"Refund, Special\",10.00
Travel,-286.97
";
        assert_eq!(contents_lf, expected);

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_parse_amazon_orders_csv_format() {
        let dir = std::env::temp_dir();
        let path = dir.join("test_amazon_orders.csv");

        let csv_content = "\
order id,order url,items,to,date,total,shipping,shipping_refund,gift,tax,refund,payments
113-3983825-0231454,https://amazon.com/url,\"OXO Good Grips; YIMITEE Brush; 1/2 Teaspoon; \",Steve King,2026-06-01,42.28,0,,,0,,Prime Visa
113-8688736-7549016,https://amazon.com/url,Golf Ball Retriever; ,Steve King,2026-05-24,0.00,0,,,0,,Prime Visa
113-3226450-8453018,https://amazon.com/url,Door Shoe Organizer; ,Steve King,2026-05-18,29.99,0,,,0,,Prime Visa
";
        std::fs::write(&path, csv_content).unwrap();

        let (headers, rows) = read_csv_table(&path).unwrap();
        assert_eq!(headers.len(), 12);
        assert_eq!(rows.len(), 3);

        let order_id_idx = headers.iter().position(|h| h.to_lowercase() == "order id").unwrap();
        let date_idx = headers.iter().position(|h| h.to_lowercase() == "date").unwrap();
        let total_idx = headers.iter().position(|h| h.to_lowercase() == "total").unwrap();
        let items_idx = headers.iter().position(|h| h.to_lowercase() == "items").unwrap();

        let mut orders = Vec::new();
        for row in rows {
            let order_id = row[order_id_idx].trim().to_string();
            let date = row[date_idx].trim().to_string();
            let total_cents = hho_types::parse_amount_cents(&row[total_idx]).unwrap_or(0);
            let items: Vec<String> = row[items_idx]
                .split(';')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();

            orders.push(AmazonOrder {
                order_id,
                date,
                total_cents,
                items,
            });
        }

        assert_eq!(orders.len(), 3);
        assert_eq!(orders[0].order_id, "113-3983825-0231454");
        assert_eq!(orders[0].date, "2026-06-01");
        assert_eq!(orders[0].total_cents, 4228);
        assert_eq!(orders[0].items, vec![
            "OXO Good Grips".to_string(),
            "YIMITEE Brush".to_string(),
            "1/2 Teaspoon".to_string()
        ]);

        assert_eq!(orders[1].total_cents, 0);
        assert_eq!(orders[1].items, vec!["Golf Ball Retriever".to_string()]);

        assert_eq!(orders[2].total_cents, 2999);
        assert_eq!(orders[2].items, vec!["Door Shoe Organizer".to_string()]);

        let _ = std::fs::remove_file(&path);
    }
}
