// Modal component for creating auto-assign rules.
// Allows user to match transactions by regex and assign them to target panes.

use leptos::prelude::*;
use wasm_bindgen_futures::spawn_local;
use crate::state::AppState;
use crate::logic::Item;
use hho_types::AutoAssignRule;

/// Extract vendor string from the formatted item label.
fn get_vendor_for_item(state: AppState, item: &Item) -> String {
    let txns = state.raw_transactions.get_untracked();
    for t in txns {
        if t.date == item.date && t.amount_cents == item.amount_cents && t.direction == item.direction {
            if item.label.contains(&t.vendor) {
                return t.vendor;
            }
        }
    }
    let parts: Vec<&str> = item.label.split(" │ ").collect();
    if parts.len() >= 2 {
        parts[1].to_string()
    } else {
        "".to_string()
    }
}

#[component]
pub fn AssignModal(item: Item) -> impl IntoView {
    let state = use_context::<AppState>().expect("AppState missing from context");
    let vendor = get_vendor_for_item(state, &item);
    let vendor_for_memo = vendor.clone();
    let vendor_for_view = vendor.clone();

    let escaped_vendor = crate::logic::escape_regex(&vendor);
    let (regex_input, set_regex_input) = signal(escaped_vendor);
    let (target_pane, set_target_pane) = signal("left".to_string()); // "left" | "right" | "bottom"

    // Tracks regex match result and compilation errors reactively.
    let match_memo = Memo::new(move |_| {
        let regex_val = regex_input.get();
        if regex_val.is_empty() {
            (None, None)
        } else {
            match regex::Regex::new(&regex_val) {
                Ok(re) => {
                    let range = re.find(&vendor_for_memo).map(|m| (m.start(), m.end()));
                    (range, None)
                }
                Err(e) => {
                    (None, Some(e.to_string()))
                }
            }
        }
    });

    let on_cancel = move |_| {
        state.assign_modal_item.set(None);
    };

    let on_save = move |_| {
        let regex_val = regex_input.get_untracked();
        let pane_val = target_pane.get_untracked();
        
        // Prevent saving if the regex is empty or invalid.
        if regex_val.is_empty() || regex::Regex::new(&regex_val).is_err() {
            return;
        }

        let rule = AutoAssignRule {
            regex: regex_val.clone(),
            pane: pane_val.clone(),
        };

        spawn_local(async move {
            state.log(format!(
                "[AutoAssign] saving rule: regex=\"{}\" target={}",
                rule.regex, rule.pane
            ));
            if let Err(e) = crate::ipc::save_auto_assign_rule(rule.clone()).await {
                state.log(format!("[AutoAssign] failed to save rule: {e}"));
            } else {
                state.auto_assign_rules.update(|rules| rules.push(rule));
                state.apply_month_filter();
            }
            state.assign_modal_item.set(None);
        });
    };

    view! {
        <div class="modal-overlay" on:click=on_cancel>
            <div class="modal-container assign-modal" on:click=|ev| ev.stop_propagation()>
                <h2>"Create Auto-Move Rule"</h2>
                
                <div class="modal-field">
                    <label>"Original Vendor Name"</label>
                    <div class="vendor-preview-box">
                        {move || {
                            let (range, _) = match_memo.get();
                            if let Some((start, end)) = range {
                                let prefix = vendor_for_view[..start].to_string();
                                let matched = vendor_for_view[start..end].to_string();
                                let suffix = vendor_for_view[end..].to_string();
                                view! {
                                    <span>
                                        {prefix}
                                        <span class="vendor-preview-highlight">{matched}</span>
                                        {suffix}
                                    </span>
                                }.into_any()
                            } else {
                                view! { <span>{vendor_for_view.clone()}</span> }.into_any()
                            }
                        }}
                    </div>
                </div>

                <div class="modal-field">
                    <label for="regex-input">"Match Transaction (Regex)"</label>
                    <input
                        id="regex-input"
                        type="text"
                        prop:value=regex_input
                        on:input=move |ev| set_regex_input.set(event_target_value(&ev))
                        placeholder="Enter regex or substring"
                        autofocus
                    />
                    {move || {
                        let (_, error) = match_memo.get();
                        error.map(|err| view! {
                            <div class="error-text">"Invalid pattern: " {err}</div>
                        })
                    }}
                </div>

                <div class="modal-field">
                    <label>"Destination Pane"</label>
                    <div class="pane-selector-row">
                        <button
                            type="button"
                            class=move || if target_pane.get() == "left" { "pane-select-btn active" } else { "pane-select-btn" }
                            on:click=move |_| set_target_pane.set("left".to_string())
                        >
                            "Joint"
                        </button>
                        <button
                            type="button"
                            class=move || if target_pane.get() == "right" { "pane-select-btn active" } else { "pane-select-btn" }
                            on:click=move |_| set_target_pane.set("right".to_string())
                        >
                            "Mine"
                        </button>
                        <button
                            type="button"
                            class=move || if target_pane.get() == "bottom" { "pane-select-btn active" } else { "pane-select-btn" }
                            on:click=move |_| set_target_pane.set("bottom".to_string())
                        >
                            "Ignored"
                        </button>
                    </div>
                </div>

                <div class="modal-actions">
                    <button type="button" class="btn btn-secondary" on:click=on_cancel>
                        "Cancel"
                    </button>
                    <button
                        type="button"
                        class="btn btn-primary"
                        on:click=on_save
                        disabled=move || {
                            let regex_val = regex_input.get();
                            let (_, error) = match_memo.get();
                            regex_val.is_empty() || error.is_some()
                        }
                    >
                        "Save & Apply"
                    </button>
                </div>
            </div>
        </div>
    }
}
