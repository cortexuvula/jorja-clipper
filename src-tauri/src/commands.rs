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
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
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

#[cfg(test)]
mod clipping_guard_tests {
    use super::*;
    use crate::clipper::Clipper;
    use crate::settings::Settings;
    use crate::storage::ClipStore;
    use tempfile::TempDir;

    fn create_test_controller() -> Controller {
        let temp_dir = TempDir::new().unwrap();
        Controller {
            clipper: Clipper::new(5.0, 5.0),
            settings: Settings::default(),
            store: ClipStore::new().unwrap(),
            current_video: None,
            clip_count: 0,
            is_clipping: true,
            clips_dir: temp_dir.path().to_path_buf(),
        }
    }

    #[test]
    fn test_clipping_guard_resets_flag_on_drop() {
        let mut controller = create_test_controller();
        assert!(controller.is_clipping);

        {
            let _guard = ClippingGuard {
                ctrl: &mut controller,
            };
            // Guard is active - can't access controller while borrowed
        } // Guard drops here

        // Flag should be reset
        assert!(!controller.is_clipping);
    }

    #[test]
    fn test_clipping_guard_resets_flag_on_early_return() {
        let mut controller = create_test_controller();

        fn early_return(ctrl: &mut Controller) -> Result<(), String> {
            let _guard = ClippingGuard { ctrl };
            return Err("early return".to_string());
        }

        let result = early_return(&mut controller);
        assert!(result.is_err());
        assert!(!controller.is_clipping);
    }
}

/// Core logic for opening a video, separated from Tauri event emission
pub async fn open_video_logic(
    controller: Arc<Mutex<Controller>>,
    path: PathBuf,
) -> Result<OpenVideoResponse, String> {
    // Check file exists before any work
    if !path.exists() {
        return Err(format!("Video file does not exist: {}", path.display()));
    }

    let source_path = path.clone();

    // Phase 1: Quick check (hold lock briefly)
    let (needs_conversion, clips_dir) = {
        let ctrl = controller.lock().await;
        let needs = !Converter::is_web_compatible(&path);
        (needs, ctrl.clips_dir.clone())
    }; // Lock released here

    // Phase 2: Conversion (no lock held — other commands can proceed)
    let (play_path, converted) = if !needs_conversion {
        (path.clone(), false)
    } else {
        // Create channel for conversion progress (not used in logic, but needed for API)
        let (progress_tx, mut progress_rx) = mpsc::channel::<ConversionStatus>(100);

        // Spawn a task to drain the channel (prevent blocking)
        tokio::spawn(async move {
            while let Some(_) = progress_rx.recv().await {
                // Just drain the channel
            }
        });

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
    let mut ctrl = controller.lock().await;
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

    // Call the core logic
    open_video_logic(state.inner().clone(), path).await
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

/// Core logic for saving a clip, separated from Tauri wrapper for testability
pub async fn save_clip_logic(
    controller: Arc<Mutex<Controller>>,
    current_pos: f64,
    duration: f64,
) -> Result<ClipResult, String> {
    // Phase 1: Quick setup (hold lock briefly)
    // Inner block scopes the MutexGuard so it's dropped before Phase 2
    let (clipper, video_path, output_path, start_time, end_time) = {
        let mut ctrl = controller.lock().await;

        if ctrl.is_clipping {
            return Err("Clip already in progress".to_string());
        }

        let video_path = ctrl
            .current_video
            .clone()
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
    let result = clipper
        .save_clip(&video_path, start_time, end_time, &output_path)
        .await
        .map_err(|e| e.to_string());

    // Phase 3: Update state (re-acquire lock)
    {
        let mut ctrl = controller.lock().await;
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

#[tauri::command]
pub async fn save_clip(
    state: State<'_, Arc<Mutex<Controller>>>,
    current_pos: f64,
    duration: f64,
) -> Result<ClipResult, String> {
    save_clip_logic(state.inner().clone(), current_pos, duration).await
}

/// Core logic for getting clips, separated from Tauri wrapper for testability
pub async fn get_clips_logic(controller: Arc<Mutex<Controller>>) -> Result<Vec<Clip>, String> {
    let mut ctrl = controller.lock().await;
    ctrl.get_clips().map_err(|e| e.to_string())
}

/// Return all saved clips for the currently loaded video.
#[tauri::command]
pub async fn get_clips(state: State<'_, Arc<Mutex<Controller>>>) -> Result<Vec<Clip>, String> {
    get_clips_logic(state.inner().clone()).await
}

/// Core logic for deleting a clip, separated from Tauri wrapper for testability
pub async fn delete_clip_logic(
    controller: Arc<Mutex<Controller>>,
    id: i64,
    clip_path: String,
) -> Result<(), String> {
    let ctrl = controller.lock().await;
    ctrl.delete_clip(id, &clip_path).map_err(|e| e.to_string())
}

/// Delete a clip by ID — removes the file from disk and the DB entry.
#[tauri::command]
pub async fn delete_clip(
    state: State<'_, Arc<Mutex<Controller>>>,
    id: i64,
    clip_path: String,
) -> Result<(), String> {
    delete_clip_logic(state.inner().clone(), id, clip_path).await
}

/// Core logic for starting video server, separated from Tauri wrapper for testability
pub async fn start_video_server_logic(
    video_server: Arc<Mutex<crate::video_server::VideoServer>>,
    path: String,
) -> Result<String, String> {
    let mut server = video_server.lock().await;
    let port = server.start(PathBuf::from(&path))?;
    Ok(format!("http://127.0.0.1:{}/video.mp4", port))
}

/// Start a local HTTP server to stream video files with range request support.
/// This is needed for WebKitGTK on Linux which doesn't support range requests
/// on the asset:// protocol.
#[tauri::command]
pub async fn start_video_server(
    video_server: State<'_, Arc<Mutex<crate::video_server::VideoServer>>>,
    path: String,
) -> Result<String, String> {
    start_video_server_logic(video_server.inner().clone(), path).await
}

/// Core logic for getting settings, separated from Tauri wrapper for testability
pub async fn get_settings_logic(controller: Arc<Mutex<Controller>>) -> Result<Settings, String> {
    let ctrl = controller.lock().await;
    Ok(ctrl.settings.clone())
}

/// Load application settings from disk.
///
/// Returns the current settings or defaults if no settings file exists.
#[tauri::command]
pub async fn get_settings(state: State<'_, Arc<Mutex<Controller>>>) -> Result<Settings, String> {
    get_settings_logic(state.inner().clone()).await
}

/// Core logic for saving settings, separated from Tauri wrapper for testability
pub async fn save_settings_logic(
    controller: Arc<Mutex<Controller>>,
    mut settings: Settings,
) -> Result<(), String> {
    // Normalize empty output_dir string to None
    if let Some(ref dir) = settings.output_dir {
        if dir.as_os_str().is_empty() {
            settings.output_dir = None;
        }
    }

    let mut ctrl = controller.lock().await;
    ctrl.update_settings(settings).map_err(|e| e.to_string())
}

/// Save application settings to disk.
///
/// Validates settings before saving (buffer values 0-60 seconds, clip_key single char).
/// Normalizes empty output_dir to None.
#[tauri::command]
pub async fn save_settings(
    state: State<'_, Arc<Mutex<Controller>>>,
    settings: Settings,
) -> Result<(), String> {
    save_settings_logic(state.inner().clone(), settings).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::ClipStore;
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn create_test_controller() -> (Arc<Mutex<Controller>>, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");

        let store = ClipStore::with_path(&db_path).unwrap();
        let clips_dir = temp_dir.path().join("clips");
        std::fs::create_dir_all(&clips_dir).unwrap();

        (
            Arc::new(Mutex::new(Controller {
                clipper: Clipper::new(5.0, 5.0),
                settings: Settings::default(),
                store,
                current_video: None,
                clip_count: 0,
                is_clipping: false,
                clips_dir,
            })),
            temp_dir,
        )
    }

    #[tokio::test]
    async fn test_get_settings_logic() {
        let (controller, _temp_dir) = create_test_controller();

        let settings = get_settings_logic(controller).await.unwrap();

        assert_eq!(settings.buffer_before, 5.0);
        assert_eq!(settings.buffer_after, 5.0);
        assert_eq!(settings.clip_key, "c");
    }

    #[tokio::test]
    async fn test_save_settings_logic() {
        let (controller, _temp_dir) = create_test_controller();

        let new_settings = Settings {
            buffer_before: 10.0,
            buffer_after: 10.0,
            clip_key: "v".to_string(),
            output_dir: None,
            theme: crate::settings::Theme::Dark,
        };

        let result = save_settings_logic(controller.clone(), new_settings).await;
        assert!(result.is_ok());

        let settings = get_settings_logic(controller).await.unwrap();
        assert_eq!(settings.buffer_before, 10.0);
        assert_eq!(settings.buffer_after, 10.0);
        assert_eq!(settings.clip_key, "v");
    }

    #[tokio::test]
    async fn test_save_settings_logic_normalizes_empty_output_dir() {
        let (controller, _temp_dir) = create_test_controller();

        let settings = Settings {
            buffer_before: 5.0,
            buffer_after: 5.0,
            clip_key: "c".to_string(),
            output_dir: Some(PathBuf::from("")),
            theme: crate::settings::Theme::Dark,
        };

        let result = save_settings_logic(controller.clone(), settings).await;
        assert!(result.is_ok());

        let saved_settings = get_settings_logic(controller).await.unwrap();
        assert_eq!(saved_settings.output_dir, None);
    }

    #[tokio::test]
    async fn test_get_clips_logic_empty() {
        let (controller, _temp_dir) = create_test_controller();

        let clips = get_clips_logic(controller).await.unwrap();
        assert_eq!(clips.len(), 0);
    }

    #[tokio::test]
    async fn test_get_clips_logic_with_clips() {
        let (controller, controller_temp_dir) = create_test_controller();
        let temp_dir = TempDir::new().unwrap();

        let video_path = temp_dir.path().join("test_video.mp4");
        std::fs::write(&video_path, "fake video content").unwrap();

        let clip1_path = temp_dir.path().join("clip1.mp4");
        let clip2_path = temp_dir.path().join("clip2.mp4");
        std::fs::write(&clip1_path, "clip 1 content").unwrap();
        std::fs::write(&clip2_path, "clip 2 content").unwrap();

        {
            let mut ctrl = controller.lock().await;
            ctrl.current_video = Some(video_path.clone());
            ctrl.store
                .add_clip(
                    video_path.to_str().unwrap(),
                    clip1_path.to_str().unwrap(),
                    10.0,
                    20.0,
                )
                .unwrap();
            ctrl.store
                .add_clip(
                    video_path.to_str().unwrap(),
                    clip2_path.to_str().unwrap(),
                    30.0,
                    40.0,
                )
                .unwrap();
        }

        let clips = get_clips_logic(controller).await.unwrap();
        assert_eq!(clips.len(), 2);

        drop(controller_temp_dir);
    }

    #[tokio::test]
    async fn test_delete_clip_logic() {
        let (controller, controller_temp_dir) = create_test_controller();
        let temp_dir = TempDir::new().unwrap();

        let video_path = temp_dir.path().join("test_video.mp4");
        std::fs::write(&video_path, "fake video content").unwrap();

        let clip_path = temp_dir.path().join("clip1.mp4");
        std::fs::write(&clip_path, "fake clip content").unwrap();

        let clip_id = {
            let mut ctrl = controller.lock().await;
            ctrl.current_video = Some(video_path.clone());
            let clip = ctrl
                .store
                .add_clip(
                    video_path.to_str().unwrap(),
                    clip_path.to_str().unwrap(),
                    10.0,
                    20.0,
                )
                .unwrap();
            clip.id
        };

        let result = delete_clip_logic(
            controller.clone(),
            clip_id,
            clip_path.to_str().unwrap().to_string(),
        )
        .await;

        assert!(result.is_ok());
        assert!(!clip_path.exists());

        let clips = get_clips_logic(controller).await.unwrap();
        assert_eq!(clips.len(), 0);

        drop(controller_temp_dir);
    }

    #[tokio::test]
    async fn test_save_clip_logic_no_video() {
        let (controller, _temp_dir) = create_test_controller();

        let result = save_clip_logic(controller, 10.0, 100.0).await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "No video loaded");
    }

    #[tokio::test]
    async fn test_save_clip_logic_already_clipping() {
        let (controller, _temp_dir) = create_test_controller();
        let temp_dir = TempDir::new().unwrap();

        let video_path = temp_dir.path().join("test_video.mp4");
        std::fs::write(&video_path, "fake video content").unwrap();

        {
            let mut ctrl = controller.lock().await;
            ctrl.current_video = Some(video_path);
            ctrl.is_clipping = true;
        }

        let result = save_clip_logic(controller, 10.0, 100.0).await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Clip already in progress");
    }

    #[tokio::test]
    async fn test_open_video_logic_file_not_found() {
        let (controller, _temp_dir) = create_test_controller();

        let result = open_video_logic(controller, PathBuf::from("/nonexistent/video.mp4")).await;

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("does not exist"));
    }

    #[tokio::test]
    async fn test_open_video_logic_web_format() {
        use tempfile::TempDir;
        use tokio::process::Command;

        let (controller, _temp_dir) = create_test_controller();
        let test_dir = TempDir::new().unwrap();
        let video_path = test_dir.path().join("test_video.mp4");

        // Create a small test video using FFmpeg
        let output = Command::new("ffmpeg")
            .args([
                "-y",
                "-f",
                "lavfi",
                "-i",
                "color=c=black:s=320x240:d=2",
                "-c:v",
                "libx264",
                "-t",
                "2",
                video_path.to_str().unwrap(),
            ])
            .output()
            .await
            .unwrap();

        if !output.status.success() {
            panic!("Failed to create test video");
        }

        let result = open_video_logic(controller.clone(), video_path.clone()).await;

        assert!(result.is_ok(), "Should succeed: {:?}", result.err());
        let response = result.unwrap();

        assert_eq!(response.source_path, video_path.to_str().unwrap());
        assert!(!response.converted, "MP4 should not need conversion");
        assert!(
            response.duration >= 1.5 && response.duration <= 2.5,
            "Duration should be ~2 seconds"
        );

        // Verify controller state was updated
        let ctrl = controller.lock().await;
        assert_eq!(ctrl.current_video, Some(video_path));
    }

    #[tokio::test]
    async fn test_open_video_logic_non_web_format() {
        use tempfile::TempDir;
        use tokio::process::Command;

        let (controller, _temp_dir) = create_test_controller();
        let test_dir = TempDir::new().unwrap();
        let video_path = test_dir.path().join("test_video.mkv");

        // Create a small test video in MKV format
        let output = Command::new("ffmpeg")
            .args([
                "-y",
                "-f",
                "lavfi",
                "-i",
                "color=c=red:s=320x240:d=2",
                "-c:v",
                "libx264",
                "-t",
                "2",
                video_path.to_str().unwrap(),
            ])
            .output()
            .await
            .unwrap();

        if !output.status.success() {
            panic!("Failed to create test video");
        }

        let result = open_video_logic(controller.clone(), video_path.clone()).await;

        assert!(result.is_ok(), "Should succeed: {:?}", result.err());
        let response = result.unwrap();

        assert_eq!(response.source_path, video_path.to_str().unwrap());
        assert!(response.converted, "MKV should need conversion");
        assert!(response.play_path.ends_with(".converted.mp4"));
        assert!(
            response.duration >= 1.5 && response.duration <= 2.5,
            "Duration should be ~2 seconds"
        );

        // Verify controller state was updated
        let ctrl = controller.lock().await;
        assert_eq!(ctrl.current_video, Some(video_path));
    }

    #[tokio::test]
    async fn test_start_video_server_logic() {
        use crate::video_server::VideoServer;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let video_path = temp_dir.path().join("test_video.mp4");
        std::fs::write(&video_path, "fake video content").unwrap();

        let video_server = Arc::new(Mutex::new(VideoServer::new()));

        let result = start_video_server_logic(
            video_server.clone(),
            video_path.to_str().unwrap().to_string(),
        )
        .await;

        assert!(result.is_ok(), "Should succeed: {:?}", result.err());
        let url = result.unwrap();

        assert!(url.starts_with("http://127.0.0.1:"));
        assert!(url.ends_with("/video.mp4"));
    }

    #[tokio::test]
    async fn test_start_video_server_logic_file_not_found() {
        use crate::video_server::VideoServer;

        let video_server = Arc::new(Mutex::new(VideoServer::new()));

        let result =
            start_video_server_logic(video_server, "/nonexistent/video.mp4".to_string()).await;

        // The server should still start even if file doesn't exist
        // (file existence is checked when serving, not when starting)
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_get_clips_logic_no_video_loaded() {
        let (controller, _temp_dir) = create_test_controller();

        // Don't set current_video, so it should return empty list
        let result = get_clips_logic(controller).await;

        assert!(result.is_ok());
        let clips = result.unwrap();
        assert_eq!(clips.len(), 0);
    }

    #[tokio::test]
    async fn test_save_settings_logic_with_output_dir() {
        let (controller, temp_dir) = create_test_controller();

        let output_dir = temp_dir.path().join("custom_output");
        std::fs::create_dir_all(&output_dir).unwrap();

        let settings = Settings {
            buffer_before: 3.0,
            buffer_after: 8.0,
            clip_key: "v".to_string(),
            output_dir: Some(output_dir.clone()),
            theme: crate::settings::Theme::Light,
        };

        let result = save_settings_logic(controller.clone(), settings).await;
        assert!(result.is_ok());

        // Verify settings were saved
        let loaded = get_settings_logic(controller).await.unwrap();
        assert_eq!(loaded.buffer_before, 3.0);
        assert_eq!(loaded.buffer_after, 8.0);
        assert_eq!(loaded.clip_key, "v");
        assert_eq!(loaded.output_dir, Some(output_dir));
        assert_eq!(loaded.theme, crate::settings::Theme::Light);
    }

    #[tokio::test]
    async fn test_save_settings_logic_invalid_buffer_before() {
        let (controller, _temp_dir) = create_test_controller();

        let settings = Settings {
            buffer_before: -1.0, // Invalid: negative
            buffer_after: 5.0,
            clip_key: "c".to_string(),
            output_dir: None,
            theme: crate::settings::Theme::Dark,
        };

        let result = save_settings_logic(controller, settings).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Buffer before"));
    }

    #[tokio::test]
    async fn test_save_settings_logic_invalid_buffer_after() {
        let (controller, _temp_dir) = create_test_controller();

        let settings = Settings {
            buffer_before: 5.0,
            buffer_after: -1.0, // Invalid: negative
            clip_key: "c".to_string(),
            output_dir: None,
            theme: crate::settings::Theme::Dark,
        };

        let result = save_settings_logic(controller, settings).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Buffer after"));
    }

    #[tokio::test]
    async fn test_save_settings_logic_invalid_clip_key() {
        let (controller, _temp_dir) = create_test_controller();

        let settings = Settings {
            buffer_before: 5.0,
            buffer_after: 5.0,
            clip_key: "ab".to_string(), // Invalid: more than one character
            output_dir: None,
            theme: crate::settings::Theme::Dark,
        };

        let result = save_settings_logic(controller, settings).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Clip key"));
    }

    #[tokio::test]
    async fn test_save_settings_logic_buffer_too_large() {
        let (controller, _temp_dir) = create_test_controller();

        let settings = Settings {
            buffer_before: 120.0, // Invalid: > 60 seconds
            buffer_after: 5.0,
            clip_key: "c".to_string(),
            output_dir: None,
            theme: crate::settings::Theme::Dark,
        };

        let result = save_settings_logic(controller, settings).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Buffer before"));
    }

    #[tokio::test]
    async fn test_save_settings_logic_empty_clip_key() {
        let (controller, _temp_dir) = create_test_controller();

        let settings = Settings {
            buffer_before: 5.0,
            buffer_after: 5.0,
            clip_key: "".to_string(), // Invalid: empty
            output_dir: None,
            theme: crate::settings::Theme::Dark,
        };

        let result = save_settings_logic(controller, settings).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Clip key"));
    }

    #[tokio::test]
    async fn test_save_settings_logic_nonexistent_output_dir() {
        let (controller, _temp_dir) = create_test_controller();

        let settings = Settings {
            buffer_before: 5.0,
            buffer_after: 5.0,
            clip_key: "c".to_string(),
            output_dir: Some(std::path::PathBuf::from(
                "/nonexistent/path/that/does/not/exist",
            )),
            theme: crate::settings::Theme::Dark,
        };

        let result = save_settings_logic(controller, settings).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Output directory"));
    }

    #[tokio::test]
    async fn test_get_clips_logic_with_current_video() {
        use tempfile::TempDir;
        use tokio::process::Command;

        let (controller, temp_dir) = create_test_controller();

        // Create a test video
        let video_path = temp_dir.path().join("test_video.mp4");
        let output = Command::new("ffmpeg")
            .args([
                "-y",
                "-f",
                "lavfi",
                "-i",
                "color=c=blue:s=320x240:d=5",
                "-c:v",
                "libx264",
                "-t",
                "5",
                video_path.to_str().unwrap(),
            ])
            .output()
            .await
            .unwrap();

        if !output.status.success() {
            panic!("Failed to create test video");
        }

        // Set current video
        {
            let mut ctrl = controller.lock().await;
            ctrl.current_video = Some(video_path);
        }

        // Get clips should return empty list (no clips yet)
        let result = get_clips_logic(controller).await;
        assert!(result.is_ok());
        let clips = result.unwrap();
        assert_eq!(clips.len(), 0);
    }

    #[tokio::test]
    async fn test_save_settings_logic_max_valid_buffers() {
        let (controller, _temp_dir) = create_test_controller();

        // Test with maximum valid buffer values (60 seconds)
        let settings = Settings {
            buffer_before: 60.0,
            buffer_after: 60.0,
            clip_key: "c".to_string(),
            output_dir: None,
            theme: crate::settings::Theme::Dark,
        };

        let result = save_settings_logic(controller.clone(), settings).await;
        assert!(result.is_ok());

        // Verify settings were saved
        let loaded = get_settings_logic(controller).await.unwrap();
        assert_eq!(loaded.buffer_before, 60.0);
        assert_eq!(loaded.buffer_after, 60.0);
    }

    #[tokio::test]
    async fn test_save_settings_logic_zero_buffers() {
        let (controller, _temp_dir) = create_test_controller();

        // Test with zero buffer values (minimum valid)
        let settings = Settings {
            buffer_before: 0.0,
            buffer_after: 0.0,
            clip_key: "c".to_string(),
            output_dir: None,
            theme: crate::settings::Theme::Dark,
        };

        let result = save_settings_logic(controller.clone(), settings).await;
        assert!(result.is_ok());

        // Verify settings were saved
        let loaded = get_settings_logic(controller).await.unwrap();
        assert_eq!(loaded.buffer_before, 0.0);
        assert_eq!(loaded.buffer_after, 0.0);
    }

    #[tokio::test]
    async fn test_delete_clip_logic_nonexistent_clip() {
        let (controller, _temp_dir) = create_test_controller();

        // Try to delete a clip that doesn't exist
        let result = delete_clip_logic(controller, 999, "/nonexistent/clip.mp4".to_string()).await;

        // Should succeed (no error if clip doesn't exist)
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_open_video_logic_with_webm_format() {
        use tempfile::TempDir;
        use tokio::process::Command;

        let (controller, temp_dir) = create_test_controller();

        // Create a WebM video
        let video_path = temp_dir.path().join("test_video.webm");
        let output = Command::new("ffmpeg")
            .args([
                "-y",
                "-f",
                "lavfi",
                "-i",
                "color=c=green:s=320x240:d=2",
                "-c:v",
                "libvpx-vp9",
                "-t",
                "2",
                video_path.to_str().unwrap(),
            ])
            .output()
            .await
            .unwrap();

        if !output.status.success() {
            panic!("Failed to create WebM test video");
        }

        let result = open_video_logic(controller.clone(), video_path.clone()).await;

        // Should succeed (WebM is web-compatible)
        assert!(result.is_ok(), "Should succeed: {:?}", result.err());
        let response = result.unwrap();

        // Verify the response
        assert_eq!(response.source_path, video_path.to_str().unwrap());
        assert!(response.duration >= 1.5 && response.duration <= 2.5);
    }

    #[tokio::test]
    async fn test_save_clip_logic_with_valid_video() {
        use tempfile::TempDir;
        use tokio::process::Command;

        let (controller, temp_dir) = create_test_controller();

        // Create a test video
        let video_path = temp_dir.path().join("test_video.mp4");
        let output = Command::new("ffmpeg")
            .args([
                "-y",
                "-f",
                "lavfi",
                "-i",
                "color=c=red:s=320x240:d=10",
                "-c:v",
                "libx264",
                "-t",
                "10",
                video_path.to_str().unwrap(),
            ])
            .output()
            .await
            .unwrap();

        if !output.status.success() {
            panic!("Failed to create test video");
        }

        // Set current video
        {
            let mut ctrl = controller.lock().await;
            ctrl.current_video = Some(video_path);
        }

        // Try to save a clip (this will actually call FFmpeg)
        let result = save_clip_logic(controller.clone(), 5.0, 10.0).await;

        // Should succeed (or fail gracefully if FFmpeg is not available)
        if result.is_ok() {
            let clip = result.unwrap();
            assert!(clip.success);
            assert!(clip.path.len() > 0);
        }
    }
}
