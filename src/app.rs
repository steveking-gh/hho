// Root application component.
// Owns AppState, provides it via context, and registers all global event
// listeners: keyboard (nav / zoom), mouse (drag-resize), window resize.

use leptos::prelude::*;
use leptos::ev;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;

use crate::logic::{ActivePane, Item, nav_up, nav_down, next_item_id, pane_left, pane_right};
use crate::state::{AppState, DragState, DragTarget, PANE_MIN_H, PANE_MIN_W};
use crate::components::{
    debug_log::DebugLog,
    pane::Pane,
    resize_handle::{ResizeDir, ResizeHandle},
};

// ── Scale constants ───────────────────────────────────────────────────────────

const SCALE_MIN:     f32 = 7.0;
const SCALE_MAX:     f32 = 20.0;
const SCALE_STEP:    f32 = 1.0;
const SCALE_DEFAULT: f32 = 10.0;

// Minimum interval between window-size saves (ms) — simple rate-limiter.
const WIN_SAVE_INTERVAL_MS: f64 = 500.0;

// ── Tauri JS bindings ─────────────────────────────────────────────────────────

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "event"], js_name = "listen")]
    async fn tauri_listen(event: &str, handler: &js_sys::Function) -> JsValue;

    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "core"], js_name = "invoke")]
    async fn tauri_invoke(cmd: &str, args: JsValue) -> JsValue;
}

// ── IPC arg / result types ────────────────────────────────────────────────────

#[derive(serde::Deserialize)]
struct CsvFile { path: String, rows: Vec<String> }

#[derive(serde::Serialize)]
struct OpenCsvArgs { path: String }

#[derive(serde::Deserialize)]
struct LayoutConfig {
    left_width:  f32,
    right_width: f32,
    bottom_h:    f32,
    debug_h:     f32,
}

#[derive(serde::Serialize)]
struct SaveLayoutArgs {
    left_width:  f32,
    right_width: f32,
    bottom_h:    f32,
    debug_h:     f32,
}

#[derive(serde::Serialize)]
struct SaveWindowSizeArgs { width: f64, height: f64 }

// ── DOM helpers ───────────────────────────────────────────────────────────────

/// Set `font-size` on `<html>` — all rem-based sizes scale proportionally.
fn apply_font_scale(px: f32) {
    let _ = web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.document_element())
        .and_then(|el| el.dyn_into::<web_sys::HtmlElement>().ok())
        .map(|html| {
            let _ = html.style().set_property("font-size", &format!("{:.1}px", px));
        });
}

/// Set (or clear) a global drag cursor on `<html>` so the cursor stays
/// consistent even when the pointer travels over non-handle elements.
fn set_drag_cursor(cursor: &str) {
    let _ = web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.document_element())
        .and_then(|el| el.dyn_into::<web_sys::HtmlElement>().ok())
        .map(|html| {
            let style = html.style();
            if cursor.is_empty() {
                let _ = style.remove_property("cursor");
                let _ = style.remove_property("user-select");
            } else {
                let _ = style.set_property("cursor", cursor);
                let _ = style.set_property("user-select", "none");
            }
        });
}

// ── Pane population ───────────────────────────────────────────────────────────

fn populate_middle_pane(state: AppState, csv: CsvFile) {
    let count = csv.rows.len();
    let items: Vec<Item> = csv.rows.into_iter()
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

fn parse_csv_result(value: JsValue) -> Option<CsvFile> {
    if value.is_null() || value.is_undefined() { return None; }
    serde_wasm_bindgen::from_value(value).ok()
}

// ── Tauri menu listener ───────────────────────────────────────────────────────

fn setup_menu_listener(state: AppState) {
    let handler = Closure::wrap(Box::new(move |event: JsValue| {
        let payload = js_sys::Reflect::get(&event, &JsValue::from_str("payload"))
            .unwrap_or(JsValue::NULL);
        let action = js_sys::Reflect::get(&payload, &JsValue::from_str("action"))
            .ok().and_then(|v| v.as_string()).unwrap_or_default();

        state.log(format!("[Event] hho-menu action=\"{}\"", action));

        match action.as_str() {
            "open" => {
                spawn_local(async move {
                    state.log("[Event] invoking pick_csv".to_string());
                    let result = tauri_invoke("pick_csv", JsValue::NULL).await;
                    match parse_csv_result(result) {
                        Some(csv) => populate_middle_pane(state, csv),
                        None      => state.log("[File] open cancelled".to_string()),
                    }
                });
            }
            "open-recent" => {
                let path = js_sys::Reflect::get(&payload, &JsValue::from_str("path"))
                    .ok().and_then(|v| v.as_string()).unwrap_or_default();
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
                        None      => state.log("[File] open-recent returned null".to_string()),
                    }
                });
            }
            other => state.log(format!("[Event] hho-menu: unknown action \"{other}\"")),
        }
    }) as Box<dyn FnMut(JsValue)>);

    spawn_local(async move {
        let _ = tauri_listen("hho-menu", handler.as_ref().unchecked_ref()).await;
        handler.forget();
    });
}

// ── Layout restore ────────────────────────────────────────────────────────────

fn load_layout_from_config(state: AppState) {
    spawn_local(async move {
        let result = tauri_invoke("get_layout", JsValue::NULL).await;
        if let Ok(layout) = serde_wasm_bindgen::from_value::<LayoutConfig>(result) {
            state.left_width.set(layout.left_width);
            state.right_width.set(layout.right_width);
            state.bottom_h.set(layout.bottom_h);
            state.debug_h.set(layout.debug_h);
            state.log(format!(
                "[Init] layout restored: left={:.0}px right={:.0}px \
                 bottom={:.0}px debug={:.0}px",
                layout.left_width, layout.right_width,
                layout.bottom_h,   layout.debug_h,
            ));
        }
    });
}

// ── App component ─────────────────────────────────────────────────────────────

#[component]
pub fn App() -> impl IntoView {
    let state = AppState::new();
    provide_context(state);

    setup_menu_listener(state);
    load_layout_from_config(state);

    // ── Global mouse-move handler (drag-resize) ───────────────────────────────
    let move_handle = window_event_listener(ev::mousemove, move |ev| {
        let Some(drag) = state.drag.get_untracked() else { return };

        let cx = ev.client_x() as f32;
        let cy = ev.client_y() as f32;
        let dx = cx - drag.last_x;
        let dy = cy - drag.last_y;

        // Update last position for incremental delta on next event.
        state.drag.set(Some(DragState { last_x: cx, last_y: cy, ..drag }));

        match drag.target {
            // Drag right: left pane grows, Uncategorized shrinks automatically.
            DragTarget::LeftHandle => {
                let new = (state.left_width.get_untracked() + dx).max(PANE_MIN_W);
                state.left_width.set(new);
            }
            // Drag right: Mine pane's left edge moves right → Mine shrinks.
            DragTarget::RightHandle => {
                let new = (state.right_width.get_untracked() - dx).max(PANE_MIN_W);
                state.right_width.set(new);
            }
            // Drag down: top section grows (flex:1), Ignored pane shrinks.
            DragTarget::TopHandle => {
                let new = (state.bottom_h.get_untracked() - dy).max(PANE_MIN_H);
                state.bottom_h.set(new);
            }
            // Drag down: Ignored grows, Debug shrinks (both have fixed heights).
            DragTarget::BottomHandle => {
                let new_bot = (state.bottom_h.get_untracked() + dy).max(PANE_MIN_H);
                let new_dbg = (state.debug_h.get_untracked()  - dy).max(PANE_MIN_H);
                state.bottom_h.set(new_bot);
                state.debug_h.set(new_dbg);
            }
        }
    });

    // ── Global mouse-up handler (end drag, save layout) ───────────────────────
    let up_handle = window_event_listener(ev::mouseup, move |_ev| {
        let Some(drag) = state.drag.get_untracked() else { return };
        state.drag.set(None);
        set_drag_cursor(""); // restore default cursor

        let left_w  = state.left_width.get_untracked();
        let right_w = state.right_width.get_untracked();
        let bot_h   = state.bottom_h.get_untracked();
        let dbg_h   = state.debug_h.get_untracked();

        state.log(format!(
            "[Drag] end {:?} → left={left_w:.0}px right={right_w:.0}px \
             bottom={bot_h:.0}px debug={dbg_h:.0}px  (saving…)",
            drag.target,
        ));

        spawn_local(async move {
            let args = serde_wasm_bindgen::to_value(&SaveLayoutArgs {
                left_width:  left_w,
                right_width: right_w,
                bottom_h:    bot_h,
                debug_h:     dbg_h,
            }).unwrap_or(JsValue::NULL);
            let _ = tauri_invoke("save_layout", args).await;
        });
    });

    // ── Global key-down handler ───────────────────────────────────────────────
    let key_handle = window_event_listener(ev::keydown, move |ev| {
        let key   = ev.key();
        let shift = ev.shift_key();
        let ctrl  = ev.ctrl_key();

        // ── Ctrl+zoom ─────────────────────────────────────────────────────────
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
            (false, "ArrowLeft") => {
                let next = pane_left(pane);
                state.active_pane.set(next);
                if next == pane { "switch left → no-op".into() }
                else            { format!("switch left → active=\"{}\"", next) }
            }
            (false, "ArrowRight") => {
                let next = pane_right(pane);
                state.active_pane.set(next);
                if next == pane { "switch right → no-op".into() }
                else            { format!("switch right → active=\"{}\"", next) }
            }
            (true, "ArrowLeft") => match pane {
                ActivePane::Middle => state.transfer(ActivePane::Middle, ActivePane::Left),
                ActivePane::Right  => state.transfer(ActivePane::Right,  ActivePane::Middle),
                ActivePane::Left   => "no-op: no pane left of Joint".into(),
                ActivePane::Bottom => "no-op: Ignored has no left neighbor".into(),
            },
            (true, "ArrowRight") => match pane {
                ActivePane::Left   => state.transfer(ActivePane::Left,   ActivePane::Middle),
                ActivePane::Middle => state.transfer(ActivePane::Middle, ActivePane::Right),
                ActivePane::Right  => "no-op: no pane right of Mine".into(),
                ActivePane::Bottom => "no-op: Ignored has no right neighbor".into(),
            },
            (true, "ArrowDown") => match pane {
                ActivePane::Left | ActivePane::Middle | ActivePane::Right => {
                    state.transfer(pane, ActivePane::Bottom)
                }
                ActivePane::Bottom => "no-op: already in Ignored pane".into(),
            },
            (true, "ArrowUp") => match pane {
                ActivePane::Bottom => state.transfer(ActivePane::Bottom, ActivePane::Middle),
                _ => "no-op: Shift+Up only applies from Ignored pane".into(),
            },
            _ => return,
        };

        state.log(format!("{}  →  {}", prefix, detail));
    });

    // ── Window resize → save dimensions (rate-limited) ────────────────────────
    // last-save timestamp stored in a thread-local to survive closure re-creation.
    thread_local! {
        static LAST_WIN_SAVE: std::cell::Cell<f64> = const { std::cell::Cell::new(0.0) };
    }

    let resize_handle = window_event_listener(ev::resize, move |_ev| {
        let now  = js_sys::Date::now();
        let last = LAST_WIN_SAVE.with(|c| c.get());
        if now - last < WIN_SAVE_INTERVAL_MS { return; }
        LAST_WIN_SAVE.with(|c| c.set(now));

        let Some(w) = web_sys::window() else { return };
        let width  = w.inner_width().ok().and_then(|v| v.as_f64()).unwrap_or(1024.0);
        let height = w.inner_height().ok().and_then(|v| v.as_f64()).unwrap_or(700.0);

        state.log(format!("[Window] resize → {width:.0}×{height:.0} (saving…)"));

        spawn_local(async move {
            let args = serde_wasm_bindgen::to_value(
                &SaveWindowSizeArgs { width, height }
            ).unwrap_or(JsValue::NULL);
            let _ = tauri_invoke("save_window_size", args).await;
        });
    });

    on_cleanup(move || {
        drop(key_handle);
        drop(move_handle);
        drop(up_handle);
        drop(resize_handle);
    });

    // Set global drag cursor whenever the drag signal changes so the cursor
    // stays correct even when the pointer travels off the handle.
    Effect::new(move || {
        match state.drag.get() {
            None => set_drag_cursor(""),
            Some(drag) => {
                let cursor = match drag.target {
                    DragTarget::LeftHandle | DragTarget::RightHandle => "col-resize",
                    DragTarget::TopHandle  | DragTarget::BottomHandle => "row-resize",
                };
                set_drag_cursor(cursor);
            }
        }
    });

    view! {
        <div class="app-container">
            <div class="main-area">
                <div class="top-section">
                    <Pane title="Joint"         pane_id=ActivePane::Left />
                    <ResizeHandle dir=ResizeDir::Horizontal target=DragTarget::LeftHandle />
                    <Pane title="Uncategorized" pane_id=ActivePane::Middle />
                    <ResizeHandle dir=ResizeDir::Horizontal target=DragTarget::RightHandle />
                    <Pane title="Mine"          pane_id=ActivePane::Right />
                </div>
                <ResizeHandle dir=ResizeDir::Vertical target=DragTarget::TopHandle />
                <Pane title="Ignored" pane_id=ActivePane::Bottom is_bottom=true />
                <ResizeHandle dir=ResizeDir::Vertical target=DragTarget::BottomHandle />
                <DebugLog />
            </div>
        </div>
    }
}
