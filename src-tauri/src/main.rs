#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod clipper;
mod commands;
mod controller;
mod converter;
mod error;
mod settings;
mod storage;
mod util;
mod video_server;

use std::sync::Arc;

use tokio::sync::Mutex;

#[tokio::main]
async fn main() {
    let controller = Arc::new(Mutex::new(
        controller::Controller::new()
            .await
            .expect("Failed to initialize controller"),
    ));

    let video_server = Arc::new(Mutex::new(video_server::VideoServer::new()));

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            // Initialize sidecar paths for bundled FFmpeg binaries
            util::init_sidecar_paths(app.handle());
            Ok(())
        })
        .manage(controller)
        .manage(video_server)
        .invoke_handler(tauri::generate_handler![
            commands::open_video,
            commands::save_clip,
            commands::get_clips,
            commands::delete_clip,
            commands::start_video_server,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
