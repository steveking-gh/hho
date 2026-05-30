// Root application component.
// Owns AppState, provides it via context, registers the global key handler,
// and sets up the Tauri menu-event listener that drives file loading.

use leptos::prelude::*;
use leptos::ev;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;

use crate::logic::{ActivePane, Item, nav_up, nav_down, next_item_id, pane_left, pane_right};
use crate::state::AppState;
use crate::components::{debug_log::DebugLog, pane::Pane};

// ── Scale constants ───────────────────────────────────────────────────────────

const SCALE_MIN:     f32 = 7.0;
const SCALE_MAX:     f32 = 20.0;
const SCALE_STEP:    f32 = 1.0;
const SCALE_DEFAULT: f32 = 10.0;

// ── Tauri JS bindings ─────────────────────────────────────────────────────────
// withGlobalTauri: true in tauri.conf.json exposes these namespaces globally.

#[wasm_bindgen]
extern "C" {
    /// Subscribe to a Tauri backend event; returns an unlisten function.
    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "event"], js_name = "listen")]
    async fn tauri_listen(event: &str, handler: &js_sys::Function) -> JsValue;

    /// Call a Tauri command and await the result.
    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "core"], js_name = "invoke")]
    async fn tauri_invoke(cmd: &str, args: JsValue) -> JsValue;
}

// ── CSV file result ───────────────────────────────────────────────────────────

#[derive(serde::Deserialize)]
struct CsvFile {
    path: String,
    rows: Vec<String>,
}

#[derive(serde::Serialize)]
struct OpenCsvArgs {
    path: String,
}

// ── Font-scale helper ─────────────────────────────────────────────────────────

/// Set the `<html>` element font-size so all rem-based sizes scale uniformly.
fn apply_font_scale(px: f32) {
    let _ = web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.document_element())
        .and_then(|el| el.dyn_into::<web_sys::HtmlElement>().ok())
        .map(|html| {
            let _ = html
                .style()
                .set_property("font-size", &format!("{:.1}px", px));
        });
}

// ── Pane population ───────────────────────────────────────────────────────────

/// Replace the Uncategorized (middle) pane contents with `csv` rows.
/// Activates the pane and selects the first row.
fn populate_middle_pane(state: AppState, csv: CsvFile) {
    let count = csv.rows.len();
    let items: Vec<Item> = csv
        .rows
        .into_iter()
        .map(|label| Item { id: next_item_id(), label })
        .collect();

    state.middle_items.set(items);
    state.middle_sel.set(if count > 0 { Some(0) } else { None });
    state.active_pane.set(ActivePane::Middle);

    state.log(format!(
        "[File] opened \"{}\" → {count} rows loaded into Uncategorized",
        csv.path,
    ));
}

/// Deserialize a JsValue returned from tauri_invoke into CsvFile.
/// Returns None when the command returned null (user cancelled dialog).
fn parse_csv_result(value: JsValue) -> Option<CsvFile> {
    if value.is_null() || value.is_undefined() {
        return None;
    }
    serde_wasm_bindgen::from_value(value).ok()
}

// ── Tauri menu listener ───────────────────────────────────────────────────────

/// Register a persistent listener for "hho-menu" events emitted by the Rust backend.
/// Dispatches to the appropriate invoke command and populates the middle pane.
fn setup_menu_listener(state: AppState) {
    // FnMut closure: receives every "hho-menu" event as a raw JsValue.
    let handler = Closure::wrap(Box::new(move |event: JsValue| {
        // event shape: { id, payload: { action: string, path?: string } }
        let payload = js_sys::Reflect::get(&event, &JsValue::from_str("payload"))
            .unwrap_or(JsValue::NULL);

        let action = js_sys::Reflect::get(&payload, &JsValue::from_str("action"))
            .ok()
            .and_then(|v| v.as_string())
            .unwrap_or_default();

        state.log(format!(
            "[Event] hho-menu action=\"{}\"", action
        ));

        match action.as_str() {
            // File > Open — show native file picker via Tauri command
            "open" => {
                spawn_local(async move {
                    state.log("[Event] invoking pick_csv".to_string());
                    let result = tauri_invoke("pick_csv", JsValue::NULL).await;
                    match parse_csv_result(result) {
                        Some(csv) => populate_middle_pane(state, csv),
                        None => state.log("[File] open cancelled".to_string()),
                    }
                });
            }

            // File > Open Recent — read a specific path via Tauri command
            "open-recent" => {
                let path = js_sys::Reflect::get(&payload, &JsValue::from_str("path"))
                    .ok()
                    .and_then(|v| v.as_string())
                    .unwrap_or_default();

                if path.is_empty() {
                    state.log("[Event] open-recent: missing path".to_string());
                    return;
                }

                state.log(format!("[Event] invoking open_csv path=\"{path}\""));

                spawn_local(async move {
                    let args = serde_wasm_bindgen::to_value(&OpenCsvArgs { path })
                        .unwrap_or(JsValue::NULL);
                    let result = tauri_invoke("open_csv", args).await;
                    match parse_csv_result(result) {
                        Some(csv) => populate_middle_pane(state, csv),
                        None => state.log("[File] open-recent returned null".to_string()),
                    }
                });
            }

            other => {
                state.log(format!("[Event] hho-menu: unknown action \"{other}\""));
            }
        }
    }) as Box<dyn FnMut(JsValue)>);

    // Spawn the async listen call; forget the closure so it stays alive forever.
    spawn_local(async move {
        let _ = tauri_listen("hho-menu", handler.as_ref().unchecked_ref()).await;
        handler.forget(); // deliberate leak: listener must outlive the component
    });
}

// ── App component ─────────────────────────────────────────────────────────────

#[component]
pub fn App() -> impl IntoView {
    let state = AppState::new();
    provide_context(state);

    // Register the Tauri menu-event listener (WASM-only; no-op in test builds).
    setup_menu_listener(state);

    // ── Global keyboard handler ───────────────────────────────────────────────
    // Attached to `window` so no DOM element needs focus.
    let handle = window_event_listener(ev::keydown, move |ev| {
        let key   = ev.key();
        let shift = ev.shift_key();
        let ctrl  = ev.ctrl_key();

        // ── Ctrl+zoom shortcuts ───────────────────────────────────────────────
        if ctrl && matches!(key.as_str(), "=" | "+" | "-" | "0") {
            ev.prevent_default();
            let current = state.font_scale.get_untracked();
            let (new_scale, action) = match key.as_str() {
                "=" | "+" => ((current + SCALE_STEP).min(SCALE_MAX), "zoom in"),
                "-"       => ((current - SCALE_STEP).max(SCALE_MIN), "zoom out"),
                _         => (SCALE_DEFAULT,                           "zoom reset"),
            };
            state.font_scale.set(new_scale);
            apply_font_scale(new_scale);
            state.log(format!(
                "[KeyDown] Ctrl+{key}  →  {action} | scale={new_scale:.1}px/rem"
            ));
            return;
        }

        // ── Arrow-key guard ───────────────────────────────────────────────────
        if !matches!(key.as_str(), "ArrowUp" | "ArrowDown" | "ArrowLeft" | "ArrowRight") {
            return;
        }

        let pane = state.active_pane.get_untracked();
        ev.prevent_default();

        let prefix = format!(
            "[KeyDown] {shift_str}{key:<14} active={pane:<14} sel={sel:?}",
            shift_str = if shift { "Shift+" } else { "" },
            key       = key,
            pane      = pane.to_string(),
            sel       = state.sel_for(pane).get_untracked(),
        );

        let detail: String = match (shift, key.as_str()) {

            // ── Row navigation ────────────────────────────────────────────────
            (false, "ArrowUp") => {
                let items   = state.items_for(pane).get_untracked();
                let sel     = state.sel_for(pane).get_untracked();
                let new_sel = nav_up(&items, sel);
                state.sel_for(pane).set(new_sel);
                format!("nav up → sel={:?}", new_sel)
            }

            (false, "ArrowDown") => {
                let items   = state.items_for(pane).get_untracked();
                let sel     = state.sel_for(pane).get_untracked();
                let new_sel = nav_down(&items, sel);
                state.sel_for(pane).set(new_sel);
                format!("nav down → sel={:?}", new_sel)
            }

            // ── Pane switching ────────────────────────────────────────────────
            (false, "ArrowLeft") => {
                let next = pane_left(pane);
                state.active_pane.set(next);
                if next == pane {
                    "switch left → no-op (already leftmost or bottom pane)".into()
                } else {
                    format!("switch left → active=\"{}\"", next)
                }
            }

            (false, "ArrowRight") => {
                let next = pane_right(pane);
                state.active_pane.set(next);
                if next == pane {
                    "switch right → no-op (already rightmost or bottom pane)".into()
                } else {
                    format!("switch right → active=\"{}\"", next)
                }
            }

            // ── Item movement: Shift+Left ─────────────────────────────────────
            (true, "ArrowLeft") => match pane {
                ActivePane::Middle => state.transfer(ActivePane::Middle, ActivePane::Left),
                ActivePane::Right  => state.transfer(ActivePane::Right,  ActivePane::Middle),
                ActivePane::Left   => "no-op: no pane left of Joint".into(),
                ActivePane::Bottom => "no-op: Ignored has no left neighbor".into(),
            },

            // ── Item movement: Shift+Right ────────────────────────────────────
            (true, "ArrowRight") => match pane {
                ActivePane::Left   => state.transfer(ActivePane::Left,   ActivePane::Middle),
                ActivePane::Middle => state.transfer(ActivePane::Middle, ActivePane::Right),
                ActivePane::Right  => "no-op: no pane right of Mine".into(),
                ActivePane::Bottom => "no-op: Ignored has no right neighbor".into(),
            },

            // ── Item movement: Shift+Down ─────────────────────────────────────
            (true, "ArrowDown") => match pane {
                ActivePane::Left | ActivePane::Middle | ActivePane::Right => {
                    state.transfer(pane, ActivePane::Bottom)
                }
                ActivePane::Bottom => "no-op: already in Ignored pane".into(),
            },

            // ── Item movement: Shift+Up ───────────────────────────────────────
            (true, "ArrowUp") => match pane {
                ActivePane::Bottom => state.transfer(ActivePane::Bottom, ActivePane::Middle),
                _ => "no-op: Shift+Up only applies from Ignored pane".into(),
            },

            _ => return,
        };

        state.log(format!("{}  →  {}", prefix, detail));
    });

    on_cleanup(move || drop(handle));

    view! {
        <div class="app-container">
            <div class="main-area">
                <div class="top-section">
                    <Pane title="Joint"         pane_id=ActivePane::Left />
                    <Pane title="Uncategorized" pane_id=ActivePane::Middle />
                    <Pane title="Mine"          pane_id=ActivePane::Right />
                </div>
                <Pane title="Ignored" pane_id=ActivePane::Bottom is_bottom=true />
            </div>
            <DebugLog />
        </div>
    }
}
