// Generic pane component — renders a titled column of selectable row items.
// Reads AppState from Leptos context; no prop-drilling required.

use leptos::prelude::*;
use crate::logic::ActivePane;
use crate::state::AppState;

/// A pane displaying a vertically scrollable list of items.
///
/// Click the pane background to activate it without changing row selection.
/// Click a row to activate the pane and select that row.
#[component]
pub fn Pane(
    /// Title shown in the header bar at the top of the pane.
    title: &'static str,
    /// Which logical pane this instance represents.
    pane_id: ActivePane,
    /// Apply bottom-pane CSS variant (fixed height, top orange border).
    #[prop(default = false)] is_bottom: bool,
) -> impl IntoView {
    let state: AppState = use_context().expect("AppState must be provided at root");

    let items_sig = state.items_for(pane_id);
    let sel_sig   = state.sel_for(pane_id);
    let is_active = move || state.active_pane.get() == pane_id;

    view! {
        <div
            class="pane"
            class:active=is_active
            class:bottom=is_bottom
            on:click=move |_| {
                // Background click: activate pane, preserve existing row selection.
                let was = state.active_pane.get_untracked();
                state.active_pane.set(pane_id);
                state.log(format!(
                    "[Click] pane background \"{}\"  (was \"{}\")",
                    pane_id, was
                ));
            }
        >
            <div class="pane-header">{title}</div>
            <div class="pane-rows">
                {move || {
                    let items = items_sig.get();
                    items
                        .into_iter()
                        .enumerate()
                        .map(|(i, item)| {
                            // Capture label before item is consumed by move closure.
                            let label = item.label.clone();
                            view! {
                                <div
                                    class="row-item"
                                    // Reactive: updates on selection change without re-rendering the list.
                                    class:selected=move || sel_sig.get() == Some(i)
                                    on:click=move |e| {
                                        // Stop bubble: pane background handler must not also fire.
                                        e.stop_propagation();
                                        let was_pane = state.active_pane.get_untracked();
                                        let was_sel  = state.sel_for(pane_id).get_untracked();
                                        state.active_pane.set(pane_id);
                                        state.sel_for(pane_id).set(Some(i));
                                        state.log(format!(
                                            "[Click] row {i} \"{label}\" in \"{}\"  \
                                             (was pane=\"{}\" sel={:?}) → sel={}",
                                            pane_id, was_pane, was_sel, i
                                        ));
                                    }
                                >
                                    {item.label}
                                </div>
                            }
                        })
                        .collect_view()
                }}
            </div>
        </div>
    }
}
