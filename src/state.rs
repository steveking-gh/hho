// Reactive application state built on Leptos signals.
// AppState is Copy (all fields are RwSignal, which is Copy + 'static).

use crate::dto::{PendingMapping, Transaction};
use crate::logic::{classify_transactions, transfer_item, ActivePane, Item};
use leptos::prelude::*;

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
    pub nickname_rules: RwSignal<Vec<hho_types::NicknameRule>>,
    pub nickname_modal_item: RwSignal<Option<Item>>,
    pub is_nickname_manager_open: RwSignal<bool>,
    // State signal controlling the manual transaction creation modal.
    pub is_create_transaction_modal_open: RwSignal<bool>,
    // State signal representing the active print target pane.
    pub print_target: RwSignal<Option<ActivePane>>,
    // State signal representing the visibility of the debug log panel.
    pub show_debug_log: RwSignal<bool>,
    // State signal controlling the transaction editing modal.
    pub editing_transaction_item: RwSignal<Option<Item>>,
    // State signal controlling the transaction splitting modal.
    pub split_transaction_item: RwSignal<Option<Item>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
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
            nickname_rules: RwSignal::new(vec![]),
            nickname_modal_item: RwSignal::new(None),
            is_nickname_manager_open: RwSignal::new(false),
            is_create_transaction_modal_open: RwSignal::new(false),
            print_target: RwSignal::new(None),
            show_debug_log: RwSignal::new(false),
            editing_transaction_item: RwSignal::new(None),
            split_transaction_item: RwSignal::new(None),
        }
    }

    /// Returns true if any modal or overlay is currently open to gate header and main actions.
    pub fn any_modal_open(self) -> bool {
        self.pending_mapping.get_untracked().is_some()
            || self.assign_modal_item.get_untracked().is_some()
            || self.nickname_modal_item.get_untracked().is_some()
            || self.is_nickname_manager_open.get_untracked()
            || self.editing_transaction_item.get_untracked().is_some()
            || self.split_transaction_item.get_untracked().is_some()
            || self.is_month_modal_open.get_untracked()
            || self.is_rules_modal_open.get_untracked()
            || self.is_create_transaction_modal_open.get_untracked()
    }

    /// Replace the Unassigned pane with parsed transactions, select the
    /// first row, and activate the pane.
    pub fn populate_transactions(self, institution: &str, mut txns: Vec<Transaction>) {
        for (i, t) in txns.iter_mut().enumerate() {
            t.id = Some(i as u32);
        }
        self.raw_transactions.set(txns);
        self.current_institution.set(Some(institution.to_string()));
        self.apply_month_filter();
        self.active_pane.set(ActivePane::Middle);
        self.refresh_recent_files();
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
        let state = self;
        spawn_local(async move {
            match crate::ipc::get_recent_files(state).await {
                Ok(files) => state.recent_files.set(files),
                Err(e) => state.log(format!("[File] failed to get recent files: {e}")),
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
                let txn_id = from_items[idx].txn.id;

                let rule_pane = match to {
                    ActivePane::Left => hho_types::RulePane::Joint,
                    ActivePane::Right => hho_types::RulePane::Personal,
                    ActivePane::Bottom => hho_types::RulePane::Ignored,
                    ActivePane::Middle => hho_types::RulePane::Unassigned,
                };
                from_items[idx].txn.manual_pane = Some(rule_pane);

                if let Some(tid) = txn_id {
                    self.raw_transactions.update(|raw| {
                        if let Some(t) = raw.iter_mut().find(|t| t.id == Some(tid)) {
                            t.manual_pane = Some(rule_pane);
                        }
                    });
                }

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
        let nickname_rules = self.nickname_rules.get_untracked();
        let (left, middle, right, bottom) = classify_transactions(filtered, &rules, &nickname_rules);

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
                id: None,
                date: "2026-05-15".to_string(),
                vendor: "STARBUCKS COFFEE".to_string(),
                category: "Coffee & Tea".to_string(),
                amount_cents: 450,
                direction: Direction::Debit,
                manual_pane: None,
                ..Default::default()
            },
            Transaction {
                id: None,
                date: "2026-05-16".to_string(),
                vendor: "NETFLIX".to_string(),
                category: "Entertainment".to_string(),
                amount_cents: 1599,
                direction: Direction::Debit,
                manual_pane: None,
                ..Default::default()
            },
        ]);
        state.auto_assign_rules.set(vec![
            AutoAssignRule {
                regex: Some("STARBUCKS.*".to_string()),
                vendor_regex: None,
                description_regex: None,
                pane: RulePane::Joint,
                category_override: Some("Coffee & Tea".to_string()),
            },
            AutoAssignRule {
                regex: Some("NETFLIX".to_string()),
                vendor_regex: None,
                description_regex: None,
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

    #[test]
    fn test_transaction_id_and_editing_flow() {
        let state = AppState::new();

        // Populate transactions and verify sequential IDs are assigned.
        let mut txns = vec![
            Transaction {
                id: None,
                date: "2026-05-15".to_string(),
                vendor: "STARBUCKS".to_string(),
                category: "Coffee".to_string(),
                amount_cents: 450,
                direction: Direction::Debit,
                manual_pane: None,
                ..Default::default()
            },
            Transaction {
                id: None,
                date: "2026-05-16".to_string(),
                vendor: "NETFLIX".to_string(),
                category: "Streaming".to_string(),
                amount_cents: 1599,
                direction: Direction::Debit,
                manual_pane: None,
                ..Default::default()
            },
        ];
        for (i, t) in txns.iter_mut().enumerate() {
            t.id = Some(i as u32);
        }
        state.raw_transactions.set(txns);

        let populated = state.raw_transactions.get();
        assert_eq!(populated[0].id, Some(0));
        assert_eq!(populated[1].id, Some(1));

        // Locate a transaction by ID and update it.
        state.raw_transactions.update(|raw| {
            if let Some(t) = raw.iter_mut().find(|t| t.id == Some(1)) {
                t.vendor = "NETFLIX PREMIUM".to_string();
                t.amount_cents = 2299;
            }
        });

        // Verify the edits were correctly saved in AppState.
        let updated = state.raw_transactions.get();
        assert_eq!(updated[1].vendor, "NETFLIX PREMIUM");
        assert_eq!(updated[1].amount_cents, 2299);
    }

    #[test]
    fn test_transaction_splitting_flow() {
        let state = AppState::new();
        state.selected_year.set(2026);
        state.selected_month.set(5);

        // Populate a single transaction to split.
        state.raw_transactions.set(vec![Transaction {
            id: Some(10),
            date: "2026-05-15".to_string(),
            vendor: "TARGET".to_string(),
            category: "Shopping".to_string(),
            amount_cents: 10000,
            direction: Direction::Debit,
            manual_pane: None,
            ..Default::default()
        }]);

        // Define splits representing the desired target amounts, descriptions, and panes.
        let splits = [
            (3000, "Split part 1".to_string(), RulePane::Joint),
            (7000, "Split part 2".to_string(), RulePane::Personal),
        ];

        let tx_id = Some(10);

        // Execute transaction split logic.
        state.raw_transactions.update(|txns| {
            if let Some(pos) = txns.iter().position(|t| t.id == tx_id) {
                let mut next_id = txns.iter().filter_map(|t| t.id).max().unwrap_or(0) + 1;
                let base_txn = txns[pos].clone();

                // Modify original transaction in-place for first split portion.
                txns[pos].amount_cents = splits[0].0;
                txns[pos].description = splits[0].1.clone();
                txns[pos].manual_pane = Some(splits[0].2);

                // Append copied transactions for remaining split portions.
                #[allow(clippy::explicit_counter_loop)]
                for split in splits.iter().skip(1) {
                    let mut new_txn = base_txn.clone();
                    new_txn.id = Some(next_id);
                    next_id += 1;
                    new_txn.amount_cents = split.0;
                    new_txn.description = split.1.clone();
                    new_txn.manual_pane = Some(split.2);
                    txns.push(new_txn);
                }
            }
        });

        // Verify state updates in raw transactions.
        let updated = state.raw_transactions.get();
        assert_eq!(updated.len(), 2);

        // Validate first split transaction properties.
        assert_eq!(updated[0].id, Some(10));
        assert_eq!(updated[0].amount_cents, 3000);
        assert_eq!(updated[0].description, "Split part 1");
        assert_eq!(updated[0].manual_pane, Some(RulePane::Joint));
        assert_eq!(updated[0].vendor, "TARGET");
        assert_eq!(updated[0].date, "2026-05-15");

        // Validate second split transaction properties.
        assert_eq!(updated[1].id, Some(11));
        assert_eq!(updated[1].amount_cents, 7000);
        assert_eq!(updated[1].description, "Split part 2");
        assert_eq!(updated[1].manual_pane, Some(RulePane::Personal));
        assert_eq!(updated[1].vendor, "TARGET");
        assert_eq!(updated[1].date, "2026-05-15");

        // Apply month filter to update interactive UI panes.
        state.apply_month_filter();

        // Verify routing of split transactions to designated panes.
        let left_items = state.left_items.get();
        assert_eq!(left_items.len(), 1);
        assert_eq!(left_items[0].txn.id, Some(10));
        assert_eq!(left_items[0].txn.description, "Split part 1");

        let right_items = state.right_items.get();
        assert_eq!(right_items.len(), 1);
        assert_eq!(right_items[0].txn.id, Some(11));
        assert_eq!(right_items[0].txn.description, "Split part 2");
    }

    #[test]

    fn test_apply_month_filter_ignored_pane_credit() {
        let state = AppState::new();
        state.selected_year.set(2026);
        state.selected_month.set(5);
        state.raw_transactions.set(vec![
            Transaction {
                id: None,
                date: "2026-05-15".to_string(),
                vendor: "PAYMENT RECEIVED".to_string(),
                category: "Income".to_string(),
                amount_cents: 5000,
                direction: Direction::Credit,
                manual_pane: None,
                ..Default::default()
            },
        ]);
        state.auto_assign_rules.set(vec![
            AutoAssignRule {
                regex: Some("PAYMENT.*".to_string()),
                vendor_regex: None,
                description_regex: None,
                pane: RulePane::Ignored,
                category_override: None,
            },
        ]);

        state.apply_month_filter();

        // Verify the transaction was routed to the Bottom (Ignored) pane
        assert_eq!(state.bottom_items.get().len(), 1);
        assert_eq!(state.bottom_items.get()[0].txn.vendor, "PAYMENT RECEIVED");
        assert_eq!(state.bottom_items.get()[0].txn.direction, Direction::Credit);
        assert!(state.bottom_items.get()[0].auto_matched);
        assert_eq!(state.bottom_sel.get(), Some(0));
    }

    #[test]
    fn test_apply_month_filter_preserves_manual_assignments() {
        let state = AppState::new();
        state.selected_year.set(2026);
        state.selected_month.set(5);
        state.raw_transactions.set(vec![
            Transaction {
                id: Some(1),
                date: "2026-05-15".to_string(),
                vendor: "STARBUCKS".to_string(),
                category: "Coffee".to_string(),
                amount_cents: 450,
                direction: Direction::Debit,
                manual_pane: Some(RulePane::Personal), // manually moved to Personal
                ..Default::default()
            },
            Transaction {
                id: Some(2),
                date: "2026-05-16".to_string(),
                vendor: "NETFLIX".to_string(),
                category: "Streaming".to_string(),
                amount_cents: 1599,
                direction: Direction::Debit,
                manual_pane: None, // routes normally by rules
                ..Default::default()
            },
        ]);
        state.auto_assign_rules.set(vec![
            AutoAssignRule {
                regex: Some("STARBUCKS".to_string()),
                vendor_regex: None,
                description_regex: None,
                pane: RulePane::Joint, // Rule would route Starbucks to Joint
                category_override: None,
            },
            AutoAssignRule {
                regex: Some("NETFLIX".to_string()),
                vendor_regex: None,
                description_regex: None,
                pane: RulePane::Joint, // Rule routes Netflix to Joint
                category_override: None,
            },
        ]);

        state.apply_month_filter();

        // Starbucks should be in the Personal (right) pane because of the manual override
        let right_items = state.right_items.get();
        assert_eq!(right_items.len(), 1);
        assert_eq!(right_items[0].txn.vendor, "STARBUCKS");
        assert!(!right_items[0].auto_matched); // Manual movement is not auto-matched

        // Netflix should be in the Joint (left) pane because it has no override and matches rules
        let left_items = state.left_items.get();
        assert_eq!(left_items.len(), 1);
        assert_eq!(left_items[0].txn.vendor, "NETFLIX");
        assert!(left_items[0].auto_matched);

        // Neither Starbucks nor Netflix should be in the Middle (Unassigned) pane
        assert_eq!(state.middle_items.get().len(), 0);
    }
}
