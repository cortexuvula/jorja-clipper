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
        let db_path = Self::db_path()?;

        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let conn = Connection::open(&db_path)?;
        let store = Self {
            conn: Mutex::new(conn),
        };
        store.init_schema()?;

        Ok(store)
    }

    fn init_schema(&self) -> AppResult<()> {
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

    fn db_path() -> AppResult<PathBuf> {
        let config_dir = dirs::config_dir()
            .ok_or_else(|| AppError::Storage("Could not determine config directory".to_string()))?;

        Ok(config_dir.join("jorja-clipper").join("clips.db"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_and_retrieve_clip() {
        let temp_dir = std::env::temp_dir().join("jorja-clipper-test-db");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();

        let db_path = temp_dir.join("test.db");
        let conn = Connection::open(&db_path).unwrap();

        let store = ClipStore {
            conn: Mutex::new(conn),
        };
        store.init_schema().unwrap();

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
        let conn = Connection::open(&db_path).unwrap();

        let store = ClipStore {
            conn: Mutex::new(conn),
        };
        store.init_schema().unwrap();

        let clip = store
            .add_clip("/path/to/video.mp4", "/path/to/clip.mp4", 10.0, 20.0)
            .unwrap();

        store.delete_clip(clip.id).unwrap();

        let clips = store.get_clips_for_video("/path/to/video.mp4").unwrap();
        assert_eq!(clips.len(), 0);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
