// Frontend serde mirrors of the Tauri backend IPC types.
// Kept in sync manually with src-tauri/src/{mapping.rs, lib.rs}.

use serde::{Deserialize, Serialize};

/// One normalized transaction returned by the backend.
#[derive(Deserialize, Clone, Debug)]
pub struct Txn {
    pub date: String,
    pub vendor: String,
    pub amount_cents: i64,
    pub direction: String, // "debit" | "credit"
}

/// Heuristic mapping suggestion used to pre-fill the modal.
#[derive(Deserialize, Clone, Debug)]
pub struct Suggested {
    pub date_col: usize,
    pub vendor_col: usize,
    pub amount_col: usize,
    pub type_col: Option<usize>,
    pub scheme: String,
    pub debit_is_negative: bool,
    pub ignore_cols: Vec<usize>,
}

/// Result of opening a CSV (mirrors backend `OpenResult`).
#[derive(Deserialize, Clone, Debug)]
#[serde(tag = "status", rename_all = "kebab-case")]
pub enum OpenResult {
    Mapped {
        institution: String,
        transactions: Vec<Txn>,
    },
    NeedsMapping {
        fingerprint: String,
        headers: Vec<String>,
        sample_rows: Vec<Vec<String>>,
        pending_path: String,
        suggested: Suggested,
    },
    Cancelled,
}

/// Data the modal needs while open; stored in a signal.
#[derive(Clone, Debug)]
pub struct PendingMapping {
    pub fingerprint: String,
    pub headers: Vec<String>,
    pub sample_rows: Vec<Vec<String>>,
    pub pending_path: String,
    pub suggested: Suggested,
}

/// Institution payload sent to the `save_mapping` command.
/// Flat shape matching the backend's flattened, internally-tagged enum:
/// `amount_scheme` discriminates, and only the relevant variant fields are set.
#[derive(Serialize, Clone, Debug)]
pub struct InstitutionDto {
    pub name: String,
    pub fingerprint: String,
    pub date_col: usize,
    pub vendor_col: usize,
    pub ignore_cols: Vec<usize>,
    pub amount_scheme: String, // "single_signed" | "type_column"
    pub amount_col: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub debit_is_negative: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub type_col: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub debit_labels: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub credit_labels: Option<Vec<String>>,
}

/// Argument object for the `save_mapping` command.
/// Tauri v2 matches command arguments by camelCase keys, so `pending_path`
/// must serialize as `pendingPath`. (Nested `institution` fields are
/// deserialized by serde directly and stay snake_case.)
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SaveMappingArgs {
    pub institution: InstitutionDto,
    pub pending_path: String,
}
