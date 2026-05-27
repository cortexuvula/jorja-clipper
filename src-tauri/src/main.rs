#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod clipper;
mod commands;
mod controller;
mod error;
mod player;
mod settings;
mod storage;

use std::sync::Arc;

use tokio::sync::Mutex;

#[tokio::main]
async fn main() {
    // Force GTK (used by tao/Tauri) to use the X11 backend via XWayland, even
    // when running under a Wayland compositor. This is required so that the mpv
    // overlay window's native handle is an Xlib/Xcb window ID that mpv's --wid
    // flag can render into. Under native Wayland, window_handle() returns a
    // Wayland surface handle which mpv cannot embed into.
    //
    // Safety: std::env::set_var is safe in Rust 2021 edition. This MUST run
    // before tauri::Builder::run() which initializes GTK.
    std::env::set_var("GDK_BACKEND", "x11");

    let controller = Arc::new(Mutex::new(
        controller::Controller::new()
            .await
            .expect("Failed to initialize controller"),
    ));

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(controller)
        .invoke_handler(tauri::generate_handler![
            commands::create_mpv_window,
            commands::position_mpv_window,
            commands::open_video,
            commands::toggle_pause,
            commands::seek,
            commands::get_position,
            commands::save_clip,
            commands::get_clips,
            commands::shutdown,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
