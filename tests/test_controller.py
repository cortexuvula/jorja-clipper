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


def test_controller_shutdown_interrupts_active_worker():
    """shutdown() cancels and waits on a running clip worker."""
    from jorja_clipper.worker import ClipWorker

    player = MagicMock()

    # Build a mock worker that reports itself as still running so that
    # shutdown() enters the cancellation branch.  We can't use a real
    # ClipWorker with a blocking save_clip because shutdown() calls
    # wait(5000) which would deadlock the test thread.
    worker = MagicMock(spec=ClipWorker)
    worker.isRunning.return_value = True
    worker.wait.return_value = True  # thread finished within timeout

    ctrl = ClipController(player, MagicMock(), MagicMock(), ClipListModel())
    ctrl._active_worker = worker

    ctrl.shutdown()

    worker.cancel.assert_called_once()
    worker.requestInterruption.assert_called_once()
    worker.wait.assert_called_once_with(5000)
    player.shutdown.assert_called_once()


def test_controller_shutdown_interrupts_batch_worker():
    """shutdown() cancels and waits on a running batch worker."""
    from jorja_clipper.batch_queue import BatchWorker

    player = MagicMock()

    worker = MagicMock(spec=BatchWorker)
    worker.isRunning.return_value = True
    worker.wait.return_value = True

    ctrl = ClipController(player, MagicMock(), MagicMock(), ClipListModel())
    ctrl._batch_worker = worker

    ctrl.shutdown()

    worker.cancel.assert_called_once()
    worker.requestInterruption.assert_called_once()
    worker.wait.assert_called_once_with(5000)
    player.shutdown.assert_called_once()


def test_controller_shutdown_no_workers():
    """shutdown() works cleanly when no workers are running."""
    player = MagicMock()
    ctrl = ClipController(player, MagicMock(), MagicMock(), ClipListModel())
    ctrl.shutdown()
    player.shutdown.assert_called_once()


def test_controller_shutdown_warns_on_worker_timeout(caplog):
    """shutdown() logs a warning when a worker doesn't finish within 5 s."""
    import logging

    from jorja_clipper.worker import ClipWorker

    player = MagicMock()

    worker = MagicMock(spec=ClipWorker)
    worker.isRunning.return_value = True
    worker.wait.return_value = False  # timeout expired

    ctrl = ClipController(player, MagicMock(), MagicMock(), ClipListModel())
    ctrl._active_worker = worker

    with caplog.at_level(logging.WARNING):
        ctrl.shutdown()

    assert any(
        "Clip worker did not finish within 5 s" in record.message
        for record in caplog.records
    )
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
    # Return a real ClipResult so queued signals from the background
    # thread don't crash if the event loop delivers them later.
    clipper.save_clip.return_value = ClipResult(
        path="/tmp/clips/game_clip_001.mp4",
        start_time=25.0,
        end_time=35.0,
        success=True,
    )
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
    ctrl._on_clip_finished(worker, result)
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
    # Return a real ClipResult so queued signals from the background
    # thread don't crash if the event loop delivers them later.
    ctrl._clipper.save_clip.return_value = ClipResult(
        path="/tmp/clips/game_clip_001.mp4",
        start_time=25.0,
        end_time=35.0,
        success=True,
    )

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


# ---------------------------------------------------------------------------
# Race-condition tests for worker cleanup (Bug 4)
# ---------------------------------------------------------------------------


def test_on_clip_finished_ignores_stale_worker():
    """A stale worker's signal must not delete the current active worker.

    Simulates the narrow window where:
    1. Worker A finishes and its signal is queued but not yet delivered.
    2. User presses hotkey again, creating Worker B (now _active_worker).
    3. Worker A's signal is delivered — must NOT clean up Worker B.
    """
    player = MagicMock()
    player.current_pos = 30.0
    player.duration = 120.0

    clipper = MagicMock()
    clipper.calculate_times.return_value = (25.0, 35.0)
    # Return a real ClipResult so queued signals from the background
    # thread don't crash if the event loop delivers them later.
    clipper.save_clip.return_value = ClipResult(
        path="/tmp/clips/game_clip_001.mp4",
        start_time=25.0,
        end_time=35.0,
        success=True,
    )
    model = ClipListModel()
    ctrl = ClipController(player, clipper, MagicMock(), model)
    ctrl._current_video = Path("/tmp/game.mp4")

    from jorja_clipper.worker import ClipWorker

    # Create "worker A" — the first worker that will finish
    worker_a = ctrl.save_clip()
    assert isinstance(worker_a, ClipWorker)

    # Pretend worker A finished: clear _active_worker so save_clip() allows
    # a second worker (simulating the race window where A's signal is queued
    # but B hasn't been created yet — in reality is_clipping would still be
    # true, but we force the scenario for unit testing).
    ctrl._active_worker = None

    # Create "worker B" — the newer worker that is now active
    worker_b = ctrl.save_clip()
    assert isinstance(worker_b, ClipWorker)
    assert ctrl._active_worker is worker_b

    # Now deliver worker A's (stale) finished signal directly
    result_a = ClipResult(
        path="/tmp/clips/game_clip_001.mp4",
        start_time=25.0,
        end_time=35.0,
        success=True,
    )
    ctrl._on_clip_finished(worker_a, result_a)

    # Worker A's signal should have been processed (clip count incremented)
    # but _active_worker must still point to worker_b — not cleared.
    assert ctrl._active_worker is worker_b
    assert ctrl.clip_count == 1

    # Deliver worker B's legitimate signal
    result_b = ClipResult(
        path="/tmp/clips/game_clip_002.mp4",
        start_time=25.0,
        end_time=35.0,
        success=True,
    )
    ctrl._on_clip_finished(worker_b, result_b)

    # Now _active_worker should be cleared and clip_count should be 2
    assert ctrl._active_worker is None
    assert ctrl.clip_count == 2

    # Cleanup
    worker_a.deleteLater()


def test_on_clip_finished_rejects_mismatched_worker():
    """_on_clip_finished must not touch _active_worker when identity differs."""
    player = MagicMock()
    player.current_pos = 30.0
    player.duration = 120.0

    clipper = MagicMock()
    clipper.calculate_times.return_value = (25.0, 35.0)
    # Return a real ClipResult so queued signals from the background
    # thread don't crash if the event loop delivers them later.
    clipper.save_clip.return_value = ClipResult(
        path="/tmp/clips/game_clip_001.mp4",
        start_time=25.0,
        end_time=35.0,
        success=True,
    )
    model = ClipListModel()
    ctrl = ClipController(player, clipper, MagicMock(), model)
    ctrl._current_video = Path("/tmp/game.mp4")

    from jorja_clipper.worker import ClipWorker

    real_worker = ctrl.save_clip()
    assert isinstance(real_worker, ClipWorker)

    # Create a different (stale) worker mock
    stale_worker = MagicMock(spec=ClipWorker)

    result = ClipResult(
        path="/tmp/clips/game_clip_001.mp4",
        start_time=25.0,
        end_time=35.0,
        success=True,
    )
    # Call with the stale worker — should process the result but NOT
    # deleteLater() the real active worker.
    ctrl._on_clip_finished(stale_worker, result)

    # The real worker must still be the active one and untouched
    assert ctrl._active_worker is real_worker
    stale_worker.deleteLater.assert_not_called()

    # Cleanup
    real_worker.deleteLater()
    ctrl._active_worker = None


def test_on_batch_finished_ignores_stale_worker():
    """A stale batch worker's signal must not delete the current batch worker."""
    player = MagicMock()
    player.current_pos = 30.0
    player.duration = 120.0

    clipper = MagicMock()
    clipper.calculate_times.return_value = (25.0, 35.0)
    # Return a real ClipResult so queued signals from the background
    # thread don't crash if the event loop delivers them later.
    clipper.save_clip.return_value = ClipResult(
        path="/tmp/clips/game_clip_001.mp4",
        start_time=25.0,
        end_time=35.0,
        success=True,
    )
    model = ClipListModel()
    ctrl = ClipController(player, clipper, MagicMock(), model)
    ctrl._current_video = Path("/tmp/game.mp4")

    from jorja_clipper.batch_queue import BatchWorker

    # Enqueue items for two batches
    ctrl.queue_clip()
    ctrl.queue_clip()

    # Start first batch
    worker_a = ctrl.process_batch()
    assert isinstance(worker_a, BatchWorker)

    # Force the scenario: pretend worker A's signal is queued, and a new
    # batch was started in the meantime.
    ctrl._batch_worker = None
    ctrl.queue_clip()
    worker_b = ctrl.process_batch()
    assert isinstance(worker_b, BatchWorker)
    assert ctrl._batch_worker is worker_b

    # Deliver worker A's stale signal directly
    ctrl._on_batch_finished(worker_a, [])

    # worker_b must still be the active batch worker
    assert ctrl._batch_worker is worker_b

    # Deliver worker B's legitimate signal
    ctrl._on_batch_finished(worker_b, [])
    assert ctrl._batch_worker is None

    # Cleanup
    worker_a.deleteLater()


# ---------------------------------------------------------------------------
# Bug 6: Clip list not cleared when opening new video
# ---------------------------------------------------------------------------


def test_open_file_clears_clip_model(tmp_path):
    """Opening a new video must clear clips from the previous video."""
    from jorja_clipper.clip_store import ClipStore

    db_path = tmp_path / "clips.db"
    store = ClipStore(db_path)

    # Persist clips for two different videos
    store.add_clip(
        clip_path="/tmp/clips/a_clip_001.mp4",
        source_video_path="/tmp/video_a.mp4",
        start_time=10.0,
        end_time=20.0,
    )
    store.add_clip(
        clip_path="/tmp/clips/b_clip_001.mp4",
        source_video_path="/tmp/video_b.mp4",
        start_time=30.0,
        end_time=40.0,
    )

    player = MagicMock()
    player.load.return_value = True
    model = ClipListModel()
    ctrl = ClipController(player, MagicMock(), MagicMock(), model, clip_store=store)

    # Open video A — should load 1 clip
    ctrl.open_file(Path("/tmp/video_a.mp4"))
    assert model.rowCount() == 1
    assert ctrl.clip_count == 1

    # Open video B — should clear video A's clips and load video B's clips
    ctrl.open_file(Path("/tmp/video_b.mp4"))
    assert model.rowCount() == 1
    assert ctrl.clip_count == 1
    # Verify the remaining clip is from video B
    clip = model.clip_at(0)
    assert clip is not None
    assert "b_clip_001" in clip.path


def test_open_file_clears_undo_info(tmp_path):
    """Opening a new video must clear undo info from the previous video."""
    from jorja_clipper.clip_store import ClipStore

    db_path = tmp_path / "clips.db"
    store = ClipStore(db_path)

    store.add_clip(
        clip_path="/tmp/clips/a_clip_001.mp4",
        source_video_path="/tmp/video_a.mp4",
        start_time=10.0,
        end_time=20.0,
    )

    player = MagicMock()
    player.load.return_value = True
    ctrl = ClipController(
        player, MagicMock(), MagicMock(), ClipListModel(), clip_store=store
    )

    # Open video A to populate undo info
    ctrl.open_file(Path("/tmp/video_a.mp4"))
    # Manually set undo info to simulate a clip having been made
    ctrl._last_undo_info = (store.get_last_clip(), 10.0)
    assert ctrl._last_undo_info is not None

    # Open video B — should clear undo info
    ctrl.open_file(Path("/tmp/video_b.mp4"))
    assert ctrl._last_undo_info is None


# ---------------------------------------------------------------------------
# Bug 7: _clip_count not incremented during batch
# ---------------------------------------------------------------------------


def test_batch_progress_increments_clip_count(tmp_path):
    """_on_batch_progress must increment _clip_count for each successful clip."""
    from jorja_clipper.clip_store import ClipStore

    db_path = tmp_path / "clips.db"
    store = ClipStore(db_path)

    player = MagicMock()
    model = ClipListModel()
    ctrl = ClipController(player, MagicMock(), MagicMock(), model, clip_store=store)
    ctrl._current_video = Path("/tmp/game.mp4")
    ctrl._clip_count = 2  # simulate 2 clips already saved

    # Simulate two batch progress events
    result_1 = ClipResult(
        path="/tmp/clips/game_clip_003.mp4",
        start_time=25.0,
        end_time=35.0,
        success=True,
    )
    ctrl._on_batch_progress(1, 2, result_1)
    assert ctrl.clip_count == 3
    assert model.rowCount() == 1

    result_2 = ClipResult(
        path="/tmp/clips/game_clip_004.mp4",
        start_time=50.0,
        end_time=60.0,
        success=True,
    )
    ctrl._on_batch_progress(2, 2, result_2)
    assert ctrl.clip_count == 4
    assert model.rowCount() == 2


def test_batch_progress_does_not_increment_on_failure(tmp_path):
    """_on_batch_progress must not increment _clip_count for failed clips."""
    from jorja_clipper.clip_store import ClipStore

    db_path = tmp_path / "clips.db"
    store = ClipStore(db_path)

    ctrl = ClipController(
        MagicMock(), MagicMock(), MagicMock(), ClipListModel(), clip_store=store
    )
    ctrl._current_video = Path("/tmp/game.mp4")
    ctrl._clip_count = 1

    result = ClipResult(
        path="",
        start_time=0.0,
        end_time=0.0,
        success=False,
        error="ffmpeg error",
    )
    ctrl._on_batch_progress(1, 1, result)
    assert ctrl.clip_count == 1


# ---------------------------------------------------------------------------
# Bug 8: _last_undo_info not set for batch clips
# ---------------------------------------------------------------------------


def test_batch_progress_sets_undo_info(tmp_path):
    """_on_batch_progress must set _last_undo_info for successful clips."""
    from jorja_clipper.clip_store import ClipStore

    db_path = tmp_path / "clips.db"
    store = ClipStore(db_path)

    ctrl = ClipController(
        MagicMock(), MagicMock(), MagicMock(), ClipListModel(), clip_store=store
    )
    ctrl._current_video = Path("/tmp/game.mp4")

    result = ClipResult(
        path="/tmp/clips/game_clip_001.mp4",
        start_time=25.0,
        end_time=35.0,
        success=True,
    )
    ctrl._on_batch_progress(1, 1, result)

    assert ctrl._last_undo_info is not None
    stored_clip, restore_pos = ctrl._last_undo_info
    assert stored_clip.clip_path == "/tmp/clips/game_clip_001.mp4"
    assert restore_pos == 25.0


def test_batch_progress_updates_undo_info_per_clip(tmp_path):
    """_last_undo_info should reflect the most recent batch clip."""
    from jorja_clipper.clip_store import ClipStore

    db_path = tmp_path / "clips.db"
    store = ClipStore(db_path)

    ctrl = ClipController(
        MagicMock(), MagicMock(), MagicMock(), ClipListModel(), clip_store=store
    )
    ctrl._current_video = Path("/tmp/game.mp4")

    result_1 = ClipResult(
        path="/tmp/clips/game_clip_001.mp4",
        start_time=10.0,
        end_time=20.0,
        success=True,
    )
    ctrl._on_batch_progress(1, 2, result_1)

    result_2 = ClipResult(
        path="/tmp/clips/game_clip_002.mp4",
        start_time=40.0,
        end_time=50.0,
        success=True,
    )
    ctrl._on_batch_progress(2, 2, result_2)

    # Undo info should point to the second (most recent) clip
    assert ctrl._last_undo_info is not None
    stored_clip, restore_pos = ctrl._last_undo_info
    assert stored_clip.clip_path == "/tmp/clips/game_clip_002.mp4"
    assert restore_pos == 40.0


def test_batch_progress_does_not_set_undo_info_on_failure(tmp_path):
    """_last_undo_info should remain None if a batch clip fails."""
    from jorja_clipper.clip_store import ClipStore

    db_path = tmp_path / "clips.db"
    store = ClipStore(db_path)

    ctrl = ClipController(
        MagicMock(), MagicMock(), MagicMock(), ClipListModel(), clip_store=store
    )
    ctrl._current_video = Path("/tmp/game.mp4")
    ctrl._last_undo_info = None

    result = ClipResult(
        path="",
        start_time=0.0,
        end_time=0.0,
        success=False,
        error="ffmpeg error",
    )
    ctrl._on_batch_progress(1, 1, result)
    assert ctrl._last_undo_info is None
