use leptos::prelude::*;
use verwatch_frontend::App;

// Use lol_alloc as the global allocator for smaller WASM size
#[cfg(target_arch = "wasm32")]
use lol_alloc::{AssumeSingleThreaded, FreeListAllocator};

#[cfg(target_arch = "wasm32")]
#[global_allocator]
static ALLOCATOR: AssumeSingleThreaded<FreeListAllocator> =
    unsafe { AssumeSingleThreaded::new(FreeListAllocator::new()) };

pub fn main() {
    console_error_panic_hook::set_once();
    mount_to_body(App);
}
