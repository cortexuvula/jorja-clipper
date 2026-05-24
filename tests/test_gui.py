"""Tests for GUI components."""

import os
import sys
from unittest.mock import MagicMock

import pytest

from jorja_clipper.gui.clip_list import ClipListModel

# Skip widget-creating tests on headless Linux CI (xvfb can crash with
# certain Qt widgets like QKeySequenceEdit).
_needs_display = pytest.mark.skipif(
    sys.platform == "linux" and not os.environ.get("DISPLAY"),
    reason="No display available (headless Linux CI)",
)


def test_clip_list_model_empty():
    """Model starts empty."""
    model = ClipListModel()
    assert model.rowCount() == 0


def test_clip_list_model_add_clip():
    """Adding a clip increases row count."""
    model = ClipListModel()
    model.add_clip("/tmp/test.mp4", 25.0, 35.0)
    assert model.rowCount() == 1


def test_clip_list_model_clip_data():
    """Model returns correct data for a clip."""
    model = ClipListModel()
    model.add_clip("/tmp/test.mp4", 25.0, 35.0)
    index = model.index(0, 0)
    assert "test" in model.data(index)


def test_clip_list_model_clip_at():
    """clip_at returns the correct ClipEntry."""
    model = ClipListModel()
    model.add_clip("/tmp/test.mp4", 25.0, 35.0)
    entry = model.clip_at(0)
    assert entry is not None
    assert entry.path == "/tmp/test.mp4"
    assert entry.start_time == 25.0
    assert model.clip_at(99) is None


def test_clip_list_model_remove_last():
    """remove_last pops the most recent clip and updates rowCount."""
    model = ClipListModel()
    model.add_clip("/tmp/a.mp4", 10.0, 20.0)
    model.add_clip("/tmp/b.mp4", 30.0, 40.0)
    entry = model.remove_last()
    assert entry is not None
    assert entry.path == "/tmp/b.mp4"
    assert model.rowCount() == 1
    assert model.clip_at(0).path == "/tmp/a.mp4"


def test_clip_list_model_remove_last_empty():
    """remove_last on an empty model returns None."""
    model = ClipListModel()
    assert model.remove_last() is None


@_needs_display
def test_settings_dialog_reads_settings(qtbot):
    """SettingsDialog populates fields from the passed Settings."""
    from jorja_clipper.gui.settings_dialog import SettingsDialog
    from jorja_clipper.settings import Settings

    settings = Settings(config_path=None)
    settings.buffer_before = 12.0
    settings.buffer_after = 3.0
    settings.clip_key = "X"
    dialog = SettingsDialog(settings)
    qtbot.addWidget(dialog)
    assert dialog._spin_before.value() == 12.0
    assert dialog._spin_after.value() == 3.0
    assert dialog._key_clip.keySequence().toString() == "X"


@_needs_display
def test_video_widget_init(qtbot):
    """VideoWidget stores player reference."""
    from PySide6.QtWidgets import QWidget

    from jorja_clipper.gui.video_widget import VideoWidget

    player = MagicMock()
    parent = QWidget()
    qtbot.addWidget(parent)
    widget = VideoWidget(player, parent=parent)
    assert widget._player is player
    assert widget._mpv_initialized is False
