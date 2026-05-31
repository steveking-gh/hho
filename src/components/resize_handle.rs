// Draggable divider between adjacent panes.
// Mousedown starts a drag recorded in AppState::drag; the global
// mousemove handler in app.rs applies the resulting deltas.

use crate::state::{AppState, DragState, DragTarget};
use leptos::prelude::*;

/// Orientation of the resize boundary.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ResizeDir {
    /// Vertical bar — separates panes side-by-side (col-resize cursor).
    Horizontal,
    /// Horizontal bar — separates panes stacked vertically (row-resize cursor).
    Vertical,
}

#[component]
pub fn ResizeHandle(
    /// Which resize boundary this handle represents.
    target: DragTarget,
    /// Visual orientation: Horizontal = col-resize, Vertical = row-resize.
    dir: ResizeDir,
) -> impl IntoView {
    let state: AppState = use_context().expect("AppState must be provided at root");

    let css_class = match dir {
        ResizeDir::Horizontal => "resize-handle resize-handle-h",
        ResizeDir::Vertical => "resize-handle resize-handle-v",
    };

    // Bottom handle uses the green debug-panel color instead of orange.
    let css_class = if target == DragTarget::BottomHandle {
        "resize-handle resize-handle-v resize-handle-debug"
    } else {
        css_class
    };

    view! {
        <div
            class=css_class
            on:mousedown=move |ev| {
                // prevent_default stops text selection from beginning on drag.
                ev.prevent_default();
                state.drag.set(Some(DragState {
                    target,
                    last_x: ev.client_x() as f32,
                    last_y: ev.client_y() as f32,
                }));
                state.log(format!("[Drag] start {:?}", target));
            }
        />
    }
}
