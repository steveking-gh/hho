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
    format!("{} │ {} │ {}${}.{:02}", t.date, t.vendor, sign, dollars, cents)
}

// ── Drag types ────────────────────────────────────────────────────────────────

/// Identifies which resize boundary is being dragged.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DragTarget {
    LeftHandle,    // vertical divider: Joint | Uncategorized
    RightHandle,   // vertical divider: Uncategorized | Mine
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
    pub right_width:  RwSignal<f32>,   // Mine pane width
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

    /// Replace the Uncategorized pane with parsed transactions, select the
    /// first row, and activate the pane.
    pub fn populate_transactions(self, institution: &str, txns: Vec<Transaction>) {
        self.raw_transactions.set(txns);
        self.current_institution.set(Some(institution.to_string()));
        self.apply_month_filter();
        self.active_pane.set(ActivePane::Middle);
        self.refresh_recent_files();
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

    /// Move selected item in `from` to the end of `to`; return log description.
    pub fn transfer(self, from: ActivePane, to: ActivePane) -> String {
        let from_items = self.items_for(from).get_untracked();
        let to_items   = self.items_for(to).get_untracked();
        let from_sel   = self.sel_for(from).get_untracked();

        match from_sel {
            None => format!("no-op: {} has no selection", from),
            Some(idx) if idx >= from_items.len() => {
                format!("no-op: {} sel={idx} out of range", from)
            }
            Some(idx) => {
                let moved_label = from_items[idx].label.clone();
                let (new_from, new_to, new_sel) =
                    transfer_item(from_items, to_items, Some(idx));
                self.items_for(from).set(new_from);
                self.items_for(to).set(new_to);
                self.sel_for(from).set(new_sel);
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

        let filtered: Vec<Transaction> = txns
            .into_iter()
            .filter(|t| crate::logic::match_month_year(&t.date, year, month))
            .collect();

        let count = filtered.len();
        let items: Vec<Item> = filtered
            .iter()
            .map(|t| Item { id: next_item_id(), label: format_txn(t) })
            .collect();
        
        self.middle_items.set(items);
        self.middle_sel.set(if count > 0 { Some(0) } else { None });
        self.log(format!(
            "[Filter] Applied {year}-{month:02} to \"{inst}\" → {count} transactions loaded into Uncategorized"
        ));
    }
}
