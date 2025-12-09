use leptos::prelude::*;
use verwatch_frontend::App;

pub fn main() {
    console_error_panic_hook::set_once();
    mount_to_body(App);
}
