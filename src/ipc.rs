// Typed wrappers over the Tauri command bridge.
//
// Each backend command has exactly one wrapper here: the command-name string and
// argument shape live in a single place, and every call site is fully typed.
// A wrong field name or return type is now a compile error, not a runtime one.
//
// Every wrapper also mirrors its request, response, and any error into the debug
// log via the global AppState handle (`crate::state::get_global_state`). The
// fire-and-forget wrappers (save_layout, save_window_size, exit_app) currently
// repeat that logging inline; a `call_discard` helper could collapse them.
// NOTE: responses are logged verbatim — large payloads (e.g. a full transaction
// list) produce large log entries; consider truncating if this grows.

use wasm_bindgen::prelude::*;
use crate::state::AppState;


use hho_types::{
    AutoAssignRule, Institution, LayoutConfig, OpenCsvArgs, OpenResult, SaveAutoAssignRulesArgs,
    SaveLayoutArgs, SaveMappingArgs, SavePaneTransactionsArgs, SaveWindowSizeArgs, Transaction,
};

#[wasm_bindgen]
extern "C" {
    /// Subscribe to a backend event; resolves to an unlisten function.
    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "event"], js_name = "listen")]
    pub async fn listen(event: &str, handler: &js_sys::Function) -> JsValue;

    /// Raw command bridge. `catch` turns a rejected promise (command `Err`, or
    /// argument-deserialization failure) into `Err(JsValue)` instead of a panic.
    #[wasm_bindgen(catch, js_namespace = ["window", "__TAURI__", "core"], js_name = "invoke")]
    async fn invoke_raw(cmd: &str, args: JsValue) -> Result<JsValue, JsValue>;
}

/// Serialize command arguments to a JS value (camelCase keys via the arg structs).
fn to_args<T: serde::Serialize>(args: &T) -> JsValue {
    serde_wasm_bindgen::to_value(args).unwrap_or(JsValue::NULL)
}

/// Stringifies a JsValue payload to a JSON string representation.
fn stringify_js(val: &JsValue) -> String {
    if val.is_null() || val.is_undefined() {
        return "null".to_string();
    }
    match js_sys::JSON::stringify(val) {
        Ok(js_str) => js_str.into(),
        Err(_) => format!("{:?}", val),
    }
}

/// Summarizes a JsValue payload for debug logging to avoid large/unbounded log entries.
fn summarize_js(val: &JsValue) -> String {
    if val.is_null() || val.is_undefined() {
        return "null".to_string();
    }
    if js_sys::Array::is_array(val) {
        let arr = js_sys::Array::from(val);
        return format!("[Array of {} items]", arr.length());
    }
    if let Some(s) = val.as_string() {
        if s.len() > 100 {
            return format!("\"{}...\" ({} chars)", &s[..97], s.len());
        }
        return format!("\"{}\"", s);
    }
    let stringified = stringify_js(val);
    if stringified.len() < 120 {
        return stringified;
    }
    if let Ok(keys) = js_sys::Reflect::own_keys(val) {
        let mut parts = Vec::new();
        for i in 0..keys.length() {
            let key = keys.get(i);
            if let Some(key_str) = key.as_string() {
                if let Ok(prop_val) = js_sys::Reflect::get(val, &key) {
                    let prop_summary = if js_sys::Array::is_array(&prop_val) {
                        format!("[Array of {} items]", js_sys::Array::from(&prop_val).length())
                    } else {
                        let prop_str = stringify_js(&prop_val);
                        if prop_str.len() > 120 {
                            format!("[Object ({} bytes)]", prop_str.len())
                        } else {
                            prop_str
                        }
                    };
                    parts.push(format!("{}: {}", key_str, prop_summary));
                }
            }
        }
        return format!("{{ {} }}", parts.join(", "));
    }
    stringified
}

/// Invoke a command with no arguments and deserialize its result.
async fn call_unit<R: serde::de::DeserializeOwned>(state: AppState, cmd: &str) -> Result<R, String> {
    state.log(format!("[IPC Request] {cmd} with no args"));
    let res = invoke_raw(cmd, JsValue::NULL).await;
    match res {
        Ok(v) => {
            let res_str = summarize_js(&v);
            state.log(format!("[IPC Response] {cmd} success: {res_str}"));
            serde_wasm_bindgen::from_value(v).map_err(|e| {
                let err_msg = e.to_string();
                state.log(format!("[IPC Error] {cmd} deserialization failure: {err_msg}"));
                err_msg
            })
        }
        Err(e) => {
            let err_str = stringify_js(&e);
            state.log(format!("[IPC Error] {cmd} backend error: {err_str}"));
            Err(err_str)
        }
    }
}

/// Invoke a command with arguments and deserialize its result.
async fn call<A, R>(state: AppState, cmd: &str, args: &A) -> Result<R, String>
where
    A: serde::Serialize,
    R: serde::de::DeserializeOwned,
{
    let args_val = to_args(args);
    let args_str = summarize_js(&args_val);
    state.log(format!("[IPC Request] {cmd} with args: {args_str}"));
    let res = invoke_raw(cmd, args_val).await;
    match res {
        Ok(v) => {
            let res_str = summarize_js(&v);
            state.log(format!("[IPC Response] {cmd} success: {res_str}"));
            serde_wasm_bindgen::from_value(v).map_err(|e| {
                let err_msg = e.to_string();
                state.log(format!("[IPC Error] {cmd} deserialization failure: {err_msg}"));
                err_msg
            })
        }
        Err(e) => {
            let err_str = stringify_js(&e);
            state.log(format!("[IPC Error] {cmd} backend error: {err_str}"));
            Err(err_str)
        }
    }
}

// ── Command wrappers ──────────────────────────────────────────────────────────

/// Open a native file picker and read the chosen CSV.
pub async fn pick_csv(state: AppState) -> Result<OpenResult, String> {
    call_unit(state, "pick_csv").await
}

/// Read a CSV at a known path (Open Recent flow).
pub async fn open_csv(state: AppState, path: String) -> Result<OpenResult, String> {
    call(state, "open_csv", &OpenCsvArgs { path }).await
}

/// Persist a column mapping, then parse the pending file using the saved mapping.
pub async fn save_mapping(
    state: AppState,
    institution: Institution,
    pending_path: String,
) -> Result<Vec<Transaction>, String> {
    call(
        state,
        "save_mapping",
        &SaveMappingArgs {
            institution,
            pending_path,
        },
    )
    .await
}

/// Fetch persisted pane dimensions.
pub async fn get_layout(state: AppState) -> Result<LayoutConfig, String> {
    call_unit(state, "get_layout").await
}

/// Persist pane dimensions (best-effort; ignores failures).
pub async fn save_layout(state: AppState, left_width: f32, right_width: f32, bottom_h: f32, debug_h: f32) {
    let args = SaveLayoutArgs {
        left_width,
        right_width,
        bottom_h,
        debug_h,
    };
    let args_val = to_args(&args);
    let args_str = summarize_js(&args_val);
    state.log(format!("[IPC Request] save_layout with args: {args_str}"));
    let res = invoke_raw("save_layout", args_val).await;
    match res {
        Ok(v) => {
            let res_str = summarize_js(&v);
            state.log(format!("[IPC Response] save_layout success: {res_str}"));
        }
        Err(e) => {
            let err_str = stringify_js(&e);
            state.log(format!("[IPC Error] save_layout backend error: {err_str}"));
        }
    }
}

/// Persist window dimensions (best-effort; ignores failures).
pub async fn save_window_size(state: AppState, width: f64, height: f64) {
    let args = SaveWindowSizeArgs { width, height };
    let args_val = to_args(&args);
    let args_str = summarize_js(&args_val);
    state.log(format!("[IPC Request] save_window_size with args: {args_str}"));
    let res = invoke_raw("save_window_size", args_val).await;
    match res {
        Ok(v) => {
            let res_str = summarize_js(&v);
            state.log(format!("[IPC Response] save_window_size success: {res_str}"));
        }
        Err(e) => {
            let err_str = stringify_js(&e);
            state.log(format!("[IPC Error] save_window_size backend error: {err_str}"));
        }
    }
}

/// Fetches the recent CSV file paths.
pub async fn get_recent_files(state: AppState) -> Result<Vec<String>, String> {
    call_unit(state, "get_recent_files").await
}

/// Fetches the auto-assign rules.
pub async fn get_auto_assign_rules(state: AppState) -> Result<Vec<AutoAssignRule>, String> {
    call_unit(state, "get_auto_assign_rules").await
}

/// Saves the complete list of auto-assign rules.
pub async fn save_auto_assign_rules(state: AppState, rules: Vec<AutoAssignRule>) -> Result<(), String> {
    call(state, "save_auto_assign_rules", &SaveAutoAssignRulesArgs { rules }).await
}

/// Saves transactions in a selected pane to a CSV file.
pub async fn save_pane_transactions(
    state: AppState,
    pane_title: String,
    month_name: String,
    year: i32,
    transactions: Vec<Transaction>,
) -> Result<(), String> {
    let args = SavePaneTransactionsArgs {
        pane_title,
        month_name,
        year,
        transactions,
    };
    call(state, "save_pane_transactions", &args).await
}

/// Closes the application cleanly.
pub async fn exit_app(state: AppState) {
    state.log("[IPC Request] exit_app with no args".to_string());
    let res = invoke_raw("exit_app", JsValue::NULL).await;
    match res {
        Ok(v) => {
            let res_str = summarize_js(&v);
            state.log(format!("[IPC Response] exit_app success: {res_str}"));
        }
        Err(e) => {
            let err_str = stringify_js(&e);
            state.log(format!("[IPC Error] exit_app backend error: {err_str}"));
        }
    }
}
