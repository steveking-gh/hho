// Reactive application state built on Leptos signals.
// AppState is Copy (all fields are RwSignal, which is Copy + 'static).

use leptos::prelude::*;
use crate::dto::{Direction, PendingMapping, Transaction};
use crate::logic::{ActivePane, Item, next_item_id, transfer_item};

/// Render a transaction as a single-line pane label.
/// Debit shows a leading "-", credit a leading "+".
fn format_txn(t: &Transaction) -> String {
    let dollars = t.amount_cents / 100;
    let cents = (t.amount_cents % 100).abs();
    let sign = match t.direction {
        Direction::Debit => "-",
        Direction::Credit => "+",
    };
    format!("{} │ {} │ {}${}.{:02} │ {}", t.date, t.vendor, sign, dollars, cents, t.category)
}

// ── Drag types ────────────────────────────────────────────────────────────────

/// Identifies which resize boundary is being dragged.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DragTarget {
    LeftHandle,    // vertical divider: Joint | Unassigned
    RightHandle,   // vertical divider: Unassigned | Personal
    TopHandle,     // horizontal divider: top-section | Ignored pane
    BottomHandle,  // horizontal divider: Ignored pane | Debug panel
}

/// Live drag state stored in a signal; None when no drag is in progress.
#[derive(Clone, Copy, Debug)]
pub struct DragState {
    pub target: DragTarget,
    pub last_x: f32,   // client-x at last mousemove event
    pub last_y: f32,   // client-y at last mousemove event
}

// ── Minimum pane dimensions ───────────────────────────────────────────────────

pub const PANE_MIN_W: f32 = 60.0;   // minimum width for left / right panes
pub const PANE_MIN_H: f32 = 40.0;   // minimum height for bottom / debug panes

// ── AppState ──────────────────────────────────────────────────────────────────

/// Central reactive state for the application.
/// All fields are RwSignal (Copy + 'static) so AppState itself is Copy.
#[derive(Clone, Copy)]
pub struct AppState {
    // ── Pane focus / items ────────────────────────────────────────────────────
    pub active_pane:  RwSignal<ActivePane>,
    pub left_items:   RwSignal<Vec<Item>>,
    pub middle_items: RwSignal<Vec<Item>>,
    pub right_items:  RwSignal<Vec<Item>>,
    pub bottom_items: RwSignal<Vec<Item>>,
    pub left_sel:     RwSignal<Option<usize>>,
    pub middle_sel:   RwSignal<Option<usize>>,
    pub right_sel:    RwSignal<Option<usize>>,
    pub bottom_sel:   RwSignal<Option<usize>>,

    // ── Debug log ─────────────────────────────────────────────────────────────
    pub debug_log:    RwSignal<Vec<String>>,

    // ── Accessibility zoom ────────────────────────────────────────────────────
    pub font_scale:   RwSignal<f32>,

    // ── Layout sizes (px; updated by resize handles and restored from config) ─
    pub left_width:   RwSignal<f32>,   // Joint pane width
    pub right_width:  RwSignal<f32>,   // Personal pane width
    pub bottom_h:     RwSignal<f32>,   // Ignored pane height
    pub debug_h:      RwSignal<f32>,   // Debug panel height

    // ── Active drag state ─────────────────────────────────────────────────────
    pub drag:         RwSignal<Option<DragState>>,

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
}

impl AppState {
    pub fn new() -> Self {
        let app = Self {
            active_pane:  RwSignal::new(ActivePane::Middle),
            left_items:   RwSignal::new(vec![]),
            middle_items: RwSignal::new(vec![]),
            right_items:  RwSignal::new(vec![]),
            bottom_items: RwSignal::new(vec![]),
            left_sel:     RwSignal::new(None),
            middle_sel:   RwSignal::new(None),
            right_sel:    RwSignal::new(None),
            bottom_sel:   RwSignal::new(None),
            debug_log:    RwSignal::new(vec![]),
            font_scale:   RwSignal::new(10.0),
            // Defaults match the Tauri-side DEFAULT_* constants; overridden
            // on startup by the get_layout invoke in app.rs.
            left_width:   RwSignal::new(200.0),
            right_width:  RwSignal::new(200.0),
            bottom_h:     RwSignal::new(200.0),
            debug_h:      RwSignal::new(150.0),
            drag:         RwSignal::new(None),
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
        };
        // Sets default month and year to previous calendar month from current date.
        let now = js_sys::Date::new_0();
        let current_y = now.get_full_year() as i32;
        let current_m = now.get_month() as i32 + 1;
        let (prev_y, prev_m) = crate::logic::get_previous_month_year(current_y, current_m);
        app.selected_year.set(prev_y);
        app.selected_month.set(prev_m);
        app
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
            ActivePane::Left   => self.left_items,
            ActivePane::Middle => self.middle_items,
            ActivePane::Right  => self.right_items,
            ActivePane::Bottom => self.bottom_items,
        }
    }

    pub fn sel_for(self, pane: ActivePane) -> RwSignal<Option<usize>> {
        match pane {
            ActivePane::Left   => self.left_sel,
            ActivePane::Middle => self.middle_sel,
            ActivePane::Right  => self.right_sel,
            ActivePane::Bottom => self.bottom_sel,
        }
    }

    /// Append `msg` to the debug log (newest first, capped at 500) and echo
    /// to the browser console.
    pub fn log(self, msg: String) {
        leptos::logging::log!("{}", msg);
        self.debug_log.update(|log| {
            log.insert(0, msg);
            if log.len() > 500 { log.truncate(500); }
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
        let to_items   = self.items_for(to).get_untracked();
        let from_sel   = self.sel_for(from).get_untracked();
        let to_sel     = self.sel_for(to).get_untracked();

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
                let selected_id_in_to = to_sel.and_then(|t_idx| to_items.get(t_idx).map(|item| item.id));

                let (new_from, new_to, new_sel) =
                    transfer_item(from_items, to_items, Some(idx));
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
        let inst = self.current_institution.get_untracked().unwrap_or_else(|| "CSV".to_string());

        let mut filtered: Vec<Transaction> = txns
            .into_iter()
            .filter(|t| crate::logic::match_month_year(&t.date, year, month))
            .collect();

        // Sorts transactions chronologically from oldest to youngest.
        filtered.sort_by(|a, b| a.date.cmp(&b.date));

        let rules = self.auto_assign_rules.get_untracked();
        let compiled_rules: Vec<(regex::Regex, ActivePane, Option<String>)> = rules
            .iter()
            .filter_map(|r| {
                let pane = match r.pane.as_str() {
                    "left" => ActivePane::Left,
                    "right" => ActivePane::Right,
                    "bottom" => ActivePane::Bottom,
                    _ => return None,
                };
                let anchored = format!("^(?:{})$", r.regex);
                regex::Regex::new(&anchored).ok().map(|re| (re, pane, r.category_override.clone()))
            })
            .collect();

        let mut left = vec![];
        let mut middle = vec![];
        let mut right = vec![];
        let mut bottom = vec![];

        for t in filtered {
            let mut matched_pane = None;
            let mut overridden_category = None;
            for (re, pane, cat_override) in &compiled_rules {
                if re.is_match(&t.vendor) {
                    matched_pane = Some(*pane);
                    overridden_category = cat_override.clone();
                    break;
                }
            }

            let category = overridden_category.unwrap_or_else(|| t.category.clone());

            let item = Item {
                id: next_item_id(),
                label: format_txn(&Transaction {
                    date: t.date.clone(),
                    vendor: t.vendor.clone(),
                    category: category.clone(),
                    amount_cents: t.amount_cents,
                    direction: t.direction,
                }),
                amount_cents: t.amount_cents,
                direction: t.direction,
                date: t.date.clone(),
                auto_matched: matched_pane.is_some(),
                category,
            };

            match matched_pane {
                Some(ActivePane::Left) => left.push(item),
                Some(ActivePane::Right) => right.push(item),
                Some(ActivePane::Bottom) => bottom.push(item),
                _ => middle.push(item),
            }
        }

        let left_len = left.len();
        let middle_len = middle.len();
        let right_len = right.len();
        let bottom_len = bottom.len();

        self.left_items.set(left);
        self.middle_items.set(middle);
        self.right_items.set(right);
        self.bottom_items.set(bottom);

        self.left_sel.set(if left_len > 0 { Some(0) } else { None });
        self.middle_sel.set(if middle_len > 0 { Some(0) } else { None });
        self.right_sel.set(if right_len > 0 { Some(0) } else { None });
        self.bottom_sel.set(if bottom_len > 0 { Some(0) } else { None });

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
    use hho_types::AutoAssignRule;

    fn create_test_state() -> AppState {
        AppState {
            active_pane:  RwSignal::new(ActivePane::Middle),
            left_items:   RwSignal::new(vec![]),
            middle_items: RwSignal::new(vec![]),
            right_items:  RwSignal::new(vec![]),
            bottom_items: RwSignal::new(vec![]),
            left_sel:     RwSignal::new(None),
            middle_sel:   RwSignal::new(None),
            right_sel:    RwSignal::new(None),
            bottom_sel:   RwSignal::new(None),
            debug_log:    RwSignal::new(vec![]),
            font_scale:   RwSignal::new(10.0),
            left_width:   RwSignal::new(200.0),
            right_width:  RwSignal::new(200.0),
            bottom_h:     RwSignal::new(200.0),
            debug_h:      RwSignal::new(150.0),
            drag:         RwSignal::new(None),
            pending_mapping: RwSignal::new(None),
            recent_files: RwSignal::new(vec![]),
            selected_year: RwSignal::new(2026),
            selected_month: RwSignal::new(5),
            raw_transactions: RwSignal::new(vec![]),
            current_institution: RwSignal::new(None),
            is_month_modal_open: RwSignal::new(false),
            is_loading_file: RwSignal::new(false),
            auto_assign_rules: RwSignal::new(vec![]),
            assign_modal_item: RwSignal::new(None),
            is_rules_modal_open: RwSignal::new(false),
            is_create_transaction_modal_open: RwSignal::new(false),
        }
    }

    #[test]
    fn test_apply_month_filter_with_category_override() {
        let state = create_test_state();
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
                pane: "left".to_string(),
                category_override: Some("Coffee & Tea".to_string()),
            },
            AutoAssignRule {
                regex: "NETFLIX".to_string(),
                pane: "right".to_string(),
                category_override: None,
            },
        ]);

        state.apply_month_filter();

        // Verify Starbucks matches the rule and applies the category override
        let left = state.left_items.get();
        assert_eq!(left.len(), 1);
        assert_eq!(left[0].category, "Coffee & Tea");
        assert!(left[0].label.contains("Coffee & Tea"));
        assert!(!left[0].label.contains("Uncategorized"));
        assert!(left[0].auto_matched);

        // Verify Netflix matches the rule but retains the original category
        let right = state.right_items.get();
        assert_eq!(right.len(), 1);
        assert_eq!(right[0].category, "Entertainment");
        assert!(right[0].label.contains("Entertainment"));
        assert!(right[0].auto_matched);
    }
}
