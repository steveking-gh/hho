use crate::logic::Item;
use hho_types::{parse_amount_cents, format_cents, Direction, Transaction};
use leptos::prelude::*;
use crate::components::draggable::use_draggable;

fn parse_cents(s: &str) -> Result<i64, String> {
    let cents = parse_amount_cents(s).ok_or_else(|| "Invalid number format".to_string())?;
    if cents <= 0 {
        return Err("Amount must be greater than zero".to_string());
    }
    Ok(cents)
}

#[component]
pub fn TransactionEditorModal<S, C>(
    item: Item,
    on_save: S,
    on_cancel: C,
) -> impl IntoView
where
    S: Fn(Transaction) + Send + Sync + Clone + 'static,
    C: Fn() + Send + Sync + Clone + 'static,
{
    let default_date = item.txn.date.clone();
    let default_vendor = item.txn.vendor.clone();
    let default_category = item.txn.category.clone();
    let default_amount = format_cents(item.txn.amount_cents);
    let default_direction = item.txn.direction;

    let (date_input, set_date_input) = signal(default_date);
    let (vendor_input, set_vendor_input) = signal(default_vendor);
    let (category_input, set_category_input) = signal(default_category);
    let (amount_input, set_amount_input) = signal(default_amount);
    let (direction_input, set_direction_input) = signal(default_direction);

    let on_cancel_overlay = {
        let on_cancel = on_cancel.clone();
        move |_| on_cancel()
    };

    let on_cancel_btn = {
        let on_cancel = on_cancel.clone();
        move |_| on_cancel()
    };

    let on_save_click = {
        let on_save = on_save.clone();
        let item = item.clone();
        move |_| {
            let date_val = date_input.get_untracked();
            let vendor_val = vendor_input.get_untracked().trim().to_string();
            let category_val = category_input.get_untracked().trim().to_string();
            let amount_val = amount_input.get_untracked();
            let direction_val = direction_input.get_untracked();

            if date_val.is_empty() || date_val.len() != 10 || vendor_val.is_empty() {
                return;
            }

            let cents = match parse_cents(&amount_val) {
                Ok(c) => c,
                Err(_) => return,
            };

            let updated_txn = Transaction {
                id: item.txn.id,
                date: date_val.clone(),
                vendor: vendor_val.clone(),
                category: category_val.clone(),
                amount_cents: cents,
                direction: direction_val,
                manual_pane: item.txn.manual_pane,
            };

            on_save(updated_txn);
        }
    };


    let on_keydown = {
        let on_save = on_save.clone();
        let item = item.clone();
        move |ev: web_sys::KeyboardEvent| {
            if ev.key() == "Enter" {
                ev.prevent_default();
                ev.stop_propagation();
                let date_val = date_input.get_untracked();
                let vendor_val = vendor_input.get_untracked().trim().to_string();
                let category_val = category_input.get_untracked().trim().to_string();
                let amount_val = amount_input.get_untracked();
                let direction_val = direction_input.get_untracked();

                if date_val.is_empty() || date_val.len() != 10 || vendor_val.is_empty() {
                    return;
                }

                let cents = match parse_cents(&amount_val) {
                    Ok(c) => c,
                    Err(_) => return,
                };

                let updated_txn = Transaction {
                    id: item.txn.id,
                    date: date_val,
                    vendor: vendor_val,
                    category: category_val,
                    amount_cents: cents,
                    direction: direction_val,
                    manual_pane: item.txn.manual_pane,
                };

                on_save(updated_txn);
            }
        }
    };

    let (drag_style, on_drag_start) = use_draggable();

    view! {
        <div class="modal-overlay" on:click=on_cancel_overlay>
            <div class="modal-container assign-modal" style=drag_style on:click=|ev| ev.stop_propagation() on:keydown=on_keydown>
                <h2 on:mousedown=on_drag_start>"Edit Transaction"</h2>

                <div class="modal-field">
                    <label for="edit-date">"Date"</label>
                    <input
                        id="edit-date"
                        type="date"
                        prop:value=date_input
                        on:input=move |ev| set_date_input.set(event_target_value(&ev))
                        autocomplete="off"
                    />
                </div>

                <div class="modal-field">
                    <label for="edit-vendor">"Vendor Name"</label>
                    <input
                        id="edit-vendor"
                        type="text"
                        prop:value=vendor_input
                        on:input=move |ev| set_vendor_input.set(event_target_value(&ev))
                        placeholder="Enter vendor name"
                        autofocus
                        autocomplete="off"
                    />
                </div>

                <div class="modal-field">
                    <label for="edit-category">"Category"</label>
                    <input
                        id="edit-category"
                        type="text"
                        prop:value=category_input
                        on:input=move |ev| set_category_input.set(event_target_value(&ev))
                        placeholder="Enter category (optional)"
                        autocomplete="off"
                    />
                </div>

                <div class="modal-field">
                    <label for="edit-amount">"Amount"</label>
                    <input
                        id="edit-amount"
                        type="text"
                        prop:value=amount_input
                        on:input=move |ev| set_amount_input.set(event_target_value(&ev))
                        placeholder="0.00"
                        autocomplete="off"
                    />
                    {move || {
                        let val = amount_input.get();
                        if !val.is_empty() {
                            if let Err(e) = parse_cents(&val) {
                                view! { <div class="error-text">{e}</div> }.into_any()
                            } else {
                                view! { <div style="display: none;"></div> }.into_any()
                            }
                        } else {
                            view! { <div style="display: none;"></div> }.into_any()
                        }
                    }}
                </div>

                <div class="modal-field">
                    <label>"Flow Direction"</label>
                    <div class="pane-selector-row">
                        <button
                            type="button"
                            class=move || if direction_input.get() == Direction::Debit { "pane-select-btn active" } else { "pane-select-btn" }
                            on:click=move |_| set_direction_input.set(Direction::Debit)
                        >
                            "Debit (Money Out)"
                        </button>
                        <button
                            type="button"
                            class=move || if direction_input.get() == Direction::Credit { "pane-select-btn active" } else { "pane-select-btn" }
                            on:click=move |_| set_direction_input.set(Direction::Credit)
                        >
                            "Credit (Money In)"
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
                            let d = date_input.get();
                            let v = vendor_input.get();
                            let a = amount_input.get();
                            d.is_empty() || d.len() != 10 || v.trim().is_empty() || parse_cents(&a).is_err()
                        }
                    >
                        "OK"
                    </button>
                </div>
            </div>
        </div>
    }
}
