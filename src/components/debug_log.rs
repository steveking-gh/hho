// Scrollable debug log panel.
// Height driven by the debug_h signal so the resize handle above it works.

use crate::state::AppState;
use leptos::prelude::*;

#[component]
pub fn DebugLog() -> impl IntoView {
    let state: AppState = use_context().expect("AppState must be provided at root");
    let log = state.debug_log;

    // Inline height from signal; all other styles stay in CSS.
    let debug_style = move || format!("height: {}px;", state.debug_h.get());

    view! {
        <div class="debug-panel" style=debug_style>
            <div class="debug-header">"Debug"</div>
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
