// Generic pane component.
// Derives its own width/height from AppState signals so no size props are
// required; the parent simply places ResizeHandle components between panes.

use leptos::prelude::*;
use crate::logic::ActivePane;
use crate::state::AppState;

#[component]
pub fn Pane(
    /// Title shown in the pane header.
    title: &'static str,
    /// Which logical pane this instance represents.
    pane_id: ActivePane,
    /// Apply bottom-pane CSS variant (top border colour comes from CSS class).
    #[prop(default = false)] is_bottom: bool,
) -> impl IntoView {
    let state: AppState = use_context().expect("AppState must be provided at root");

    let items_sig = state.items_for(pane_id);
    let sel_sig   = state.sel_for(pane_id);
    let is_active = move || state.active_pane.get() == pane_id;

    // Derive inline style from layout signals.
    // Left / right panes: explicit width, flex: none.
    // Middle pane:        flex: 1, min-width: 0 (fill remaining space).
    // Bottom pane:        explicit height driven by bottom_h signal.
    let pane_style = move || match pane_id {
        ActivePane::Left => {
            format!("width: {}px; flex: none;", state.left_width.get())
        }
        ActivePane::Middle => {
            "flex: 1; min-width: 0;".to_string()
        }
        ActivePane::Right => {
            format!("width: {}px; flex: none;", state.right_width.get())
        }
        ActivePane::Bottom => {
            format!("height: {}px;", state.bottom_h.get())
        }
    };

    view! {
        <div
            class="pane"
            class:active=is_active
            class:bottom=is_bottom
            style=pane_style
            on:click=move |_| {
                let was = state.active_pane.get_untracked();
                state.active_pane.set(pane_id);
                state.log(format!(
                    "[Click] pane background \"{}\"  (was \"{}\")",
                    pane_id, was
                ));
            }
        >
            <div class="pane-header">
                {move || {
                    let items = items_sig.get();
                    // Sums all transaction amounts, treating debits as negative and credits as positive.
                    let total_cents = crate::logic::calculate_total_cents(&items);
                    let abs_cents = total_cents.abs();
                    let dollars = abs_cents / 100;
                    let cents = abs_cents % 100;
                    let sign = if total_cents < 0 { "-" } else { "" };
                    format!("{}:  {}${}.{:02}", title, sign, dollars, cents)
                }}
            </div>
            <div class="pane-rows">
                {move || {
                    let items = items_sig.get();
                    items
                        .into_iter()
                        .enumerate()
                        .map(|(i, item)| {
                            let label = item.label.clone();
                            view! {
                                <div
                                    class="row-item"
                                    class:selected=move || sel_sig.get() == Some(i)
                                    on:click=move |e| {
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
