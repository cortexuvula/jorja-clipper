"""Tests for MainWindow."""

import os
import sys
from unittest.mock import MagicMock

import pytest

from jorja_clipper.gui.clip_list import ClipListModel
from jorja_clipper.gui.theme import ThemeManager

# Skip widget-creating tests in CI environments where Qt crashes
_needs_display = pytest.mark.skipif(
    os.environ.get("CI") == "true"
    or (sys.platform == "linux" and not os.environ.get("DISPLAY")),
    reason="Qt GUI tests skipped in CI or headless environment",
)


def _make_window(qtbot, *, is_batch_running: bool = False):
    """Build a MainWindow with a fully mocked controller."""
    from jorja_clipper.gui.main_window import MainWindow

    player = MagicMock()
    clipper = MagicMock()
    settings = MagicMock()
    settings.clip_key = "C"
    settings.theme = "dark"
    model = ClipListModel()

    controller = MagicMock()
    controller.player = player
    controller.clipper = clipper
    controller.settings = settings
    controller.clip_model = model
    controller.is_batch_running = is_batch_running
    controller.batch_queue = MagicMock()
    controller.batch_queue.__len__ = MagicMock(return_value=0)
    controller.plugin_loader = MagicMock()

    theme_manager = ThemeManager()
    window = MainWindow(controller, theme_manager)
    qtbot.addWidget(window)
    return window, controller


@_needs_display
def test_queue_clip_shortcut_blocked_during_batch(qtbot):
    """Pressing Q (calling _on_queue_clip) does nothing while a batch runs."""
    window, controller = _make_window(qtbot, is_batch_running=True)

    window._on_queue_clip()

    controller.queue_clip.assert_not_called()


@_needs_display
def test_queue_clip_shortcut_works_when_idle(qtbot):
    """Pressing Q (calling _on_queue_clip) queues a clip when no batch runs."""
    window, controller = _make_window(qtbot, is_batch_running=False)
    controller.queue_clip.return_value = None  # success path

    window._on_queue_clip()

    controller.queue_clip.assert_called_once()
