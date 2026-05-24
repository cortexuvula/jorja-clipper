"""SQLite-based persistence for clip metadata."""

import sqlite3
from dataclasses import dataclass
from datetime import datetime
from pathlib import Path

__all__ = ["StoredClip", "ClipStore"]


@dataclass
class StoredClip:
    """A clip as stored in the SQLite database."""

    id: int
    clip_path: str
    source_video_path: str
    start_time: float
    end_time: float
    duration: float
    created_at: str


class ClipStore:
    """Persists clip metadata to an SQLite database."""

    def __init__(self, db_path: Path | None = None) -> None:
        if db_path is None:
            db_path = Path.home() / ".config" / "jorja-clipper" / "clips.db"
        self._db_path = db_path
        self._db_path.parent.mkdir(parents=True, exist_ok=True)
        self._ensure_schema()

    def _ensure_schema(self) -> None:
        with sqlite3.connect(self._db_path) as conn:
            conn.execute(
                """
                CREATE TABLE IF NOT EXISTS clips (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    clip_path TEXT NOT NULL,
                    source_video_path TEXT NOT NULL,
                    start_time REAL NOT NULL,
                    end_time REAL NOT NULL,
                    duration REAL NOT NULL,
                    created_at TEXT NOT NULL
                )
                """
            )

    def add_clip(
        self,
        clip_path: str,
        source_video_path: str,
        start_time: float,
        end_time: float,
        created_at: str | None = None,
    ) -> int:
        """Persist a new clip and return its row id."""
        if created_at is None:
            created_at = datetime.now().isoformat()
        duration = end_time - start_time
        with sqlite3.connect(self._db_path) as conn:
            cursor = conn.execute(
                """
                INSERT INTO clips (
                    clip_path, source_video_path,
                    start_time, end_time, duration, created_at
                )
                VALUES (?, ?, ?, ?, ?, ?)
                """,
                (
                    clip_path, source_video_path,
                    start_time, end_time, duration, created_at
                ),
            )
            conn.commit()
            return cursor.lastrowid or 0

    def get_all_clips(self) -> list[StoredClip]:
        """Return all clips ordered by newest first."""
        with sqlite3.connect(self._db_path) as conn:
            rows = conn.execute(
                """
                SELECT id, clip_path, source_video_path, start_time,
                       end_time, duration, created_at
                FROM clips
                ORDER BY created_at DESC
                """
            ).fetchall()
            return [StoredClip(*row) for row in rows]

    def get_clips_for_video(self, video_path: str) -> list[StoredClip]:
        """Return clips for a specific source video."""
        with sqlite3.connect(self._db_path) as conn:
            rows = conn.execute(
                """
                SELECT id, clip_path, source_video_path, start_time,
                       end_time, duration, created_at
                FROM clips
                WHERE source_video_path = ?
                ORDER BY created_at DESC
                """,
                (str(video_path),),
            ).fetchall()
            return [StoredClip(*row) for row in rows]

    def delete_clip(self, clip_id: int) -> bool:
        """Delete a clip by id. Returns True if a row was deleted."""
        with sqlite3.connect(self._db_path) as conn:
            cursor = conn.execute("DELETE FROM clips WHERE id = ?", (clip_id,))
            conn.commit()
            return cursor.rowcount > 0

    def get_last_clip(self) -> StoredClip | None:
        """Return the most recently created clip, or None."""
        with sqlite3.connect(self._db_path) as conn:
            row = conn.execute(
                """
                SELECT id, clip_path, source_video_path, start_time,
                       end_time, duration, created_at
                FROM clips
                ORDER BY created_at DESC
                LIMIT 1
                """
            ).fetchone()
            if row is None:
                return None
            return StoredClip(*row)

    def clear_all(self) -> None:
        """Remove every clip record."""
        with sqlite3.connect(self._db_path) as conn:
            conn.execute("DELETE FROM clips")
            conn.commit()
