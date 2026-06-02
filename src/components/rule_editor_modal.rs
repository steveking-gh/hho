// Presentational modal component for creating and editing auto-assign rules.
// Renders match highlighting preview, validation state, and destination buttons.
// Invokes callbacks on save and cancel actions.

use hho_types::{AutoAssignRule, RulePane};
use leptos::prelude::*;
use crate::components::draggable::use_draggable;

#[component]
pub fn RuleEditorModal<S, C>(
    /// The vendor name to show in the preview highlight box.
    preview_vendor: String,
    /// The description to show in the preview highlight box.
    preview_description: String,
    /// The pre-populated vendor match regex pattern.
    initial_vendor_regex: String,
    /// The pre-populated description match regex pattern.
    initial_description_regex: String,
    /// The pre-populated target pane.
    initial_pane: RulePane,
    /// The pre-populated category override value.
    initial_category_override: String,
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
    let desc_for_memo = preview_description.clone();
    let desc_for_view = preview_description.clone();

    let (vendor_regex_input, set_vendor_regex_input) = signal(initial_vendor_regex);
    let (desc_regex_input, set_desc_regex_input) = signal(initial_description_regex);
    let (target_pane, set_target_pane) = signal(initial_pane);
    let (category_override_input, set_category_override_input) = signal(initial_category_override);

    let input_ref = NodeRef::<leptos::html::Input>::new();

    Effect::new(move |_| {
        if let Some(input) = input_ref.get() {
            let _ = input.focus();
        }
    });

    let is_override_active = Memo::new(move |_| !category_override_input.get().trim().is_empty());

    // Tracks regex match result and compilation errors reactively for vendor.
    let vendor_match_memo = Memo::new(move |_| {
        let regex_val = vendor_regex_input.get();
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

    // Tracks regex match result and compilation errors reactively for description.
    let desc_match_memo = Memo::new(move |_| {
        let regex_val = desc_regex_input.get();
        if regex_val.trim().is_empty() {
            (None, None)
        } else {
            match crate::logic::compile_rule(&regex_val) {
                Ok(re) => {
                    let range = re.find(&desc_for_memo).map(|m| (m.start(), m.end()));
                    (range, None)
                }
                Err(e) => (None, Some(e.to_string())),
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
        let v_regex_val = vendor_regex_input.get_untracked().trim().to_string();
        let d_regex_val = desc_regex_input.get_untracked().trim().to_string();
        let pane_val = target_pane.get_untracked();
        let cat_override = category_override_input.get_untracked().trim().to_string();
        let category_override = if cat_override.is_empty() {
            None
        } else {
            Some(cat_override)
        };

        if v_regex_val.is_empty() && d_regex_val.is_empty() {
            return;
        }

        if !v_regex_val.is_empty() && regex::Regex::new(&v_regex_val).is_err() {
            return;
        }
        if !d_regex_val.is_empty() && regex::Regex::new(&d_regex_val).is_err() {
            return;
        }

        let vendor_regex = if v_regex_val.is_empty() { None } else { Some(v_regex_val) };
        let description_regex = if d_regex_val.is_empty() { None } else { Some(d_regex_val) };

        on_save(AutoAssignRule {
            regex: None,
            vendor_regex,
            description_regex,
            pane: pane_val,
            category_override,
        });
    };

    let (drag_style, on_drag_start) = use_draggable();

    view! {
        <div class="modal-overlay nested-modal-overlay" on:click=on_cancel_overlay>
            <div class="modal-container assign-modal" style=drag_style on:click=|ev| ev.stop_propagation()>
                <h2 on:mousedown=on_drag_start>"Edit Auto-Move Rule"</h2>

                <div class="modal-field">
                    <div class="label-row">
                        <label>"Original Vendor Name"</label>
                        {move || {
                            let (range, _) = vendor_match_memo.get();
                            let regex_val = vendor_regex_input.get();
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
                            let regex_val = vendor_regex_input.get();
                            !regex_val.trim().is_empty() && vendor_match_memo.get().0.is_some()
                        }
                        class:unmatched=move || {
                            let regex_val = vendor_regex_input.get();
                            !regex_val.trim().is_empty() && vendor_match_memo.get().0.is_none()
                        }
                    >
                        {move || {
                            let (range, _) = vendor_match_memo.get();
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
                        prop:value=vendor_regex_input
                        on:input=move |ev| set_vendor_regex_input.set(event_target_value(&ev))
                        placeholder="Enter regex or substring for vendor name"
                        autofocus
                        autocomplete="off"
                    />
                    {move || {
                        let (_, error) = vendor_match_memo.get();
                        error.map(|err| view! {
                            <div class="error-text">"Invalid pattern: " {err}</div>
                        })
                    }}
                </div>

                <div class="modal-field">
                    <div class="label-row">
                        <label>"Original Description"</label>
                        {move || {
                            let (range, _) = desc_match_memo.get();
                            let regex_val = desc_regex_input.get();
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
                            let regex_val = desc_regex_input.get();
                            !regex_val.trim().is_empty() && desc_match_memo.get().0.is_some()
                        }
                        class:unmatched=move || {
                            let regex_val = desc_regex_input.get();
                            !regex_val.trim().is_empty() && desc_match_memo.get().0.is_none()
                        }
                    >
                        {move || {
                            let (range, _) = desc_match_memo.get();
                            if let Some((start, end)) = range {
                                let prefix = desc_for_view[..start].to_string();
                                let matched = desc_for_view[start..end].to_string();
                                let suffix = desc_for_view[end..].to_string();
                                view! {
                                     <span>
                                         {prefix}
                                         <span class="vendor-preview-highlight">{matched}</span>
                                         {suffix}
                                     </span>
                                }.into_any()
                            } else {
                                view! { <span>{desc_for_view.clone()}</span> }.into_any()
                            }
                        }}
                    </div>
                </div>

                <div class="modal-field">
                    <label for="desc-regex-input">"Match Description (Regex)"</label>
                    <input
                        id="desc-regex-input"
                        type="text"
                        prop:value=desc_regex_input
                        on:input=move |ev| set_desc_regex_input.set(event_target_value(&ev))
                        placeholder="Enter regex or substring for description (optional)"
                        autocomplete="off"
                    />
                    {move || {
                        let (_, error) = desc_match_memo.get();
                        error.map(|err| view! {
                            <div class="error-text">"Invalid pattern: " {err}</div>
                        })
                    }}
                </div>

                <div class="modal-field">
                    <label>"Destination Pane"</label>
                    <div class="pane-selector-row">
                        {
                            [RulePane::Joint, RulePane::Personal, RulePane::Ignored].into_iter().map(|p| {
                                view! {
                                    <button
                                        type="button"
                                        class=move || if target_pane.get() == p { "pane-select-btn active" } else { "pane-select-btn" }
                                        on:click=move |_| set_target_pane.set(p)
                                    >
                                        {p.display_title()}
                                    </button>
                                }
                            }).collect_view()
                        }
                    </div>
                </div>

                <div class="modal-field">
                    <div class="label-row">
                        <label for="category-override-input">"Category Override (Optional)"</label>
                        {move || {
                            if is_override_active.get() {
                                view! { <span class="override-on-badge">"Override On"</span> }.into_any()
                            } else {
                                view! { <span style="display: none;"></span> }.into_any()
                            }
                        }}
                    </div>
                    <input
                        id="category-override-input"
                        type="text"
                        class="category-override-field"
                        class:override-active=is_override_active
                        prop:value=category_override_input
                        on:input=move |ev| set_category_override_input.set(event_target_value(&ev))
                        placeholder="Enter category override"
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
                            let v_regex = vendor_regex_input.get();
                            let d_regex = desc_regex_input.get();
                            let (_, v_error) = vendor_match_memo.get();
                            let (_, d_error) = desc_match_memo.get();
                            
                            (v_regex.trim().is_empty() && d_regex.trim().is_empty())
                                || v_error.is_some()
                                || d_error.is_some()
                        }
                    >
                        "OK"
                    </button>
                </div>
            </div>
        </div>
    }
}
