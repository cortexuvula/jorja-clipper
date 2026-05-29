use std::path::PathBuf;
use std::sync::Arc;

use tauri::{Emitter, State};
use tokio::sync::{mpsc, Mutex};

use crate::clipper::ClipResult;
use crate::controller::{Controller, OpenVideoResponse};
use crate::converter::ConversionStatus;
use crate::storage::Clip;

/// Open a video file, converting if necessary for web playback.
///
/// Returns information about the video including the path to play and duration.
/// Emits progress events to the frontend during conversion.
#[tauri::command]
pub async fn open_video(
    app: tauri::AppHandle,
    state: State<'_, Arc<Mutex<Controller>>>,
    path: String,
) -> Result<OpenVideoResponse, String> {
    let path = PathBuf::from(&path);

    // Create channel for conversion progress
    let (progress_tx, mut progress_rx) = mpsc::channel::<ConversionStatus>(100);

    // Spawn task to forward progress to frontend
    let app_clone = app.clone();
    tokio::spawn(async move {
        while let Some(status) = progress_rx.recv().await {
            match status {
                ConversionStatus::Started { duration } => {
                    let _ = app_clone.emit("conversion-started", duration);
                }
                ConversionStatus::Progress(percent) => {
                    let _ = app_clone.emit("conversion-progress", percent);
                }
                ConversionStatus::Completed { output_path } => {
                    let _ = app_clone.emit(
                        "conversion-completed",
                        output_path.to_string_lossy().to_string(),
                    );
                }
                ConversionStatus::Failed(error) => {
                    let _ = app_clone.emit("conversion-failed", error);
                }
            }
        }
    });

    let mut ctrl = state.lock().await;
    ctrl.open_video(path, Some(progress_tx))
        .await
        .map_err(|e| e.to_string())
}

/// Save a clip at the specified position.
///
/// The frontend provides the current playback position and video duration.
#[tauri::command]
pub async fn save_clip(
    state: State<'_, Arc<Mutex<Controller>>>,
    current_pos: f64,
    duration: f64,
) -> Result<ClipResult, String> {
    let mut ctrl = state.lock().await;
    ctrl.save_clip(current_pos, duration)
        .await
        .map_err(|e| e.to_string())
}

/// Return all saved clips for the currently loaded video.
#[tauri::command]
pub async fn get_clips(state: State<'_, Arc<Mutex<Controller>>>) -> Result<Vec<Clip>, String> {
    let ctrl = state.lock().await;
    ctrl.get_clips().map_err(|e| e.to_string())
}

/// Delete a clip by ID — removes the file from disk and the DB entry.
#[tauri::command]
pub async fn delete_clip(
    state: State<'_, Arc<Mutex<Controller>>>,
    id: i64,
    clip_path: String,
) -> Result<(), String> {
    let ctrl = state.lock().await;
    ctrl.delete_clip(id, &clip_path).map_err(|e| e.to_string())
}

/// Start a local HTTP server to stream video files with range request support.
/// This is needed for WebKitGTK on Linux which doesn't support range requests
/// on the asset:// protocol.
#[tauri::command]
pub async fn start_video_server(
    video_server: State<'_, Arc<Mutex<crate::video_server::VideoServer>>>,
    path: String,
) -> Result<String, String> {
    let mut server = video_server.lock().await;
    let port = server.start(PathBuf::from(&path))?;
    Ok(format!("http://127.0.0.1:{}/video.mp4", port))
}
