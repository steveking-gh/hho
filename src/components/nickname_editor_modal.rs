// Presentational modal component for creating and editing nickname rules.
// Renders match highlighting preview, validation state, and save buttons.

use hho_types::NicknameRule;
use leptos::prelude::*;
use crate::components::draggable::use_draggable;

#[component]
pub fn NicknameEditorModal<S, C>(
    /// The vendor name to show in the preview highlight box.
    preview_vendor: String,
    /// The pre-populated vendor match regex pattern.
    initial_regex: String,
    /// The pre-populated nickname value.
    initial_nickname: String,
    /// Callback to execute when the user saves the rule.
    on_save: S,
    /// Callback to execute when the user cancels.
    on_cancel: C,
) -> impl IntoView
where
    S: Fn(NicknameRule) + 'static + Send + Sync + Clone,
    C: Fn() + 'static + Send + Sync + Clone,
{
    let vendor_for_memo = preview_vendor.clone();
    let vendor_for_view = preview_vendor.clone();

    let (regex_input, set_regex_input) = signal(initial_regex);
    let (nickname_input, set_nickname_input) = signal(initial_nickname);

    let input_ref = NodeRef::<leptos::html::Input>::new();

    Effect::new(move |_| {
        if let Some(input) = input_ref.get() {
            let _ = input.focus();
        }
    });

    // Tracks regex match result and compilation errors reactively.
    let match_memo = Memo::new(move |_| {
        let regex_val = regex_input.get();
        if regex_val.trim().is_empty() {
            (None, None)
        } else {
            match crate::logic::compile_rule(&regex_val) {
                Ok(re) => {
                    let range = re.find(&vendor_for_memo).map(|m| (m.start(), m.end()));
                    (range, None)
                }
                Err(e) => (None, Some(e.to_string())),
            }
        }
    });

    let on_cancel_btn = {
        let on_cancel = on_cancel.clone();
        move |_| on_cancel()
    };

    let on_save_click = move |_| {
        let regex_val = regex_input.get_untracked().trim().to_string();
        let nickname_val = nickname_input.get_untracked().trim().to_string();

        if regex_val.is_empty() || nickname_val.is_empty() {
            return;
        }

        if regex::Regex::new(&regex_val).is_err() {
            return;
        }

        on_save(NicknameRule {
            regex: regex_val,
            nickname: nickname_val,
        });
    };

    let (drag_style, on_drag_start) = use_draggable();

    view! {
        <div class="modal-overlay nested-modal-overlay">
            <div class="modal-container assign-modal" style=drag_style on:click=|ev| ev.stop_propagation()>
                <h2 on:mousedown=on_drag_start>"Edit Vendor Nickname Rule"</h2>

                <div class="modal-field">
                    <div class="label-row">
                        <label>"Original Vendor Name"</label>
                        {move || {
                            let (range, _) = match_memo.get();
                            let regex_val = regex_input.get();
                            if !regex_val.trim().is_empty() && range.is_none() {
                                view! { <span class="no-match-badge">"No Match"</span> }.into_any()
                            } else {
                                view! { <span style="display: none;"></span> }.into_any()
                            }
                        }}
                    </div>
                    <div
                        class="vendor-preview-box"
                        class:matched=move || {
                            let regex_val = regex_input.get();
                            !regex_val.trim().is_empty() && match_memo.get().0.is_some()
                        }
                        class:unmatched=move || {
                            let regex_val = regex_input.get();
                            !regex_val.trim().is_empty() && match_memo.get().0.is_none()
                        }
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
                    <label for="vendor-regex-input">"Match Vendor (Regex)"</label>
                    <input
                        node_ref=input_ref
                        id="vendor-regex-input"
                        type="text"
                        prop:value=regex_input
                        on:input=move |ev| set_regex_input.set(event_target_value(&ev))
                        placeholder="Enter regex or substring for vendor name"
                        autofocus
                        autocomplete="off"
                    />
                    {move || {
                        let (_, error) = match_memo.get();
                        error.map(|err| view! {
                            <div class="error-text">"Invalid pattern: " {err}</div>
                        })
                    }}
                </div>

                <div class="modal-field">
                    <label for="nickname-input">"Vendor Nickname"</label>
                    <input
                        id="nickname-input"
                        type="text"
                        prop:value=nickname_input
                        on:input=move |ev| set_nickname_input.set(event_target_value(&ev))
                        placeholder="Enter shortened nickname"
                        autocomplete="off"
                    />
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
                            let r_val = regex_input.get();
                            let n_val = nickname_input.get();
                            let (_, v_error) = match_memo.get();

                            r_val.trim().is_empty()
                                || n_val.trim().is_empty()
                                || v_error.is_some()
                        }
                    >
                        "OK"
                    </button>
                </div>
            </div>
        </div>
    }
}
