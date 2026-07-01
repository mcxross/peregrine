#![allow(clippy::unnecessary_to_owned, clippy::too_many_arguments, clippy::collapsible_if)]
#![allow(clippy::unwrap_used, clippy::expect_used, unused_imports, clippy::match_like_matches_macro)]
// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // Tauri's embedded tokio runtime needs a larger stack size for the app server
    unsafe {
        std::env::set_var("RUST_MIN_STACK", "16777216");
    }
    peregrine_lib::run();
}
