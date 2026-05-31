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

#[component]
fn ColumnSelect(label: &'static str, headers: Vec<String>, value: RwSignal<usize>) -> impl IntoView {
    view! {
        <label class="modal-field">
            <span>{label}</span>
            <select on:change=move |e| {
                // Option values are always valid indices so parsing succeeds.
                value.set(event_target_value(&e).parse().unwrap_or(0));
            }>
                {headers.into_iter().enumerate().map(|(i, h)| view! {
                    <option value=i.to_string() prop:selected=move || value.get() == i>
                        {format!("{i}: {h}")}
                    </option>
                }).collect_view()}
            </select>
        </label>
    }
}

#[component]
fn OptionalColumnSelect(
    label: &'static str,
    headers: Vec<String>,
    value: RwSignal<Option<usize>>,
) -> impl IntoView {
    view! {
        <label class="modal-field">
            <span>{label}</span>
            <select
                on:change=move |e| {
                    let val = event_target_value(&e);
                    if val == "none" {
                        value.set(None);
                    } else {
                        // Option values are always valid indices or "none", so parsing succeeds.
                        value.set(val.parse().ok());
                    }
                }
            >
                <option value="none" prop:selected=move || value.get().is_none()>"None"</option>
                {headers.into_iter().enumerate().map(|(i, h)| view! {
                    <option value=i.to_string() prop:selected=move || value.get() == Some(i)>
                        {format!("{i}: {h}")}
                    </option>
                }).collect_view()}
            </select>
        </label>
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
            match crate::ipc::save_mapping(state, institution, path).await {
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
                <ColumnSelect label="Transaction Date column" headers=headers.clone() value=date_col />
                <ColumnSelect label="Vendor Name column" headers=headers.clone() value=vendor_col />
                <OptionalColumnSelect label="Category column (optional)" headers=headers.clone() value=category_col />

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

                <ColumnSelect label="Amount column" headers=headers.clone() value=amount_col />

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
                {
                    let headers = headers.clone();
                    move || (scheme.get() == AmountSchemeTag::TypeColumn).then({
                        let headers = headers.clone();
                        move || view! {
                            <ColumnSelect label="Type column" headers=headers.clone() value=type_col />
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
