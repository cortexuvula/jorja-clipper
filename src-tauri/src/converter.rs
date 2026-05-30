use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc;
use tokio::time::Duration;

use crate::error::{AppError, AppResult};

/// Web-compatible video formats (can be played directly in HTML5 video)
const WEB_FORMATS: &[&str] = &["mp4", "webm", "ogg", "ogv", "m4v"];

/// Generate a helpful "FFmpeg not found" error message
fn ffmpeg_not_found_error(operation: &str) -> AppError {
    AppError::Clip(format!(
        "FFmpeg not found while {}. Please install FFmpeg:\n\
        • macOS: brew install ffmpeg\n\
        • Windows: Download from https://ffmpeg.org/download.html\n\
        • Linux: sudo apt install ffmpeg",
        operation
    ))
}

/// Conversion progress update
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum ConversionStatus {
    /// Conversion started, total duration in seconds
    Started { duration: f64 },
    /// Progress update, percentage 0-100
    Progress(f64),
    /// Conversion completed successfully
    Completed { output_path: PathBuf },
    /// Conversion failed
    Failed(String),
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
            }
        }

        // Transcode (slower but more compatible)
        Self::convert_with_transcode(input_path, &output_path, duration, &progress_tx).await?;

        let _ = progress_tx
            .send(ConversionStatus::Completed {
                output_path: output_path.clone(),
            })
            .await;

        Ok(output_path)
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

        let output = cmd
            .output()
            .await
            .map_err(|e| {
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

        let mut child = cmd
            .spawn()
            .map_err(|e| {
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
                "-c:v", "libx264",
                "-preset", "fast",
                "-crf", "23",
                "-c:a", "aac",
                "-b:a", "128k",
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
                        let progress = (current_time / duration * 100.0).min(100.0);

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
