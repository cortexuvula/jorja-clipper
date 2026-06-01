use chrono::{DateTime, NaiveDateTime, Utc};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Mutex;

use crate::error::{AppError, AppResult};

/// Parse a datetime string that may be RFC 3339 (with timezone) or naive (without timezone).
/// Naive datetimes are treated as UTC.
fn parse_datetime(s: &str) -> Result<DateTime<Utc>, String> {
    // Try RFC 3339 first (e.g. "2026-05-24T07:46:33.067665Z" or "+00:00")
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Ok(dt.with_timezone(&Utc));
    }
    // Fall back to naive datetime (e.g. "2026-05-24T07:46:33.067665") — assume UTC
    NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S%.f")
        .map(|ndt| ndt.and_utc())
        .map_err(|e| format!("Failed to parse datetime '{}': {}", s, e))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Clip {
    pub id: i64,
    pub video_path: String,
    pub clip_path: String,
    pub start_time: f64,
    pub end_time: f64,
    pub created_at: DateTime<Utc>,
}

pub struct ClipStore {
    conn: Mutex<Connection>,
}

impl ClipStore {
    pub fn new() -> AppResult<Self> {
        let db_path = Self::db_path();
        Self::with_path(&db_path)
    }

    /// Create a ClipStore with a custom database path (useful for testing)
    pub fn with_path(db_path: &std::path::Path) -> AppResult<Self> {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let conn = Connection::open(db_path)?;
        let store = Self {
            conn: Mutex::new(conn),
        };
        store.init_schema_internal()?;

        Ok(store)
    }

    #[cfg(test)]
    pub fn init_schema(&self) -> AppResult<()> {
        self.init_schema_internal()
    }

    fn init_schema_internal(&self) -> AppResult<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| AppError::Storage(e.to_string()))?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS clips (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                video_path TEXT NOT NULL,
                clip_path TEXT NOT NULL,
                start_time REAL NOT NULL,
                end_time REAL NOT NULL,
                created_at TEXT NOT NULL
            )",
            [],
        )?;

        Ok(())
    }

    pub fn add_clip(
        &self,
        video_path: &str,
        clip_path: &str,
        start_time: f64,
        end_time: f64,
    ) -> AppResult<Clip> {
        let created_at = Utc::now();
        let conn = self
            .conn
            .lock()
            .map_err(|e| AppError::Storage(e.to_string()))?;

        conn.execute(
            "INSERT INTO clips (video_path, clip_path, start_time, end_time, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                video_path,
                clip_path,
                start_time,
                end_time,
                created_at.to_rfc3339()
            ],
        )?;

        let id = conn.last_insert_rowid();

        Ok(Clip {
            id,
            video_path: video_path.to_string(),
            clip_path: clip_path.to_string(),
            start_time,
            end_time,
            created_at,
        })
    }

    pub fn get_clips_for_video(&self, video_path: &str) -> AppResult<Vec<Clip>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| AppError::Storage(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, video_path, clip_path, start_time, end_time, created_at
             FROM clips
             WHERE video_path = ?1
             ORDER BY created_at DESC",
        )?;

        let clips = stmt.query_map(params![video_path], |row| {
            let created_str: String = row.get(5)?;
            let created_at = parse_datetime(&created_str).map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    5,
                    rusqlite::types::Type::Text,
                    Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, e)),
                )
            })?;
            Ok(Clip {
                id: row.get(0)?,
                video_path: row.get(1)?,
                clip_path: row.get(2)?,
                start_time: row.get(3)?,
                end_time: row.get(4)?,
                created_at,
            })
        })?;

        let mut result = Vec::new();
        for clip in clips {
            result.push(clip?);
        }

        Ok(result)
    }

    pub fn delete_clip(&self, id: i64) -> AppResult<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| AppError::Storage(e.to_string()))?;
        conn.execute("DELETE FROM clips WHERE id = ?1", params![id])?;
        Ok(())
    }

    fn db_path() -> PathBuf {
        crate::util::app_config_dir().join("clips.db")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Datelike;

    #[test]
    fn test_add_and_retrieve_clip() {
        let temp_dir = std::env::temp_dir().join("jorja-clipper-test-db");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();

        let db_path = temp_dir.join("test.db");
        let store = ClipStore::with_path(&db_path).unwrap();

        let clip = store
            .add_clip("/path/to/video.mp4", "/path/to/clip.mp4", 10.0, 20.0)
            .unwrap();

        assert_eq!(clip.start_time, 10.0);
        assert_eq!(clip.end_time, 20.0);

        let clips = store.get_clips_for_video("/path/to/video.mp4").unwrap();
        assert_eq!(clips.len(), 1);
        assert_eq!(clips[0].clip_path, "/path/to/clip.mp4");

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_delete_clip() {
        let temp_dir = std::env::temp_dir().join("jorja-clipper-test-db2");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();

        let db_path = temp_dir.join("test.db");
        let store = ClipStore::with_path(&db_path).unwrap();

        let clip = store
            .add_clip("/path/to/video.mp4", "/path/to/clip.mp4", 10.0, 20.0)
            .unwrap();

        store.delete_clip(clip.id).unwrap();

        let clips = store.get_clips_for_video("/path/to/video.mp4").unwrap();
        assert_eq!(clips.len(), 0);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_parse_datetime_rfc3339() {
        let result = parse_datetime("2026-05-24T07:46:33.067665Z").unwrap();
        assert_eq!(result.year(), 2026);
        assert_eq!(result.month(), 5);
        assert_eq!(result.day(), 24);
    }

    #[test]
    fn test_parse_datetime_rfc3339_with_offset() {
        let result = parse_datetime("2026-05-24T07:46:33+00:00").unwrap();
        assert_eq!(result.year(), 2026);
    }

    #[test]
    fn test_parse_datetime_naive() {
        let result = parse_datetime("2026-05-24T07:46:33.067665").unwrap();
        assert_eq!(result.year(), 2026);
        assert_eq!(result.month(), 5);
        assert_eq!(result.day(), 24);
    }

    #[test]
    fn test_parse_datetime_invalid() {
        let result = parse_datetime("not a date");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_datetime_invalid_format() {
        let result = parse_datetime("2026/05/24 07:46:33");
        assert!(result.is_err());
    }

    #[test]
    fn test_get_clips_for_video_empty() {
        let temp_dir = std::env::temp_dir().join("jorja-clipper-test-db3");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();

        let db_path = temp_dir.join("test.db");
        let store = ClipStore::with_path(&db_path).unwrap();

        let clips = store.get_clips_for_video("/nonexistent/video.mp4").unwrap();
        assert_eq!(clips.len(), 0);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_multiple_clips_same_video() {
        let temp_dir = std::env::temp_dir().join("jorja-clipper-test-db4");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();

        let db_path = temp_dir.join("test.db");
        let store = ClipStore::with_path(&db_path).unwrap();

        let video_path = "/test/video.mp4";
        for i in 0..5 {
            let clip_path = format!("/test/clip{}.mp4", i);
            store.add_clip(video_path, &clip_path, i as f64 * 10.0, i as f64 * 10.0 + 5.0).unwrap();
        }

        let clips = store.get_clips_for_video(video_path).unwrap();
        assert_eq!(clips.len(), 5);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_delete_nonexistent_clip() {
        let temp_dir = std::env::temp_dir().join("jorja-clipper-test-db5");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();

        let db_path = temp_dir.join("test.db");
        let store = ClipStore::with_path(&db_path).unwrap();

        // Delete clip that doesn't exist - should succeed (no rows affected)
        let result = store.delete_clip(999);
        assert!(result.is_ok());

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_clip_serialization() {
        let clip = Clip {
            id: 1,
            video_path: "/test/video.mp4".to_string(),
            clip_path: "/test/clip.mp4".to_string(),
            start_time: 10.5,
            end_time: 20.3,
            created_at: Utc::now(),
        };

        let json = serde_json::to_string(&clip).unwrap();
        let deserialized: Clip = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.id, clip.id);
        assert_eq!(deserialized.video_path, clip.video_path);
        assert_eq!(deserialized.clip_path, clip.clip_path);
    }

    #[test]
    fn test_concurrent_store_access() {
        use std::sync::Arc;
        use std::thread;

        let temp_dir = std::env::temp_dir().join("jorja-clipper-test-db6");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();

        let db_path = temp_dir.join("test.db");
        let store = Arc::new(ClipStore::with_path(&db_path).unwrap());

        let mut handles = vec![];
        for i in 0..5 {
            let store_clone = Arc::clone(&store);
            let handle = thread::spawn(move || {
                let clip_path = format!("/test/clip{}.mp4", i);
                store_clone.add_clip("/test/video.mp4", &clip_path, i as f64, i as f64 + 1.0).unwrap();
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        let clips = store.get_clips_for_video("/test/video.mp4").unwrap();
        assert_eq!(clips.len(), 5);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_storage_invalid_datetime_handling() {
        let temp_dir = std::env::temp_dir().join("jorja-clipper-test-db-invalid-dt");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();

        let db_path = temp_dir.join("test.db");
        let store = ClipStore::with_path(&db_path).unwrap();

        // Directly insert a clip with invalid datetime format
        {
            let conn = store.conn.lock().unwrap();
            conn.execute(
                "INSERT INTO clips (video_path, clip_path, start_time, end_time, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    "/test/video.mp4",
                    "/test/clip.mp4",
                    0.0,
                    10.0,
                    "invalid-datetime-format",
                ],
            ).unwrap();
        }

        // Try to retrieve clips - should handle the invalid datetime gracefully
        let clips = store.get_clips_for_video("/test/video.mp4");
        // This should return an error due to the invalid datetime
        assert!(clips.is_err());

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_init_schema_directly() {
        let temp_dir = std::env::temp_dir().join("jorja-clipper-test-db-init-schema");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();

        let db_path = temp_dir.join("test.db");
        let store = ClipStore::with_path(&db_path).unwrap();

        // Call init_schema directly (it should be idempotent)
        let result = store.init_schema();
        assert!(result.is_ok());

        // Call it again to verify idempotency
        let result = store.init_schema();
        assert!(result.is_ok());

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_with_path_creates_parent_directories() {
        let temp_dir = std::env::temp_dir().join("jorja-clipper-test-db-parent-dirs");
        let _ = std::fs::remove_dir_all(&temp_dir);

        // Create a nested path that doesn't exist yet
        let db_path = temp_dir.join("level1").join("level2").join("test.db");

        // with_path should create parent directories automatically
        let result = ClipStore::with_path(&db_path);
        assert!(result.is_ok());

        // Verify the database file was created
        assert!(db_path.exists());

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_add_clip_returns_correct_clip() {
        let temp_dir = std::env::temp_dir().join("jorja-clipper-test-db-add-clip-return");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();

        let db_path = temp_dir.join("test.db");
        let store = ClipStore::with_path(&db_path).unwrap();

        let video_path = "/test/video.mp4";
        let clip_path = "/test/clip.mp4";
        let start_time = 10.5;
        let end_time = 20.3;

        let clip = store.add_clip(video_path, clip_path, start_time, end_time).unwrap();

        // Verify the returned clip has the correct values
        assert_eq!(clip.video_path, video_path);
        assert_eq!(clip.clip_path, clip_path);
        assert_eq!(clip.start_time, start_time);
        assert_eq!(clip.end_time, end_time);
        assert!(clip.id > 0); // Should have a valid ID

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_delete_clip_returns_ok_even_if_not_found() {
        let temp_dir = std::env::temp_dir().join("jorja-clipper-test-db-delete-not-found");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();

        let db_path = temp_dir.join("test.db");
        let store = ClipStore::with_path(&db_path).unwrap();

        // Try to delete a clip that doesn't exist
        let result = store.delete_clip(999);

        // Should succeed (no error if clip doesn't exist)
        assert!(result.is_ok());

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
