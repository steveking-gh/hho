// Scrollable debug log panel — displays keyboard and click events newest-first.

use leptos::prelude::*;
use crate::state::AppState;

/// Fixed-height panel at the bottom of the window showing all logged events.
#[component]
pub fn DebugLog() -> impl IntoView {
    let state: AppState = use_context().expect("AppState must be provided at root");
    let log = state.debug_log;

    view! {
        <div class="debug-panel">
            <div class="debug-header">"Debug"</div>
            // Separate scrollable container keeps the header pinned while entries scroll.
            <div class="debug-rows">
                {move || {
                    log.get()
                        .into_iter()
                        .map(|entry| view! { <div class="debug-entry">{entry}</div> })
                        .collect_view()
                }}
            </div>
        </div>
    }
}
