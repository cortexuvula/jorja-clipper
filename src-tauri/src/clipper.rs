use crate::error::{AppError, AppResult};
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

    pub fn output_path(&self, video_path: &Path, clip_number: i32) -> AppResult<PathBuf> {
        let video_dir = video_path
            .parent()
            .ok_or_else(|| AppError::Ffmpeg("Video has no parent directory".to_string()))?;

        let clips_dir = video_dir.join("clips");
        std::fs::create_dir_all(&clips_dir)?;

        let video_stem = video_path
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or_else(|| AppError::Ffmpeg("Video has no filename".to_string()))?;

        let clip_filename = format!("{}_clip_{:04}.mp4", video_stem, clip_number);

        Ok(clips_dir.join(clip_filename))
    }

    pub async fn save_clip(
        &self,
        video_path: &Path,
        start_time: f64,
        end_time: f64,
        output_path: &Path,
    ) -> AppResult<ClipResult> {
        // Verify FFmpeg is available
        let ffmpeg_check = Command::new(crate::util::resolve_binary("ffmpeg"))
            .arg("-version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await;

        if ffmpeg_check.is_err() {
            return Err(AppError::Ffmpeg(
                "FFmpeg not found. Please install FFmpeg.".to_string(),
            ));
        }

        // Run FFmpeg with stream copy (lossless)
        let output = Command::new(crate::util::resolve_binary("ffmpeg"))
            .args([
                "-y", // Overwrite output
                "-ss",
                &format!("{:.3}", start_time),
                "-to",
                &format!("{:.3}", end_time),
                "-i",
                video_path.to_str().unwrap(),
                "-c",
                "copy", // Stream copy (no re-encoding)
                "-avoid_negative_ts",
                "1",
                output_path.to_str().unwrap(),
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await?;

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
    fn test_output_path() {
        let clipper = Clipper::new(5.0, 5.0);
        let tmp_dir = std::env::temp_dir().join("jorja_clipper_test_videos");
        let _ = std::fs::create_dir_all(&tmp_dir);
        let video_path = tmp_dir.join("game.mp4");

        let output = clipper.output_path(&video_path, 1).unwrap();

        assert!(output.to_str().unwrap().contains("clips/"));
        assert!(output.to_str().unwrap().contains("game_clip_0001.mp4"));

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
}
