// Generic pane component.
// Derives its own width/height from AppState signals so no size props are
// required; the parent simply places ResizeHandle components between panes.

use crate::logic::ActivePane;
use crate::state::AppState;
use leptos::prelude::*;
use wasm_bindgen::JsCast;

#[component]
pub fn Pane(
    /// Title shown in the pane header.
    title: &'static str,
    /// Which logical pane this instance represents.
    pane_id: ActivePane,
    /// Apply bottom-pane CSS variant (top border colour comes from CSS class).
    #[prop(default = false)]
    is_bottom: bool,
) -> impl IntoView {
    let state: AppState = use_context().expect("AppState must be provided at root");

    let items_sig = state.items_for(pane_id);
    let sel_sig = state.sel_for(pane_id);
    let is_active = move || state.active_pane.get() == pane_id;
    let container_ref = NodeRef::<leptos::html::Div>::new();

    // Scroll the selected item into view reactively when selection or items change.
    Effect::new(move |_| {
        let _ = sel_sig.get();
        let _ = items_sig.get();

        if let Some(container) = container_ref.get() {
            if let Ok(Some(el)) = container.query_selector(".row-item.selected") {
                let options = js_sys::Object::new();
                let _ = js_sys::Reflect::set(&options, &"block".into(), &"nearest".into());
                let _ = js_sys::Reflect::set(&options, &"inline".into(), &"nearest".into());
                if let Ok(method) = js_sys::Reflect::get(&el, &"scrollIntoView".into()) {
                    if let Ok(func) = method.dyn_into::<js_sys::Function>() {
                        let _ = js_sys::Reflect::apply(&func, &el, &js_sys::Array::of1(&options));
                    }
                }
            }
        }
    });

    // Derive inline style from layout signals.
    // Left / right panes: explicit width, flex: none.
    // Middle pane:        flex: 1, min-width: 0 (fill remaining space).
    // Bottom pane:        explicit height driven by bottom_h signal.
    let pane_style = move || match pane_id {
        ActivePane::Left => {
            format!("width: {}px; flex: none;", state.left_width.get())
        }
        ActivePane::Middle => "flex: 1; min-width: 0;".to_string(),
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
                    let total_cents = crate::logic::calculate_total_cents(&items);
                    let main_header = format!("{}:  {}", title, hho_types::format_dollars(total_cents));

                    let is_printable = pane_id == ActivePane::Left || pane_id == ActivePane::Right;

                    let header_title_element = if is_printable {
                        let print_target = state.print_target;
                        let handle_print = move |e: leptos::ev::MouseEvent| {
                            e.stop_propagation();
                            print_target.set(Some(pane_id));
                            if let Some(w) = web_sys::window() {
                                // Schedules the browser print trigger.
                                let cb = wasm_bindgen::closure::Closure::once_into_js(move || {
                                    if let Some(w) = web_sys::window() {
                                        let _ = w.print();
                                        print_target.set(None);
                                    }
                                });
                                let _ = w.request_animation_frame(cb.as_ref().unchecked_ref());
                            }
                        };
                        view! {
                            <div class="pane-header-top">
                                <div class="pane-header-title">{main_header.clone()}</div>
                                <button class="print-btn" on:click=handle_print title="Print transactions">
                                    <svg class="print-icon" viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                                        <polyline points="6 9 6 2 18 2 18 9"></polyline>
                                        <path d="M6 18H4a2 2 0 0 1-2-2v-5a2 2 0 0 1 2-2h16a2 2 0 0 1 2 2v5a2 2 0 0 1-2 2h-2"></path>
                                        <rect x="6" y="14" width="12" height="8"></rect>
                                    </svg>
                                    <span>"Print"</span>
                                </button>
                            </div>
                        }.into_any()
                    } else {
                        view! {
                            <div class="pane-header-title">{main_header.clone()}</div>
                        }.into_any()
                    };

                    if pane_id == ActivePane::Bottom {
                        header_title_element
                    } else {
                        let categories = hho_types::summarize_by_category(
                            items.iter().map(|item| {
                                (item.txn.category.as_str(), hho_types::net_cents(item.txn.amount_cents, item.txn.direction))
                            })
                        );

                        let cat_rows = categories
                            .into_iter()
                            .map(|(name, cat_total)| view! {
                                <div>{format!("{}:  {}", name, hho_types::format_dollars(cat_total))}</div>
                            })
                            .collect_view();

                        view! {
                            {header_title_element}
                            {cat_rows}
                        }.into_any()
                    }
                }}
            </div>
            <div class="pane-rows" node_ref=container_ref>
                {move || {
                    let items = items_sig.get();
                    items
                        .into_iter()
                        .enumerate()
                        .map(|(i, item)| {
                            let is_selected = move || sel_sig.get() == Some(i);
                            let label = item.label.clone();
                            view! {
                                <div
                                    class="row-item"
                                    class:selected=is_selected
                                    class:credit=move || item.txn.direction == hho_types::Direction::Credit
                                    class:auto-matched=move || item.auto_matched
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
