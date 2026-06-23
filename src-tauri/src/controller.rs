use std::path::PathBuf;

use crate::clipper::Clipper;
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
    /// Path of the file currently being played back (the converted MP4 when a
    /// conversion happened, otherwise the source). The background cleanup task
    /// reads this so it never deletes a converted file the user is watching.
    pub last_play_path: Option<PathBuf>,
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
            last_play_path: None,
        })
    }

    /// Return all saved clips for the currently loaded video.
    ///
    /// Returns an empty vec if no video is loaded. Automatically removes
    /// clips whose files have been deleted or moved and updates clip_count.
    pub fn get_clips(&mut self) -> AppResult<Vec<Clip>> {
        if let Some(video_path) = &self.current_video {
            let video_path_str = video_path.to_str().ok_or_else(|| {
                AppError::Clip("Video path contains non-UTF8 characters".to_string())
            })?;
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

            // NOTE: clip numbering is NOT derived from clip_count. It is
            // derived from the highest existing clip file on disk in
            // Clipper::next_clip_number, so deleting clips never causes the
            // next clip to reuse a number and overwrite an existing file.
            // We still mirror the current count here for diagnostics only.
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    /// Serialize tests that mutate process-global environment variables.
    /// Rust runs tests in parallel by default; without this, an env-mutating
    /// test can race against other tests that read the same variable
    /// (e.g. anything calling app_config_dir, which reads XDG_CONFIG_HOME).
    ///
    /// Uses tokio's async-aware Mutex so the guard can be held across `.await`
    /// points (clippy flags holding a std Mutex across an await). Wrapped in
    /// OnceLock because `tokio::sync::Mutex::new` is not const.
    async fn env_lock() -> tokio::sync::MutexGuard<'static, ()> {
        static ENV_LOCK: std::sync::OnceLock<tokio::sync::Mutex<()>> = std::sync::OnceLock::new();
        ENV_LOCK
            .get_or_init(|| tokio::sync::Mutex::new(()))
            .lock()
            .await
    }

    /// RAII guard that restores the previous value of an env var on drop.
    /// Used so env-mutating tests never leak their changes to the rest of the
    /// test process, even if they panic.
    struct EnvVarGuard {
        key: String,
        previous: Option<std::ffi::OsString>,
    }

    impl EnvVarGuard {
        fn set<K: Into<String>, V: AsRef<std::ffi::OsStr>>(key: K, value: V) -> Self {
            let key = key.into();
            let previous = std::env::var_os(&key);
            std::env::set_var(&key, value);
            Self { key, previous }
        }
    }

    impl Drop for EnvVarGuard {
        fn drop(&mut self) {
            match &self.previous {
                Some(val) => std::env::set_var(&self.key, val),
                None => std::env::remove_var(&self.key),
            }
        }
    }

    fn create_test_controller() -> (Controller, TempDir) {
        let temp_dir = TempDir::new().unwrap();

        // Create a unique database for this test
        let db_path = temp_dir.path().join("test_clips.db");
        let store = ClipStore::with_path(&db_path).unwrap();

        let settings = Settings {
            buffer_before: 5.0,
            buffer_after: 5.0,
            clip_key: "c".to_string(),
            theme: crate::settings::Theme::Dark,
            output_dir: None,
        };

        let controller = Controller {
            clipper: Clipper::new(5.0, 5.0),
            settings: settings.clone(),
            store,
            current_video: None,
            clip_count: 0,
            is_clipping: false,
            clips_dir: temp_dir.path().join("clips"),
            last_play_path: None,
        };

        (controller, temp_dir)
    }

    #[test]
    fn test_update_settings_valid_buffers() {
        let (mut controller, _temp_dir) = create_test_controller();

        let new_settings = Settings {
            buffer_before: 10.0,
            buffer_after: 15.0,
            clip_key: "x".to_string(),
            theme: crate::settings::Theme::Dark,
            output_dir: None,
        };

        let result = controller.update_settings(new_settings.clone());
        assert!(result.is_ok());
        assert_eq!(controller.settings.buffer_before, 10.0);
        assert_eq!(controller.settings.buffer_after, 15.0);
        assert_eq!(controller.settings.clip_key, "x");
    }

    #[test]
    fn test_update_settings_invalid_buffer_before() {
        let (mut controller, _temp_dir) = create_test_controller();

        let settings = Settings {
            buffer_before: -1.0,
            buffer_after: 5.0,
            clip_key: "c".to_string(),
            theme: crate::settings::Theme::Dark,
            output_dir: None,
        };

        let result = controller.update_settings(settings);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Buffer before"));

        let settings = Settings {
            buffer_before: 61.0,
            buffer_after: 5.0,
            clip_key: "c".to_string(),
            theme: crate::settings::Theme::Dark,
            output_dir: None,
        };

        let result = controller.update_settings(settings);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Buffer before"));
    }

    #[test]
    fn test_update_settings_invalid_buffer_after() {
        let (mut controller, _temp_dir) = create_test_controller();

        let settings = Settings {
            buffer_before: 5.0,
            buffer_after: -5.0,
            clip_key: "c".to_string(),
            theme: crate::settings::Theme::Dark,
            output_dir: None,
        };

        let result = controller.update_settings(settings);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Buffer after"));
    }

    #[test]
    fn test_update_settings_invalid_clip_key_empty() {
        let (mut controller, _temp_dir) = create_test_controller();

        let settings = Settings {
            buffer_before: 5.0,
            buffer_after: 5.0,
            clip_key: "".to_string(),
            theme: crate::settings::Theme::Dark,
            output_dir: None,
        };

        let result = controller.update_settings(settings);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("single character"));
    }

    #[test]
    fn test_update_settings_invalid_clip_key_multiple_chars() {
        let (mut controller, _temp_dir) = create_test_controller();

        let settings = Settings {
            buffer_before: 5.0,
            buffer_after: 5.0,
            clip_key: "ab".to_string(),
            theme: crate::settings::Theme::Dark,
            output_dir: None,
        };

        let result = controller.update_settings(settings);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("single character"));
    }

    #[test]
    fn test_update_settings_output_dir_nonexistent() {
        let (mut controller, _temp_dir) = create_test_controller();

        let settings = Settings {
            buffer_before: 5.0,
            buffer_after: 5.0,
            clip_key: "c".to_string(),
            theme: crate::settings::Theme::Dark,
            output_dir: Some(PathBuf::from("/nonexistent/directory")),
        };

        let result = controller.update_settings(settings);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("does not exist"));
    }

    #[test]
    fn test_update_settings_output_dir_valid() {
        let (mut controller, temp_dir) = create_test_controller();

        let settings = Settings {
            buffer_before: 5.0,
            buffer_after: 5.0,
            clip_key: "c".to_string(),
            theme: crate::settings::Theme::Dark,
            output_dir: Some(temp_dir.path().to_path_buf()),
        };

        let result = controller.update_settings(settings);
        assert!(result.is_ok());
    }

    #[test]
    fn test_update_settings_output_dir_not_a_directory() {
        let (mut controller, temp_dir) = create_test_controller();

        // Create a file (not a directory)
        let file_path = temp_dir.path().join("not_a_dir.txt");
        std::fs::write(&file_path, "content").unwrap();

        let settings = Settings {
            buffer_before: 5.0,
            buffer_after: 5.0,
            clip_key: "c".to_string(),
            theme: crate::settings::Theme::Dark,
            output_dir: Some(file_path),
        };

        let result = controller.update_settings(settings);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not a directory"));
    }

    #[test]
    fn test_update_settings_output_dir_not_writable() {
        let (mut controller, temp_dir) = create_test_controller();

        // Create a directory without write permissions
        let readonly_dir = temp_dir.path().join("readonly");
        std::fs::create_dir(&readonly_dir).unwrap();

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = std::fs::Permissions::from_mode(0o555);
            std::fs::set_permissions(&readonly_dir, perms).unwrap();
        }

        let settings = Settings {
            buffer_before: 5.0,
            buffer_after: 5.0,
            clip_key: "c".to_string(),
            theme: crate::settings::Theme::Dark,
            output_dir: Some(readonly_dir.clone()),
        };

        let result = controller.update_settings(settings);

        // Restore permissions for cleanup
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = std::fs::Permissions::from_mode(0o755);
            std::fs::set_permissions(&readonly_dir, perms).unwrap();
        }

        #[cfg(unix)]
        {
            assert!(result.is_err());
            assert!(result.unwrap_err().to_string().contains("not writable"));
        }

        #[cfg(not(unix))]
        {
            // On non-Unix, just ensure it doesn't panic
            let _ = result;
        }
    }

    #[test]
    fn test_get_clips_no_video_loaded() {
        let (mut controller, _temp_dir) = create_test_controller();

        let clips = controller.get_clips().unwrap();
        assert_eq!(clips.len(), 0);
    }

    #[test]
    fn test_delete_clip_removes_file_and_db_entry() {
        let (controller, temp_dir) = create_test_controller();

        // Create a test clip file
        let clip_file = temp_dir.path().join("test_clip.mp4");
        std::fs::write(&clip_file, "fake video data").unwrap();
        assert!(clip_file.exists());

        // Add clip to database
        let video_path = "/test/video.mp4";
        controller
            .store
            .add_clip(video_path, clip_file.to_str().unwrap(), 10.0, 20.0)
            .unwrap();

        // Get the clip ID
        let clips = controller.store.get_clips_for_video(video_path).unwrap();
        assert_eq!(clips.len(), 1);
        let clip_id = clips[0].id;

        // Delete the clip
        controller
            .delete_clip(clip_id, clip_file.to_str().unwrap())
            .unwrap();

        // File should be removed
        assert!(!clip_file.exists());

        // DB entry should be removed
        let remaining = controller.store.get_clips_for_video(video_path).unwrap();
        assert_eq!(remaining.len(), 0);
    }

    #[test]
    fn test_delete_clip_handles_missing_file() {
        let (controller, _temp_dir) = create_test_controller();

        // Add clip with nonexistent file path
        let video_path = "/test/video.mp4";
        controller
            .store
            .add_clip(video_path, "/nonexistent/clip.mp4", 10.0, 20.0)
            .unwrap();

        let clips = controller.store.get_clips_for_video(video_path).unwrap();
        let clip_id = clips[0].id;

        // Delete should succeed even if file doesn't exist
        let result = controller.delete_clip(clip_id, "/nonexistent/clip.mp4");
        assert!(result.is_ok());

        // DB entry should be removed
        let remaining = controller.store.get_clips_for_video(video_path).unwrap();
        assert_eq!(remaining.len(), 0);
    }

    #[test]
    fn test_get_clips_with_video_loaded() {
        let (mut controller, temp_dir) = create_test_controller();

        // Create test video and clip files
        let video_file = temp_dir.path().join("test_video.mp4");
        let clip1_file = temp_dir.path().join("clip1.mp4");
        let clip2_file = temp_dir.path().join("clip2.mp4");

        std::fs::write(&video_file, "fake video").unwrap();
        std::fs::write(&clip1_file, "clip1 data").unwrap();
        std::fs::write(&clip2_file, "clip2 data").unwrap();

        // Set current video
        controller.current_video = Some(video_file.clone());

        // Add clips to database
        controller
            .store
            .add_clip(
                video_file.to_str().unwrap(),
                clip1_file.to_str().unwrap(),
                10.0,
                20.0,
            )
            .unwrap();

        controller
            .store
            .add_clip(
                video_file.to_str().unwrap(),
                clip2_file.to_str().unwrap(),
                30.0,
                40.0,
            )
            .unwrap();

        // Get clips should return both
        let clips = controller.get_clips().unwrap();
        assert_eq!(clips.len(), 2);
        assert_eq!(controller.clip_count, 2);
    }

    #[test]
    fn test_get_clips_filters_missing_files() {
        let (mut controller, temp_dir) = create_test_controller();

        // Create test video
        let video_file = temp_dir.path().join("test_video.mp4");
        std::fs::write(&video_file, "fake video").unwrap();

        // Create only one clip file
        let clip1_file = temp_dir.path().join("clip1.mp4");
        std::fs::write(&clip1_file, "clip1 data").unwrap();

        // Second clip file doesn't exist
        let clip2_file = temp_dir.path().join("clip2_missing.mp4");

        // Set current video
        controller.current_video = Some(video_file.clone());

        // Add both clips to database
        controller
            .store
            .add_clip(
                video_file.to_str().unwrap(),
                clip1_file.to_str().unwrap(),
                10.0,
                20.0,
            )
            .unwrap();

        controller
            .store
            .add_clip(
                video_file.to_str().unwrap(),
                clip2_file.to_str().unwrap(),
                30.0,
                40.0,
            )
            .unwrap();

        // Get clips should only return the one with existing file
        let clips = controller.get_clips().unwrap();
        assert_eq!(clips.len(), 1);
        assert_eq!(clips[0].clip_path, clip1_file.to_str().unwrap());
        assert_eq!(controller.clip_count, 1);

        // Missing clip should be removed from database
        let remaining = controller
            .store
            .get_clips_for_video(video_file.to_str().unwrap())
            .unwrap();
        assert_eq!(remaining.len(), 1);
    }

    #[tokio::test]
    async fn test_controller_new_creates_clips_directory() {
        // This test mutates a process-global env var (XDG_CONFIG_HOME). Other
        // tests in the crate call app_config_dir() and would observe this
        // value if it leaked. Use ENV_LOCK to serialize against any other
        // env-mutating test, and EnvVarGuard to restore the prior value.
        let _env_lock = env_lock().await;
        let temp_dir = tempfile::tempdir().unwrap();
        let _guard = EnvVarGuard::set("XDG_CONFIG_HOME", temp_dir.path());

        let controller = Controller::new().await.unwrap();

        // Controller should be created successfully
        assert_eq!(controller.clip_count, 0);
        assert!(controller.last_play_path.is_none());
        assert!(!controller.is_clipping);
        assert!(controller.current_video.is_none());

        // Clips directory should exist
        assert!(controller.clips_dir.exists());
        assert!(controller.clips_dir.is_dir());
    }

    #[test]
    fn test_controller_default_state() {
        let (controller, _temp_dir) = create_test_controller();

        assert_eq!(controller.clip_count, 0);
        assert!(!controller.is_clipping);
        assert!(controller.current_video.is_none());
        assert_eq!(controller.settings.buffer_before, 5.0);
        assert_eq!(controller.settings.buffer_after, 5.0);
    }
}
