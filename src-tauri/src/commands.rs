use std::path::PathBuf;
use std::sync::Arc;

use tauri::{Emitter, State};
use tokio::sync::{mpsc, Mutex};

use crate::clipper::ClipResult;
use crate::controller::{Controller, OpenVideoResponse};
use crate::converter::ConversionStatus;
use crate::settings::Settings;
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
                ConversionStatus::FallbackToTranscode => {
                    let _ = app_clone.emit("conversion-fallback", ());
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
/// This command uses a 3-phase approach to minimize lock contention:
/// 1. Quick setup (hold lock briefly to check state and calculate paths)
/// 2. FFmpeg execution (no lock held - can take seconds)
/// 3. Update state (re-acquire lock to update storage)
#[tauri::command]
pub async fn save_clip(
    state: State<'_, Arc<Mutex<Controller>>>,
    current_pos: f64,
    duration: f64,
) -> Result<ClipResult, String> {
    // Phase 1: Quick setup (hold lock briefly)
    let (clipper, video_path, output_path, start_time, end_time) = {
        let mut ctrl = state.lock().await;

        if ctrl.is_clipping {
            return Err("Clip already in progress".to_string());
        }

        let video_path = ctrl.current_video.clone()
            .ok_or_else(|| "No video loaded".to_string())?;

        ctrl.is_clipping = true;

        let (start, end) = ctrl.clipper.calculate_times(current_pos, duration);
        let clip_number = ctrl.clip_count + 1;
        let output = ctrl.clipper.output_path(
            &video_path,
            clip_number,
            ctrl.settings.output_dir.as_deref()
        ).map_err(|e| e.to_string())?;

        (ctrl.clipper.clone(), video_path, output, start, end)
    }; // Lock released here

    // Phase 2: FFmpeg execution (no lock held - other commands can proceed)
    let result = clipper.save_clip(&video_path, start_time, end_time, &output_path)
        .await
        .map_err(|e| e.to_string());

    // Phase 3: Update state (re-acquire lock)
    {
        let mut ctrl = state.lock().await;
        ctrl.is_clipping = false;

        if let Ok(ref clip_result) = result {
            if clip_result.success {
                if let Some(video_path_str) = video_path.to_str() {
                    let _ = ctrl.store.add_clip(
                        video_path_str,
                        &clip_result.path,
                        start_time,
                        end_time,
                    );
                    ctrl.clip_count += 1;
                }
            }
        }
    }

    result
}

/// Return all saved clips for the currently loaded video.
#[tauri::command]
pub async fn get_clips(state: State<'_, Arc<Mutex<Controller>>>) -> Result<Vec<Clip>, String> {
    let mut ctrl = state.lock().await;
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

/// Load application settings from disk.
///
/// Returns the current settings or defaults if no settings file exists.
#[tauri::command]
pub async fn get_settings(
    state: State<'_, Arc<Mutex<Controller>>>,
) -> Result<Settings, String> {
    let ctrl = state.lock().await;
    Ok(ctrl.settings.clone())
}

/// Save application settings to disk.
///
/// Validates settings before saving (buffer values 0-60 seconds, clip_key single char).
#[tauri::command]
pub async fn save_settings(
    state: State<'_, Arc<Mutex<Controller>>>,
    settings: Settings,
) -> Result<(), String> {
    let mut ctrl = state.lock().await;
    ctrl.update_settings(settings).map_err(|e| e.to_string())
}
