use std::panic;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(inline_js = "export function get_stack() { return new Error().stack; }")]
extern "C" {
    fn get_stack() -> String;
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn error(msg: &str);
}

/// A panic hook for use with
/// [`std::panic::set_hook`](https://doc.rust-lang.org/nightly/std/panic/fn.set_hook.html)
/// that logs panics into
/// [`console.error`](https://developer.mozilla.org/en-US/docs/Web/API/Console/error).
///
/// On non-wasm targets, prints the panic to `stderr`.
#[allow(dead_code)]
pub fn hook(info: &panic::PanicHookInfo) {
    #[cfg(target_arch = "wasm32")]
    {
        let msg = format!("{}\n\nStack:\n\n{}\n\n", info, get_stack());
        error(&msg);
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        use std::io::{self, Write};
        let _ = writeln!(io::stderr(), "{}", info);
    }
}

/// Automatically registers the panic hook when the WASM module is instantiated.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(start)]
pub fn init_panic_hook() {
    panic::set_hook(Box::new(hook));
}
