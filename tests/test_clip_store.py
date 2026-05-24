"""Tests for the SQLite clip persistence layer."""


import pytest

from jorja_clipper.clip_store import ClipStore


@pytest.fixture
def store(tmp_path):
    """A fresh ClipStore using a temporary database."""
    db = tmp_path / "test_clips.db"
    return ClipStore(db_path=db)


def test_store_add_clip(store):
    """Adding a clip returns a positive row id and stores fields."""
    row_id = store.add_clip(
        clip_path="/tmp/game_clip_001.mp4",
        source_video_path="/tmp/game.mp4",
        start_time=25.0,
        end_time=35.0,
    )
    assert isinstance(row_id, int)
    assert row_id > 0


def test_store_get_all_clips(store):
    """get_all_clips returns clips ordered newest first."""
    store.add_clip("/tmp/a.mp4", "/tmp/game.mp4", 10.0, 20.0, "2026-01-01T10:00:00")
    store.add_clip("/tmp/b.mp4", "/tmp/game.mp4", 30.0, 40.0, "2026-01-01T11:00:00")
    clips = store.get_all_clips()
    assert len(clips) == 2
    assert clips[0].clip_path == "/tmp/b.mp4"
    assert clips[1].clip_path == "/tmp/a.mp4"


def test_store_get_clips_for_video(store):
    """get_clips_for_video filters by source path."""
    store.add_clip("/tmp/a.mp4", "/tmp/game.mp4", 10.0, 20.0)
    store.add_clip("/tmp/b.mp4", "/tmp/other.mp4", 30.0, 40.0)
    game_clips = store.get_clips_for_video("/tmp/game.mp4")
    assert len(game_clips) == 1
    assert game_clips[0].clip_path == "/tmp/a.mp4"


def test_store_delete_clip(store):
    """delete_clip removes a clip by id."""
    row_id = store.add_clip("/tmp/a.mp4", "/tmp/game.mp4", 10.0, 20.0)
    assert store.delete_clip(row_id) is True
    assert store.get_all_clips() == []
    assert store.delete_clip(row_id) is False


def test_store_get_last_clip(store):
    """get_last_clip returns the newest clip."""
    store.add_clip("/tmp/a.mp4", "/tmp/game.mp4", 10.0, 20.0, "2026-01-01T10:00:00")
    store.add_clip("/tmp/b.mp4", "/tmp/game.mp4", 30.0, 40.0, "2026-01-01T11:00:00")
    last = store.get_last_clip()
    assert last is not None
    assert last.clip_path == "/tmp/b.mp4"


def test_store_get_last_clip_empty(store):
    """get_last_clip returns None when empty."""
    assert store.get_last_clip() is None


def test_store_clear_all(store):
    """clear_all removes every clip."""
    store.add_clip("/tmp/a.mp4", "/tmp/game.mp4", 10.0, 20.0)
    store.add_clip("/tmp/b.mp4", "/tmp/game.mp4", 30.0, 40.0)
    store.clear_all()
    assert store.get_all_clips() == []


def test_store_calculates_duration(store):
    """Duration is derived from end - start."""
    store.add_clip("/tmp/a.mp4", "/tmp/game.mp4", 10.0, 25.0)
    clip = store.get_last_clip()
    assert clip.duration == 15.0


def test_store_uses_default_created_at(store):
    """When created_at is omitted, it defaults to a valid ISO timestamp."""
    store.add_clip("/tmp/a.mp4", "/tmp/game.mp4", 10.0, 20.0)
    clip = store.get_last_clip()
    assert clip.created_at
    assert "T" in clip.created_at
