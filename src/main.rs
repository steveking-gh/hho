mod app;
mod components;
mod dto;
mod ipc;
mod logic;
mod state;

use app::App;
use leptos::prelude::*;

fn main() {
    console_error_panic_hook::set_once();
    mount_to_body(|| view! { <App /> })
}
