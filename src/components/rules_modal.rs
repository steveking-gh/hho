// Modal component for viewing and managing auto-assign rules.
// Allows user to select rules, delete rules, and edit rule matching regexes.

use crate::components::rule_editor_modal::RuleEditorModal;
use crate::state::AppState;
use hho_types::AutoAssignRule;
use leptos::prelude::*;
use crate::components::draggable::use_draggable;
use wasm_bindgen_futures::spawn_local;

/// Finds a matching transaction from the loaded CSV to serve as a preview.
fn find_preview_txn(state: AppState, rule: &AutoAssignRule) -> (String, String) {
    let txns = state.raw_transactions.get_untracked();
    let v_pat = rule.vendor_pattern().filter(|s| !s.is_empty());
    let d_pat = rule.description_pattern().filter(|s| !s.is_empty());

    for t in txns {
        let matches_vendor = match v_pat {
            Some(pat) => {
                if let Ok(re) = regex::Regex::new(pat) {
                    re.is_match(&t.vendor)
                } else {
                    false
                }
            }
            None => true,
        };
        let matches_desc = match d_pat {
            Some(pat) => {
                if let Ok(re) = regex::Regex::new(pat) {
                    re.is_match(&t.description)
                } else {
                    false
                }
            }
            None => true,
        };
        if matches_vendor && matches_desc && (v_pat.is_some() || d_pat.is_some()) {
            return (t.vendor.clone(), t.description.clone());
        }
    }
    // Fallback if no matching txn is found:
    (
        v_pat.unwrap_or("").to_string(),
        d_pat.unwrap_or("").to_string(),
    )
}

#[component]
pub fn RulesModal() -> impl IntoView {
    let state = use_context::<AppState>().expect("AppState missing from context");

    // Copy the existing persistent rules to a local draft list on initialization.
    let initial_rules = state.auto_assign_rules.get_untracked();
    let rules_draft = RwSignal::new(initial_rules.clone());

    let initial_sel = if initial_rules.is_empty() {
        None
    } else {
        Some(0)
    };
    let rules_sel = RwSignal::new(initial_sel);
    let editing_rule_index = RwSignal::new(None::<usize>);

    let on_cancel = move |_| {
        state.is_rules_modal_open.set(false);
    };

    let on_save = move |_| {
        let draft = rules_draft.get_untracked();
        let state = state;
        spawn_local(async move {
            state.log(format!(
                "[AutoAssign] saving {} rules to persistent config",
                draft.len()
            ));
            if let Err(e) = crate::ipc::save_auto_assign_rules(state, draft.clone()).await {
                state.log(format!("[AutoAssign] failed to save rules list: {e}"));
            } else {
                state.auto_assign_rules.set(draft);
                state.apply_month_filter();
            }
            state.is_rules_modal_open.set(false);
        });
    };

    // Register local keyboard navigation event listener for rules list.
    let key_handle = window_event_listener(leptos::ev::keydown, move |ev| {
        if !state.is_rules_modal_open.get_untracked() {
            return;
        }
        // Suppress list navigation while the nested editor is open.
        if editing_rule_index.get_untracked().is_some() {
            return;
        }

        let key = ev.key();
        if key == "Escape" {
            ev.prevent_default();
            state.is_rules_modal_open.set(false);
        } else if key == "ArrowUp" {
            ev.prevent_default();
            let draft = rules_draft.get_untracked();
            if !draft.is_empty() {
                let cur: usize = rules_sel.get_untracked().unwrap_or(0);
                let new_sel = cur.saturating_sub(1);
                rules_sel.set(Some(new_sel));
            }
        } else if key == "ArrowDown" {
            ev.prevent_default();
            let draft = rules_draft.get_untracked();
            if !draft.is_empty() {
                let cur: usize = rules_sel.get_untracked().unwrap_or(0);
                let new_sel = (cur + 1).min(draft.len() - 1);
                rules_sel.set(Some(new_sel));
            }
        } else if key == "Enter" {
            ev.prevent_default();
            if let Some(idx) = rules_sel.get_untracked() {
                let draft = rules_draft.get_untracked();
                if idx < draft.len() {
                    editing_rule_index.set(Some(idx));
                }
            }
        }
    });

    on_cleanup(move || {
        drop(key_handle);
    });

    let (drag_style, on_drag_start) = use_draggable();

    view! {
        <div class="modal-overlay" on:click=on_cancel>
            <div class="modal-container rules-modal-container" style=drag_style on:click=|ev| ev.stop_propagation()>
                <h2 on:mousedown=on_drag_start>"Manage Auto-Move Rules"</h2>

                <div class="rules-list-container">
                    <div class="rules-list-header">
                        <div class="col-pattern">"Pattern"</div>
                        <div class="col-pane">"Destination"</div>
                        <div class="col-actions"></div>
                    </div>

                    <div class="rules-list-rows">
                        {move || {
                            let draft = rules_draft.get();
                            if draft.is_empty() {
                                view! { <div class="rules-empty-state">"No rules defined"</div> }.into_any()
                            } else {
                                draft.into_iter().enumerate().map(|(i, rule)| {
                                    let is_selected = move || rules_sel.get() == Some(i);
                                    let rule_clone = rule.clone();

                                    let on_delete_click = move |ev: web_sys::MouseEvent| {
                                        ev.stop_propagation();
                                        rules_draft.update(|rules| {
                                            if i < rules.len() {
                                                rules.remove(i);
                                            }
                                        });
                                        let current_draft = rules_draft.get_untracked();
                                        if current_draft.is_empty() {
                                            rules_sel.set(None);
                                        } else {
                                            let cur = rules_sel.get_untracked().unwrap_or(0);
                                            rules_sel.set(Some(cur.min(current_draft.len() - 1)));
                                        }
                                    };

                                    let on_edit_click = move |ev: web_sys::MouseEvent| {
                                        ev.stop_propagation();
                                        rules_sel.set(Some(i));
                                        editing_rule_index.set(Some(i));
                                    };

                                    let on_row_click = move |_| {
                                        rules_sel.set(Some(i));
                                    };

                                    let on_row_double_click = move |_| {
                                        rules_sel.set(Some(i));
                                        editing_rule_index.set(Some(i));
                                    };

                                    let display_pane = rule_clone.pane.display_title();

                                    view! {
                                        <div
                                            class="rule-row-item"
                                            class:selected=is_selected
                                            on:click=on_row_click
                                            on:dblclick=on_row_double_click
                                        >
                                            <div class="col-pattern">{rule_clone.display_pattern()}</div>
                                            <div class="col-pane">{display_pane}</div>
                                            <div class="col-actions">
                                                <button
                                                    type="button"
                                                    class="btn-edit-rule"
                                                    title="Edit"
                                                    on:click=on_edit_click
                                                >
                                                    "✏️"
                                                </button>
                                                <button
                                                    type="button"
                                                    class="btn-delete-rule"
                                                    title="Delete"
                                                    on:click=on_delete_click
                                                >
                                                    "❌"
                                                </button>
                                            </div>
                                        </div>
                                    }
                                }).collect_view().into_any()
                            }
                        }}
                    </div>
                </div>

                <div class="modal-actions">
                    <button type="button" class="btn btn-secondary" on:click=on_cancel>
                        "Cancel"
                    </button>
                    <button type="button" class="btn btn-primary" on:click=on_save>
                        "Save All"
                    </button>
                </div>
            </div>
        </div>

        // Renders the nested dialog when editing a rule.
        {move || editing_rule_index.get().map(|idx| {
            let draft = rules_draft.get_untracked();
            if idx >= draft.len() { return Option::<String>::None.into_any(); }

            let rule_to_edit = draft[idx].clone();
            let (preview_vendor, preview_description) = find_preview_txn(state, &rule_to_edit);
            let initial_vendor_regex = rule_to_edit.vendor_regex.as_deref().or(rule_to_edit.regex.as_deref()).unwrap_or("").to_string();
            let initial_description_regex = rule_to_edit.description_regex.clone().unwrap_or_default();

            view! {
                <RuleEditorModal
                    preview_vendor=preview_vendor
                    preview_description=preview_description
                    initial_vendor_regex=initial_vendor_regex
                    initial_description_regex=initial_description_regex
                    initial_pane=rule_to_edit.pane
                    initial_category_override=rule_to_edit.category_override.clone().unwrap_or_default()
                    on_save=move |updated_rule: AutoAssignRule| {
                        rules_draft.update(|rules| {
                            if idx < rules.len() {
                                rules[idx] = updated_rule;
                            }
                        });
                        editing_rule_index.set(None);
                    }
                    on_cancel=move || {
                        editing_rule_index.set(None);
                    }
                />
            }.into_any()
        })}
    }
}
