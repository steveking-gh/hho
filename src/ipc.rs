// Thin wrappers over the global Tauri JS bridge.
// withGlobalTauri: true (tauri.conf.json) exposes these namespaces on `window`.

use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    /// Subscribe to a backend event; resolves to an unlisten function.
    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "event"], js_name = "listen")]
    pub async fn listen(event: &str, handler: &js_sys::Function) -> JsValue;

    /// Invoke a Tauri command and await its JSON-serialized result.
    /// `catch` maps a rejected promise (command returned `Err`, or argument
    /// deserialization failed) to `Err(JsValue)` instead of panicking the task.
    #[wasm_bindgen(catch, js_namespace = ["window", "__TAURI__", "core"], js_name = "invoke")]
    pub async fn invoke(cmd: &str, args: JsValue) -> Result<JsValue, JsValue>;
}
