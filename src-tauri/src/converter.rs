use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc;
use tokio::time::Duration;

use crate::error::{ffmpeg_not_found_error, AppError, AppResult};

/// Web-compatible video formats (can be played directly in HTML5 video)
const WEB_FORMATS: &[&str] = &["mp4", "webm", "ogg", "ogv", "m4v"];

/// Conversion progress update
#[derive(Debug, Clone)]
pub enum ConversionStatus {
    /// Conversion started, total duration in seconds
    Started { duration: f64 },
    /// Progress update, percentage 0-100
    Progress(f64),
    /// Conversion completed successfully
    Completed { output_path: PathBuf },
    /// Conversion failed
    Failed(String),
    /// Stream copy failed, falling back to transcode (slower)
    FallbackToTranscode,
}

/// Video converter that transforms non-web formats into MP4
pub struct Converter;

impl Converter {
    /// Check if a file format is web-compatible
    pub fn is_web_compatible(path: &Path) -> bool {
        path.extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| WEB_FORMATS.contains(&ext.to_lowercase().as_str()))
            .unwrap_or(false)
    }

    /// Convert a video file to MP4 format
    ///
    /// Tries stream copy first (fast), falls back to transcode (slow) if needed.
    /// Returns the path to the converted file and emits progress updates.
    pub async fn convert_to_mp4(
        input_path: &Path,
        output_dir: &Path,
        progress_tx: mpsc::Sender<ConversionStatus>,
    ) -> AppResult<PathBuf> {
        let output_path = Self::get_output_path(input_path, output_dir)?;

        // Cache: if a valid converted file already exists and is at least as
        // new as the input, reuse it instead of re-running ffmpeg. This makes
        // re-opening a previously converted video near-instant. Only Completed
        // is emitted here (no Started), so the frontend conversion overlay is
        // never shown for a cache hit — the per-video state reset happens in
        // VideoPlayer's videoPath effect.
        if Self::is_valid_cached_output(&output_path, input_path) {
            let _ = progress_tx
                .send(ConversionStatus::Completed {
                    output_path: output_path.clone(),
                })
                .await;
            return Ok(output_path);
        }

        // Get input duration first
        let duration = Self::get_duration(input_path).await?;
        let _ = progress_tx
            .send(ConversionStatus::Started { duration })
            .await;

        // Try stream copy first (fast)
        match Self::convert_with_stream_copy(input_path, &output_path, duration, &progress_tx).await
        {
            Ok(_) => {
                let _ = progress_tx
                    .send(ConversionStatus::Completed {
                        output_path: output_path.clone(),
                    })
                    .await;
                return Ok(output_path);
            }
            Err(_) => {
                // Stream copy failed, fall back to transcode
                eprintln!("Stream copy failed, falling back to transcode");
                let _ = progress_tx
                    .send(ConversionStatus::FallbackToTranscode)
                    .await;
            }
        }

        // Transcode (slower but more compatible). On failure emit a Failed
        // status so the frontend conversion overlay clears instead of hanging.
        if let Err(e) =
            Self::convert_with_transcode(input_path, &output_path, duration, &progress_tx).await
        {
            let _ = progress_tx
                .send(ConversionStatus::Failed(e.to_string()))
                .await;
            return Err(e);
        }

        let _ = progress_tx
            .send(ConversionStatus::Completed {
                output_path: output_path.clone(),
            })
            .await;

        Ok(output_path)
    }

    /// Returns true if `output_path` is a usable cached conversion: it exists,
    /// is non-empty, and was modified at or after the input file's mtime.
    fn is_valid_cached_output(output_path: &Path, input_path: &Path) -> bool {
        let (Ok(out_meta), Ok(in_meta)) = (
            std::fs::metadata(output_path),
            std::fs::metadata(input_path),
        ) else {
            return false;
        };
        if out_meta.len() == 0 {
            return false;
        }
        match (out_meta.modified(), in_meta.modified()) {
            (Ok(out_m), Ok(in_m)) => out_m >= in_m,
            _ => false,
        }
    }

    /// Get output path for converted file
    fn get_output_path(input_path: &Path, output_dir: &Path) -> AppResult<PathBuf> {
        let stem = input_path
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or_else(|| AppError::Clip("Invalid input filename".to_string()))?;

        let output_filename = format!("{}.converted.mp4", stem);
        Ok(output_dir.join(output_filename))
    }

    /// Get video duration in seconds using ffprobe
    pub async fn get_duration(input_path: &Path) -> AppResult<f64> {
        let input_str = input_path
            .to_str()
            .ok_or_else(|| AppError::Clip("Invalid input path".to_string()))?;

        let mut cmd = Command::new(crate::util::resolve_binary("ffprobe"));
        cmd.args([
            "-v",
            "error",
            "-show_entries",
            "format=duration",
            "-of",
            "default=noprint_wrappers=1:nokey=1",
            input_str,
        ]);

        // Prevent console window from appearing on Windows
        #[cfg(windows)]
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW

        let output = cmd.output().await.map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                ffmpeg_not_found_error("getting video duration")
            } else {
                AppError::Clip(format!("Failed to run ffprobe: {}", e))
            }
        })?;

        if !output.status.success() {
            return Err(AppError::Clip("ffprobe failed".to_string()));
        }

        let duration_str = String::from_utf8_lossy(&output.stdout);
        let duration: f64 = duration_str
            .trim()
            .parse()
            .map_err(|_| AppError::Clip("Failed to parse duration".to_string()))?;

        Ok(duration)
    }

    /// Run FFmpeg with the given arguments and parse progress
    async fn run_ffmpeg(
        input_path: &Path,
        output_path: &Path,
        args: &[&str],
        duration: f64,
        progress_tx: &mpsc::Sender<ConversionStatus>,
        operation: &str,
    ) -> AppResult<()> {
        let input_str = input_path
            .to_str()
            .ok_or_else(|| AppError::Clip("Invalid input path".to_string()))?;
        let output_str = output_path
            .to_str()
            .ok_or_else(|| AppError::Clip("Invalid output path".to_string()))?;

        let mut cmd_args = vec!["-i", input_str];
        cmd_args.extend_from_slice(args);
        cmd_args.extend_from_slice(&["-movflags", "+faststart", "-y", output_str]);

        let mut cmd = Command::new(crate::util::resolve_binary("ffmpeg"));
        cmd.args(&cmd_args)
            .stdout(Stdio::null())
            .stderr(Stdio::piped());

        // Prevent console window from appearing on Windows
        #[cfg(windows)]
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW

        let mut child = cmd.spawn().map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                ffmpeg_not_found_error(operation)
            } else {
                AppError::Clip(format!("Failed to spawn ffmpeg: {}", e))
            }
        })?;

        Self::parse_progress(&mut child, duration, progress_tx).await?;

        let status = child
            .wait()
            .await
            .map_err(|e| AppError::Clip(format!("Failed to wait for ffmpeg: {}", e)))?;

        if !status.success() {
            // Clean up partial output file
            if output_path.exists() {
                let _ = std::fs::remove_file(output_path);
            }
            return Err(AppError::Clip(format!("ffmpeg {} failed", operation)));
        }

        Ok(())
    }

    /// Convert using stream copy (fast, no re-encoding)
    async fn convert_with_stream_copy(
        input_path: &Path,
        output_path: &Path,
        duration: f64,
        progress_tx: &mpsc::Sender<ConversionStatus>,
    ) -> AppResult<()> {
        Self::run_ffmpeg(
            input_path,
            output_path,
            &["-c", "copy"],
            duration,
            progress_tx,
            "stream copy",
        )
        .await
    }

    /// Convert using transcode (slower but more compatible)
    async fn convert_with_transcode(
        input_path: &Path,
        output_path: &Path,
        duration: f64,
        progress_tx: &mpsc::Sender<ConversionStatus>,
    ) -> AppResult<()> {
        Self::run_ffmpeg(
            input_path,
            output_path,
            &[
                "-c:v", "libx264", "-preset", "fast", "-crf", "23", "-c:a", "aac", "-b:a", "128k",
            ],
            duration,
            progress_tx,
            "transcode",
        )
        .await
    }

    /// Parse FFmpeg progress output and emit updates
    async fn parse_progress(
        child: &mut tokio::process::Child,
        duration: f64,
        progress_tx: &mpsc::Sender<ConversionStatus>,
    ) -> AppResult<()> {
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| AppError::Clip("Failed to capture stderr".to_string()))?;

        let mut reader = BufReader::new(stderr).lines();
        let mut last_progress_time = std::time::Instant::now();

        while let Some(line) = reader.next_line().await? {
            // Parse time from FFmpeg output: "frame=  123 fps= 60 q=28.0 size=    1024kB time=00:00:05.12 bitrate=1638.4kbits/s speed=1.2x"
            if let Some(time_str) = line.split("time=").nth(1) {
                if let Some(time_end) = time_str.split_whitespace().next() {
                    if let Ok(current_time) = Self::parse_ffmpeg_time(time_end) {
                        // Guard against duration == 0 (malformed containers),
                        // which would otherwise produce inf/NaN progress.
                        let progress = if duration > 0.0 {
                            (current_time / duration * 100.0).min(100.0)
                        } else {
                            0.0
                        };

                        // Throttle updates to max 10 per second
                        if last_progress_time.elapsed() > Duration::from_millis(100) {
                            let _ = progress_tx.send(ConversionStatus::Progress(progress)).await;
                            last_progress_time = std::time::Instant::now();
                        }
                    }
                }
            }
        }

        // Ensure we report 100% at the end
        let _ = progress_tx.send(ConversionStatus::Progress(100.0)).await;

        Ok(())
    }

    /// Parse FFmpeg time format (HH:MM:SS.mmm or SS.mmm)
    fn parse_ffmpeg_time(time_str: &str) -> AppResult<f64> {
        let parts: Vec<&str> = time_str.split(':').collect();

        match parts.len() {
            3 => {
                // HH:MM:SS.mmm
                let hours: f64 = parts[0]
                    .parse()
                    .map_err(|_| AppError::Clip("Invalid hours".to_string()))?;
                let minutes: f64 = parts[1]
                    .parse()
                    .map_err(|_| AppError::Clip("Invalid minutes".to_string()))?;
                let seconds: f64 = parts[2]
                    .parse()
                    .map_err(|_| AppError::Clip("Invalid seconds".to_string()))?;
                Ok(hours * 3600.0 + minutes * 60.0 + seconds)
            }
            1 => {
                // SS.mmm
                parts[0]
                    .parse()
                    .map_err(|_| AppError::Clip("Invalid seconds".to_string()))
            }
            _ => Err(AppError::Clip("Invalid time format".to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_web_compatible_mp4() {
        assert!(Converter::is_web_compatible(Path::new("video.mp4")));
        assert!(Converter::is_web_compatible(Path::new("/path/to/clip.MP4")));
        assert!(Converter::is_web_compatible(Path::new("test.Mp4")));
    }

    #[test]
    fn test_is_web_compatible_webm() {
        assert!(Converter::is_web_compatible(Path::new("video.webm")));
        assert!(Converter::is_web_compatible(Path::new("clip.WEBM")));
    }

    #[test]
    fn test_is_web_compatible_ogg() {
        assert!(Converter::is_web_compatible(Path::new("video.ogg")));
        assert!(Converter::is_web_compatible(Path::new("clip.ogv")));
    }

    #[test]
    fn test_is_web_compatible_m4v() {
        assert!(Converter::is_web_compatible(Path::new("video.m4v")));
    }

    #[test]
    fn test_is_web_compatible_non_web_formats() {
        assert!(!Converter::is_web_compatible(Path::new("video.mkv")));
        assert!(!Converter::is_web_compatible(Path::new("clip.avi")));
        assert!(!Converter::is_web_compatible(Path::new("test.mov")));
        assert!(!Converter::is_web_compatible(Path::new("file.ts")));
        assert!(!Converter::is_web_compatible(Path::new("video.wmv")));
        assert!(!Converter::is_web_compatible(Path::new("clip.flv")));
    }

    #[test]
    fn test_is_web_compatible_no_extension() {
        assert!(!Converter::is_web_compatible(Path::new("noextension")));
        assert!(!Converter::is_web_compatible(Path::new("")));
    }

    #[test]
    fn test_parse_ffmpeg_time_hhmmss() {
        let result = Converter::parse_ffmpeg_time("01:30:45.50").unwrap();
        assert!((result - (3600.0 + 1800.0 + 45.5)).abs() < 0.01);
    }

    #[test]
    fn test_parse_ffmpeg_time_hhmmss_zero() {
        let result = Converter::parse_ffmpeg_time("00:00:00.00").unwrap();
        assert!((result - 0.0).abs() < 0.01);
    }

    #[test]
    fn test_parse_ffmpeg_time_seconds_only() {
        let result = Converter::parse_ffmpeg_time("123.456").unwrap();
        assert!((result - 123.456).abs() < 0.001);
    }

    #[test]
    fn test_parse_ffmpeg_time_invalid_format() {
        assert!(Converter::parse_ffmpeg_time("01:30").is_err()); // Only HH:MM
        assert!(Converter::parse_ffmpeg_time("invalid").is_err());
    }

    #[test]
    fn test_parse_ffmpeg_time_invalid_values() {
        assert!(Converter::parse_ffmpeg_time("abc:30:45.00").is_err());
        assert!(Converter::parse_ffmpeg_time("01:abc:45.00").is_err());
        assert!(Converter::parse_ffmpeg_time("01:30:abc").is_err());
    }

    #[test]
    fn test_get_output_path_normal() {
        let input = Path::new("/videos/my_video.mkv");
        let output_dir = Path::new("/output");

        let result = Converter::get_output_path(input, output_dir).unwrap();
        assert_eq!(result, PathBuf::from("/output/my_video.converted.mp4"));
    }

    #[test]
    fn test_get_output_path_complex_name() {
        let input = Path::new("/home/user/Videos/2024-game-highlights.avi");
        let output_dir = Path::new("/tmp/clips");

        let result = Converter::get_output_path(input, output_dir).unwrap();
        assert_eq!(
            result,
            PathBuf::from("/tmp/clips/2024-game-highlights.converted.mp4")
        );
    }

    #[test]
    fn test_get_output_path_invalid_input() {
        let input = Path::new("");
        let output_dir = Path::new("/output");

        let result = Converter::get_output_path(input, output_dir);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_ffmpeg_time_edge_cases() {
        // Test with just seconds (no decimal)
        let result = Converter::parse_ffmpeg_time("123").unwrap();
        assert_eq!(result, 123.0);

        // Test with zero
        let result = Converter::parse_ffmpeg_time("0").unwrap();
        assert_eq!(result, 0.0);

        // Test with HH:MM:SS format
        let result = Converter::parse_ffmpeg_time("01:02:03.456").unwrap();
        assert_eq!(result, 3723.456);

        // Test with large hours
        let result = Converter::parse_ffmpeg_time("99:59:59.999").unwrap();
        assert!((result - 359999.999).abs() < 0.001);
    }

    #[test]
    fn test_parse_ffmpeg_time_invalid_parts() {
        // Too many colons
        assert!(Converter::parse_ffmpeg_time("01:02:03:04").is_err());

        // Empty string
        assert!(Converter::parse_ffmpeg_time("").is_err());

        // Only colons
        assert!(Converter::parse_ffmpeg_time("::").is_err());
    }

    #[test]
    fn test_parse_ffmpeg_time_negative_values() {
        // Note: The parser accepts negative values (they parse as valid f64)
        // This might not be ideal for video timestamps, but it's the current behavior
        let result = Converter::parse_ffmpeg_time("-1:02:03.456");
        assert!(result.is_ok());
        let expected = -1.0 * 3600.0 + 2.0 * 60.0 + 3.456;
        assert!((result.unwrap() - expected).abs() < 0.001);

        // Negative minutes
        let result = Converter::parse_ffmpeg_time("01:-2:03.456");
        assert!(result.is_ok());
        let expected = 1.0 * 3600.0 + (-2.0) * 60.0 + 3.456;
        assert!((result.unwrap() - expected).abs() < 0.001);

        // Negative seconds
        let result = Converter::parse_ffmpeg_time("01:02:-3.456");
        assert!(result.is_ok());
        let expected = 1.0 * 3600.0 + 2.0 * 60.0 + (-3.456);
        assert!((result.unwrap() - expected).abs() < 0.001);
    }

    #[test]
    fn test_parse_ffmpeg_time_overflow_values() {
        // Minutes > 59 (should still parse, just unusual)
        let result = Converter::parse_ffmpeg_time("01:99:03.456");
        assert!(result.is_ok());
        let expected = 3600.0 + 99.0 * 60.0 + 3.456;
        assert!((result.unwrap() - expected).abs() < 0.001);
    }

    #[test]
    fn test_is_web_compatible_case_sensitivity() {
        // Should handle case-insensitive extensions
        assert!(Converter::is_web_compatible(Path::new("video.MP4")));
        assert!(Converter::is_web_compatible(Path::new("video.WEBM")));
        assert!(Converter::is_web_compatible(Path::new("video.OGG")));
        assert!(Converter::is_web_compatible(Path::new("video.M4V")));
    }

    #[test]
    fn test_is_web_compatible_with_path() {
        // Should work with full paths
        assert!(Converter::is_web_compatible(Path::new(
            "/home/user/videos/test.mp4"
        )));
        assert!(Converter::is_web_compatible(Path::new(
            "C:\\Users\\test\\video.webm"
        )));
        assert!(!Converter::is_web_compatible(Path::new(
            "/home/user/videos/test.mkv"
        )));
    }

    #[test]
    fn test_get_output_path_with_special_characters() {
        let input = Path::new("/videos/my video (2024).mkv");
        let output_dir = Path::new("/output");

        let result = Converter::get_output_path(input, output_dir).unwrap();
        assert_eq!(
            result,
            PathBuf::from("/output/my video (2024).converted.mp4")
        );
    }

    #[test]
    fn test_get_output_path_unicode() {
        let input = Path::new("/videos/видео.mkv");
        let output_dir = Path::new("/output");

        let result = Converter::get_output_path(input, output_dir).unwrap();
        assert_eq!(result, PathBuf::from("/output/видео.converted.mp4"));
    }

    #[tokio::test]
    async fn test_get_duration_real_video() {
        use tempfile::TempDir;
        use tokio::process::Command;

        // Create a temporary directory
        let temp_dir = TempDir::new().unwrap();
        let test_video = temp_dir.path().join("test_video.mp4");

        // Create a small test video using FFmpeg (5 seconds of black video)
        let output = Command::new("ffmpeg")
            .args([
                "-y",
                "-f",
                "lavfi",
                "-i",
                "color=c=black:s=320x240:d=5",
                "-c:v",
                "libx264",
                "-t",
                "5",
                test_video.to_str().unwrap(),
            ])
            .output()
            .await
            .unwrap();

        if !output.status.success() {
            eprintln!(
                "FFmpeg output: {:?}",
                String::from_utf8_lossy(&output.stderr)
            );
            panic!("Failed to create test video");
        }

        // Test get_duration
        let duration = Converter::get_duration(&test_video).await.unwrap();

        // Duration should be approximately 5 seconds (allow some tolerance)
        assert!(
            duration >= 4.5 && duration <= 5.5,
            "Expected ~5 seconds, got {}",
            duration
        );
    }

    #[tokio::test]
    async fn test_convert_to_mp4_web_format() {
        use tempfile::TempDir;
        use tokio::process::Command;
        use tokio::sync::mpsc;

        let temp_dir = TempDir::new().unwrap();
        let input_video = temp_dir.path().join("input.mp4");
        let output_dir = temp_dir.path().join("output");
        std::fs::create_dir_all(&output_dir).unwrap();

        // Create a small test video (MP4 format, already web-compatible)
        let output = Command::new("ffmpeg")
            .args([
                "-y",
                "-f",
                "lavfi",
                "-i",
                "color=c=blue:s=320x240:d=2",
                "-c:v",
                "libx264",
                "-t",
                "2",
                input_video.to_str().unwrap(),
            ])
            .output()
            .await
            .unwrap();

        if !output.status.success() {
            panic!("Failed to create test video");
        }

        // Create channel for progress updates
        let (tx, mut rx) = mpsc::channel(100);

        // Convert the video
        let result = Converter::convert_to_mp4(&input_video, &output_dir, tx).await;

        // Should succeed
        assert!(result.is_ok(), "Conversion failed: {:?}", result.err());

        let output_path = result.unwrap();
        assert!(output_path.exists(), "Output file should exist");
        assert!(output_path.to_str().unwrap().ends_with(".converted.mp4"));

        // Check that we received progress updates
        let mut received_started = false;
        let mut received_completed = false;

        while let Ok(status) = rx.try_recv() {
            match status {
                ConversionStatus::Started { .. } => received_started = true,
                ConversionStatus::Completed { .. } => received_completed = true,
                _ => {}
            }
        }

        assert!(received_started, "Should receive Started status");
        assert!(received_completed, "Should receive Completed status");
    }

    #[tokio::test]
    async fn test_convert_to_mp4_non_web_format() {
        use tempfile::TempDir;
        use tokio::process::Command;
        use tokio::sync::mpsc;

        let temp_dir = TempDir::new().unwrap();
        let input_video = temp_dir.path().join("input.mkv");
        let output_dir = temp_dir.path().join("output");
        std::fs::create_dir_all(&output_dir).unwrap();

        // Create a small test video in MKV format (non-web format)
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
                input_video.to_str().unwrap(),
            ])
            .output()
            .await
            .unwrap();

        if !output.status.success() {
            panic!("Failed to create test video");
        }

        // Create channel for progress updates
        let (tx, mut rx) = mpsc::channel(100);

        // Convert the video
        let result = Converter::convert_to_mp4(&input_video, &output_dir, tx).await;

        // Should succeed
        assert!(result.is_ok(), "Conversion failed: {:?}", result.err());

        let output_path = result.unwrap();
        assert!(output_path.exists(), "Output file should exist");

        // Verify the output is actually MP4 format
        let output = Command::new("ffprobe")
            .args([
                "-v",
                "error",
                "-show_entries",
                "format=format_name",
                "-of",
                "default=noprint_wrappers=1:nokey=1",
                output_path.to_str().unwrap(),
            ])
            .output()
            .await
            .unwrap();

        let format = String::from_utf8_lossy(&output.stdout);
        assert!(
            format.contains("mp4"),
            "Output should be MP4 format, got: {}",
            format
        );
    }

    #[tokio::test]
    async fn test_get_duration_invalid_file() {
        let non_existent = Path::new("/nonexistent/video.mp4");
        let result = Converter::get_duration(non_existent).await;
        assert!(result.is_err(), "Should fail for non-existent file");
    }

    #[tokio::test]
    async fn test_get_duration_corrupted_file() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let corrupted_file = temp_dir.path().join("corrupted.mp4");

        // Write random bytes to create a corrupted file
        std::fs::write(&corrupted_file, b"This is not a valid video file").unwrap();

        let result = Converter::get_duration(&corrupted_file).await;
        assert!(result.is_err(), "Should fail for corrupted file");
    }

    #[tokio::test]
    async fn test_get_duration_empty_file() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let empty_file = temp_dir.path().join("empty.mp4");

        // Create an empty file
        std::fs::write(&empty_file, b"").unwrap();

        let result = Converter::get_duration(&empty_file).await;
        assert!(result.is_err(), "Should fail for empty file");
    }

    #[tokio::test]
    async fn test_convert_to_mp4_with_transcode_fallback() {
        use tempfile::TempDir;
        use tokio::process::Command;
        use tokio::sync::mpsc;

        let temp_dir = TempDir::new().unwrap();
        let input_video = temp_dir.path().join("input_vp9.webm");
        let output_dir = temp_dir.path().join("output");
        std::fs::create_dir_all(&output_dir).unwrap();

        // Create a video with VP9 codec and Opus audio in WebM container
        // This combination should force transcode when converting to MP4
        let output = Command::new("ffmpeg")
            .args([
                "-y",
                "-f",
                "lavfi",
                "-i",
                "color=c=green:s=320x240:d=2",
                "-f",
                "lavfi",
                "-i",
                "sine=frequency=440:duration=2",
                "-c:v",
                "libvpx-vp9",
                "-c:a",
                "libopus",
                "-t",
                "2",
                input_video.to_str().unwrap(),
            ])
            .output()
            .await
            .unwrap();

        if !output.status.success() {
            eprintln!(
                "FFmpeg output: {:?}",
                String::from_utf8_lossy(&output.stderr)
            );
            panic!("Failed to create test video with VP9 codec");
        }

        // Create channel for progress updates
        let (tx, mut rx) = mpsc::channel(100);

        // Convert the video - should trigger fallback to transcode
        let result = Converter::convert_to_mp4(&input_video, &output_dir, tx).await;

        // Should succeed (either via stream copy or transcode)
        assert!(
            result.is_ok(),
            "Conversion should succeed: {:?}",
            result.err()
        );

        let output_path = result.unwrap();
        assert!(output_path.exists(), "Output file should exist");

        // Drain the channel and check what statuses we received
        let mut statuses = vec![];
        while let Ok(status) = rx.try_recv() {
            statuses.push(format!("{:?}", status));
        }

        eprintln!("Received statuses: {:?}", statuses);

        // The conversion should have completed (either way)
        assert!(
            statuses.iter().any(|s| s.contains("Completed")),
            "Should receive Completed status"
        );
    }

    #[tokio::test]
    async fn test_convert_to_mp4_nonexistent_input() {
        use tempfile::TempDir;
        use tokio::sync::mpsc;

        let temp_dir = TempDir::new().unwrap();
        let nonexistent_video = temp_dir.path().join("nonexistent.mp4");
        let output_dir = temp_dir.path().join("output");
        std::fs::create_dir_all(&output_dir).unwrap();

        let (tx, _rx) = mpsc::channel(100);

        let result = Converter::convert_to_mp4(&nonexistent_video, &output_dir, tx).await;

        assert!(result.is_err(), "Should fail for non-existent input file");
    }

    #[tokio::test]
    async fn test_convert_to_mp4_with_progress_updates() {
        use tempfile::TempDir;
        use tokio::process::Command;
        use tokio::sync::mpsc;

        let temp_dir = TempDir::new().unwrap();
        let input_video = temp_dir.path().join("long_video.mp4");
        let output_dir = temp_dir.path().join("output");
        std::fs::create_dir_all(&output_dir).unwrap();

        // Create a longer video (10 seconds) to ensure we get progress updates
        let output = Command::new("ffmpeg")
            .args([
                "-y",
                "-f",
                "lavfi",
                "-i",
                "color=c=blue:s=640x480:d=10",
                "-c:v",
                "libx264",
                "-preset",
                "ultrafast",
                "-t",
                "10",
                input_video.to_str().unwrap(),
            ])
            .output()
            .await
            .unwrap();

        if !output.status.success() {
            panic!("Failed to create test video");
        }

        let (tx, mut rx) = mpsc::channel(100);

        // Convert the video
        let result = Converter::convert_to_mp4(&input_video, &output_dir, tx).await;

        assert!(
            result.is_ok(),
            "Conversion should succeed: {:?}",
            result.err()
        );

        // Collect all progress updates
        let mut progress_updates = vec![];
        while let Ok(status) = rx.try_recv() {
            if let ConversionStatus::Progress(percent) = status {
                progress_updates.push(percent);
            }
        }

        // We should have received at least one progress update
        assert!(
            !progress_updates.is_empty(),
            "Should receive progress updates"
        );

        // The last update should be 100%
        assert!(
            progress_updates
                .last()
                .map(|&p| (p - 100.0).abs() < 0.1)
                .unwrap_or(false),
            "Last progress should be 100%"
        );
    }

    #[tokio::test]
    async fn test_convert_to_mp4_with_corrupted_video() {
        use std::fs;
        use tempfile::tempdir;

        let temp_dir = tempdir().unwrap();
        let input_path = temp_dir.path().join("corrupted.mkv");
        let output_dir = temp_dir.path().to_path_buf();

        // Write invalid data that FFmpeg can't parse
        fs::write(&input_path, b"This is not a valid video file").unwrap();

        let (tx, _rx) = mpsc::channel(100);
        let result = Converter::convert_to_mp4(&input_path, &output_dir, tx).await;

        // Should fail during conversion (either at get_duration or convert step)
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_convert_with_invalid_output_directory() {
        use std::fs;
        use tempfile::tempdir;
        use tokio::process::Command;

        let temp_dir = tempdir().unwrap();
        let input_path = temp_dir.path().join("input.mp4");
        let output_dir = temp_dir.path().join("nonexistent").join("nested");

        // Create a valid input video
        let output = Command::new("ffmpeg")
            .args(&[
                "-y",
                "-f",
                "lavfi",
                "-i",
                "testsrc=duration=1:size=320x240:rate=1",
                "-c:v",
                "libx264",
                input_path.to_str().unwrap(),
            ])
            .output()
            .await
            .unwrap();

        if !output.status.success() {
            panic!(
                "Failed to create test video: {:?}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        let (tx, _rx) = mpsc::channel(100);
        let result = Converter::convert_to_mp4(&input_path, &output_dir, tx).await;

        // Should fail because output directory doesn't exist and can't be created
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_convert_to_mp4_with_existing_output_directory() {
        use tempfile::TempDir;
        use tokio::process::Command;

        let temp_dir = TempDir::new().unwrap();
        let input_path = temp_dir.path().join("input.mp4");
        let output_dir = temp_dir.path().join("output_subdir");

        // Create the output directory first
        std::fs::create_dir_all(&output_dir).unwrap();

        // Create a valid input video
        let output = Command::new("ffmpeg")
            .args([
                "-y",
                "-f",
                "lavfi",
                "-i",
                "color=c=green:s=320x240:d=2",
                "-c:v",
                "libx264",
                input_path.to_str().unwrap(),
            ])
            .output()
            .await
            .unwrap();

        if !output.status.success() {
            panic!("Failed to create test video");
        }

        let (tx, _rx) = mpsc::channel(100);

        // Should succeed
        let result = Converter::convert_to_mp4(&input_path, &output_dir, tx).await;
        assert!(
            result.is_ok(),
            "Conversion should succeed: {:?}",
            result.err()
        );

        // Verify the output file exists in the output directory
        let output_path = result.unwrap();
        assert!(output_path.exists(), "Output file should exist");
        assert!(
            output_path.starts_with(&output_dir),
            "Output should be in the specified directory"
        );
    }

    #[tokio::test]
    async fn test_get_duration_very_short_video() {
        use tempfile::TempDir;
        use tokio::process::Command;

        let temp_dir = TempDir::new().unwrap();
        let video_path = temp_dir.path().join("short_video.mp4");

        // Create a very short video (0.1 seconds)
        let output = Command::new("ffmpeg")
            .args([
                "-y",
                "-f",
                "lavfi",
                "-i",
                "color=c=black:s=320x240:d=0.1",
                "-c:v",
                "libx264",
                "-t",
                "0.1",
                video_path.to_str().unwrap(),
            ])
            .output()
            .await
            .unwrap();

        if !output.status.success() {
            panic!("Failed to create test video");
        }

        let duration = Converter::get_duration(&video_path).await.unwrap();

        // Duration should be very small but >= 0
        assert!(duration >= 0.0);
        assert!(duration < 1.0);
    }

    #[tokio::test]
    async fn test_get_duration_longer_video() {
        use tempfile::TempDir;
        use tokio::process::Command;

        let temp_dir = TempDir::new().unwrap();
        let video_path = temp_dir.path().join("longer_video.mp4");

        // Create a longer video (30 seconds)
        let output = Command::new("ffmpeg")
            .args([
                "-y",
                "-f",
                "lavfi",
                "-i",
                "color=c=blue:s=320x240:d=30",
                "-c:v",
                "libx264",
                "-t",
                "30",
                video_path.to_str().unwrap(),
            ])
            .output()
            .await
            .unwrap();

        if !output.status.success() {
            panic!("Failed to create test video");
        }

        let duration = Converter::get_duration(&video_path).await.unwrap();

        // Duration should be approximately 30 seconds
        assert!(duration >= 29.0);
        assert!(duration <= 31.0);
    }

    #[tokio::test]
    async fn test_convert_to_mp4_with_audio() {
        use tempfile::TempDir;
        use tokio::process::Command;

        let temp_dir = TempDir::new().unwrap();
        let input_path = temp_dir.path().join("input_with_audio.mp4");
        let output_dir = temp_dir.path().join("output");
        std::fs::create_dir_all(&output_dir).unwrap();

        // Create a video with audio
        let output = Command::new("ffmpeg")
            .args([
                "-y",
                "-f",
                "lavfi",
                "-i",
                "color=c=red:s=320x240:d=2",
                "-f",
                "lavfi",
                "-i",
                "sine=frequency=440:duration=2",
                "-c:v",
                "libx264",
                "-c:a",
                "aac",
                input_path.to_str().unwrap(),
            ])
            .output()
            .await
            .unwrap();

        if !output.status.success() {
            panic!("Failed to create test video with audio");
        }

        let (tx, _rx) = mpsc::channel(100);

        // Should succeed
        let result = Converter::convert_to_mp4(&input_path, &output_dir, tx).await;
        assert!(
            result.is_ok(),
            "Conversion should succeed: {:?}",
            result.err()
        );

        let output_path = result.unwrap();
        assert!(output_path.exists(), "Output file should exist");
    }

    #[tokio::test]
    async fn test_convert_to_mp4_webm_format() {
        use tempfile::TempDir;
        use tokio::process::Command;

        let temp_dir = TempDir::new().unwrap();
        let input_path = temp_dir.path().join("input.webm");
        let output_dir = temp_dir.path().join("output");
        std::fs::create_dir_all(&output_dir).unwrap();

        // Create a WebM video
        let output = Command::new("ffmpeg")
            .args([
                "-y",
                "-f",
                "lavfi",
                "-i",
                "color=c=green:s=320x240:d=2",
                "-c:v",
                "libvpx-vp9",
                input_path.to_str().unwrap(),
            ])
            .output()
            .await
            .unwrap();

        if !output.status.success() {
            panic!("Failed to create WebM test video");
        }

        let (tx, _rx) = mpsc::channel(100);

        // Should succeed (WebM is web-compatible but may need conversion)
        let result = Converter::convert_to_mp4(&input_path, &output_dir, tx).await;
        assert!(
            result.is_ok(),
            "Conversion should succeed: {:?}",
            result.err()
        );
    }

    #[tokio::test]
    async fn test_convert_to_mp4_ogg_format() {
        use tempfile::TempDir;
        use tokio::process::Command;

        let temp_dir = TempDir::new().unwrap();
        let input_path = temp_dir.path().join("input.ogg");
        let output_dir = temp_dir.path().join("output");
        std::fs::create_dir_all(&output_dir).unwrap();

        // Create an OGG video
        let output = Command::new("ffmpeg")
            .args([
                "-y",
                "-f",
                "lavfi",
                "-i",
                "color=c=blue:s=320x240:d=2",
                "-c:v",
                "libtheora",
                "-q:v",
                "5",
                input_path.to_str().unwrap(),
            ])
            .output()
            .await
            .unwrap();

        if !output.status.success() {
            // OGG/Theora might not be available, skip this test
            eprintln!("Skipping OGG test - codec not available");
            return;
        }

        let (tx, _rx) = mpsc::channel(100);

        // Should succeed
        let result = Converter::convert_to_mp4(&input_path, &output_dir, tx).await;
        assert!(
            result.is_ok(),
            "Conversion should succeed: {:?}",
            result.err()
        );
    }
}
