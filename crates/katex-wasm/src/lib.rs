//! `wasm-bindgen` shim around [`katex`].

#![forbid(unsafe_code)]

use katex::{Settings, render_to_mathml_string};
use wasm_bindgen::prelude::*;

/// `opts` may be `undefined`, `null`, `{}`, or a partial object whose keys
/// mirror upstream KaTeX's JS `Settings` (camelCased: `displayMode`,
/// `throwOnError`, `errorColor`, …).
#[wasm_bindgen(js_name = renderToString)]
pub fn render_to_string(tex: &str, opts: JsValue) -> Result<String, JsError> {
    console_error_panic_hook::set_once();
    let settings: Option<Settings> =
        serde_wasm_bindgen::from_value(opts).map_err(|e| JsError::new(&e.to_string()))?;
    render_to_mathml_string(tex, &settings.unwrap_or_default())
        .map_err(|e| JsError::new(&e.to_string()))
}
