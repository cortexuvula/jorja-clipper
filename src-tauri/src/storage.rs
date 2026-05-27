use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::error::{AppError, AppResult};

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
    conn: Connection,
}

impl ClipStore {
    pub fn new() -> AppResult<Self> {
        let db_path = Self::db_path()?;

        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let conn = Connection::open(&db_path)?;
        let store = Self { conn };
        store.init_schema()?;

        Ok(store)
    }

    fn init_schema(&self) -> AppResult<()> {
        self.conn.execute(
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

        self.conn.execute(
            "INSERT INTO clips (video_path, clip_path, start_time, end_time, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![video_path, clip_path, start_time, end_time, created_at.to_rfc3339()],
        )?;

        let id = self.conn.last_insert_rowid();

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
        let mut stmt = self.conn.prepare(
            "SELECT id, video_path, clip_path, start_time, end_time, created_at
             FROM clips
             WHERE video_path = ?1
             ORDER BY created_at DESC",
        )?;

        let clips = stmt.query_map(params![video_path], |row| {
            Ok(Clip {
                id: row.get(0)?,
                video_path: row.get(1)?,
                clip_path: row.get(2)?,
                start_time: row.get(3)?,
                end_time: row.get(4)?,
                created_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(5)?)
                    .unwrap()
                    .with_timezone(&Utc),
            })
        })?;

        let mut result = Vec::new();
        for clip in clips {
            result.push(clip?);
        }

        Ok(result)
    }

    pub fn delete_clip(&self, id: i64) -> AppResult<()> {
        self.conn
            .execute("DELETE FROM clips WHERE id = ?1", params![id])?;
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

        let store = ClipStore { conn };
        store.init_schema().unwrap();

        let clip = store
            .add_clip("/path/to/video.mp4", "/path/to/clip.mp4", 10.0, 20.0)
            .unwrap();

        assert_eq!(clip.start_time, 10.0);
        assert_eq!(clip.end_time, 20.0);

        let clips = store
            .get_clips_for_video("/path/to/video.mp4")
            .unwrap();
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

        let store = ClipStore { conn };
        store.init_schema().unwrap();

        let clip = store
            .add_clip("/path/to/video.mp4", "/path/to/clip.mp4", 10.0, 20.0)
            .unwrap();

        store.delete_clip(clip.id).unwrap();

        let clips = store
            .get_clips_for_video("/path/to/video.mp4")
            .unwrap();
        assert_eq!(clips.len(), 0);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
