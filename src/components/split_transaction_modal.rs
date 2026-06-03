// Presentational modal component for splitting a transaction into multiple entries.
// Manages lists of split amounts, descriptions, and pane targets.
// Performs mathematical validation to ensure total allocation matches the original sum.

use leptos::prelude::*;
use hho_types::RulePane;
use crate::logic::Item;
use crate::components::draggable::use_draggable;

#[derive(Clone, Debug)]
struct SplitDraftRow {
    amount_input: RwSignal<String>,
    description: RwSignal<String>,
    target_pane: RwSignal<RulePane>,
}

#[component]
pub fn SplitTransactionModal<S, C>(
    /// Transaction item target to split.
    item: Item,
    /// Callback to run on split save.
    on_save: S,
    /// Callback to run on split cancel.
    on_cancel: C,
) -> impl IntoView
where
    S: Fn(Vec<(i64, String, RulePane)>) + 'static + Send + Sync + Clone,
    C: Fn() + 'static + Send + Sync + Clone,
{
    // Extract transaction details for template rendering.
    let original_desc = item.txn.description.clone();
    let original_vendor = item.txn.vendor.clone();
    let original_total_cents = item.txn.amount_cents;
    
    // Parse target pane to seed draft row destinations.
    let original_pane = match item.txn.manual_pane {
        Some(pane) => pane,
        None => RulePane::Unassigned,
    };

    // Instantiate two initial draft rows to represent the default split setup.
    let initial_rows = vec![
        SplitDraftRow {
            amount_input: RwSignal::new("".to_string()),
            description: RwSignal::new(original_desc.clone()),
            target_pane: RwSignal::new(RulePane::Joint),
        },
        SplitDraftRow {
            amount_input: RwSignal::new("".to_string()),
            description: RwSignal::new(original_desc.clone()),
            target_pane: RwSignal::new(RulePane::Personal),
        },
    ];
    let draft_rows = RwSignal::new(initial_rows);

    // Calculate sum of all parsed amounts dynamically in cents.
    let current_sum_cents = Memo::new(move |_| {
        draft_rows.get().iter().map(|row| {
            let input = row.amount_input.get();
            hho_types::parse_amount_cents(&input).map(|c| c.abs()).unwrap_or(0)
        }).sum::<i64>()
    });

    // Determine correctness of split validation state.
    let is_valid = Memo::new(move |_| {
        let rows = draft_rows.get();
        if rows.len() < 2 {
            return false;
        }

        // Validate that each individual row has non-empty fields and positive parsed amount.
        for row in &rows {
            let desc = row.description.get();
            if desc.trim().is_empty() {
                return false;
            }

            let amt_str = row.amount_input.get();
            let parsed = hho_types::parse_amount_cents(&amt_str);
            match parsed {
                None => return false,
                Some(cents) if cents <= 0 => return false,
                _ => {}
            }
        }

        // Ensure sum of all portions matches original transaction value.
        current_sum_cents.get() == original_total_cents.abs()
    });

    // Handle cancel actions.
    let on_cancel_click = {
        let on_cancel = on_cancel.clone();
        move |_| on_cancel()
    };

    // Add new split draft row.
    let add_split_row = move |_| {
        draft_rows.update(|rows| {
            rows.push(SplitDraftRow {
                amount_input: RwSignal::new("".to_string()),
                description: RwSignal::new(original_desc.clone()),
                target_pane: RwSignal::new(original_pane),
            });
        });
    };

    // Save configuration and trigger save callback.
    let on_split_save = move |_| {
        if !is_valid.get() {
            return;
        }
        let parsed_splits: Vec<(i64, String, RulePane)> = draft_rows.get_untracked().into_iter().map(|row| {
            let cents = hho_types::parse_amount_cents(&row.amount_input.get_untracked())
                .map(|c| c.abs())
                .unwrap_or(0);
            let desc = row.description.get_untracked().trim().to_string();
            let target = row.target_pane.get_untracked();
            (cents, desc, target)
        }).collect();

        on_save(parsed_splits);
    };

    // Keyboard event listener for escape key cancel handler.
    let on_keydown = {
        let on_cancel = on_cancel.clone();
        move |ev: web_sys::KeyboardEvent| {
            if ev.key() == "Escape" {
                ev.prevent_default();
                on_cancel();
            }
        }
    };

    // Enable custom viewport dragging hook.
    let (drag_style, on_drag_start) = use_draggable();

    view! {
        <div class="modal-overlay">
            <div
                class="modal-container split-modal"
                style=drag_style
                on:click=|ev| ev.stop_propagation()
                on:keydown=on_keydown
            >
                <h2 on:mousedown=on_drag_start>"Split Transaction"</h2>

                // Original Transaction Reference Details Panel
                <div class="split-reference-panel">
                    <div class="ref-title">"Original Transaction"</div>
                    <div class="ref-grid">
                        <div>
                            <span class="ref-label">"Vendor:"</span>
                            <span class="ref-val">{original_vendor.clone()}</span>
                        </div>
                        <div>
                            <span class="ref-label">"Date:"</span>
                            <span class="ref-val">{item.txn.date.clone()}</span>
                        </div>
                        <div>
                            <span class="ref-label">"Total:"</span>
                            <span class="ref-val">{hho_types::format_dollars(original_total_cents)}</span>
                        </div>
                    </div>
                </div>

                // Split Rows Editor Grid Headers
                <div class="split-headers">
                    <div class="header-desc">"Description"</div>
                    <div class="header-amount">"Amount"</div>
                    <div class="header-pane">"Destination Pane"</div>
                    <div class="header-action"></div>
                </div>

                // Dynamic Split Rows Editor List
                <div class="split-rows-list">
                    {move || {
                        let rows = draft_rows.get();
                        rows.into_iter().enumerate().map(|(idx, row)| {
                            let on_remove_row = move |_| {
                                draft_rows.update(|r| {
                                    if r.len() > 1 {
                                        r.remove(idx);
                                    }
                                });
                            };

                            view! {
                                <div class="split-row-item">
                                    // Description Input Field
                                    <input
                                        type="text"
                                        class="split-row-desc"
                                        prop:value=move || row.description.get()
                                        on:input=move |e| row.description.set(event_target_value(&e))
                                        placeholder="Description"
                                        autocomplete="off"
                                    />

                                    // Amount Input Field
                                    <input
                                        type="text"
                                        class="split-row-amount"
                                        prop:value=move || row.amount_input.get()
                                        on:input=move |e| row.amount_input.set(event_target_value(&e))
                                        placeholder="$0.00"
                                        autocomplete="off"
                                    />

                                    // Target Destination Pane Selector Row
                                    <div class="split-row-panes">
                                        {
                                            [RulePane::Joint, RulePane::Personal, RulePane::Ignored, RulePane::Unassigned]
                                                .into_iter()
                                                .map(|p| {
                                                    let is_active = move || row.target_pane.get() == p;
                                                    let set_pane = move |_| row.target_pane.set(p);
                                                    view! {
                                                        <button
                                                            type="button"
                                                            class=move || if is_active() { "split-pane-btn active" } else { "split-pane-btn" }
                                                            on:click=set_pane
                                                        >
                                                            {p.display_title()}
                                                        </button>
                                                    }
                                                }).collect_view()
                                        }
                                    </div>

                                    // Action Delete Row Button
                                    <div class="split-row-action">
                                        {move || (draft_rows.get().len() > 1).then(|| view! {
                                            <button
                                                type="button"
                                                class="split-row-delete"
                                                title="Delete portion"
                                                on:click=on_remove_row
                                            >
                                                "✕"
                                            </button>
                                        })}
                                    </div>
                                </div>
                            }
                        }).collect_view()
                    }}
                </div>

                // Add Row Controls Bar
                <div class="split-controls-row">
                    <button
                        type="button"
                        class="btn btn-secondary add-split-row-btn"
                        on:click=add_split_row
                    >
                        "+ Add Split Portion"
                    </button>
                </div>

                // Validation Allocation Monitor Panel
                <div class="split-validation-panel">
                    {move || {
                        let target = original_total_cents.abs();
                        let sum = current_sum_cents.get();
                        let diff = target - sum;

                        if diff == 0 {
                            view! {
                                <div class="validation-status status-valid">
                                    <span class="status-icon">"✓"</span>
                                    <span>"Balanced! Allocation matches original total."</span>
                                </div>
                            }.into_any()
                        } else if diff > 0 {
                            view! {
                                <div class="validation-status status-invalid">
                                    <span class="status-icon">"⚠"</span>
                                    <span>"Remaining unallocated: " {hho_types::format_dollars(diff)}</span>
                                </div>
                            }.into_any()
                        } else {
                            view! {
                                <div class="validation-status status-invalid">
                                    <span class="status-icon">"⚠"</span>
                                    <span>"Exceeded original total by: " {hho_types::format_dollars(-diff)}</span>
                                </div>
                            }.into_any()
                        }
                    }}
                </div>

                // Action Confirm/Cancel Triggers
                <div class="modal-actions">
                    <button type="button" class="btn btn-secondary" on:click=on_cancel_click>
                        "Cancel"
                    </button>
                    <button
                        type="button"
                        class="btn btn-primary"
                        disabled=move || !is_valid.get()
                        on:click=on_split_save
                    >
                        "Split"
                    </button>
                </div>
            </div>
        </div>
    }
}
