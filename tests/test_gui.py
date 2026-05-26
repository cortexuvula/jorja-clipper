"""Tests for GUI components."""

import os
import sys
from unittest.mock import MagicMock

import pytest

from jorja_clipper.gui.clip_list import ClipListModel

# Skip widget-creating tests in CI environments where Qt crashes
# (headless Linux, macOS segfaults, Windows event loop errors)
_needs_display = pytest.mark.skipif(
    os.environ.get("CI") == "true"
    or (sys.platform == "linux" and not os.environ.get("DISPLAY")),
    reason="Qt GUI tests skipped in CI or headless environment",
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
def test_titlebar_tear_off_from_maximized(qtbot):
    """Dragging the titlebar while maximized restores the window (tear-off).

    On Linux the app uses a frameless custom titlebar.  Native titlebars
    "tear off" in one gesture — drag from maximized immediately restores
    and follows the cursor.  Verify our _TitleBar does the same.
    """
    from PySide6.QtCore import QPoint, Qt
    from PySide6.QtGui import QMouseEvent
    from PySide6.QtWidgets import QMainWindow

    from jorja_clipper.gui.main_window import _TitleBar
    from jorja_clipper.gui.theme import ThemeManager

    window = QMainWindow()
    qtbot.addWidget(window)
    theme_manager = ThemeManager()
    title_bar = _TitleBar(window, theme_manager)
    window.setCentralWidget(title_bar)
    window.resize(800, 600)
    window.show()

    # Move to a known position so we can detect a move after tear-off.
    window.move(50, 50)
    qtbot.wait(50)
    normal_top_left = window.frameGeometry().topLeft()

    # Press at the centre of the title bar to begin a drag.
    press_local = QPoint(title_bar.width() // 2, title_bar.height() // 2)
    press_global = title_bar.mapToGlobal(press_local)
    press_event = QMouseEvent(
        QMouseEvent.Type.MouseButtonPress,
        press_local,
        press_global,
        Qt.MouseButton.LeftButton,
        Qt.MouseButton.LeftButton,
        Qt.KeyboardModifier.NoModifier,
    )
    title_bar.mousePressEvent(press_event)
    assert title_bar._drag_pos is not None

    # Maximise the window — simulates the user having double-clicked first.
    window.showMaximized()
    qtbot.wait(50)
    assert window.isMaximized()

    # Drag a little to the right — this must trigger the tear-off.
    move_local = QPoint(press_local.x() + 40, press_local.y())
    move_global = title_bar.mapToGlobal(move_local)
    move_event = QMouseEvent(
        QMouseEvent.Type.MouseMove,
        move_local,
        move_global,
        Qt.MouseButton.NoButton,
        Qt.MouseButton.LeftButton,
        Qt.KeyboardModifier.NoModifier,
    )
    title_bar.mouseMoveEvent(move_event)

    # The window must no longer be maximised and must have been moved.
    assert not window.isMaximized(), "Tear-off should restore the window"
    assert window.frameGeometry().topLeft() != normal_top_left, (
        "Tear-off should reposition the window under the cursor"
    )


@_needs_display
def test_video_widget_init(qtbot):
    """VideoWidget stores player reference."""
    from PySide6.QtWidgets import QWidget

    from jorja_clipper.gui.theme import ThemeManager
    from jorja_clipper.gui.video_widget import VideoWidget

    player = MagicMock()
    theme_manager = ThemeManager()
    parent = QWidget()
    qtbot.addWidget(parent)
    widget = VideoWidget(player, theme_manager, parent=parent)
    assert widget._player is player
    # On macOS, widget is QOpenGLWidget (render API); on Linux/Windows it's
    # QWidget (--wid embedding) with an _mpv_initialized flag.
    if sys.platform != "darwin":
        assert widget._mpv_initialized is False
