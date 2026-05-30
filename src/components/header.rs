// Custom header component rendering application branding, open actions, recent files dropdown, and quit operations.

use leptos::prelude::*;
use wasm_bindgen_futures::spawn_local;
use crate::state::AppState;
use crate::app::handle_open_result;

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

#[component]
pub fn Header() -> impl IntoView {
    let state: AppState = use_context().expect("AppState must be provided at root");
    let is_dropdown_open = RwSignal::new(false);

    // Handles CSV file pick action.
    let on_open = move |_| {
        state.log("[Header] Open CSV clicked".to_string());
        spawn_local(async move {
            match crate::ipc::pick_csv().await {
                Ok(r)  => handle_open_result(state, r),
                Err(e) => state.log(format!("[File] pick_csv failed: {e}")),
            }
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
        e.stop_propagation();
        is_dropdown_open.update(|v| *v = !*v);
    };

    // Month / Year button to open period selection modal.
    let on_toggle_month = move |_| {
        state.is_month_modal_open.set(true);
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
                                            state.log(format!("[Header] Opening recent file: {path_clone}"));
                                            let p = path_clone.clone();
                                            spawn_local(async move {
                                                match crate::ipc::open_csv(p).await {
                                                    Ok(r)  => handle_open_result(state, r),
                                                    Err(e) => state.log(format!("[File] open_csv failed: {e}")),
                                                }
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
