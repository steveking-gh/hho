// Custom header component rendering application branding, open actions, recent files dropdown, and quit operations.

use crate::app::handle_open_result;
use crate::state::AppState;
use hho_types::Transaction;
use leptos::prelude::*;
use wasm_bindgen_futures::spawn_local;

/// Extracts filename from path string.
fn get_filename(path: &str) -> &str {
    let sep = if path.contains('\\') { '\\' } else { '/' };
    path.rsplit(sep).next().unwrap_or(path)
}

/// Formats month number and year to display text.
fn format_month_year(month: i32, year: i32) -> String {
    let month_name = match month {
        1 => "Jan",
        2 => "Feb",
        3 => "Mar",
        4 => "Apr",
        5 => "May",
        6 => "Jun",
        7 => "Jul",
        8 => "Aug",
        9 => "Sep",
        10 => "Oct",
        11 => "Nov",
        12 => "Dec",
        _ => "",
    };
    format!("{} {}", month_name, year)
}

/// Translates month indices into full English month name strings.
fn get_month_name(month: i32) -> &'static str {
    match month {
        1 => "January",
        2 => "February",
        3 => "March",
        4 => "April",
        5 => "May",
        6 => "June",
        7 => "July",
        8 => "August",
        9 => "September",
        10 => "October",
        11 => "November",
        12 => "December",
        _ => "Unknown",
    }
}

#[component]
pub fn Header() -> impl IntoView {
    let state: AppState = use_context().expect("AppState must be provided at root");
    let is_dropdown_open = RwSignal::new(false);

    // Handles CSV file pick action.
    let on_open = move |_| {
        if state.is_loading_file.get_untracked() || state.pending_mapping.get_untracked().is_some()
        {
            return;
        }
        state.is_loading_file.set(true);
        state.log("[Header] Open CSV clicked".to_string());
        spawn_local(async move {
            match crate::ipc::pick_csv().await {
                Ok(r) => handle_open_result(state, r),
                Err(e) => state.log(format!("[File] pick_csv failed: {e}")),
            }
            state.is_loading_file.set(false);
        });
    };

    // Handles application quit action.
    let on_quit = move |_| {
        state.log("[Header] Quit clicked".to_string());
        spawn_local(async move {
            crate::ipc::exit_app().await;
        });
    };

    // Toggles dropdown visibility state.
    let toggle_dropdown = move |e: web_sys::MouseEvent| {
        if state.pending_mapping.get_untracked().is_some() {
            return;
        }
        e.stop_propagation();
        is_dropdown_open.update(|v| *v = !*v);
    };

    // Month / Year button to open period selection modal.
    let on_toggle_month = move |_| {
        if state.pending_mapping.get_untracked().is_some() {
            return;
        }
        state.is_month_modal_open.set(true);
    };

    // Opens the rules editor modal.
    let on_edit_rules = move |_| {
        if state.pending_mapping.get_untracked().is_some() {
            return;
        }
        state.is_rules_modal_open.set(true);
    };

    // Opens the manual transaction creation modal.
    let on_create_transaction = move |_| {
        if state.pending_mapping.get_untracked().is_some() {
            return;
        }
        state.is_create_transaction_modal_open.set(true);
    };

    // Triggers file export of the Joint pane transactions to CSV.
    let on_save_joint = move |_| {
        if state.pending_mapping.get_untracked().is_some() {
            return;
        }
        let items = state.left_items.get_untracked();
        let txns: Vec<Transaction> = items.iter().map(|item| item.to_transaction()).collect();
        let month = state.selected_month.get_untracked();
        let year = state.selected_year.get_untracked();
        let month_name = get_month_name(month).to_string();

        spawn_local(async move {
            state.log(format!(
                "[Header] Saving Joint transactions to CSV (count={})",
                txns.len()
            ));
            if let Err(e) =
                crate::ipc::save_pane_transactions("Joint".to_string(), month_name, year, txns)
                    .await
            {
                state.log(format!("[Header] Failed to save Joint transactions: {e}"));
            }
        });
    };

    // Triggers file export of the Personal pane transactions to CSV.
    let on_save_personal = move |_| {
        if state.pending_mapping.get_untracked().is_some() {
            return;
        }
        let items = state.right_items.get_untracked();
        let txns: Vec<Transaction> = items.iter().map(|item| item.to_transaction()).collect();
        let month = state.selected_month.get_untracked();
        let year = state.selected_year.get_untracked();
        let month_name = get_month_name(month).to_string();

        spawn_local(async move {
            state.log(format!(
                "[Header] Saving Personal transactions to CSV (count={})",
                txns.len()
            ));
            if let Err(e) =
                crate::ipc::save_pane_transactions("Personal".to_string(), month_name, year, txns)
                    .await
            {
                state.log(format!(
                    "[Header] Failed to save Personal transactions: {e}"
                ));
            }
        });
    };

    // Register window click listener to auto-close dropdown when clicking outside.
    let close_handle = window_event_listener(leptos::ev::click, move |_| {
        is_dropdown_open.set(false);
    });

    on_cleanup(move || {
        drop(close_handle);
    });

    view! {
        <header class="header-bar">
            <div class="header-actions">
                <button class="header-btn" on:click=on_open>
                    <span class="btn-icon">"📂"</span>
                    "Open CSV"
                </button>

                <div class="dropdown">
                    <button class="header-btn" on:click=toggle_dropdown>
                        <span class="btn-icon">"🕒"</span>
                        "Open Recent"
                        <span class="dropdown-arrow">{move || if is_dropdown_open.get() { " ▴" } else { " ▾" }}</span>
                    </button>

                    {move || is_dropdown_open.get().then(|| {
                        let recents = state.recent_files.get();
                        view! {
                            <div class="dropdown-menu">
                                {if recents.is_empty() {
                                    view! { <div class="dropdown-empty">"No Recent Files"</div> }.into_any()
                                } else {
                                    recents.into_iter().map(|path| {
                                        let path_clone = path.clone();
                                        let on_recent_click = move |_| {
                                            if state.is_loading_file.get_untracked() || state.pending_mapping.get_untracked().is_some() {
                                                return;
                                            }
                                            state.is_loading_file.set(true);
                                            state.log(format!("[Header] Opening recent file: {path_clone}"));
                                            let p = path_clone.clone();
                                            spawn_local(async move {
                                                match crate::ipc::open_csv(p).await {
                                                    Ok(r)  => handle_open_result(state, r),
                                                    Err(e) => state.log(format!("[File] open_csv failed: {e}")),
                                                }
                                                state.is_loading_file.set(false);
                                            });
                                        };
                                        view! {
                                            <button
                                                class="dropdown-item"
                                                title=path.clone()
                                                on:click=on_recent_click
                                            >
                                                {get_filename(&path).to_string()}
                                            </button>
                                        }
                                    }).collect_view().into_any()
                                }}
                            </div>
                        }
                    })}
                </div>

                <button class="header-btn" on:click=on_toggle_month>
                    <span class="btn-icon">"📅"</span>
                    {move || format_month_year(state.selected_month.get(), state.selected_year.get())}
                </button>

                <button class="header-btn" on:click=on_edit_rules>
                    <span class="btn-icon">"📝"</span>
                    "Edit Rules"
                </button>

                <button class="header-btn" on:click=on_create_transaction>
                    <span class="btn-icon header-icon-orange">"+"</span>
                    "New Transaction"
                </button>

                <button class="header-btn" on:click=on_save_joint>
                    <span class="btn-icon">"💾"</span>
                    "Save Joint"
                </button>

                <button class="header-btn" on:click=on_save_personal>
                    <span class="btn-icon">"💾"</span>
                    "Save Personal"
                </button>
            </div>

            <div class="header-branding">
                "HHO TRANSACTION MAPPER"
            </div>

            <div class="header-actions">
                <button class="header-btn header-btn-danger" on:click=on_quit>
                    <span class="btn-icon">"❌"</span>
                    "Quit"
                </button>
            </div>
        </header>
    }
}
