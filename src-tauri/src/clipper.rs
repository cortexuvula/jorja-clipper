use crate::error::{ffmpeg_not_found_error, AppError, AppResult};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipResult {
    pub success: bool,
    pub path: String,
    pub start_time: f64,
    pub end_time: f64,
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Clipper {
    pub buffer_before: f64,
    pub buffer_after: f64,
}

impl Clipper {
    pub fn new(buffer_before: f64, buffer_after: f64) -> Self {
        Self {
            buffer_before,
            buffer_after,
        }
    }

    pub fn calculate_times(&self, current_pos: f64, duration: f64) -> (f64, f64) {
        let start = (current_pos - self.buffer_before).max(0.0);
        let end = (current_pos + self.buffer_after).min(duration);
        (start, end)
    }

    /// Validate that the clip has a positive duration.
    pub fn validate_times(start: f64, end: f64) -> Result<(), String> {
        if end <= start {
            return Err(format!(
                "Invalid clip: start ({:.1}s) >= end ({:.1}s). Adjust position or buffer settings.",
                start, end
            ));
        }
        // Use tolerance for floating-point precision (e.g. 10.1 - 10.0 ≈ 0.0999...)
        if end - start < 0.099 {
            return Err("Clip duration too short (minimum 0.1s)".to_string());
        }
        Ok(())
    }

    pub fn output_path(
        &self,
        video_path: &Path,
        clip_number: i32,
        output_dir: Option<&Path>,
    ) -> AppResult<PathBuf> {
        let clips_dir = match output_dir {
            Some(dir) => dir.to_path_buf(),
            None => video_path
                .parent()
                .ok_or_else(|| AppError::Ffmpeg("Video has no parent directory".to_string()))?
                .join("clips"),
        };

        std::fs::create_dir_all(&clips_dir)?;

        let video_stem = video_path
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or_else(|| AppError::Ffmpeg("Video has no filename".to_string()))?;

        let clip_filename = format!("{}_clip_{:05}.mp4", video_stem, clip_number);

        Ok(clips_dir.join(clip_filename))
    }

    pub async fn save_clip(
        &self,
        video_path: &Path,
        start_time: f64,
        end_time: f64,
        output_path: &Path,
    ) -> AppResult<ClipResult> {
        // Convert paths to strings, returning a graceful error for non-UTF8 paths
        let video_path_str = video_path.to_str().ok_or_else(|| {
            AppError::Ffmpeg("Video path contains non-UTF8 characters".to_string())
        })?;
        let output_path_str = output_path.to_str().ok_or_else(|| {
            AppError::Ffmpeg("Output path contains non-UTF8 characters".to_string())
        })?;

        // Run FFmpeg with stream copy (lossless)
        let mut cmd = Command::new(crate::util::resolve_binary("ffmpeg"));
        cmd.args([
            "-y", // Overwrite output
            "-ss",
            &format!("{:.3}", start_time),
            "-to",
            &format!("{:.3}", end_time),
            "-i",
            video_path_str,
            "-c",
            "copy", // Stream copy (no re-encoding)
            "-avoid_negative_ts",
            "1",
            output_path_str,
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

        // Prevent console window from appearing on Windows
        #[cfg(windows)]
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW

        let output = cmd.output().await.map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                ffmpeg_not_found_error("saving clip")
            } else {
                AppError::Ffmpeg(format!("Failed to run FFmpeg: {}", e))
            }
        })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);

            // Clean up partial output
            if output_path.exists() {
                let _ = std::fs::remove_file(output_path);
            }

            return Err(AppError::Ffmpeg(format!("FFmpeg failed: {}", stderr)));
        }

        Ok(ClipResult {
            success: true,
            path: output_path.to_string_lossy().to_string(),
            start_time,
            end_time,
            error: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_times() {
        let clipper = Clipper::new(5.0, 5.0);

        let (start, end) = clipper.calculate_times(30.0, 120.0);
        assert_eq!(start, 25.0);
        assert_eq!(end, 35.0);

        // Test clamping at start
        let (start, end) = clipper.calculate_times(2.0, 120.0);
        assert_eq!(start, 0.0);
        assert_eq!(end, 7.0);

        // Test clamping at end
        let (start, end) = clipper.calculate_times(118.0, 120.0);
        assert_eq!(start, 113.0);
        assert_eq!(end, 120.0);
    }

    #[test]
    fn test_validate_times() {
        // Valid clip
        assert!(Clipper::validate_times(10.0, 20.0).is_ok());

        // Zero duration
        assert!(Clipper::validate_times(10.0, 10.0).is_err());

        // Negative duration (start > end)
        assert!(Clipper::validate_times(20.0, 10.0).is_err());

        // Too short (< 0.1s)
        assert!(Clipper::validate_times(10.0, 10.05).is_err());

        // Minimum valid duration
        assert!(Clipper::validate_times(10.0, 10.1).is_ok());
    }

    #[test]
    fn test_output_path() {
        let clipper = Clipper::new(5.0, 5.0);
        let tmp_dir = std::env::temp_dir().join("jorja_clipper_test_videos");
        let _ = std::fs::create_dir_all(&tmp_dir);
        let video_path = tmp_dir.join("game.mp4");

        let output = clipper.output_path(&video_path, 1, None).unwrap();

        assert!(output.to_str().unwrap().contains("clips/"));
        assert!(output.to_str().unwrap().contains("game_clip_00001.mp4"));

        // Clean up
        let _ = std::fs::remove_dir_all(&tmp_dir);
    }

    #[tokio::test]
    async fn test_save_clip_with_invalid_video() {
        let clipper = Clipper::new(5.0, 5.0);
        let video_path = Path::new("/nonexistent/video.mp4");
        let output_path = Path::new("/tmp/test_clip.mp4");

        let result = clipper.save_clip(video_path, 0.0, 10.0, output_path).await;

        assert!(result.is_err());
    }

    #[test]
    fn test_calculate_times_edge_cases() {
        let clipper = Clipper::new(5.0, 5.0);

        // At exact start
        let (start, end) = clipper.calculate_times(0.0, 120.0);
        assert_eq!(start, 0.0);
        assert_eq!(end, 5.0);

        // At exact end
        let (start, end) = clipper.calculate_times(120.0, 120.0);
        assert_eq!(start, 115.0);
        assert_eq!(end, 120.0);

        // Short video
        let (start, end) = clipper.calculate_times(5.0, 10.0);
        assert_eq!(start, 0.0);
        assert_eq!(end, 10.0);
    }

    #[test]
    fn test_calculate_times_custom_buffers() {
        let clipper = Clipper::new(10.0, 20.0);
        let (start, end) = clipper.calculate_times(50.0, 120.0);
        assert_eq!(start, 40.0);
        assert_eq!(end, 70.0);
    }

    #[test]
    fn test_validate_times_boundary() {
        // Just above tolerance threshold (0.099)
        assert!(Clipper::validate_times(10.0, 10.11).is_ok());

        // Exactly at tolerance boundary (0.099) - should pass since it's not < 0.099
        assert!(Clipper::validate_times(10.0, 10.099).is_ok());

        // Just below tolerance threshold - should fail
        assert!(Clipper::validate_times(10.0, 10.098).is_err());
    }

    #[test]
    fn test_output_path_with_custom_dir() {
        let clipper = Clipper::new(5.0, 5.0);
        let tmp_dir = std::env::temp_dir().join("jorja_clipper_test_custom");
        let _ = std::fs::create_dir_all(&tmp_dir);
        let video_path = Path::new("/test/video.mp4");

        let output = clipper.output_path(video_path, 42, Some(&tmp_dir)).unwrap();

        assert!(output.starts_with(&tmp_dir));
        assert!(output.to_str().unwrap().contains("video_clip_00042.mp4"));

        let _ = std::fs::remove_dir_all(&tmp_dir);
    }

    #[test]
    fn test_output_path_creates_directory() {
        let clipper = Clipper::new(5.0, 5.0);
        let tmp_dir = std::env::temp_dir().join("jorja_clipper_test_new_dir");
        let _ = std::fs::remove_dir_all(&tmp_dir);

        let video_path = Path::new("/test/video.mp4");
        let result = clipper.output_path(video_path, 1, Some(&tmp_dir));

        assert!(result.is_ok());
        assert!(tmp_dir.exists());

        let _ = std::fs::remove_dir_all(&tmp_dir);
    }

    #[test]
    fn test_clip_result_serialization() {
        let result = ClipResult {
            success: true,
            path: "/test/clip.mp4".to_string(),
            start_time: 10.5,
            end_time: 20.3,
            error: None,
        };

        let json = serde_json::to_string(&result).unwrap();
        let deserialized: ClipResult = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.success, result.success);
        assert_eq!(deserialized.path, result.path);
        assert_eq!(deserialized.start_time, result.start_time);
    }

    #[test]
    fn test_clipper_clone() {
        let clipper = Clipper::new(5.0, 10.0);
        let cloned = clipper.clone();

        assert_eq!(cloned.buffer_before, 5.0);
        assert_eq!(cloned.buffer_after, 10.0);
    }

    #[tokio::test]
    async fn test_save_clip_with_real_video() {
        use tempfile::TempDir;
        use tokio::process::Command;

        let temp_dir = TempDir::new().unwrap();
        let video_path = temp_dir.path().join("test_video.mp4");
        let output_path = temp_dir.path().join("output_clip.mp4");

        // Create a small test video using FFmpeg (10 seconds)
        let output = Command::new("ffmpeg")
            .args([
                "-y",
                "-f",
                "lavfi",
                "-i",
                "color=c=blue:s=320x240:d=10",
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

        let clipper = Clipper::new(2.0, 2.0);

        // Save a clip from 5.0 to 7.0 seconds
        let result = clipper.save_clip(&video_path, 5.0, 7.0, &output_path).await;

        assert!(result.is_ok(), "Should succeed: {:?}", result.err());
        let clip_result = result.unwrap();

        assert!(clip_result.success);
        assert_eq!(clip_result.path, output_path.to_str().unwrap());
        assert_eq!(clip_result.start_time, 5.0);
        assert_eq!(clip_result.end_time, 7.0);
        assert!(clip_result.error.is_none());
        assert!(output_path.exists(), "Output clip should exist");
    }

    #[tokio::test]
    async fn test_save_clip_with_corrupted_video() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let video_path = temp_dir.path().join("corrupted_video.mp4");
        let output_path = temp_dir.path().join("output_clip.mp4");

        // Create a corrupted video file (just write some random bytes)
        std::fs::write(&video_path, b"This is not a valid video file").unwrap();

        let clipper = Clipper::new(2.0, 2.0);

        // Try to save a clip from the corrupted video
        let result = clipper.save_clip(&video_path, 1.0, 3.0, &output_path).await;

        // Should fail because the video is corrupted
        assert!(result.is_err(), "Should fail for corrupted video");
    }

    #[tokio::test]
    async fn test_save_clip_with_nonexistent_video() {
        let clipper = Clipper::new(2.0, 2.0);

        // Try to save a clip from a non-existent video
        let result = clipper
            .save_clip(
                Path::new("/nonexistent/video.mp4"),
                1.0,
                3.0,
                Path::new("/tmp/output.mp4"),
            )
            .await;

        // Should fail because the video doesn't exist
        assert!(result.is_err());
    }

    #[test]
    fn test_output_path_with_special_characters_in_filename() {
        use tempfile::TempDir;

        let clipper = Clipper::new(5.0, 5.0);
        let temp_dir = TempDir::new().unwrap();

        // Test with special characters in filename
        let video_path = temp_dir.path().join("my video (2024) [HD].mp4");
        let result = clipper.output_path(&video_path, 1, None);

        assert!(result.is_ok());
        let output_path = result.unwrap();
        assert!(output_path
            .to_str()
            .unwrap()
            .contains("my video (2024) [HD]"));
    }

    #[test]
    fn test_output_path_with_unicode_filename() {
        use tempfile::TempDir;

        let clipper = Clipper::new(5.0, 5.0);
        let temp_dir = TempDir::new().unwrap();

        // Test with unicode characters in filename
        let video_path = temp_dir.path().join("视频文件.mp4");
        let result = clipper.output_path(&video_path, 1, None);

        assert!(result.is_ok());
        let output_path = result.unwrap();
        assert!(output_path.to_str().unwrap().contains("视频文件"));
    }

    #[test]
    fn test_output_path_with_different_clip_numbers() {
        use tempfile::TempDir;

        let clipper = Clipper::new(5.0, 5.0);
        let temp_dir = TempDir::new().unwrap();
        let video_path = temp_dir.path().join("video.mp4");

        // Test with different clip numbers
        for i in 1..=5 {
            let result = clipper.output_path(&video_path, i, None);
            assert!(result.is_ok());
            let output_path = result.unwrap();
            // Clip numbers are zero-padded to 5 digits
            assert!(output_path
                .to_str()
                .unwrap()
                .contains(&format!("clip_{:05}", i)));
        }
    }

    #[test]
    fn test_clipper_new_with_zero_buffers() {
        let clipper = Clipper::new(0.0, 0.0);
        assert_eq!(clipper.buffer_before, 0.0);
        assert_eq!(clipper.buffer_after, 0.0);
    }

    #[test]
    fn test_clipper_new_with_large_buffers() {
        let clipper = Clipper::new(120.0, 120.0);
        assert_eq!(clipper.buffer_before, 120.0);
        assert_eq!(clipper.buffer_after, 120.0);
    }
}
