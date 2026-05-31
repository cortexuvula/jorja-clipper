use std::path::PathBuf;
use tokio::sync::mpsc;

use crate::clipper::Clipper;
use crate::converter::{ConversionStatus, Converter};
use crate::error::{AppError, AppResult};
use crate::settings::Settings;
use crate::storage::{Clip, ClipStore};

impl Controller {
    /// Update application settings with validation.
    ///
    /// Validates buffer values (0-60 seconds), clip_key (single character),
    /// and output_dir (must exist and be writable if specified).
    /// Updates the Clipper with new buffer values, saves to disk, and updates
    /// the Controller's settings.
    pub fn update_settings(&mut self, new_settings: Settings) -> AppResult<()> {
        // Validate buffer values
        if new_settings.buffer_before < 0.0 || new_settings.buffer_before > 60.0 {
            return Err(AppError::Clip("Buffer before must be 0-60 seconds".into()));
        }
        if new_settings.buffer_after < 0.0 || new_settings.buffer_after > 60.0 {
            return Err(AppError::Clip("Buffer after must be 0-60 seconds".into()));
        }
        // Validate clip_key is a single character
        if new_settings.clip_key.is_empty() || new_settings.clip_key.chars().count() != 1 {
            return Err(AppError::Clip("Clip key must be a single character".into()));
        }

        // Validate output_dir if specified
        if let Some(ref output_dir) = new_settings.output_dir {
            if !output_dir.exists() {
                return Err(AppError::Clip(format!(
                    "Output directory does not exist: {}",
                    output_dir.display()
                )));
            }
            if !output_dir.is_dir() {
                return Err(AppError::Clip(format!(
                    "Output path is not a directory: {}",
                    output_dir.display()
                )));
            }
            // Check if directory is writable by trying to create a temp file
            let test_file = output_dir.join(".write_test");
            match std::fs::File::create(&test_file) {
                Ok(_) => {
                    let _ = std::fs::remove_file(&test_file);
                }
                Err(e) => {
                    return Err(AppError::Clip(format!(
                        "Output directory is not writable: {}",
                        e
                    )));
                }
            }
        }

        // Update clipper with new buffer values
        self.clipper = Clipper::new(new_settings.buffer_before, new_settings.buffer_after);

        // Save to disk
        new_settings.save()?;
        self.settings = new_settings;

        Ok(())
    }
}

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
        let clips_dir = crate::util::app_config_dir().join("clips");

        // Create clips directory if it doesn't exist
        if !clips_dir.exists() {
            std::fs::create_dir_all(&clips_dir).map_err(AppError::Io)?;
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
    /// Times out after 4 hours to prevent infinite hangs.
    pub async fn open_video(
        &mut self,
        path: PathBuf,
        progress_tx: Option<mpsc::Sender<ConversionStatus>>,
    ) -> AppResult<OpenVideoResponse> {
        // Check if file exists before doing any work
        if !path.exists() {
            return Err(AppError::Clip(format!(
                "Video file does not exist: {}",
                path.display()
            )));
        }

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

            // Wrap conversion in timeout (4 hours max)
            let timeout_duration = tokio::time::Duration::from_secs(4 * 60 * 60);
            let converted_path = tokio::time::timeout(
                timeout_duration,
                Converter::convert_to_mp4(&path, &self.clips_dir, progress_tx)
            )
            .await
            .map_err(|_| AppError::Clip("Conversion timed out after 4 hours".to_string()))??;

            (converted_path, true)
        };

        // Get duration using ffprobe
        let duration = Converter::get_duration(&source_path).await?;

        // Store the original source path for clipping
        self.current_video = Some(source_path.clone());

        // Load and validate clips for this video (get_clips validates file existence and updates clip_count)
        self.get_clips()?;

        Ok(OpenVideoResponse {
            play_path: play_path.to_string_lossy().to_string(),
            source_path: source_path.to_string_lossy().to_string(),
            duration,
            converted,
        })
    }

    /// Return all saved clips for the currently loaded video.
    ///
    /// Returns an empty vec if no video is loaded. Automatically removes
    /// clips whose files have been deleted or moved and updates clip_count.
    pub fn get_clips(&mut self) -> AppResult<Vec<Clip>> {
        if let Some(video_path) = &self.current_video {
            let video_path_str = video_path
                .to_str()
                .ok_or_else(|| AppError::Clip("Video path contains non-UTF8 characters".to_string()))?;
            let clips = self.store.get_clips_for_video(video_path_str)?;

            // Filter out clips whose files no longer exist on disk
            let mut valid_clips = Vec::new();
            for clip in clips {
                let clip_path = std::path::Path::new(&clip.clip_path);
                // Check if file exists and is accessible
                if clip_path.exists() && clip_path.is_file() {
                    valid_clips.push(clip);
                } else {
                    // Remove stale entry from database
                    let _ = self.store.delete_clip(clip.id);
                }
            }

            // Update clip_count to reflect actual number of valid clips
            self.clip_count = valid_clips.len() as i32;

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
