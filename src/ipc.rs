// Typed wrappers over the Tauri command bridge.
//
// Each backend command has exactly one wrapper here: the command-name string and
// argument shape live in a single place, and every call site is fully typed.
// A wrong field name or return type is now a compile error, not a runtime one.

use wasm_bindgen::prelude::*;

use hho_types::{
    Institution, LayoutConfig, OpenCsvArgs, OpenResult, SaveLayoutArgs, SaveMappingArgs,
    SaveWindowSizeArgs, Transaction,
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

/// Invoke a command with no arguments and deserialize its result.
async fn call_unit<R: serde::de::DeserializeOwned>(cmd: &str) -> Result<R, String> {
    let v = invoke_raw(cmd, JsValue::NULL).await.map_err(|e| format!("{e:?}"))?;
    serde_wasm_bindgen::from_value(v).map_err(|e| e.to_string())
}

/// Invoke a command with arguments and deserialize its result.
async fn call<A, R>(cmd: &str, args: &A) -> Result<R, String>
where
    A: serde::Serialize,
    R: serde::de::DeserializeOwned,
{
    let v = invoke_raw(cmd, to_args(args)).await.map_err(|e| format!("{e:?}"))?;
    serde_wasm_bindgen::from_value(v).map_err(|e| e.to_string())
}

// ── Command wrappers ──────────────────────────────────────────────────────────

/// Open a native file picker and read the chosen CSV.
pub async fn pick_csv() -> Result<OpenResult, String> {
    call_unit("pick_csv").await
}

/// Read a CSV at a known path (Open Recent flow).
pub async fn open_csv(path: String) -> Result<OpenResult, String> {
    call("open_csv", &OpenCsvArgs { path }).await
}

/// Persist a column mapping, then parse the pending file with it.
pub async fn save_mapping(
    institution: Institution,
    pending_path: String,
) -> Result<Vec<Transaction>, String> {
    call("save_mapping", &SaveMappingArgs { institution, pending_path }).await
}

/// Fetch persisted pane dimensions.
pub async fn get_layout() -> Result<LayoutConfig, String> {
    call_unit("get_layout").await
}

/// Persist pane dimensions (best-effort; ignores failures).
pub async fn save_layout(left_width: f32, right_width: f32, bottom_h: f32, debug_h: f32) {
    let args = SaveLayoutArgs { left_width, right_width, bottom_h, debug_h };
    let _ = invoke_raw("save_layout", to_args(&args)).await;
}

/// Persist window dimensions (best-effort; ignores failures).
pub async fn save_window_size(width: f64, height: f64) {
    let args = SaveWindowSizeArgs { width, height };
    let _ = invoke_raw("save_window_size", to_args(&args)).await;
}
