"""Tests for the player wrapper."""

from unittest.mock import MagicMock, patch

import pytest

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
    """seek calls mpv.command with the correct offset."""
    p = Player.__new__(Player)
    p._mpv = MagicMock()
    p.seek(10.0)
    p._mpv.command.assert_called_with("seek", 10.0, "relative")
