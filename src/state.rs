// Reactive application state built on Leptos signals.
// AppState is Copy (all fields are RwSignal, which is Copy + 'static),
// so it can be moved into closures without cloning.

use leptos::prelude::*;
use crate::logic::{ActivePane, Item, transfer_item};

/// Central reactive state for the four-pane application.
#[derive(Clone, Copy)]
pub struct AppState {
    pub active_pane:  RwSignal<ActivePane>,

    // Item lists for each pane.
    pub left_items:   RwSignal<Vec<Item>>,
    pub middle_items: RwSignal<Vec<Item>>,
    pub right_items:  RwSignal<Vec<Item>>,
    pub bottom_items: RwSignal<Vec<Item>>,

    // Currently selected row index per pane (None = no selection).
    pub left_sel:     RwSignal<Option<usize>>,
    pub middle_sel:   RwSignal<Option<usize>>,
    pub right_sel:    RwSignal<Option<usize>>,
    pub bottom_sel:   RwSignal<Option<usize>>,

    // Debug log entries (newest first).
    pub debug_log:    RwSignal<Vec<String>>,

    // Current root font-size in px (1rem); Ctrl+/-/0 adjusts this.
    pub font_scale:   RwSignal<f32>,
}

impl AppState {
    /// Construct initial state: Uncategorized active, three seed items, row 0 selected.
    pub fn new() -> Self {
        Self {
            active_pane:  RwSignal::new(ActivePane::Middle),
            left_items:   RwSignal::new(vec![]),
            middle_items: RwSignal::new(vec![]),
            right_items:  RwSignal::new(vec![]),
            bottom_items: RwSignal::new(vec![]),
            left_sel:     RwSignal::new(None),
            middle_sel:   RwSignal::new(Some(0)),
            right_sel:    RwSignal::new(None),
            bottom_sel:   RwSignal::new(None),
            debug_log:    RwSignal::new(vec![]),
            font_scale:   RwSignal::new(10.0),
        }
    }

    /// Return the item-list signal for the given pane.
    pub fn items_for(self, pane: ActivePane) -> RwSignal<Vec<Item>> {
        match pane {
            ActivePane::Left   => self.left_items,
            ActivePane::Middle => self.middle_items,
            ActivePane::Right  => self.right_items,
            ActivePane::Bottom => self.bottom_items,
        }
    }

    /// Return the selection signal for the given pane.
    pub fn sel_for(self, pane: ActivePane) -> RwSignal<Option<usize>> {
        match pane {
            ActivePane::Left   => self.left_sel,
            ActivePane::Middle => self.middle_sel,
            ActivePane::Right  => self.right_sel,
            ActivePane::Bottom => self.bottom_sel,
        }
    }

    /// Append `msg` to the debug log (newest first, capped at 500 entries)
    /// and echo to the browser console.
    pub fn log(self, msg: String) {
        leptos::logging::log!("{}", msg);
        self.debug_log.update(|log| {
            log.insert(0, msg);
            if log.len() > 500 {
                log.truncate(500);
            }
        });
    }

    /// Move the selected item in `from` to the end of `to`.
    /// Focus stays in `from`; its selection adjusts to the next available row.
    /// Returns a human-readable description for the debug log.
    pub fn transfer(self, from: ActivePane, to: ActivePane) -> String {
        let from_items = self.items_for(from).get_untracked();
        let to_items   = self.items_for(to).get_untracked();
        let from_sel   = self.sel_for(from).get_untracked();

        match from_sel {
            None => {
                format!("no-op: {} has no selection", from)
            }
            Some(idx) if idx >= from_items.len() => {
                format!("no-op: {} sel={idx} out of range", from)
            }
            Some(idx) => {
                let moved_label = from_items[idx].label.clone();
                let (new_from, new_to, new_sel) =
                    transfer_item(from_items, to_items, Some(idx));

                // reactive_graph batches synchronous signal writes automatically.
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
}
