// Shared IPC contract between the hho frontend (WASM/Leptos) and backend (Tauri).
//
// Every type that crosses the command boundary lives here exactly once, so the
// two separately-compiled crates cannot drift: a renamed field or changed type
// becomes a compile error on whichever side is stale.
//
// Argument structs carry `#[serde(rename_all = "camelCase")]` because Tauri v2
// matches command arguments by camelCase keys.

use serde::{Deserialize, Serialize};

// ── Domain types ──────────────────────────────────────────────────────────────

/// Direction of money flow for a transaction.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    Debit,  // money out
    Credit, // money in
}

/// How an institution encodes amount magnitude and debit/credit direction.
/// Internally tagged on `amount_scheme` and flattened into Institution so the
/// persisted TOML and the IPC JSON both stay flat and readable.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "amount_scheme", rename_all = "snake_case")]
pub enum AmountScheme {
    /// One signed amount column; sign determines direction.
    SingleSigned {
        amount_col: usize,
        debit_is_negative: bool,
    },
    /// Magnitude in one column; a separate text column labels the direction.
    TypeColumn {
        amount_col: usize,
        type_col: usize,
        debit_labels: Vec<String>,
        credit_labels: Vec<String>,
    },
}

/// A saved per-institution column mapping, keyed by header fingerprint.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Institution {
    pub name: String,
    pub fingerprint: String,
    pub date_col: usize,
    pub vendor_col: usize,
    #[serde(default)]
    pub ignore_cols: Vec<usize>,
    #[serde(flatten)]
    pub amount: AmountScheme,
}

/// A normalized transaction produced by applying an Institution to a CSV row.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Transaction {
    pub date: String,      // canonical "YYYY-MM-DD"
    pub vendor: String,
    pub amount_cents: i64, // magnitude, always >= 0
    pub direction: Direction,
}

/// Heuristic mapping suggestion shown as the modal's initial state.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SuggestedMapping {
    pub date_col: usize,
    pub vendor_col: usize,
    pub amount_col: usize,
    pub type_col: Option<usize>,
    pub scheme: String, // "single_signed" | "type_column"
    pub debit_is_negative: bool,
    pub ignore_cols: Vec<usize>,
}

// ── Command results ───────────────────────────────────────────────────────────

/// Outcome of opening a CSV.
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "status", rename_all = "kebab-case")]
pub enum OpenResult {
    Mapped {
        institution: String,
        transactions: Vec<Transaction>,
    },
    NeedsMapping {
        fingerprint: String,
        headers: Vec<String>,
        sample_rows: Vec<Vec<String>>,
        pending_path: String,
        suggested: SuggestedMapping,
    },
    Cancelled,
}

/// Persisted pane dimensions returned by `get_layout`.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct LayoutConfig {
    pub left_width: f32,
    pub right_width: f32,
    pub bottom_h: f32,
    pub debug_h: f32,
}

// ── Command argument structs (frontend → backend) ─────────────────────────────

/// Arguments for the `open_csv` command.
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct OpenCsvArgs {
    pub path: String,
}

/// Arguments for the `save_mapping` command.
/// `pending_path` serializes as `pendingPath`; the nested `institution` keeps
/// its own (snake_case) field names, which the backend deserializes directly.
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SaveMappingArgs {
    pub institution: Institution,
    pub pending_path: String,
}

/// Arguments for the `save_layout` command.
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SaveLayoutArgs {
    pub left_width: f32,
    pub right_width: f32,
    pub bottom_h: f32,
    pub debug_h: f32,
}

/// Arguments for the `save_window_size` command.
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SaveWindowSizeArgs {
    pub width: f64,
    pub height: f64,
}

/// A regex rule mapping a vendor name to a destination pane.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct AutoAssignRule {
    pub regex: String,
    pub pane: String, // "left" | "right" | "bottom"
}
