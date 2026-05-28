use std::path::PathBuf;
use tokio::sync::mpsc;

use crate::clipper::{ClipResult, Clipper};
use crate::converter::{ConversionStatus, Converter};
use crate::error::{AppError, AppResult};
use crate::settings::Settings;
use crate::storage::{Clip, ClipStore};

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

/// Central orchestrator that owns all backend components.
///
/// The controller is the single entry point for every high-level operation
/// (opening a video, saving a clip, etc.). The GUI/frontend never touches
/// Clipper, Settings, or ClipStore directly — it always goes through the controller.
pub struct Controller {
    pub clipper: Clipper,
    #[allow(dead_code)]
    pub settings: Settings,
    pub store: ClipStore,
    pub current_video: Option<PathBuf>,
    pub clip_count: i32,
    pub is_clipping: bool,
    pub clips_dir: PathBuf,
}

impl Controller {
    /// Create a new controller, loading persisted settings and initializing
    /// the clip database.
    pub async fn new() -> AppResult<Self> {
        let settings = Settings::load()?;
        let store = ClipStore::new()?;
        let clipper = Clipper::new(settings.buffer_before, settings.buffer_after);

        // Use clips directory for converted files
        let clips_dir = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("jorja-clipper")
            .join("clips");

        // Create clips directory if it doesn't exist
        if !clips_dir.exists() {
            std::fs::create_dir_all(&clips_dir)
                .map_err(|e| AppError::Io(e))?;
        }

        Ok(Self {
            clipper,
            settings,
            store,
            current_video: None,
            clip_count: 0,
            is_clipping: false,
            clips_dir,
        })
    }

    /// Open a video file, converting if necessary for web playback.
    ///
    /// Returns information about the video including the path to play and duration.
    /// Emits progress updates via the channel for non-web formats that need conversion.
    pub async fn open_video(
        &mut self,
        path: PathBuf,
        progress_tx: Option<mpsc::Sender<ConversionStatus>>,
    ) -> AppResult<OpenVideoResponse> {
        let source_path = path.clone();

        // Check if file is web-compatible
        let (play_path, converted) = if Converter::is_web_compatible(&path) {
            // Direct play, no conversion needed
            (path.clone(), false)
        } else {
            // Need to convert
            let progress_tx = progress_tx.ok_or_else(|| {
                AppError::Clip("Progress channel required for conversion".to_string())
            })?;

            let converted_path =
                Converter::convert_to_mp4(&path, &self.clips_dir, progress_tx).await?;

            (converted_path, true)
        };

        // Get duration using ffprobe
        let duration = Converter::get_duration(&source_path).await?;

        // Store the original source path for clipping
        self.current_video = Some(source_path.clone());

        // Load clips for this video
        let clips = self
            .store
            .get_clips_for_video(source_path.to_str().unwrap_or(""))?;
        self.clip_count = clips.len() as i32;

        Ok(OpenVideoResponse {
            play_path: play_path.to_string_lossy().to_string(),
            source_path: source_path.to_string_lossy().to_string(),
            duration,
            converted,
        })
    }

    /// Save a clip at the specified position.
    ///
    /// Uses the configured pre/post buffers to calculate the clip window.
    /// Rejects the request if a clip is already being saved or no video is loaded.
    /// The `is_clipping` flag is always reset, even when the operation fails.
    pub async fn save_clip(
        &mut self,
        current_pos: f64,
        duration: f64,
    ) -> AppResult<ClipResult> {
        if self.is_clipping {
            return Err(AppError::ClipInProgress);
        }

        let video_path = self
            .current_video
            .as_ref()
            .ok_or(AppError::NoVideoLoaded)?
            .clone();

        self.is_clipping = true;

        let result = async {
            let (start_time, end_time) = self.clipper.calculate_times(current_pos, duration);
            let clip_number = self.clip_count + 1;
            let output_path = self.clipper.output_path(&video_path, clip_number)?;

            let clip_result = self
                .clipper
                .save_clip(&video_path, start_time, end_time, &output_path)
                .await?;

            if clip_result.success {
                let _clip = self.store.add_clip(
                    video_path.to_str().unwrap_or(""),
                    &clip_result.path,
                    start_time,
                    end_time,
                )?;
                self.clip_count += 1;
            }

            Ok(clip_result)
        }
        .await;

        self.is_clipping = false;

        result
    }

    /// Return all saved clips for the currently loaded video.
    ///
    /// Returns an empty vec if no video is loaded. Automatically removes
    /// clips whose files have been deleted from disk.
    pub fn get_clips(&self) -> AppResult<Vec<Clip>> {
        if let Some(video_path) = &self.current_video {
            let clips = self
                .store
                .get_clips_for_video(video_path.to_str().unwrap_or(""))?;

            // Filter out clips whose files no longer exist on disk
            let mut valid_clips = Vec::new();
            for clip in clips {
                if std::path::Path::new(&clip.clip_path).exists() {
                    valid_clips.push(clip);
                } else {
                    // Remove stale entry from database
                    let _ = self.store.delete_clip(clip.id);
                }
            }

            Ok(valid_clips)
        } else {
            Ok(Vec::new())
        }
    }

    /// Delete a clip by ID — removes the file from disk and the DB entry.
    pub fn delete_clip(&self, id: i64, clip_path: &str) -> AppResult<()> {
        // Delete the file from disk (ignore errors if already gone)
        let _ = std::fs::remove_file(clip_path);
        // Remove from database
        self.store.delete_clip(id)
    }
}
