"""Tests for the player wrapper."""

from unittest.mock import MagicMock, patch

from jorja_clipper.player import Player


def test_player_initial_state():
    """Player starts with no file loaded."""
    p = Player.__new__(Player)
    p._mpv = MagicMock()
    p._duration = 0.0
    p._current_pos = 0.0
    p._paused = True
    assert p.duration == 0.0
    assert p.current_pos == 0.0
    assert p.paused is True


def test_player_toggle_pause():
    """toggle_pause flips the paused state."""
    p = Player.__new__(Player)
    p._mpv = MagicMock()
    p._paused = True
    p.toggle_pause()
    assert p._paused is False
    assert p._mpv.pause == "no"
    p.toggle_pause()
    assert p._paused is True
    assert p._mpv.pause == "yes"


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
    p.seek(5.0)       # should not raise


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
