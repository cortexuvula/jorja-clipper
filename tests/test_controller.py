"""Tests for ClipController and logging setup."""

from pathlib import Path
from unittest.mock import MagicMock

from PySide6.QtCore import QObject

from jorja_clipper.clipper import ClipResult
from jorja_clipper.controller import ClipController
from jorja_clipper.gui.clip_list import ClipListModel


def test_controller_is_qobject():
    """ClipController must be a QObject so worker signals use queued delivery."""
    ctrl = ClipController(MagicMock(), MagicMock(), MagicMock(), ClipListModel())
    assert isinstance(ctrl, QObject)


def test_controller_open_file_success():
    """open_file delegates to player.load and updates state."""
    player = MagicMock()
    player.load.return_value = True
    clipper = MagicMock()
    settings = MagicMock()
    settings.clip_key = "C"
    model = ClipListModel()
    ctrl = ClipController(player, clipper, settings, model)

    path = Path("/tmp/game.mp4")
    assert ctrl.open_file(path) is True
    assert ctrl.current_video == path
    player.load.assert_called_once_with(path)


def test_controller_open_file_failure():
    """open_file returns False when player.load fails."""
    player = MagicMock()
    player.load.return_value = False
    ctrl = ClipController(player, MagicMock(), MagicMock(), ClipListModel())
    path = Path("/tmp/game.mp4")
    assert ctrl.open_file(path) is False


def test_controller_toggle_play():
    """toggle_play delegates to player.toggle_pause."""
    player = MagicMock()
    ctrl = ClipController(player, MagicMock(), MagicMock(), ClipListModel())
    ctrl.toggle_play()
    player.toggle_pause.assert_called_once()


def test_controller_seek():
    """Seek delegates to player.seek."""
    player = MagicMock()
    ctrl = ClipController(player, MagicMock(), MagicMock(), ClipListModel())
    ctrl.seek(5.0)
    player.seek.assert_called_once_with(5.0)


def test_controller_shutdown():
    """Shutdown delegates to player.shutdown."""
    player = MagicMock()
    ctrl = ClipController(player, MagicMock(), MagicMock(), ClipListModel())
    ctrl.shutdown()
    player.shutdown.assert_called_once()


def test_controller_save_clip_no_video():
    """save_clip returns failure when no video is loaded."""
    ctrl = ClipController(MagicMock(), MagicMock(), MagicMock(), ClipListModel())
    result = ctrl.save_clip()
    assert result.success is False
    assert "No video loaded" in result.error


def test_controller_save_clip_success():
    """save_clip starts a worker and _on_clip_finished updates state."""
    player = MagicMock()
    player.current_pos = 30.0
    player.duration = 120.0

    clipper = MagicMock()
    clipper.calculate_times.return_value = (25.0, 35.0)
    model = ClipListModel()
    ctrl = ClipController(player, clipper, MagicMock(), model)
    ctrl._current_video = Path("/tmp/game.mp4")

    from jorja_clipper.worker import ClipWorker

    worker = ctrl.save_clip()
    assert isinstance(worker, ClipWorker)

    # Simulate the worker finishing
    result = ClipResult(
        path="/tmp/clips/game_clip_001.mp4",
        start_time=25.0,
        end_time=35.0,
        success=True,
    )
    ctrl._on_clip_finished(result)
    assert ctrl.clip_count == 1
    assert model.rowCount() == 1


def test_controller_save_clip_rejects_while_running():
    """save_clip returns a failure ClipResult when a worker is already active."""
    player = MagicMock()
    player.current_pos = 30.0
    player.duration = 120.0

    ctrl = ClipController(player, MagicMock(), MagicMock(), ClipListModel())
    ctrl._current_video = Path("/tmp/game.mp4")

    # pre-set calculate_times return value so save_clip doesn't break
    ctrl._clipper.calculate_times.return_value = (25.0, 35.0)

    from jorja_clipper.worker import ClipWorker

    first = ctrl.save_clip()
    # still "running" because we didn't let the thread finish
    second = ctrl.save_clip()
    assert isinstance(second, ClipResult)
    assert second.success is False
    assert "already in progress" in second.error.lower()
    # cleanup
    if isinstance(first, ClipWorker):
        first.deleteLater()
    ctrl._active_worker = None


def test_controller_apply_settings():
    """apply_settings propagates buffer values to clipper."""
    clipper = MagicMock()
    settings = MagicMock()
    settings.buffer_before = 10.0
    settings.buffer_after = 3.0
    ctrl = ClipController(MagicMock(), clipper, settings, ClipListModel())
    ctrl.apply_settings()
    assert clipper.buffer_before == 10.0
    assert clipper.buffer_after == 3.0


def test_logging_setup():
    """app._setup_logging configures root logger with two handlers."""
    import logging

    from jorja_clipper.app import _setup_logging

    # Clear any existing handlers to get a clean state
    root = logging.getLogger()
    for h in list(root.handlers):
        root.removeHandler(h)
    root.setLevel(logging.WARNING)

    _setup_logging()

    assert root.level == logging.DEBUG
    assert len(root.handlers) == 2
    handler_types = {type(h).__name__ for h in root.handlers}
    assert "StreamHandler" in handler_types
    assert "RotatingFileHandler" in handler_types

    # Cleanup
    for h in list(root.handlers):
        h.close()
        root.removeHandler(h)
