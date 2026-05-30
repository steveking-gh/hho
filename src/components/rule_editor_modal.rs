// Presentational modal component for creating and editing auto-assign rules.
// Renders match highlighting preview, validation state, and destination buttons.
// Invokes callbacks on save and cancel actions.

use leptos::prelude::*;
use hho_types::AutoAssignRule;

#[component]
pub fn RuleEditorModal<S, C>(
    /// The vendor name to show in the preview highlight box.
    preview_vendor: String,
    /// The pre-populated match regex pattern.
    initial_regex: String,
    /// The pre-populated target pane ("left" | "right" | "bottom").
    initial_pane: String,
    /// Callback executed when the user saves the rule.
    on_save: S,
    /// Callback executed when the user cancels.
    on_cancel: C,
) -> impl IntoView
where
    S: Fn(AutoAssignRule) + 'static + Send + Sync + Clone,
    C: Fn() + 'static + Send + Sync + Clone,
{
    let vendor_for_memo = preview_vendor.clone();
    let vendor_for_view = preview_vendor.clone();

    let (regex_input, set_regex_input) = signal(initial_regex);
    let (target_pane, set_target_pane) = signal(initial_pane);

    // Tracks regex match result and compilation errors reactively.
    let match_memo = Memo::new(move |_| {
        let regex_val = regex_input.get();
        if regex_val.is_empty() {
            (None, None)
        } else {
            // First check if the original pattern is a valid regex.
            match regex::Regex::new(&regex_val) {
                Ok(_) => {
                    let anchored = format!("^(?:{})$", regex_val);
                    match regex::Regex::new(&anchored) {
                        Ok(re) => {
                            let range = re.find(&vendor_for_memo).map(|m| (m.start(), m.end()));
                            (range, None)
                        }
                        Err(e) => {
                            (None, Some(e.to_string()))
                        }
                    }
                }
                Err(e) => {
                    (None, Some(e.to_string()))
                }
            }
        }
    });

    let on_cancel_overlay = {
        let on_cancel = on_cancel.clone();
        move |_| on_cancel()
    };

    let on_cancel_btn = {
        let on_cancel = on_cancel.clone();
        move |_| on_cancel()
    };

    let on_save_click = move |_| {
        let regex_val = regex_input.get_untracked();
        let pane_val = target_pane.get_untracked();
        
        if regex_val.is_empty() || regex::Regex::new(&regex_val).is_err() {
            return;
        }

        on_save(AutoAssignRule {
            regex: regex_val,
            pane: pane_val,
        });
    };

    view! {
        <div class="modal-overlay nested-modal-overlay" on:click=on_cancel_overlay>
            <div class="modal-container assign-modal" on:click=|ev| ev.stop_propagation()>
                <h2>"Edit Auto-Move Rule"</h2>
                
                <div class="modal-field">
                    <div class="label-row">
                        <label>"Original Vendor Name"</label>
                        {move || {
                            let (range, _) = match_memo.get();
                            if range.is_none() {
                                view! { <span class="no-match-badge">"No Match"</span> }.into_any()
                            } else {
                                view! { <span style="display: none;"></span> }.into_any()
                            }
                        }}
                    </div>
                    <div
                        class="vendor-preview-box"
                        class:matched=move || match_memo.get().0.is_some()
                        class:unmatched=move || match_memo.get().0.is_none()
                    >
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
                    <label for="regex-input-editor">"Match Transaction (Regex)"</label>
                    <input
                        id="regex-input-editor"
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
                    <button type="button" class="btn btn-secondary" on:click=on_cancel_btn>
                        "Cancel"
                    </button>
                    <button
                        type="button"
                        class="btn btn-primary"
                        on:click=on_save_click
                        disabled=move || {
                            let regex_val = regex_input.get();
                            let (_, error) = match_memo.get();
                            regex_val.is_empty() || error.is_some()
                        }
                    >
                        "OK"
                    </button>
                </div>
            </div>
        </div>
    }
}
