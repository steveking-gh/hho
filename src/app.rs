// Root application component.
// Owns AppState, provides it via context, and registers the global key handler.

use leptos::prelude::*;
use leptos::ev;

use crate::logic::{ActivePane, nav_up, nav_down, pane_left, pane_right};
use crate::state::AppState;
use crate::components::{debug_log::DebugLog, pane::Pane};

#[component]
pub fn App() -> impl IntoView {
    let state = AppState::new();

    // Provide state via context so all descendant components can access it
    // without prop-drilling.
    provide_context(state);

    // ── Global keyboard handler ───────────────────────────────────────────
    // Attached to `window` so no DOM element needs focus.
    // Only arrow keys (plain and Shift-modified) are processed; all others ignored.
    let handle = window_event_listener(ev::keydown, move |ev| {
        let key   = ev.key();
        let shift = ev.shift_key();
        let pane  = state.active_pane.get_untracked();

        // Early exit: only handle the eight arrow-key variants.
        if !matches!(key.as_str(), "ArrowUp" | "ArrowDown" | "ArrowLeft" | "ArrowRight") {
            return;
        }

        // Prevent browser default scroll behavior on arrow keys.
        ev.prevent_default();

        let prefix = format!(
            "[KeyDown] {shift_str}{key:<14} active={pane:<14} sel={sel:?}",
            shift_str = if shift { "Shift+" } else { "" },
            key       = key,
            pane      = pane.to_string(),
            sel       = state.sel_for(pane).get_untracked(),
        );

        let detail: String = match (shift, key.as_str()) {

            // ── Row navigation ────────────────────────────────────────────
            (false, "ArrowUp") => {
                let items   = state.items_for(pane).get_untracked();
                let sel     = state.sel_for(pane).get_untracked();
                let new_sel = nav_up(&items, sel);
                state.sel_for(pane).set(new_sel);
                format!("nav up → sel={:?}", new_sel)
            }

            (false, "ArrowDown") => {
                let items   = state.items_for(pane).get_untracked();
                let sel     = state.sel_for(pane).get_untracked();
                let new_sel = nav_down(&items, sel);
                state.sel_for(pane).set(new_sel);
                format!("nav down → sel={:?}", new_sel)
            }

            // ── Pane switching ────────────────────────────────────────────
            (false, "ArrowLeft") => {
                let next = pane_left(pane);
                state.active_pane.set(next);
                if next == pane {
                    "switch left → no-op (already leftmost or bottom pane)".into()
                } else {
                    format!("switch left → active=\"{}\"", next)
                }
            }

            (false, "ArrowRight") => {
                let next = pane_right(pane);
                state.active_pane.set(next);
                if next == pane {
                    "switch right → no-op (already rightmost or bottom pane)".into()
                } else {
                    format!("switch right → active=\"{}\"", next)
                }
            }

            // ── Item movement: Shift+Left ─────────────────────────────────
            // Middle → Left  |  Right → Middle  |  others: no-op
            (true, "ArrowLeft") => match pane {
                ActivePane::Middle => state.transfer(ActivePane::Middle, ActivePane::Left),
                ActivePane::Right  => state.transfer(ActivePane::Right,  ActivePane::Middle),
                ActivePane::Left   => "no-op: no pane left of Joint".into(),
                ActivePane::Bottom => "no-op: Ignored has no left neighbor".into(),
            },

            // ── Item movement: Shift+Right ────────────────────────────────
            // Left → Middle  |  Middle → Right  |  others: no-op
            (true, "ArrowRight") => match pane {
                ActivePane::Left   => state.transfer(ActivePane::Left,   ActivePane::Middle),
                ActivePane::Middle => state.transfer(ActivePane::Middle, ActivePane::Right),
                ActivePane::Right  => "no-op: no pane right of Mine".into(),
                ActivePane::Bottom => "no-op: Ignored has no right neighbor".into(),
            },

            // ── Item movement: Shift+Down (any top pane → Ignored) ────────
            (true, "ArrowDown") => match pane {
                ActivePane::Left | ActivePane::Middle | ActivePane::Right => {
                    state.transfer(pane, ActivePane::Bottom)
                }
                ActivePane::Bottom => "no-op: already in Ignored pane".into(),
            },

            // ── Item movement: Shift+Up (Ignored → Uncategorized) ─────────
            (true, "ArrowUp") => match pane {
                ActivePane::Bottom => state.transfer(ActivePane::Bottom, ActivePane::Middle),
                _ => "no-op: Shift+Up only applies from Ignored pane".into(),
            },

            // Unreachable: the is_arrow guard above eliminates all other keys.
            _ => return,
        };

        state.log(format!("{}  →  {}", prefix, detail));
    });

    // Retain listener for the full app lifetime.
    // on_cleanup fires when the reactive owner drops (never for the root App).
    on_cleanup(move || drop(handle));

    view! {
        <div class="app-container">
            <div class="main-area">
                <div class="top-section">
                    <Pane title="Joint"         pane_id=ActivePane::Left />
                    <Pane title="Uncategorized" pane_id=ActivePane::Middle />
                    <Pane title="Mine"          pane_id=ActivePane::Right />
                </div>
                <Pane title="Ignored" pane_id=ActivePane::Bottom is_bottom=true />
            </div>
            <DebugLog />
        </div>
    }
}
