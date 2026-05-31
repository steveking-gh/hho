use crate::state::AppState;
use hho_types::{parse_amount_cents, Direction, Transaction};
use leptos::prelude::*;

fn parse_cents(s: &str) -> Result<i64, String> {
    let cents = parse_amount_cents(s).ok_or_else(|| "Invalid number format".to_string())?;
    if cents <= 0 {
        return Err("Amount must be greater than zero".to_string());
    }
    Ok(cents)
}

#[component]
pub fn CreateTransactionModal() -> impl IntoView {
    let state = use_context::<AppState>().expect("AppState missing from context");

    let year = state.selected_year.get_untracked();
    let month = state.selected_month.get_untracked();
    let default_date = format!("{}-{:02}-01", year, month);

    let (date_input, set_date_input) = signal(default_date);
    let (vendor_input, set_vendor_input) = signal("".to_string());
    let (category_input, set_category_input) = signal("".to_string());
    let (amount_input, set_amount_input) = signal("".to_string());
    let (direction_input, set_direction_input) = signal(Direction::Debit);

    let on_cancel_overlay = move |_| {
        state.is_create_transaction_modal_open.set(false);
    };

    let on_cancel_btn = move |_| {
        state.is_create_transaction_modal_open.set(false);
    };

    let on_save_click = move |_| {
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

        let new_txn = Transaction {
            date: date_val.clone(),
            vendor: vendor_val.clone(),
            category: category_val.clone(),
            amount_cents: cents,
            direction: direction_val,
        };

        state.log(format!(
            "[CreateTransaction] adding manual transaction: date={} vendor={} category={} amount_cents={} direction={:?}",
            new_txn.date, new_txn.vendor, new_txn.category, new_txn.amount_cents, new_txn.direction
        ));

        state.raw_transactions.update(|txns| {
            txns.push(new_txn);
        });

        let txn_year: i32 = date_val[0..4].parse().unwrap_or(year);
        let txn_month: i32 = date_val[5..7].parse().unwrap_or(month);
        if txn_year != year || txn_month != month {
            state.selected_year.set(txn_year);
            state.selected_month.set(txn_month);
        }

        state.apply_month_filter();
        state.is_create_transaction_modal_open.set(false);
    };

    view! {
        <div class="modal-overlay" on:click=on_cancel_overlay>
            <div class="modal-container assign-modal" on:click=|ev| ev.stop_propagation()>
                <h2>"Create New Transaction"</h2>

                <div class="modal-field">
                    <label for="manual-date">"Date"</label>
                    <input
                        id="manual-date"
                        type="date"
                        prop:value=date_input
                        on:input=move |ev| set_date_input.set(event_target_value(&ev))
                    />
                </div>

                <div class="modal-field">
                    <label for="manual-vendor">"Vendor Name"</label>
                    <input
                        id="manual-vendor"
                        type="text"
                        prop:value=vendor_input
                        on:input=move |ev| set_vendor_input.set(event_target_value(&ev))
                        placeholder="Enter vendor name"
                        autofocus
                    />
                </div>

                <div class="modal-field">
                    <label for="manual-category">"Category"</label>
                    <input
                        id="manual-category"
                        type="text"
                        prop:value=category_input
                        on:input=move |ev| set_category_input.set(event_target_value(&ev))
                        placeholder="Enter category (optional)"
                    />
                </div>

                <div class="modal-field">
                    <label for="manual-amount">"Amount"</label>
                    <input
                        id="manual-amount"
                        type="text"
                        prop:value=amount_input
                        on:input=move |ev| set_amount_input.set(event_target_value(&ev))
                        placeholder="0.00"
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
