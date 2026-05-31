use std::path::PathBuf;
use std::sync::Arc;

use tauri::{Emitter, State};
use tokio::sync::{mpsc, Mutex};
use tokio::time::Duration;

use crate::clipper::{ClipResult, Clipper};
use crate::controller::Controller;
use crate::converter::{ConversionStatus, Converter};
use crate::settings::Settings;
use crate::storage::Clip;

/// Response from opening a video, includes path to play and metadata
#[derive(Debug, Clone, serde::Serialize)]
pub struct OpenVideoResponse {
    /// Path to the video file to play (may be converted)
    pub play_path: String,
    /// Original source path (for clipping)
    pub source_path: String,
    /// Video duration in seconds
    pub duration: f64,
    /// Whether conversion was performed
    pub converted: bool,
}

/// RAII guard that ensures `is_clipping` is reset even on early returns.
///
/// This prevents the clip flag from getting permanently stuck if Phase 1
/// or Phase 2 of `save_clip` returns an error before Phase 3 runs.
struct ClippingGuard<'a> {
    ctrl: &'a mut Controller,
}

impl Drop for ClippingGuard<'_> {
    fn drop(&mut self) {
        self.ctrl.is_clipping = false;
    }
}

/// Open a video file, converting if necessary for web playback.
///
/// Uses a 3-phase approach to avoid holding the controller lock during conversion:
/// 1. Quick check (hold lock briefly to check format compatibility)
/// 2. Conversion (no lock held — can take minutes to hours)
/// 3. Finalize (re-acquire lock to set current_video and load clips)
///
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

    // Check file exists before any work
    if !path.exists() {
        return Err(format!("Video file does not exist: {}", path.display()));
    }

    let source_path = path.clone();

    // Phase 1: Quick check (hold lock briefly)
    let (needs_conversion, clips_dir) = {
        let ctrl = state.lock().await;
        let needs = !Converter::is_web_compatible(&path);
        (needs, ctrl.clips_dir.clone())
    }; // Lock released here

    // Phase 2: Conversion (no lock held — other commands can proceed)
    let (play_path, converted) = if !needs_conversion {
        (path.clone(), false)
    } else {
        let converted_path = tokio::time::timeout(
            Duration::from_secs(4 * 60 * 60), // 4 hour max
            Converter::convert_to_mp4(&path, &clips_dir, progress_tx),
        )
        .await
        .map_err(|_| "Conversion timed out after 4 hours".to_string())?
        .map_err(|e| e.to_string())?;

        (converted_path, true)
    };

    // Phase 3: Finalize (re-acquire lock)
    let mut ctrl = state.lock().await;
    ctrl.current_video = Some(source_path.clone());
    let duration = Converter::get_duration(&source_path)
        .await
        .map_err(|e| e.to_string())?;
    ctrl.get_clips().map_err(|e| e.to_string())?;

    Ok(OpenVideoResponse {
        play_path: play_path.to_string_lossy().to_string(),
        source_path: source_path.to_string_lossy().to_string(),
        duration,
        converted,
    })
}

/// Save a clip at the specified position.
///
/// The frontend provides the current playback position and video duration.
/// This command uses a 3-phase approach to minimize lock contention:
/// 1. Quick setup (hold lock briefly to check state and calculate paths)
/// 2. FFmpeg execution (no lock held - can take seconds)
/// 3. Update state (re-acquire lock to update storage)
///
/// A RAII guard ensures `is_clipping` is always reset, even on early returns.
#[tauri::command]
pub async fn save_clip(
    state: State<'_, Arc<Mutex<Controller>>>,
    current_pos: f64,
    duration: f64,
) -> Result<ClipResult, String> {
    // Phase 1: Quick setup (hold lock briefly)
    // Inner block scopes the MutexGuard so it's dropped before Phase 2
    let (clipper, video_path, output_path, start_time, end_time) = {
        let mut ctrl = state.lock().await;

        if ctrl.is_clipping {
            return Err("Clip already in progress".to_string());
        }

        let video_path = ctrl.current_video.clone()
            .ok_or_else(|| "No video loaded".to_string())?;

        ctrl.is_clipping = true;

        // Inner scope for the guard: if anything fails between here and
        // `mem::forget(guard)`, the guard resets is_clipping on drop.
        let guard = ClippingGuard { ctrl: &mut ctrl };

        let (start, end) = guard.ctrl.clipper.calculate_times(current_pos, duration);

        // Validate clip has positive duration
        if let Err(e) = Clipper::validate_times(start, end) {
            return Err(e); // guard drops, resets is_clipping
        }

        let clip_number = guard.ctrl.clip_count + 1;
        let output = match guard.ctrl.clipper.output_path(
            &video_path,
            clip_number,
            guard.ctrl.settings.output_dir.as_deref(),
        ) {
            Ok(p) => p,
            Err(e) => return Err(e.to_string()), // guard drops, resets is_clipping
        };

        let clipper = guard.ctrl.clipper.clone();

        // Setup succeeded — prevent the guard from resetting is_clipping.
        // is_clipping stays true through Phase 2 to prevent concurrent clips.
        std::mem::forget(guard);

        (clipper, video_path, output, start, end)
    }; // MutexGuard dropped here, but is_clipping stays true

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
/// Normalizes empty output_dir to None.
#[tauri::command]
pub async fn save_settings(
    state: State<'_, Arc<Mutex<Controller>>>,
    mut settings: Settings,
) -> Result<(), String> {
    // Normalize empty output_dir string to None
    if let Some(ref dir) = settings.output_dir {
        if dir.as_os_str().is_empty() {
            settings.output_dir = None;
        }
    }

    let mut ctrl = state.lock().await;
    ctrl.update_settings(settings).map_err(|e| e.to_string())
}
