"""Embeddable video widget for mpv."""

import logging
import sys
from typing import Any

from PySide6.QtCore import Qt
from PySide6.QtGui import QShowEvent
from PySide6.QtWidgets import QWidget

logger = logging.getLogger(__name__)


class VideoWidget(QWidget):
    """A native widget that provides its window handle to mpv."""

    def __init__(self, player: Any, parent: QWidget | None = None) -> None:
        super().__init__(parent)
        self._player = player
        self._mpv_initialized = False
        self.setMinimumSize(800, 500)
        self.setStyleSheet("background-color: #1a1a2e;")

    def showEvent(self, event: QShowEvent) -> None:
        """Called when the widget is first shown; bind mpv here."""
        super().showEvent(event)
        if self._mpv_initialized:
            return
        # Force creation of a native window so winId() is valid for mpv.
        self.setAttribute(Qt.WidgetAttribute.WA_NativeWindow)
        self._mpv_initialized = True
        # winId() returns a platform-specific handle:
        #   Linux: X11 Window ID
        #   macOS: NSView / Window pointer
        #   Windows: HWND
        wid = int(self.winId())
        logger.info("Binding mpv to wid=%s on platform=%s", wid, sys.platform)
        if wid:
            self._player.init_with_wid(wid)
        else:
            logger.warning(
                "winId() returned 0; mpv embedding may fail on this platform"
            )
