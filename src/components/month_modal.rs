// Month / Year selection modal for filtering transaction rows by period.

use leptos::prelude::*;
use crate::state::AppState;

#[component]
pub fn MonthModal() -> impl IntoView {
    let state: AppState = use_context().expect("AppState must be provided at root");

    // Holds temporary year and month choices prior to application.
    let temp_year = RwSignal::new(state.selected_year.get_untracked());
    let temp_month = RwSignal::new(state.selected_month.get_untracked());

    let months = [
        (1, "Jan"), (2, "Feb"), (3, "Mar"), (4, "Apr"),
        (5, "May"), (6, "Jun"), (7, "Jul"), (8, "Aug"),
        (9, "Sep"), (10, "Oct"), (11, "Nov"), (12, "Dec")
    ];

    // Saves selected period and updates filters.
    let on_apply = move |_| {
        state.selected_year.set(temp_year.get_untracked());
        state.selected_month.set(temp_month.get_untracked());
        state.apply_month_filter();
        state.is_month_modal_open.set(false);
    };

    // Closes modal without saving changes.
    let on_cancel = move |_| {
        state.is_month_modal_open.set(false);
    };

    // Handles year decrement action.
    let prev_year = move |_| {
        temp_year.update(|y| *y -= 1);
    };

    // Handles year increment action.
    let next_year = move |_| {
        temp_year.update(|y| *y += 1);
    };

    view! {
        <div class="modal-overlay">
            <div class="modal month-modal">
                <h2 class="modal-title">"Select Period"</h2>

                <div class="year-selector">
                    <button class="year-btn" on:click=prev_year>"◀"</button>
                    <span class="year-display">{move || temp_year.get()}</span>
                    <button class="year-btn" on:click=next_year>"▶"</button>
                </div>

                <div class="month-grid">
                    {months.into_iter().map(|(num, label)| {
                        let is_selected = move || temp_month.get() == num;
                        let select_month = move |_| {
                            temp_month.set(num);
                        };
                        view! {
                            <button
                                class=move || if is_selected() { "month-btn selected" } else { "month-btn" }
                                on:click=select_month
                            >
                                {label}
                            </button>
                        }
                    }).collect_view().into_any()}
                </div>

                <div class="modal-actions">
                    <button class="modal-btn" on:click=on_cancel>"Cancel"</button>
                    <button class="modal-btn modal-btn-primary" on:click=on_apply>"Apply"</button>
                </div>
            </div>
        </div>
    }
}
