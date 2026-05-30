// Reactive application state built on Leptos signals.
// AppState is Copy (all fields are RwSignal, which is Copy + 'static).

use leptos::prelude::*;
use crate::logic::{ActivePane, Item, transfer_item};

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
}

impl AppState {
    pub fn new() -> Self {
        Self {
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
        }
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
}
