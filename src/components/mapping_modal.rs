// Column-chooser modal, shown when an unknown CSV header is opened.
// Local signals hold the in-progress mapping; Save & Apply persists it via the
// save_mapping command and populates the Unassigned pane with the result.

use leptos::prelude::*;
use wasm_bindgen_futures::spawn_local;

use crate::dto::{AmountScheme, AmountSchemeTag, Institution, PendingMapping};
use crate::state::AppState;

/// Split a comma-separated label string into trimmed, non-empty entries.
fn split_labels(s: &str) -> Vec<String> {
    s.split(',')
        .map(|x| x.trim().to_string())
        .filter(|x| !x.is_empty())
        .collect()
}

/// Build `<option>` entries `"<index>: <header>"` for a column `<select>`.
fn col_options(headers: &[String]) -> impl IntoView {
    headers
        .iter()
        .enumerate()
        .map(|(i, h)| view! { <option value=i.to_string()>{format!("{i}: {h}")}</option> })
        .collect_view()
}

/// Build dropdown option entries with a 'None' placeholder for the Category selection.
fn category_col_options(headers: &[String]) -> impl IntoView {
    let none_opt = view! { <option value="none">"None"</option> };
    let rest = headers
        .iter()
        .enumerate()
        .map(|(i, h)| view! { <option value=i.to_string()>{format!("{i}: {h}")}</option> })
        .collect_view();
    view! {
        {none_opt}
        {rest}
    }
}

#[component]
pub fn MappingModal(pm: PendingMapping) -> impl IntoView {
    let state: AppState = use_context().expect("AppState must be provided at root");

    let n = pm.headers.len();
    let s = &pm.suggested;

    // ── Form signals, seeded from the backend's heuristic suggestion ──────────
    let name = RwSignal::new(String::new());
    let date_col = RwSignal::new(s.date_col);
    let vendor_col = RwSignal::new(s.vendor_col);
    let category_col = RwSignal::new(s.category_col);
    let amount_col = RwSignal::new(s.amount_col);
    let scheme = RwSignal::new(s.scheme);
    let debit_is_negative = RwSignal::new(s.debit_is_negative);
    let type_col = RwSignal::new(s.type_col.unwrap_or(0));
    let debit_labels = RwSignal::new("Sale,DEBIT,DR".to_string());
    let credit_labels = RwSignal::new("Payment,CREDIT,CR".to_string());

    // One hidden-flag per column; seeded from suggested ignore_cols.
    let hidden = RwSignal::new({
        let mut v = vec![false; n];
        for &i in &s.ignore_cols {
            if i < n {
                v[i] = true;
            }
        }
        v
    });

    // Owned clones for the various view fragments (avoids borrow gymnastics).
    let headers = pm.headers.clone();
    let h_date = pm.headers.clone();
    let h_vendor = pm.headers.clone();
    let h_category = pm.headers.clone();
    let h_amount = pm.headers.clone();
    let h_type = pm.headers.clone();
    let sample_rows = pm.sample_rows.clone();
    let pending_path = pm.pending_path.clone();
    let fingerprint = pm.fingerprint.clone();

    // ── Save handler ──────────────────────────────────────────────────────────
    let on_save = move |_| {
        // Collect hidden column indices.
        let ignore_cols: Vec<usize> = hidden
            .get_untracked()
            .iter()
            .enumerate()
            .filter_map(|(i, &h)| h.then_some(i))
            .collect();

        // Build the scheme as a typed enum — invalid field combinations are
        // unrepresentable, and serde produces exactly what the backend expects.
        let amount = if scheme.get_untracked() == AmountSchemeTag::TypeColumn {
            AmountScheme::TypeColumn {
                amount_col: amount_col.get_untracked(),
                type_col: type_col.get_untracked(),
                debit_labels: split_labels(&debit_labels.get_untracked()),
                credit_labels: split_labels(&credit_labels.get_untracked()),
            }
        } else {
            AmountScheme::SingleSigned {
                amount_col: amount_col.get_untracked(),
                debit_is_negative: debit_is_negative.get_untracked(),
            }
        };

        let entered = name.get_untracked();
        let inst_name = if entered.trim().is_empty() {
            "Unnamed".to_string()
        } else {
            entered.trim().to_string()
        };

        let institution = Institution {
            name: inst_name.clone(),
            fingerprint: fingerprint.clone(),
            date_col: date_col.get_untracked(),
            vendor_col: vendor_col.get_untracked(),
            category_col: category_col.get_untracked(),
            ignore_cols,
            amount,
        };

        let path = pending_path.clone();
        state.log(format!(
            "[Mapping] saving \"{inst_name}\" and parsing file…"
        ));

        spawn_local(async move {
            match crate::ipc::save_mapping(institution, path).await {
                Ok(txns) => state.populate_transactions(&inst_name, txns),
                Err(e) => state.log(format!("[Mapping] save_mapping failed: {e}")),
            }
            // Always dismiss the modal, even on error, so the UI never gets stuck.
            state.pending_mapping.set(None);
        });
    };

    let on_cancel = move |_| {
        state.log("[Mapping] cancelled".to_string());
        state.pending_mapping.set(None);
    };

    view! {
        <div class="modal-overlay">
            <div class="modal">
                <h2 class="modal-title">"Map Columns — New Institution"</h2>

                <label class="modal-field">
                    <span>"Institution name"</span>
                    <input
                        class="modal-input"
                        prop:value=move || name.get()
                        on:input=move |e| name.set(event_target_value(&e))
                        placeholder="e.g. Chase Sapphire"
                    />
                </label>

                // ── Data preview ──────────────────────────────────────────────
                <div class="modal-section-label">"Preview (first rows)"</div>
                <div class="modal-preview">
                    <table>
                        <thead>
                            <tr>
                                {headers.iter().map(|h| view! { <th>{h.clone()}</th> }).collect_view()}
                            </tr>
                        </thead>
                        <tbody>
                            {sample_rows.iter().map(|row| {
                                view! {
                                    <tr>
                                        {row.iter().map(|c| view! { <td>{c.clone()}</td> }).collect_view()}
                                    </tr>
                                }
                            }).collect_view()}
                        </tbody>
                    </table>
                </div>

                // ── Date / Vendor ─────────────────────────────────────────────
                <label class="modal-field">
                    <span>"Transaction Date column"</span>
                    <select
                        prop:value=move || date_col.get().to_string()
                        on:change=move |e| date_col.set(event_target_value(&e).parse().unwrap_or(0))
                    >
                        {col_options(&h_date)}
                    </select>
                </label>

                <label class="modal-field">
                    <span>"Vendor Name column"</span>
                    <select
                        prop:value=move || vendor_col.get().to_string()
                        on:change=move |e| vendor_col.set(event_target_value(&e).parse().unwrap_or(0))
                    >
                        {col_options(&h_vendor)}
                    </select>
                </label>

                <label class="modal-field">
                    <span>"Category column (optional)"</span>
                    <select
                        prop:value=move || category_col.get().map(|c| c.to_string()).unwrap_or_else(|| "none".to_string())
                        on:change=move |e| {
                            let val = event_target_value(&e);
                            if val == "none" {
                                category_col.set(None);
                            } else {
                                category_col.set(val.parse().ok());
                            }
                        }
                    >
                        {category_col_options(&h_category)}
                    </select>
                </label>

                // ── Amount / direction scheme ─────────────────────────────────
                <div class="modal-section-label">"Amount / Direction"</div>
                <div class="modal-radios">
                    <label>
                        <input
                            type="radio" name="scheme"
                            prop:checked=move || scheme.get() == AmountSchemeTag::SingleSigned
                            on:change=move |_| scheme.set(AmountSchemeTag::SingleSigned)
                        />
                        " Single signed column"
                    </label>
                    <label>
                        <input
                            type="radio" name="scheme"
                            prop:checked=move || scheme.get() == AmountSchemeTag::TypeColumn
                            on:change=move |_| scheme.set(AmountSchemeTag::TypeColumn)
                        />
                        " Type column"
                    </label>
                </div>

                <label class="modal-field">
                    <span>"Amount column"</span>
                    <select
                        prop:value=move || amount_col.get().to_string()
                        on:change=move |e| amount_col.set(event_target_value(&e).parse().unwrap_or(0))
                    >
                        {col_options(&h_amount)}
                    </select>
                </label>

                // Single-signed extra: debit sign convention.
                {move || (scheme.get() == AmountSchemeTag::SingleSigned).then(|| view! {
                    <div class="modal-radios">
                        <span>"Debit is:"</span>
                        <label>
                            <input
                                type="radio" name="debitsign"
                                prop:checked=move || debit_is_negative.get()
                                on:change=move |_| debit_is_negative.set(true)
                            />
                            " negative"
                        </label>
                        <label>
                            <input
                                type="radio" name="debitsign"
                                prop:checked=move || !debit_is_negative.get()
                                on:change=move |_| debit_is_negative.set(false)
                            />
                            " positive"
                        </label>
                    </div>
                })}

                // Type-column extra: type column + label lists.
                {move || (scheme.get() == AmountSchemeTag::TypeColumn).then({
                    let h_type = h_type.clone();
                    move || view! {
                        <label class="modal-field">
                            <span>"Type column"</span>
                            <select
                                prop:value=move || type_col.get().to_string()
                                on:change=move |e| type_col.set(event_target_value(&e).parse().unwrap_or(0))
                            >
                                {col_options(&h_type)}
                            </select>
                        </label>
                        <label class="modal-field">
                            <span>"Debit labels (comma-separated)"</span>
                            <input
                                class="modal-input"
                                prop:value=move || debit_labels.get()
                                on:input=move |e| debit_labels.set(event_target_value(&e))
                            />
                        </label>
                        <label class="modal-field">
                            <span>"Credit labels (comma-separated)"</span>
                            <input
                                class="modal-input"
                                prop:value=move || credit_labels.get()
                                on:input=move |e| credit_labels.set(event_target_value(&e))
                            />
                        </label>
                    }
                })}

                // ── Hidden columns ────────────────────────────────────────────
                <div class="modal-section-label">"Hidden columns (excluded from the UI)"</div>
                <div class="modal-checks">
                    {headers.iter().enumerate().map(|(i, h)| {
                        let label = format!("{i}: {h}");
                        view! {
                            <label class="modal-check">
                                <input
                                    type="checkbox"
                                    prop:checked=move || hidden.get().get(i).copied().unwrap_or(false)
                                    on:change=move |_| hidden.update(|v| {
                                        if i < v.len() { v[i] = !v[i]; }
                                    })
                                />
                                {label}
                            </label>
                        }
                    }).collect_view()}
                </div>

                // ── Actions ───────────────────────────────────────────────────
                <div class="modal-actions">
                    <button class="modal-btn" on:click=on_cancel>"Cancel"</button>
                    <button class="modal-btn modal-btn-primary" on:click=on_save>"Save & Apply"</button>
                </div>
            </div>
        </div>
    }
}
