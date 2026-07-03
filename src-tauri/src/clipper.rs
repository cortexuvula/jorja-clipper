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
        let clips_dir = Self::clips_dir_for(video_path, output_dir)?;
        std::fs::create_dir_all(&clips_dir)?;

        let clip_filename = format!("{}_{:05}.mp4", Self::clip_prefix(video_path)?, clip_number);
        Ok(clips_dir.join(clip_filename))
    }

    /// Build the per-video prefix used for clip filenames.
    ///
    /// Format: `{stem}_[{hash}]` where `{hash}` is 4 hex chars derived from the
    /// video's absolute path. Including the hash disambiguates two videos that
    /// share a filename stem (e.g. two `game.mp4` files in different folders)
    /// when the user has configured a shared `output_dir`. Without it, both
    /// videos would write `{stem}_clip_NNNNN.mp4` into the same directory and
    /// silently overwrite each other's clips. The hash is stable for a given
    /// path, so reopening a video continues its numbering.
    fn clip_prefix(video_path: &Path) -> AppResult<String> {
        let stem = video_path
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or_else(|| AppError::Ffmpeg("Video has no filename".to_string()))?;

        // Hash the absolute path so two same-named files in different folders
        // produce different prefixes. std::DefaultHasher is non-cryptographic
        // but sufficient for disambiguation.
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let abs = std::fs::canonicalize(video_path).unwrap_or_else(|_| video_path.to_path_buf());
        let mut hasher = DefaultHasher::new();
        abs.hash(&mut hasher);
        let hash = hasher.finish();
        Ok(format!("{}_clip_{:08x}", stem, hash))
    }

    /// Resolve which output directory would be used for clips of `video_path`.
    fn clips_dir_for(video_path: &Path, output_dir: Option<&Path>) -> AppResult<PathBuf> {
        Ok(match output_dir {
            Some(dir) => dir.to_path_buf(),
            None => video_path
                .parent()
                .ok_or_else(|| AppError::Ffmpeg("Video has no parent directory".to_string()))?
                .join("clips"),
        })
    }

    /// Determine the next clip number for `video_path` by scanning the output
    /// directory for existing `{prefix}_NNNNN.mp4` files for *this* video.
    ///
    /// This is the authoritative source for clip numbering because it reflects
    /// what is actually on disk. Deriving the number from the DB row count or
    /// `clip_count` is unsafe: those values drop when clips are deleted (by the
    /// user or by `get_clips` pruning missing files), which would cause the
    /// next clip to reuse a number and silently overwrite an existing file.
    /// Scanning the disk guarantees the next number is strictly greater than
    /// any existing clip file for this video.
    ///
    /// Matching is keyed on the per-video `clip_prefix` (which includes a hash
    /// of the video path), so clips from other videos in the same shared
    /// `output_dir` are correctly ignored.
    pub fn next_clip_number(&self, video_path: &Path, output_dir: Option<&Path>) -> AppResult<i32> {
        let clips_dir = Self::clips_dir_for(video_path, output_dir)?;
        let prefix = format!("{}_", Self::clip_prefix(video_path)?);
        let mut max_number: i32 = 0;

        if let Ok(entries) = std::fs::read_dir(&clips_dir) {
            for entry in entries.flatten() {
                let os_name = entry.file_name();
                let Some(file_name) = os_name.to_str() else {
                    continue;
                };
                if !file_name.starts_with(&prefix) || !file_name.ends_with(".mp4") {
                    continue;
                }
                // middle = "{NNNNN}"
                let middle = &file_name[prefix.len()..file_name.len() - ".mp4".len()];
                if let Ok(n) = middle.parse::<i32>() {
                    if n > max_number {
                        max_number = n;
                    }
                }
            }
        }

        Ok(max_number + 1)
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

        // Run FFmpeg with stream copy (lossless). Input-seeking (-ss/-to before
        // -i) with -c copy is fast but can land mid-GOP, occasionally yielding
        // a tiny or empty clip. We validate the output and retry via transcode
        // when that happens.
        let copy_run = Self::run_clip_ffmpeg(
            video_path_str,
            output_path_str,
            output_path,
            &[
                "-y",
                "-ss",
                &format!("{:.3}", start_time),
                "-to",
                &format!("{:.3}", end_time),
            ],
            &["-c", "copy", "-avoid_negative_ts", "1"],
            "saving clip (stream copy)",
        )
        .await;

        let needs_transcode_retry = match copy_run {
            Ok(()) => !Self::clip_has_valid_duration(output_path, start_time, end_time).await,
            Err(_) => true,
        };

        if needs_transcode_retry {
            // Remove any partial/tiny output from the stream-copy attempt.
            if output_path.exists() {
                let _ = std::fs::remove_file(output_path);
            }

            // Fallback: output-seeking transcode. -ss/-to go AFTER -i so ffmpeg
            // decodes up to the cut point and produces an accurate, keyframe-
            // independent clip. Slower, but reliable.
            Self::run_clip_ffmpeg(
                video_path_str,
                output_path_str,
                output_path,
                &["-y"],
                &[
                    "-ss",
                    &format!("{:.3}", start_time),
                    "-to",
                    &format!("{:.3}", end_time),
                    "-c:v",
                    "libx264",
                    "-preset",
                    "fast",
                    "-crf",
                    "23",
                    "-c:a",
                    "aac",
                    "-b:a",
                    "128k",
                ],
                "saving clip (transcode fallback)",
            )
            .await?;
        }

        Ok(ClipResult {
            success: true,
            path: output_path.to_string_lossy().to_string(),
            start_time,
            end_time,
            error: None,
        })
    }

    /// Run a single ffmpeg invocation to extract a clip.
    ///
    /// `input_args` are placed before `-i <video>`, `output_args` after it,
    /// and the output path is appended last. On failure the partial output
    /// file is removed and an error describing ffmpeg's stderr is returned.
    async fn run_clip_ffmpeg(
        video_path_str: &str,
        output_path_str: &str,
        output_path: &Path,
        input_args: &[&str],
        output_args: &[&str],
        operation: &str,
    ) -> AppResult<()> {
        let mut cmd = Command::new(crate::util::resolve_binary("ffmpeg"));
        cmd.args(input_args)
            .arg("-i")
            .arg(video_path_str)
            .args(output_args)
            .arg(output_path_str)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        // Prevent console window from appearing on Windows
        #[cfg(windows)]
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW

        let output = cmd.output().await.map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                ffmpeg_not_found_error(operation)
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

        Ok(())
    }

    /// Probe the produced clip with ffprobe to decide whether the stream-copy
    /// output is usable.
    ///
    /// The earlier heuristic checked only file size in bytes, which flagged
    /// legitimately short / low-bitrate clips as "degenerate" and needlessly
    /// re-encoded them (quality loss + slower). Probing the actual duration is
    /// accurate: a clip is considered invalid only if it cannot be parsed by
    /// ffprobe or its measured duration is far below what was requested
    /// (less than half, with a small absolute floor), which is the real
    /// signature of a mid-GOP stream-copy failure.
    async fn clip_has_valid_duration(output_path: &Path, start_time: f64, end_time: f64) -> bool {
        let Some(out_str) = output_path.to_str() else {
            return false;
        };
        if !output_path.exists() {
            return false;
        }

        let mut cmd = Command::new(crate::util::resolve_binary("ffprobe"));
        cmd.args([
            "-v",
            "error",
            "-show_entries",
            "format=duration",
            "-of",
            "default=noprint_wrappers=1:nokey=1",
            out_str,
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

        // Prevent console window from appearing on Windows
        #[cfg(windows)]
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW

        let output = match cmd.output().await {
            Ok(o) => o,
            Err(_) => return false,
        };
        if !output.status.success() {
            return false;
        }
        let parsed = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let Ok(measured) = parsed.parse::<f64>() else {
            return false;
        };

        let requested = (end_time - start_time).max(0.0);
        // Allow a generous tolerance (half the requested duration, floored at
        // 0.1s) because stream-copy snaps to keyframes and may be slightly
        // shorter than requested. Anything drastically shorter is suspect.
        let floor = (requested * 0.5).max(0.1);
        measured >= floor
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
        // New format includes a per-video hash: game_clip_<hash>_00001.mp4
        assert!(
            output.to_str().unwrap().contains("game_clip_"),
            "filename should start with stem prefix: {}",
            output.display()
        );
        assert!(output.to_str().unwrap().ends_with("_00001.mp4"));

        // Clean up
        let _ = std::fs::remove_dir_all(&tmp_dir);
    }

    #[test]
    fn test_next_clip_number_empty_dir() {
        let clipper = Clipper::new(5.0, 5.0);
        let tmp_dir = tempfile::TempDir::new().unwrap();
        let video_path = tmp_dir.path().join("game.mp4");

        // No existing clips -> first number is 1.
        assert_eq!(clipper.next_clip_number(&video_path, None).unwrap(), 1);
    }

    #[test]
    fn test_next_clip_number_picks_max_plus_one() {
        let clipper = Clipper::new(5.0, 5.0);
        let tmp_dir = tempfile::TempDir::new().unwrap();
        let video_path = tmp_dir.path().join("game.mp4");

        // Pretend clips 1, 2, and 5 already exist (simulating prior deletions
        // of 3 and 4 — the next number must be 6, never reusing a gap).
        // Generate the files via output_path so they carry the real per-video
        // prefix (including the path hash) that next_clip_number matches on.
        for n in [1, 2, 5] {
            let p = clipper.output_path(&video_path, n, None).unwrap();
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(&p, "x").unwrap();
        }

        assert_eq!(clipper.next_clip_number(&video_path, None).unwrap(), 6);
    }

    #[test]
    fn test_next_clip_number_ignores_other_videos_and_non_clips() {
        let clipper = Clipper::new(5.0, 5.0);
        let tmp_dir = tempfile::TempDir::new().unwrap();
        let clips_dir = tmp_dir.path().join("clips");
        std::fs::create_dir_all(&clips_dir).unwrap();
        let video_path = tmp_dir.path().join("game.mp4");

        // A clip from a different video and an unrelated file must not count.
        std::fs::write(clips_dir.join("other_clip_00009.mp4"), "x").unwrap();
        std::fs::write(clips_dir.join("game_converted.mp4"), "x").unwrap();

        assert_eq!(clipper.next_clip_number(&video_path, None).unwrap(), 1);
    }

    #[test]
    fn test_clip_numbering_disambiguates_same_stem_in_shared_output_dir() {
        // N1 regression: two videos that share a filename stem (e.g. two
        // `game.mp4` files in different folders) must get distinct clip
        // filenames when the user configures a shared output_dir. Previously
        // both would write `game_clip_NNNNN.mp4` and overwrite each other.
        let clipper = Clipper::new(5.0, 5.0);
        let shared_dir = tempfile::TempDir::new().unwrap();
        let out = shared_dir.path().to_path_buf();

        // Simulate two source videos with the same stem but different parents.
        let video_a = tempfile::TempDir::new().unwrap().path().join("game.mp4");
        let video_b = tempfile::TempDir::new().unwrap().path().join("game.mp4");

        // Each video's clip path must be distinct, and must not equal the
        // other video's path for the same clip number.
        let a1 = clipper.output_path(&video_a, 1, Some(&out)).unwrap();
        let b1 = clipper.output_path(&video_b, 1, Some(&out)).unwrap();
        assert_ne!(
            a1, b1,
            "same-stem videos in a shared output dir must produce distinct clip filenames"
        );

        // Persist one clip for video A, then confirm video B's next number is
        // still 1 (it does NOT inherit A's count) and video A's is 2.
        std::fs::write(&a1, "x").unwrap();
        assert_eq!(clipper.next_clip_number(&video_a, Some(&out)).unwrap(), 2);
        assert_eq!(clipper.next_clip_number(&video_b, Some(&out)).unwrap(), 1);
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
        assert!(
            output.to_str().unwrap().contains("video_clip_"),
            "filename should contain stem prefix: {}",
            output.display()
        );
        assert!(output.to_str().unwrap().ends_with("_00042.mp4"));

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
    async fn test_clip_has_valid_duration_accepts_real_short_clip() {
        // N2 regression: a legitimately short (but valid) clip must be treated
        // as valid, not flagged for transcode re-encode purely on size.
        use tempfile::TempDir;
        use tokio::process::Command;

        let temp_dir = TempDir::new().unwrap();
        let clip_path = temp_dir.path().join("short.mp4");

        // Render a genuine 0.5s clip — small in bytes but valid.
        let out = Command::new("ffmpeg")
            .args([
                "-y",
                "-f",
                "lavfi",
                "-i",
                "color=c=blue:s=160x120:d=0.5",
                "-c:v",
                "libx264",
                "-t",
                "0.5",
                clip_path.to_str().unwrap(),
            ])
            .output()
            .await
            .unwrap();
        assert!(out.status.success(), "ffmpeg should produce a test clip");

        // Requested 0.5s; the file is valid, so the probe must say so.
        assert!(
            Clipper::clip_has_valid_duration(&clip_path, 0.0, 0.5).await,
            "a valid short clip must not be flagged as degenerate"
        );
    }

    #[tokio::test]
    async fn test_clip_has_valid_duration_rejects_corrupt_and_empty() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();

        // Empty file: ffprobe will fail to parse.
        let empty = temp_dir.path().join("empty.mp4");
        std::fs::write(&empty, b"").unwrap();
        assert!(
            !Clipper::clip_has_valid_duration(&empty, 0.0, 2.0).await,
            "empty file should be flagged as invalid"
        );

        // Garbage bytes: ffprobe will fail to parse.
        let corrupt = temp_dir.path().join("corrupt.mp4");
        std::fs::write(&corrupt, b"not a video at all").unwrap();
        assert!(
            !Clipper::clip_has_valid_duration(&corrupt, 0.0, 2.0).await,
            "corrupt file should be flagged as invalid"
        );
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
            // Clip numbers are zero-padded to 5 digits and appear after the
            // per-video prefix (stem_clip_<hash>_NNNNN.mp4).
            assert!(
                output_path
                    .to_str()
                    .unwrap()
                    .ends_with(&format!("_{:05}.mp4", i)),
                "expected clip number suffix _{:05}.mp4 in {}",
                i,
                output_path.display()
            );
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
