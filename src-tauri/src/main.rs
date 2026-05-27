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
