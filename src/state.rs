// Reactive application state built on Leptos signals.
// AppState is Copy (all fields are RwSignal, which is Copy + 'static).

use crate::dto::{PendingMapping, Transaction};
use crate::logic::{classify_transactions, transfer_item, ActivePane, Item};
use leptos::prelude::*;
use std::cell::Cell;

// ── Global state handle (non-reactive / async contexts) ──────────────────────
//
// The IPC layer (`crate::ipc`) mirrors every request/response into the debug log,
// but those wrappers run outside any component and lack `use_context` access to
// AppState. This thread-local holds the single AppState so `ipc.rs` can reach the
// logger. Sound here because the app is single-threaded WASM, AppState is `Copy`,
// and the cell is written exactly once in `AppState::new()`.
//
// Trade-off: a hidden global dependency. Prefer passing state explicitly; this
// exists only for the cross-cutting logging concern. Narrowing it to a logger
// handle (just the debug-log signals) would shrink the global surface.
thread_local! {
    static GLOBAL_STATE: Cell<Option<AppState>> = const { Cell::new(None) };
}

/// Returns the process-global AppState, if one has been constructed.
pub fn get_global_state() -> Option<AppState> {
    GLOBAL_STATE.with(|g| g.get())
}

// ── Drag types ────────────────────────────────────────────────────────────────

/// Identifies which resize boundary is being dragged.
#[allow(clippy::enum_variant_names)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DragTarget {
    LeftHandle,   // vertical divider: Joint | Unassigned
    RightHandle,  // vertical divider: Unassigned | Personal
    TopHandle,    // horizontal divider: top-section | Ignored pane
    BottomHandle, // horizontal divider: Ignored pane | Debug panel
}

/// Live drag state stored in a signal; None when no drag is in progress.
#[derive(Clone, Copy, Debug)]
pub struct DragState {
    pub target: DragTarget,
    pub last_x: f32, // client-x at last mousemove event
    pub last_y: f32, // client-y at last mousemove event
}

// ── Minimum pane dimensions ───────────────────────────────────────────────────

pub const PANE_MIN_W: f32 = 60.0; // minimum width for left / right panes
pub const PANE_MIN_H: f32 = 40.0; // minimum height for bottom / debug panes

// ── AppState ──────────────────────────────────────────────────────────────────

/// Central reactive state for the application.
/// All fields are RwSignal (Copy + 'static) so AppState itself is Copy.
#[derive(Clone, Copy)]
pub struct AppState {
    // ── Pane focus / items ────────────────────────────────────────────────────
    pub active_pane: RwSignal<ActivePane>,
    pub left_items: RwSignal<Vec<Item>>,
    pub middle_items: RwSignal<Vec<Item>>,
    pub right_items: RwSignal<Vec<Item>>,
    pub bottom_items: RwSignal<Vec<Item>>,
    pub left_sel: RwSignal<Option<usize>>,
    pub middle_sel: RwSignal<Option<usize>>,
    pub right_sel: RwSignal<Option<usize>>,
    pub bottom_sel: RwSignal<Option<usize>>,

    // ── Debug log ─────────────────────────────────────────────────────────────
    pub debug_log: RwSignal<Vec<(usize, String)>>,
    pub debug_log_counter: RwSignal<usize>,

    // ── Accessibility zoom ────────────────────────────────────────────────────
    pub font_scale: RwSignal<f32>,

    // ── Layout sizes (px; updated by resize handles and restored from config) ─
    pub left_width: RwSignal<f32>,  // Joint pane width
    pub right_width: RwSignal<f32>, // Personal pane width
    pub bottom_h: RwSignal<f32>,    // Ignored pane height
    pub debug_h: RwSignal<f32>,     // Debug panel height

    // ── Active drag state ─────────────────────────────────────────────────────
    pub drag: RwSignal<Option<DragState>>,

    // ── Pending column mapping (Some → modal is open) ─────────────────────────
    pub pending_mapping: RwSignal<Option<PendingMapping>>,

    // ── Recent files ──────────────────────────────────────────────────────────
    pub recent_files: RwSignal<Vec<String>>,

    // ── Selected period & transactions cache ──────────────────────────────────
    pub selected_year: RwSignal<i32>,
    pub selected_month: RwSignal<i32>,
    pub raw_transactions: RwSignal<Vec<Transaction>>,
    pub current_institution: RwSignal<Option<String>>,
    pub is_month_modal_open: RwSignal<bool>,

    // ── Async operations guard ────────────────────────────────────────────────
    pub is_loading_file: RwSignal<bool>,

    // ── Auto-assign rules and modal state ─────────────────────────────────────
    pub auto_assign_rules: RwSignal<Vec<hho_types::AutoAssignRule>>,
    pub assign_modal_item: RwSignal<Option<Item>>,
    pub is_rules_modal_open: RwSignal<bool>,
    // State signal controlling the manual transaction creation modal.
    pub is_create_transaction_modal_open: RwSignal<bool>,
    // State signal representing the active print target pane.
    pub print_target: RwSignal<Option<ActivePane>>,
    // State signal representing the visibility of the debug log panel.
    pub show_debug_log: RwSignal<bool>,
}

impl AppState {
    pub fn new() -> Self {
        let state = Self {
            active_pane: RwSignal::new(ActivePane::Middle),
            left_items: RwSignal::new(vec![]),
            middle_items: RwSignal::new(vec![]),
            right_items: RwSignal::new(vec![]),
            bottom_items: RwSignal::new(vec![]),
            left_sel: RwSignal::new(None),
            middle_sel: RwSignal::new(None),
            right_sel: RwSignal::new(None),
            bottom_sel: RwSignal::new(None),
            debug_log: RwSignal::new(vec![]),
            debug_log_counter: RwSignal::new(0),
            font_scale: RwSignal::new(10.0),
            // Defaults match the Tauri-side DEFAULT_* constants; overridden
            // on startup by the get_layout invoke in app.rs.
            left_width: RwSignal::new(200.0),
            right_width: RwSignal::new(200.0),
            bottom_h: RwSignal::new(200.0),
            debug_h: RwSignal::new(150.0),
            drag: RwSignal::new(None),
            pending_mapping: RwSignal::new(None),
            recent_files: RwSignal::new(vec![]),
            selected_year: RwSignal::new(0),
            selected_month: RwSignal::new(0),
            raw_transactions: RwSignal::new(vec![]),
            current_institution: RwSignal::new(None),
            is_month_modal_open: RwSignal::new(false),
            is_loading_file: RwSignal::new(false),
            auto_assign_rules: RwSignal::new(vec![]),
            assign_modal_item: RwSignal::new(None),
            is_rules_modal_open: RwSignal::new(false),
            is_create_transaction_modal_open: RwSignal::new(false),
            print_target: RwSignal::new(None),
            show_debug_log: RwSignal::new(false),
        };

        // Save the state globally for IPC/async logging access.
        GLOBAL_STATE.with(|g| g.set(Some(state)));

        state
    }

    /// True while a modal/overlay is open; used to gate header and main actions.
    ///
    /// NOTE: despite the name, this currently checks ONLY `pending_mapping`. The
    /// assign / month / rules / create-transaction modals are not covered, so
    /// header buttons and Ctrl+O are *not* blocked while those are open. The
    /// keydown handler in app.rs checks all of them separately — the two lists
    /// should be unified here so the gating is consistent.
    pub fn any_modal_open(self) -> bool {
        self.pending_mapping.get_untracked().is_some()
    }

    /// Replace the Unassigned pane with parsed transactions, select the
    /// first row, and activate the pane.
    pub fn populate_transactions(self, institution: &str, txns: Vec<Transaction>) {
        self.raw_transactions.set(txns);
        self.current_institution.set(Some(institution.to_string()));
        let state = self;
        wasm_bindgen_futures::spawn_local(async move {
            if let Ok(rules) = crate::ipc::get_auto_assign_rules().await {
                state.auto_assign_rules.set(rules);
            }
            state.apply_month_filter();
            state.active_pane.set(ActivePane::Middle);
            state.refresh_recent_files();
        });
    }

    pub fn items_for(self, pane: ActivePane) -> RwSignal<Vec<Item>> {
        match pane {
            ActivePane::Left => self.left_items,
            ActivePane::Middle => self.middle_items,
            ActivePane::Right => self.right_items,
            ActivePane::Bottom => self.bottom_items,
        }
    }

    pub fn sel_for(self, pane: ActivePane) -> RwSignal<Option<usize>> {
        match pane {
            ActivePane::Left => self.left_sel,
            ActivePane::Middle => self.middle_sel,
            ActivePane::Right => self.right_sel,
            ActivePane::Bottom => self.bottom_sel,
        }
    }

    /// Append a numbered log message to the debug log (newest first, capped at 500) and echo
    /// to the browser console.
    pub fn log(self, msg: String) {
        leptos::logging::log!("{}", msg);
        let counter = self.debug_log_counter.get_untracked();
        self.debug_log_counter.set(counter + 1);
        self.debug_log.update(|log| {
            log.insert(0, (counter, msg));
            if log.len() > 500 {
                log.truncate(500);
            }
        });
    }

    /// Fetches the latest list of recent files from the backend config.
    pub fn refresh_recent_files(self) {
        use wasm_bindgen_futures::spawn_local;
        spawn_local(async move {
            match crate::ipc::get_recent_files().await {
                Ok(files) => self.recent_files.set(files),
                Err(e) => self.log(format!("[File] failed to get recent files: {e}")),
            }
        });
    }

    /// Move selected item in `from` to `to`, keeping the target sorted; return log description.
    pub fn transfer(self, from: ActivePane, to: ActivePane) -> String {
        let from_items = self.items_for(from).get_untracked();
        let to_items = self.items_for(to).get_untracked();
        let from_sel = self.sel_for(from).get_untracked();
        let to_sel = self.sel_for(to).get_untracked();

        match from_sel {
            None => format!("no-op: {} has no selection", from),
            Some(idx) if idx >= from_items.len() => {
                format!("no-op: {} sel={idx} out of range", from)
            }
            Some(idx) => {
                let mut from_items = from_items;
                // Clear the auto-matched flag upon manual movement.
                from_items[idx].auto_matched = false;
                let moved_label = from_items[idx].label.clone();

                // Tracks the ID of the selected item in target pane to restore it after sorting.
                let selected_id_in_to =
                    to_sel.and_then(|t_idx| to_items.get(t_idx).map(|item| item.id));

                let (new_from, new_to, new_sel) = transfer_item(from_items, to_items, Some(idx));
                self.items_for(from).set(new_from);
                self.items_for(to).set(new_to.clone());
                self.sel_for(from).set(new_sel);

                // Restores target pane selection index based on item ID.
                if let Some(target_id) = selected_id_in_to {
                    let new_to_sel = new_to.iter().position(|item| item.id == target_id);
                    self.sel_for(to).set(new_to_sel);
                }

                format!(
                    "moved \"{moved_label}\" {} → {} | {}_sel={:?}",
                    from, to, from, new_sel
                )
            }
        }
    }

    /// Prunes transaction list by selected month/year and populates the middle pane.
    pub fn apply_month_filter(self) {
        let year = self.selected_year.get_untracked();
        let month = self.selected_month.get_untracked();
        let txns = self.raw_transactions.get_untracked();
        let inst = self
            .current_institution
            .get_untracked()
            .unwrap_or_else(|| "CSV".to_string());

        let mut filtered: Vec<Transaction> = txns
            .into_iter()
            .filter(|t| crate::logic::match_month_year(&t.date, year, month))
            .collect();

        // Sorts transactions chronologically from oldest to youngest.
        filtered.sort_by(|a, b| a.date.cmp(&b.date));

        let rules = self.auto_assign_rules.get_untracked();
        let (left, middle, right, bottom) = classify_transactions(filtered, &rules);

        let left_len = left.len();
        let middle_len = middle.len();
        let right_len = right.len();
        let bottom_len = bottom.len();

        self.left_items.set(left);
        self.middle_items.set(middle);
        self.right_items.set(right);
        self.bottom_items.set(bottom);

        self.left_sel.set(if left_len > 0 { Some(0) } else { None });
        self.middle_sel
            .set(if middle_len > 0 { Some(0) } else { None });
        self.right_sel
            .set(if right_len > 0 { Some(0) } else { None });
        self.bottom_sel
            .set(if bottom_len > 0 { Some(0) } else { None });

        let count = left_len + middle_len + right_len + bottom_len;
        self.log(format!(
            "[Filter] Applied {year}-{month:02} to \"{inst}\" → {count} transactions loaded (Unassigned={middle_len}, auto-assigned={})",
            left_len + right_len + bottom_len
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dto::{Direction, Transaction};
    use hho_types::{AutoAssignRule, RulePane};

    #[test]
    fn test_apply_month_filter_with_category_override() {
        let state = AppState::new();
        state.selected_year.set(2026);
        state.selected_month.set(5);
        state.raw_transactions.set(vec![
            Transaction {
                date: "2026-05-15".to_string(),
                vendor: "STARBUCKS COFFEE".to_string(),
                category: "Uncategorized".to_string(),
                amount_cents: 450,
                direction: Direction::Debit,
            },
            Transaction {
                date: "2026-05-16".to_string(),
                vendor: "NETFLIX".to_string(),
                category: "Entertainment".to_string(),
                amount_cents: 1599,
                direction: Direction::Debit,
            },
        ]);
        state.auto_assign_rules.set(vec![
            AutoAssignRule {
                regex: "STARBUCKS.*".to_string(),
                pane: RulePane::Joint,
                category_override: Some("Coffee & Tea".to_string()),
            },
            AutoAssignRule {
                regex: "NETFLIX".to_string(),
                pane: RulePane::Personal,
                category_override: None,
            },
        ]);

        state.apply_month_filter();

        // Verify Starbucks matches the rule and applies the category override
        let left = state.left_items.get();
        assert_eq!(left.len(), 1);
        assert_eq!(left[0].txn.category, "Coffee & Tea");
        assert!(left[0].label.contains("Coffee & Tea"));
        assert!(!left[0].label.contains("Uncategorized"));
        assert!(left[0].auto_matched);

        // Verify Netflix matches the rule but retains the original category
        let right = state.right_items.get();
        assert_eq!(right.len(), 1);
        assert_eq!(right[0].txn.category, "Entertainment");
        assert!(right[0].label.contains("Entertainment"));
        assert!(right[0].auto_matched);
    }
}
