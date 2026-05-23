"""Tests for GUI components."""

from unittest.mock import MagicMock

from PySide6.QtWidgets import QWidget

from jorja_clipper.gui.clip_list import ClipListModel
from jorja_clipper.gui.settings_dialog import SettingsDialog
from jorja_clipper.settings import Settings


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


def test_settings_dialog_reads_settings(qtbot):
    """SettingsDialog populates fields from the passed Settings."""
    settings = Settings(config_path=None)
    settings.buffer_before = 12.0
    settings.buffer_after = 3.0
    settings.clip_key = "X"
    dialog = SettingsDialog(settings)
    qtbot.addWidget(dialog)
    assert dialog._spin_before.value() == 12.0
    assert dialog._spin_after.value() == 3.0
    assert dialog._key_clip.keySequence().toString() == "X"


def test_video_widget_init(qtbot):
    """VideoWidget stores player reference."""
    from jorja_clipper.gui.video_widget import VideoWidget

    player = MagicMock()
    parent = QWidget()
    qtbot.addWidget(parent)
    widget = VideoWidget(player, parent=parent)
    assert widget._player is player
    assert widget._mpv_initialized is False
