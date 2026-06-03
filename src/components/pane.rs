// Generic pane component.
// Derives its own width/height from AppState signals so no size props are
// required; the parent simply places ResizeHandle components between panes.

use crate::logic::ActivePane;
use crate::state::AppState;
use leptos::prelude::*;
use wasm_bindgen::JsCast;

#[component]
pub fn Pane(
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
                    let main_header = format!("{}:  {}", pane_id, hho_types::format_dollars(total_cents));

                    let header_title_element = match pane_id {
                        ActivePane::Left | ActivePane::Right => {
                            let print_target = state.print_target;
                            let handle_print = move |e: leptos::ev::MouseEvent| {
                                e.stop_propagation();
                                // Setting print_target renders <PrintView> into the DOM.
                                print_target.set(Some(pane_id));
                                if let Some(w) = web_sys::window() {
                                    // Defer window.print() to the next animation frame so the
                                    // print layout has painted before the dialog snapshots the
                                    // page; clear the target afterward to remove that layout.
                                    let cb = wasm_bindgen::closure::Closure::once_into_js(move || {
                                        if let Some(w) = web_sys::window() {
                                            let _ = w.print();
                                            print_target.set(None);
                                        }
                                    });
                                    let _ = w.request_animation_frame(cb.as_ref().unchecked_ref());
                                }
                            };

                            let handle_save = move |e: leptos::ev::MouseEvent| {
                                e.stop_propagation();
                                crate::components::header::save_pane(state, pane_id, &pane_id.to_string());
                            };

                            view! {
                                <div class="pane-header-top">
                                    <div class="pane-header-title">{main_header.clone()}</div>
                                    <div class="pane-header-actions">
                                        <button class="pane-action-btn" on:click=handle_print title="Print transactions">
                                            <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                                                <polyline points="6 9 6 2 18 2 18 9"></polyline>
                                                <path d="M6 18H4a2 2 0 0 1-2-2v-5a2 2 0 0 1 2-2h16a2 2 0 0 1 2 2v5a2 2 0 0 1-2 2h-2"></path>
                                                <rect x="6" y="14" width="12" height="8"></rect>
                                            </svg>
                                            <span>"Print"</span>
                                        </button>
                                        <button class="pane-action-btn" on:click=handle_save title="Save transactions">
                                            <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                                                <path d="M19 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h11l5 5v11a2 2 0 0 1-2 2z"></path>
                                                <polyline points="17 21 17 13 7 13 7 21"></polyline>
                                                <polyline points="7 3 7 8 15 8"></polyline>
                                            </svg>
                                            <span>"Save"</span>
                                        </button>
                                    </div>
                                </div>
                            }.into_any()
                        }
                        ActivePane::Middle => {
                            let handle_new = move |e: leptos::ev::MouseEvent| {
                                e.stop_propagation();
                                state.is_create_transaction_modal_open.set(true);
                            };

                            view! {
                                <div class="pane-header-top">
                                    <div class="pane-header-title">{main_header.clone()}</div>
                                    <div class="pane-header-actions">
                                        <button class="pane-action-btn" on:click=handle_new title="New transaction">
                                            <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                                                <line x1="12" y1="5" x2="12" y2="19"></line>
                                                <line x1="5" y1="12" x2="19" y2="12"></line>
                                            </svg>
                                            <span>"New"</span>
                                        </button>
                                    </div>
                                </div>
                            }.into_any()
                        }
                        _ => {
                            view! {
                                <div class="pane-header-title">{main_header.clone()}</div>
                            }.into_any()
                        }
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
                            let label_click = label.clone();
                            let item_clone1 = item.clone();
                            let item_clone2 = item.clone();
                            let item_clone3 = item.clone();
                            let item_clone4 = item.clone();
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
                                            "[Click] row {i} \"{label_click}\" in \"{}\"  \
                                             (was pane=\"{}\" sel={:?}) → sel={}",
                                             pane_id, was_pane, was_sel, i
                                         ));
                                     }
                                >
                                    <span class="row-label">
                                        {let parts: Vec<String> = label.split(" │ ").map(|s| s.to_string()).collect();
                                        if parts.len() == 5 {
                                            let p0 = parts[0].clone();
                                            let p1 = parts[1].clone();
                                            let p2 = parts[2].clone();
                                            let p3 = parts[3].clone();
                                            let p4 = parts[4].clone();
                                            let is_nickname = item.txn.nickname.is_some();
                                            view! {
                                                {p0} " │ "
                                                <span class:nickname-applied=is_nickname>
                                                    {p1}
                                                </span>
                                                " │ "
                                                {p2} " │ "
                                                {p3} " │ "
                                                {p4}
                                            }.into_any()
                                        } else {
                                            label.clone().into_any()
                                        }}
                                    </span>
                                    {move || (pane_id == ActivePane::Middle).then(|| {
                                        let item_edit = item_clone1.clone();
                                        let item_rule = item_clone2.clone();
                                        let item_split = item_clone3.clone();
                                        let item_nickname = item_clone4.clone();
                                        view! {
                                            <div class="row-actions">
                                                <button
                                                    type="button"
                                                    class="row-action-btn edit-btn"
                                                    title="Edit transaction (Shift+Enter)"
                                                    on:click=move |e| {
                                                        e.stop_propagation();
                                                        state.editing_transaction_item.set(Some(item_edit.clone()));
                                                    }
                                                >
                                                    <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                                                        <path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7"></path>
                                                        <path d="M18.5 2.5a2.121 2.121 0 1 1 3 3L12 15l-4 1 1-4z"></path>
                                                    </svg>
                                                </button>
                                                <button
                                                    type="button"
                                                    class="row-action-btn split-btn"
                                                    title="Split Transaction (s)"
                                                    on:click=move |e| {
                                                        e.stop_propagation();
                                                        state.split_transaction_item.set(Some(item_split.clone()));
                                                    }
                                                >
                                                    <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                                                        <circle cx="6" cy="6" r="3"></circle>
                                                        <circle cx="6" cy="18" r="3"></circle>
                                                        <line x1="20" y1="4" x2="8.12" y2="15.88"></line>
                                                        <line x1="14.47" y1="14.48" x2="20" y2="20"></line>
                                                        <line x1="8.12" y1="8.12" x2="12" y2="12"></line>
                                                    </svg>
                                                </button>
                                                <button
                                                    type="button"
                                                    class="row-action-btn rule-btn"
                                                    title="Create auto-assign rule (Enter)"
                                                    on:click=move |e| {
                                                        e.stop_propagation();
                                                        state.assign_modal_item.set(Some(item_rule.clone()));
                                                    }
                                                >
                                                    <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                                                        <polygon points="13 2 3 14 12 14 11 22 21 10 12 10 13 2"></polygon>
                                                    </svg>
                                                </button>
                                                <button
                                                    type="button"
                                                    class="row-action-btn nickname-btn"
                                                    title="Assign vendor nickname (N)"
                                                    on:click=move |e| {
                                                        e.stop_propagation();
                                                        state.nickname_modal_item.set(Some(item_nickname.clone()));
                                                    }
                                                >
                                                    <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                                                        <path d="M20.59 13.41l-7.17 7.17a2 2 0 0 1-2.83 0L2 12V2h10l8.59 8.59a2 2 0 0 1 0 2.82z"></path>
                                                        <line x1="7" y1="7" x2="7.01" y2="7"></line>
                                                    </svg>
                                                </button>
                                            </div>
                                        }
                                    })}
                                </div>
                            }
                        })
                        .collect_view()
                }}
            </div>
        </div>
    }
}
