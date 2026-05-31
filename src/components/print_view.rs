// Printable view for a pane's transactions (Date / Vendor / Amount / Category
// with per-category subtotals and a grand total).
//
// Always mounted, but renders nothing until `print_target` is Some — the Pane's
// print button sets it, then triggers `window.print()` (see pane.rs). A
// `@media print` block in styles.css hides the app chrome and shows only this
// view, so the printed page / "Save as PDF" output contains just the table.

use crate::logic::ActivePane;
use crate::state::AppState;
use leptos::prelude::*;

#[component]
pub fn PrintView() -> impl IntoView {
    let state: AppState = use_context().expect("AppState must be provided at root");

    let print_target = state.print_target;

    view! {
        {move || {
            let Some(pane_id) = print_target.get() else {
                // Renders empty view if print target is none.
                return ().into_any();
            };

            let title = match pane_id {
                ActivePane::Left => "Joint",
                ActivePane::Right => "Personal",
                _ => "",
            };

            let items = state.items_for(pane_id).get();
            let year = state.selected_year.get();
            let month = state.selected_month.get();
            let month_name = crate::logic::get_month_name(month);

            let total_cents = crate::logic::calculate_total_cents(&items);

            let categories = hho_types::summarize_by_category(
                items.iter().map(|item| {
                    (item.txn.category.as_str(), hho_types::net_cents(item.txn.amount_cents, item.txn.direction))
                })
            );

            let rows = items.iter().map(|item| {
                let net = hho_types::net_cents(item.txn.amount_cents, item.txn.direction);
                let date = item.txn.date.clone();
                let vendor = item.txn.vendor.clone();
                let category = item.txn.category.clone();
                view! {
                    <tr>
                        <td>{date}</td>
                        <td>{vendor}</td>
                        <td class="amount">{hho_types::format_dollars(net)}</td>
                        <td>{category}</td>
                    </tr>
                }
            }).collect_view();

            let subtotal_rows = categories.into_iter().map(|(cat_name, cat_total)| {
                view! {
                    <tr class="subtotal-row">
                        <td colspan="2">{format!("Subtotal: {}", cat_name)}</td>
                        <td class="amount">{hho_types::format_dollars(cat_total)}</td>
                        <td></td>
                    </tr>
                }
            }).collect_view();

            Some(view! {
                <div class="print-view">
                    <h1>{format!("{} Transactions — {} {}", title, month_name, year)}</h1>
                    <table>
                        <thead>
                            <tr>
                                <th>"Date"</th>
                                <th>"Vendor"</th>
                                <th class="amount">"Amount"</th>
                                <th>"Category"</th>
                            </tr>
                        </thead>
                        <tbody>
                            {rows}
                        </tbody>
                        <tfoot>
                            {subtotal_rows}
                            <tr class="grand-total-row">
                                <td colspan="2"><strong>"Grand Total"</strong></td>
                                <td class="amount"><strong>{hho_types::format_dollars(total_cents)}</strong></td>
                                <td></td>
                            </tr>
                        </tfoot>
                    </table>
                </div>
            }).into_any()
        }}
    }
}
