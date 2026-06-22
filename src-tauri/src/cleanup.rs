use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use tokio::sync::Mutex;
use tokio::time::interval;

use crate::controller::Controller;

/// Clean up converted video files older than the specified age.
///
/// This runs as a background task to prevent accumulation of temporary
/// converted files during long application sessions. Before each sweep it reads
/// the controller's `last_play_path` so the file currently being watched is
/// never deleted out from under the player.
pub async fn start_cleanup_task(
    clips_dir: std::path::PathBuf,
    max_age_days: u64,
    controller: Arc<Mutex<Controller>>,
) {
    let mut interval = interval(Duration::from_secs(3600)); // Run every hour

    loop {
        interval.tick().await;
        let protected = {
            let ctrl = controller.lock().await;
            ctrl.last_play_path.clone()
        };
        cleanup_old_files(&clips_dir, max_age_days, protected.as_deref()).await;
    }
}

async fn cleanup_old_files(clips_dir: &Path, max_age_days: u64, protected: Option<&Path>) {
    let cutoff = SystemTime::now() - Duration::from_secs(max_age_days * 24 * 60 * 60);

    let entries = match tokio::fs::read_dir(clips_dir).await {
        Ok(entries) => entries,
        Err(e) => {
            eprintln!("[cleanup] Failed to read clips directory: {}", e);
            return;
        }
    };

    let mut entries = entries;
    while let Ok(Some(entry)) = entries.next_entry().await {
        let path = entry.path();

        // Only clean up .converted.mp4 files
        if !path.is_file() {
            continue;
        }

        let is_converted = path
            .to_str()
            .map(|s| s.ends_with(".converted.mp4"))
            .unwrap_or(false);

        if !is_converted {
            continue;
        }

        // Never delete the converted file the user is currently watching, even
        // if it's older than the cutoff — a long paused review session can keep
        // a file open for far longer than max_age_days.
        if let Some(p) = protected {
            if files_equal(&path, p) {
                continue;
            }
        }

        // Check file age
        let metadata = match tokio::fs::metadata(&path).await {
            Ok(m) => m,
            Err(e) => {
                eprintln!(
                    "[cleanup] Failed to read metadata for {}: {}",
                    path.display(),
                    e
                );
                continue;
            }
        };

        let modified = match metadata.modified() {
            Ok(m) => m,
            Err(e) => {
                eprintln!(
                    "[cleanup] Failed to get modified time for {}: {}",
                    path.display(),
                    e
                );
                continue;
            }
        };

        if modified < cutoff {
            match tokio::fs::remove_file(&path).await {
                Ok(_) => println!("[cleanup] Removed old converted file: {}", path.display()),
                Err(e) => eprintln!("[cleanup] Failed to remove {}: {}", path.display(), e),
            }
        }
    }
}

/// Compare two paths for equality, canonicalizing both first so that
/// different textual forms of the same file (e.g. relative vs absolute, or
/// symlinked paths) compare equal.
fn files_equal(a: &Path, b: &Path) -> bool {
    if a == b {
        return true;
    }
    match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
        (Ok(ca), Ok(cb)) => ca == cb,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_cleanup_removes_old_converted_files() {
        let temp_dir = TempDir::new().unwrap();
        let clips_dir = temp_dir.path().to_path_buf();

        // Create test files
        let old_file = clips_dir.join("old_video.converted.mp4");
        let new_file = clips_dir.join("new_video.converted.mp4");
        let non_converted = clips_dir.join("original.mp4");

        fs::write(&old_file, "old content").unwrap();
        fs::write(&new_file, "new content").unwrap();
        fs::write(&non_converted, "original content").unwrap();

        // Make old_file appear old by setting modified time to 10 days ago
        let old_time = SystemTime::now() - Duration::from_secs(10 * 24 * 60 * 60);
        let file = fs::File::open(&old_file).unwrap();
        file.set_modified(old_time).unwrap();

        // Run cleanup with 7-day threshold
        cleanup_old_files(&clips_dir, 7, None).await;

        // Old converted file should be removed
        assert!(!old_file.exists());
        // New converted file should remain
        assert!(new_file.exists());
        // Non-converted file should remain (even if old)
        assert!(non_converted.exists());
    }

    #[tokio::test]
    async fn test_cleanup_ignores_non_converted_files() {
        let temp_dir = TempDir::new().unwrap();
        let clips_dir = temp_dir.path().to_path_buf();

        // Create various non-.converted.mp4 files
        let files = vec![
            "video.mp4",
            "clip.mkv",
            "test.avi",
            "converted.mp4", // Missing leading dot
        ];

        for filename in &files {
            let path = clips_dir.join(filename);
            fs::write(&path, "content").unwrap();

            // Make them all old
            let old_time = SystemTime::now() - Duration::from_secs(30 * 24 * 60 * 60);
            let file = fs::File::open(&path).unwrap();
            file.set_modified(old_time).unwrap();
        }

        // Run cleanup
        cleanup_old_files(&clips_dir, 7, None).await;

        // All files should still exist
        for filename in &files {
            assert!(
                clips_dir.join(filename).exists(),
                "{} should not be deleted",
                filename
            );
        }
    }

    #[tokio::test]
    async fn test_cleanup_handles_missing_directory() {
        let nonexistent = PathBuf::from("/nonexistent/path/that/does/not/exist");

        // Should not panic, just log error
        cleanup_old_files(&nonexistent, 7, None).await;
    }

    #[tokio::test]
    async fn test_cleanup_respects_age_threshold() {
        let temp_dir = TempDir::new().unwrap();
        let clips_dir = temp_dir.path().to_path_buf();

        // Create files with different ages
        let file_5_days = clips_dir.join("5days.converted.mp4");
        let file_10_days = clips_dir.join("10days.converted.mp4");
        let file_20_days = clips_dir.join("20days.converted.mp4");

        fs::write(&file_5_days, "5 days old").unwrap();
        fs::write(&file_10_days, "10 days old").unwrap();
        fs::write(&file_20_days, "20 days old").unwrap();

        // Set modified times
        let now = SystemTime::now();
        fs::File::open(&file_5_days)
            .unwrap()
            .set_modified(now - Duration::from_secs(5 * 24 * 60 * 60))
            .unwrap();
        fs::File::open(&file_10_days)
            .unwrap()
            .set_modified(now - Duration::from_secs(10 * 24 * 60 * 60))
            .unwrap();
        fs::File::open(&file_20_days)
            .unwrap()
            .set_modified(now - Duration::from_secs(20 * 24 * 60 * 60))
            .unwrap();

        // Cleanup with 7-day threshold
        cleanup_old_files(&clips_dir, 7, None).await;

        // 5-day-old file should remain
        assert!(file_5_days.exists());
        // 10-day-old file should be removed
        assert!(!file_10_days.exists());
        // 20-day-old file should be removed
        assert!(!file_20_days.exists());
    }

    #[tokio::test]
    async fn test_cleanup_ignores_directories() {
        let temp_dir = TempDir::new().unwrap();
        let clips_dir = temp_dir.path().to_path_buf();

        // Create a directory with .converted.mp4 name
        let fake_dir = clips_dir.join("fake.converted.mp4");
        std::fs::create_dir(&fake_dir).unwrap();

        // Make it old
        let old_time = SystemTime::now() - Duration::from_secs(30 * 24 * 60 * 60);
        let file = std::fs::File::open(&fake_dir).unwrap();
        file.set_modified(old_time).unwrap();

        // Run cleanup
        cleanup_old_files(&clips_dir, 7, None).await;

        // Directory should remain (only files are cleaned)
        assert!(fake_dir.exists());
    }

    #[tokio::test]
    async fn test_cleanup_handles_empty_directory() {
        let temp_dir = TempDir::new().unwrap();
        let clips_dir = temp_dir.path().to_path_buf();

        // Run cleanup on empty directory - should not panic
        cleanup_old_files(&clips_dir, 7, None).await;
    }

    #[tokio::test]
    async fn test_cleanup_with_zero_max_age() {
        let temp_dir = TempDir::new().unwrap();
        let clips_dir = temp_dir.path().to_path_buf();

        // Create a file
        let file = clips_dir.join("test.converted.mp4");
        std::fs::write(&file, "content").unwrap();

        // Run cleanup with 0 days max age - should delete all files
        cleanup_old_files(&clips_dir, 0, None).await;

        // File should be deleted (it's older than 0 days)
        assert!(!file.exists());
    }

    #[tokio::test]
    async fn test_cleanup_with_large_max_age() {
        let temp_dir = TempDir::new().unwrap();
        let clips_dir = temp_dir.path().to_path_buf();

        // Create an old file
        let file = clips_dir.join("old.converted.mp4");
        std::fs::write(&file, "content").unwrap();

        let old_time = SystemTime::now() - Duration::from_secs(365 * 24 * 60 * 60); // 1 year old
        let f = std::fs::File::open(&file).unwrap();
        f.set_modified(old_time).unwrap();

        // Run cleanup with 2 years max age - should keep the file
        cleanup_old_files(&clips_dir, 730, None).await;

        // File should remain (it's younger than 730 days)
        assert!(file.exists());
    }

    #[tokio::test]
    async fn test_cleanup_mixed_files() {
        let temp_dir = TempDir::new().unwrap();
        let clips_dir = temp_dir.path().to_path_buf();

        // Create various files with different ages and extensions
        let old_converted = clips_dir.join("old.converted.mp4");
        let new_converted = clips_dir.join("new.converted.mp4");
        let old_regular = clips_dir.join("old.mp4");
        let new_regular = clips_dir.join("new.mp4");

        std::fs::write(&old_converted, "old converted").unwrap();
        std::fs::write(&new_converted, "new converted").unwrap();
        std::fs::write(&old_regular, "old regular").unwrap();
        std::fs::write(&new_regular, "new regular").unwrap();

        // Make some files old
        let old_time = SystemTime::now() - Duration::from_secs(30 * 24 * 60 * 60);
        let f1 = std::fs::File::open(&old_converted).unwrap();
        f1.set_modified(old_time).unwrap();
        let f2 = std::fs::File::open(&old_regular).unwrap();
        f2.set_modified(old_time).unwrap();

        // Run cleanup with 7 days max age
        cleanup_old_files(&clips_dir, 7, None).await;

        // Only old converted file should be deleted
        assert!(!old_converted.exists());
        assert!(new_converted.exists());
        assert!(old_regular.exists());
        assert!(new_regular.exists());
    }

    #[tokio::test]
    async fn test_cleanup_preserves_new_files() {
        let temp_dir = TempDir::new().unwrap();
        let clips_dir = temp_dir.path().to_path_buf();

        // Create a new file (created just now)
        let file = clips_dir.join("new.converted.mp4");
        std::fs::write(&file, "content").unwrap();

        // Run cleanup with 7 days max age
        cleanup_old_files(&clips_dir, 7, None).await;

        // File should remain (it's new)
        assert!(file.exists());
    }

    #[tokio::test]
    async fn test_cleanup_protects_currently_open_file() {
        let temp_dir = TempDir::new().unwrap();
        let clips_dir = temp_dir.path().to_path_buf();

        // A converted file that would normally be deleted because
        // max_age_days = 0 means "anything older than this instant".
        let protected_file = clips_dir.join("playing.converted.mp4");
        std::fs::write(&protected_file, "content").unwrap();

        // A second old file that should still be removed, to prove the
        // protected guard is targeted rather than disabling cleanup entirely.
        let deletable_file = clips_dir.join("stale.converted.mp4");
        std::fs::write(&deletable_file, "content").unwrap();
        // Force both files just past the cutoff: sleep so their mtimes are
        // slightly older than "now" before cleanup computes its cutoff.
        tokio::time::sleep(std::time::Duration::from_millis(30)).await;

        // Run cleanup with max_age_days = 0 and the playing file protected.
        cleanup_old_files(&clips_dir, 0, Some(&protected_file)).await;

        // The protected file survives even though it's past the cutoff.
        assert!(
            protected_file.exists(),
            "protected converted file must not be deleted"
        );
        // The unprotected old file is removed.
        assert!(
            !deletable_file.exists(),
            "non-protected old file should be removed"
        );
    }
}
