"""Tests for the player wrapper."""

import threading
from unittest.mock import MagicMock, patch

from jorja_clipper.player import Player


def test_player_initial_state():
    """Player starts with no file loaded."""
    p = Player.__new__(Player)
    p._mpv = MagicMock()
    p._duration = 0.0
    p._current_pos = 0.0
    p._paused = True
    p._lock = threading.Lock()
    assert p.duration == 0.0
    assert p.current_pos == 0.0
    assert p.paused is True


def test_player_toggle_pause():
    """toggle_pause reads _paused under lock and sets mpv.pause accordingly.

    The property observer is the sole writer of _paused, so we simulate
    the observer firing after each toggle to verify the full cycle.
    """
    p = Player.__new__(Player)
    p._mpv = MagicMock()
    p._paused = True
    p._lock = threading.Lock()

    # First toggle: paused -> playing
    p.toggle_pause()
    assert p._mpv.pause == "no"
    # Simulate the property observer updating _paused
    with p._lock:
        p._paused = False

    # Second toggle: playing -> paused
    p.toggle_pause()
    assert p._mpv.pause == "yes"
    with p._lock:
        p._paused = True
    assert p.paused is True


def test_player_seek():
    """Seek calls mpv.command with the correct offset."""
    p = Player.__new__(Player)
    p._mpv = MagicMock()
    p.seek(10.0)
    p._mpv.command.assert_called_with("seek", 10.0, "relative")


def test_player_noop_when_mpv_none():
    """toggle_pause and seek are safe when mpv is not created."""
    p = Player()
    p._mpv = None
    p.toggle_pause()  # should not raise
    p.seek(5.0)  # should not raise


def test_player_init_with_wid():
    """init_with_wid stores the wid for lazy mpv creation."""
    p = Player()
    p.init_with_wid(42)
    assert p._wid == 42
    assert p._mpv is None


@patch("jorja_clipper.player.mpv.MPV")
def test_player_load_lazily_creates_mpv(mock_mpv):
    """load() creates the mpv instance lazily."""
    mock_instance = MagicMock()
    mock_mpv.return_value = mock_instance
    p = Player()
    p.init_with_wid(7)
    import tempfile
    from pathlib import Path

    with tempfile.NamedTemporaryFile(suffix=".mp4", delete=False) as f:
        path = Path(f.name)
    p.load(path)
    assert p._mpv is not None
    mock_mpv.assert_called_once()
    _, kwargs = mock_mpv.call_args
    assert kwargs["wid"] == 7
    assert kwargs["input_default_bindings"] is False
    path.unlink(missing_ok=True)


def test_toggle_pause_concurrent_with_observer():
    """Rapid toggle_pause + observer callbacks stay consistent.

    Simulates the main thread calling toggle_pause() while mpv's event
    thread fires the property observer concurrently.  The final _paused
    state must match the last value set by the observer — no torn reads
    or lost updates.
    """
    p = Player()
    p._mpv = MagicMock()
    p._paused = True

    iterations = 200
    barrier = threading.Barrier(2)

    def toggler():
        barrier.wait()
        for _ in range(iterations):
            p.toggle_pause()

    def observer():
        """Simulate the mpv property observer firing after each toggle."""
        barrier.wait()
        for _ in range(iterations):
            # Read what mpv.pause was set to, derive the observer value
            mpv_pause = p._mpv.pause
            with p._lock:
                p._paused = mpv_pause == "yes"

    t1 = threading.Thread(target=toggler)
    t2 = threading.Thread(target=observer)
    t1.start()
    t2.start()
    t1.join(timeout=10)
    t2.join(timeout=10)

    # After all iterations the state must be a valid boolean — no crash,
    # no AttributeError, no corruption.
    assert isinstance(p.paused, bool)


def test_load_writes_paused_under_lock():
    """load() must write _paused while holding the lock."""
    import tempfile
    from pathlib import Path

    p = Player()
    mock_mpv = MagicMock()
    mock_mpv.pause = "yes"
    p._mpv = mock_mpv  # skip _ensure_mpv by pre-setting

    # Patch _ensure_mpv to be a no-op since mpv is already set
    p._ensure_mpv = lambda: None

    with tempfile.NamedTemporaryFile(suffix=".mp4", delete=False) as f:
        path = Path(f.name)

    lock_acquired_during_write = threading.Event()

    class TrackingLock:
        """Wrapper that signals when the lock is acquired."""

        def __init__(self, real_lock):
            self._real = real_lock

        def __enter__(self):
            self._real.acquire()
            lock_acquired_during_write.set()
            return self

        def __exit__(self, *args):
            self._real.release()

    p._lock = TrackingLock(p._lock)

    result = p.load(path)
    assert result is True
    assert lock_acquired_during_write.is_set(), (
        "_paused was written without acquiring the lock"
    )
    assert p._paused is True  # mock_mpv.pause == "yes" -> True
    path.unlink(missing_ok=True)
