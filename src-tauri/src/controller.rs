use std::path::PathBuf;

#[cfg(target_os = "linux")]
use crate::x11_window::X11Window;

use crate::clipper::{Clipper, ClipResult};
use crate::error::{AppError, AppResult};
use crate::player::Player;
use crate::settings::Settings;
use crate::storage::{Clip, ClipStore};

/// Central orchestrator that owns all backend components.
///
/// The controller is the single entry point for every high-level operation
/// (opening a video, saving a clip, seeking, etc.). The GUI/frontend never
/// touches Player, Clipper, Settings, or ClipStore directly — it always goes
/// through the controller.
pub struct Controller {
    pub player: Player,
    pub clipper: Clipper,
    pub settings: Settings,
    pub store: ClipStore,
    pub current_video: Option<PathBuf>,
    pub clip_count: i32,
    pub is_clipping: bool,
    #[cfg(target_os = "linux")]
    pub mpv_window: Option<X11Window>,
    pub mpv_wid: Option<u64>,
}

impl Controller {
    /// Create a new controller, loading persisted settings and initializing
    /// the clip database.
    pub async fn new() -> AppResult<Self> {
        let settings = Settings::load()?;
        let store = ClipStore::new()?;
        let clipper = Clipper::new(settings.buffer_before, settings.buffer_after);
        let player = Player::new();

        Ok(Self {
            player,
            clipper,
            settings,
            store,
            current_video: None,
            clip_count: 0,
            is_clipping: false,
            #[cfg(target_os = "linux")]
            mpv_window: None,
            mpv_wid: None,
        })
    }

    /// Open a video file in mpv, spawning the player on first call.
    ///
    /// Returns the duration of the loaded video in seconds. Also reloads
    /// any previously saved clips for this video from the database.
    pub async fn open_video(&mut self, path: PathBuf, wid: Option<u64>) -> AppResult<f64> {
        // Use stored mpv window ID if no wid provided
        let wid = wid.or(self.mpv_wid);

        // Spawn mpv if not already running
        if !self.player.is_running() {
            self.player.spawn(wid).await?;
        }

        let duration = self.player.load(&path).await?;
        self.current_video = Some(path.clone());

        // Load clips for this video
        let clips = self
            .store
            .get_clips_for_video(path.to_str().unwrap_or(""))?;
        self.clip_count = clips.len() as i32;

        Ok(duration)
    }

    /// Toggle the pause state of the currently loaded video.
    pub async fn toggle_pause(&self) -> AppResult<()> {
        self.player.toggle_pause().await
    }

    /// Seek by the given number of seconds (relative).
    pub async fn seek(&self, seconds: f64) -> AppResult<()> {
        self.player.seek(seconds, true).await
    }

    /// Return the current playback position in seconds.
    pub async fn get_position(&self) -> AppResult<f64> {
        self.player.get_position().await
    }

    /// Save a clip around the current playback position.
    ///
    /// Uses the configured pre/post buffers to calculate the clip window.
    /// Rejects the request if a clip is already being saved or no video is
    /// loaded. The `is_clipping` flag is always reset, even when the inner
    /// operation fails.
    pub async fn save_clip(&mut self) -> AppResult<ClipResult> {
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
            let current_pos = self.player.get_position().await?;
            let duration = self.player.get_duration().await?;

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

    /// Shut down the mpv child process and clean up resources.
    pub async fn shutdown(&mut self) {
        self.player.shutdown().await;
        #[cfg(target_os = "linux")]
        {
            // Drop will destroy the X11 window
            self.mpv_window.take();
        }
    }
}
