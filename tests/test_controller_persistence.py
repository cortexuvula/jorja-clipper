"""Tests for ClipController persistence and undo."""

from pathlib import Path
from unittest.mock import MagicMock

from jorja_clipper.clip_store import ClipStore
from jorja_clipper.clipper import ClipResult
from jorja_clipper.controller import ClipController
from jorja_clipper.gui.clip_list import ClipListModel


def _make_controller(tmp_path, clip_store=None):
    player = MagicMock()
    clipper = MagicMock()
    settings = MagicMock()
    settings.clip_key = "C"
    model = ClipListModel()
    return ClipController(player, clipper, settings, model, clip_store=clip_store)


def test_controller_loads_persisted_clips(tmp_path):
    """open_file loads previously persisted clips into the model."""
    store = ClipStore(tmp_path / "clips.db")
    video = Path("/tmp/game.mp4")
    store.add_clip(
        clip_path=str(tmp_path / "clips" / "game_clip_001.mp4"),
        source_video_path=str(video),
        start_time=10.0,
        end_time=20.0,
    )
    store.add_clip(
        clip_path=str(tmp_path / "clips" / "game_clip_002.mp4"),
        source_video_path=str(video),
        start_time=30.0,
        end_time=40.0,
    )

    ctrl = _make_controller(tmp_path, clip_store=store)
    ctrl.open_file(video)
    assert ctrl.clip_model.rowCount() == 2
    assert ctrl.clip_count == 2


def test_controller_persists_new_clip_on_finish(tmp_path):
    """After a successful clip, the result is persisted in the store."""
    store = ClipStore(tmp_path / "clips.db")
    ctrl = _make_controller(tmp_path, clip_store=store)
    ctrl._current_video = Path("/tmp/game.mp4")

    result = ClipResult(
        path=str(tmp_path / "out.mp4"),
        start_time=25.0,
        end_time=35.0,
        success=True,
    )
    ctrl._on_clip_finished(result)

    assert store.get_last_clip() is not None
    assert store.get_last_clip().clip_path == str(tmp_path / "out.mp4")
    assert store.get_last_clip().start_time == 25.0


def test_controller_undo_nothing_to_undo(tmp_path):
    """undo_last_clip returns False when no clip has been saved."""
    ctrl = _make_controller(tmp_path)
    assert ctrl.undo_last_clip() is False


def test_controller_undo_restores_state(tmp_path):
    """undo_last_clip removes clip from model, DB, disk, and restores position."""
    store = ClipStore(tmp_path / "clips.db")
    ctrl = _make_controller(tmp_path, clip_store=store)
    ctrl._current_video = Path("/tmp/game.mp4")

    clip_file = tmp_path / "clips" / "game_clip_001.mp4"
    clip_file.parent.mkdir(parents=True, exist_ok=True)
    clip_file.write_text("fake clip")

    result = ClipResult(
        path=str(clip_file),
        start_time=25.0,
        end_time=35.0,
        success=True,
    )
    ctrl._on_clip_finished(result)
    assert ctrl.clip_model.rowCount() == 1
    assert store.get_all_clips()
    assert clip_file.exists()

    undone = ctrl.undo_last_clip()
    assert undone is True
    assert ctrl.clip_model.rowCount() == 0
    assert not store.get_all_clips()
    assert not clip_file.exists()
    ctrl._player.seek_to.assert_called_once_with(25.0)


def test_controller_undo_updates_clip_count(tmp_path):
    """undo_last_clip decrements _clip_count."""
    store = ClipStore(tmp_path / "clips.db")
    ctrl = _make_controller(tmp_path, clip_store=store)
    ctrl._current_video = Path("/tmp/game.mp4")

    result = ClipResult(
        path=str(tmp_path / "a.mp4"),
        start_time=10.0,
        end_time=20.0,
        success=True,
    )
    ctrl._on_clip_finished(result)
    assert ctrl.clip_count == 1

    ctrl.undo_last_clip()
    assert ctrl.clip_count == 0


def test_controller_undo_cannot_double_undo(tmp_path):
    """Only the most recent clip is undoable."""
    store = ClipStore(tmp_path / "clips.db")
    ctrl = _make_controller(tmp_path, clip_store=store)
    ctrl._current_video = Path("/tmp/game.mp4")

    ctrl._on_clip_finished(
        ClipResult(
            path=str(tmp_path / "a.mp4"),
            start_time=10.0,
            end_time=20.0,
            success=True,
        )
    )
    ctrl.undo_last_clip()
    assert ctrl.undo_last_clip() is False


def test_controller_undo_deletes_db_row(tmp_path):
    """undo_last_clip removes the correct row from the database."""
    store = ClipStore(tmp_path / "clips.db")
    ctrl = _make_controller(tmp_path, clip_store=store)
    ctrl._current_video = Path("/tmp/game.mp4")

    ctrl._on_clip_finished(
        ClipResult(
            path=str(tmp_path / "a.mp4"),
            start_time=10.0,
            end_time=20.0,
            success=True,
        )
    )
    ctrl._on_clip_finished(
        ClipResult(
            path=str(tmp_path / "b.mp4"),
            start_time=30.0,
            end_time=40.0,
            success=True,
        )
    )
    assert len(store.get_all_clips()) == 2

    ctrl.undo_last_clip()
    remaining = store.get_all_clips()
    assert len(remaining) == 1
    assert remaining[0].clip_path == str(tmp_path / "a.mp4")
