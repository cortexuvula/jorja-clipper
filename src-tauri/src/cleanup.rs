use std::path::Path;
use std::time::{Duration, SystemTime};
use tokio::time::interval;

/// Clean up converted video files older than the specified age.
///
/// This runs as a background task to prevent accumulation of temporary
/// converted files during long application sessions.
pub async fn start_cleanup_task(clips_dir: std::path::PathBuf, max_age_days: u64) {
    let mut interval = interval(Duration::from_secs(3600)); // Run every hour

    loop {
        interval.tick().await;
        cleanup_old_files(&clips_dir, max_age_days).await;
    }
}

async fn cleanup_old_files(clips_dir: &Path, max_age_days: u64) {
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

        let is_converted = path.to_str()
            .map(|s| s.ends_with(".converted.mp4"))
            .unwrap_or(false);

        if !is_converted {
            continue;
        }

        // Check file age
        let metadata = match tokio::fs::metadata(&path).await {
            Ok(m) => m,
            Err(e) => {
                eprintln!("[cleanup] Failed to read metadata for {}: {}", path.display(), e);
                continue;
            }
        };

        let modified = match metadata.modified() {
            Ok(m) => m,
            Err(e) => {
                eprintln!("[cleanup] Failed to get modified time for {}: {}", path.display(), e);
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
