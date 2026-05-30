// Frontend view of the shared IPC contract, plus frontend-only UI state.
//
// All serialized types come from the shared `hho-types` crate, so the frontend
// and backend cannot disagree on field names or shapes.

pub use hho_types::*;

/// Working data for the column-chooser modal while it is open.
/// Mirrors `OpenResult::NeedsMapping`, but is plain UI state — never serialized.
#[derive(Clone, Debug)]
pub struct PendingMapping {
    pub fingerprint: String,
    pub headers: Vec<String>,
    pub sample_rows: Vec<Vec<String>>,
    pub pending_path: String,
    pub suggested: SuggestedMapping,
}
