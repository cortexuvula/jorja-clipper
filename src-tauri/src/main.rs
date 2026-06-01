#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::Arc;

use tokio::sync::Mutex;

use jorja_clipper::{cleanup, commands, controller, util, video_server};

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
        .plugin(tauri_plugin_fs::init())
        .setup(|app| {
            // Initialize sidecar paths for bundled FFmpeg binaries
            util::init_sidecar_paths(app.handle());

            // Start background cleanup task for converted files
            let clips_dir = util::app_config_dir().join("clips");
            tokio::spawn(cleanup::start_cleanup_task(clips_dir, 7));

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
            commands::get_settings,
            commands::save_settings,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
