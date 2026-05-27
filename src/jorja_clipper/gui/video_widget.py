"""Embeddable video widget for mpv.

On macOS, uses mpv's render API via QOpenGLWidget to draw frames into a
Qt-managed OpenGL context. This sidesteps the broken --wid embedding that
modern mpv's Swift cocoa-cb backend no longer supports on macOS.

On Linux/Windows, uses the standard --wid embedding which still works on
those platforms (X11 Window ID / HWND).
"""

import contextlib
import ctypes
import logging
import sys
from ctypes import CFUNCTYPE, c_char_p, c_void_p
from typing import Any

from PySide6.QtCore import QObject, Qt, Signal
from PySide6.QtGui import QCloseEvent, QShowEvent
from PySide6.QtOpenGLWidgets import QOpenGLWidget
from PySide6.QtWidgets import QWidget

from jorja_clipper.gui.theme import ThemeManager

logger = logging.getLogger(__name__)

__all__ = ["VideoWidget"]

# On macOS we use the render API. On other platforms, --wid still works.
_USE_RENDER_API = sys.platform == "darwin"


# ---------------------------------------------------------------------------
# Signal bridge: mpv's update_cb fires on mpv's thread; we marshal to Qt's
# main thread via a signal so we can safely call QWidget.update().
# ---------------------------------------------------------------------------

class _UpdateBridge(QObject):
    """Emits a Qt signal when mpv wants a repaint."""

    needs_update = Signal()


# ---------------------------------------------------------------------------
# Render-API widget (macOS)
# ---------------------------------------------------------------------------

class _RenderVideoWidget(QOpenGLWidget):
    """QOpenGLWidget that hosts an mpv render context."""

    def __init__(
        self,
        player: Any,
        theme_manager: ThemeManager,
        parent: QWidget | None = None,
    ) -> None:
        super().__init__(parent)
        self._player = player
        self._theme_manager = theme_manager
        self._render_ctx: Any = None  # mpv.MpvRenderContext
        self._bridge = _UpdateBridge()
        self._bridge.needs_update.connect(self.update)
        self.setMinimumSize(800, 500)
        # Prevent Qt from painting over mpv's rendering
        self.setAttribute(Qt.WidgetAttribute.WA_OpaquePaintEvent, True)
        self.setAttribute(Qt.WidgetAttribute.WA_NoSystemBackground, True)
        logger.info("Created _RenderVideoWidget (macOS render-API path)")

    # -- OpenGL lifecycle ---------------------------------------------------

    def initializeGL(self) -> None:  # noqa: N802
        """Called once when the OpenGL context is ready. Create mpv here."""
        ctx = self.context()
        if ctx is None:
            logger.error("No QOpenGLContext available in initializeGL")
            return

        logger.info("OpenGL context ready — creating mpv render context")

        mpv_instance = self._player.mpv_handle
        if mpv_instance is None:
            logger.error("mpv instance not available")
            return

        # get_proc_address: mpv asks us for GL function pointers by name.
        # Signature: c_void_p callback(c_void_p ctx, c_char_p name)
        # Must be wrapped in the exact CFUNCTYPE python-mpv expects.
        def _get_proc_address_raw(_ctx: int, name: bytes) -> int:
            name_str = name.decode("utf-8") if isinstance(name, bytes) else name
            ptr = ctx.getProcAddress(name_str.encode("utf-8"))
            # Qt's getProcAddress returns a QByteArray or bytes
            if hasattr(ptr, "data"):
                return ctypes.cast(bytes(ptr.data()), ctypes.c_void_p).value or 0
            if isinstance(ptr, (bytes, bytearray)):
                return ctypes.cast(bytes(ptr), ctypes.c_void_p).value or 0
            return int(ptr) if ptr else 0

        _gl_get_proc_fn = CFUNCTYPE(c_void_p, c_void_p, c_char_p)
        _get_proc_address = _gl_get_proc_fn(_get_proc_address_raw)
        # Keep a strong reference to prevent GC of the CFUNCTYPE wrapper
        self._gl_get_proc_address = _get_proc_address

        try:
            import mpv as mpv_module
            self._render_ctx = mpv_module.MpvRenderContext(
                mpv_instance,
                "opengl",
                opengl_init_params={"get_proc_address": _get_proc_address},
            )
            # mpv calls this from its thread when it wants us to repaint.
            self._render_ctx.update_cb = self._on_mpv_update
            logger.info("mpv render context created successfully")
        except Exception:
            logger.exception("Failed to create mpv render context")

    def paintGL(self) -> None:  # noqa: N802
        """Called by Qt whenever the widget needs repainting."""
        if self._render_ctx is None:
            return
        try:
            # Check if there's a new frame to render
            if self._render_ctx.update():
                fbo = self.defaultFramebufferObject()
                w = int(self.width() * self.devicePixelRatio())
                h = int(self.height() * self.devicePixelRatio())
                self._render_ctx.render(
                    opengl_fbo={"fbo": fbo, "w": w, "h": h},
                    flip_y=True,  # Flip Y to match Qt's top-down coordinate system
                )
        except Exception:
            logger.exception("Error in mpv render")

    def resizeGL(self, w: int, h: int) -> None:  # noqa: N802
        """Called when the widget is resized."""
        logger.debug("VideoWidget resized to %dx%d (GL)", w, h)

    def _on_mpv_update(self) -> None:
        """Mpv callback — runs on mpv's thread. Signal bridges to Qt main."""
        self._bridge.needs_update.emit()

    # -- cleanup ------------------------------------------------------------

    def shutdown(self) -> None:
        """Free the mpv render context. MUST be called before mpv.terminate().

        mpv aborts if its handle is destroyed while a render context still
        exists, so MainWindow must call this before controller.shutdown().
        """
        if self._render_ctx is not None:
            with contextlib.suppress(Exception):
                self._bridge.needs_update.disconnect()
            with contextlib.suppress(Exception):
                self._render_ctx.update_cb = None
            try:
                self._render_ctx.free()
                logger.info("mpv render context freed")
            except Exception:
                logger.exception("Error freeing render context")
            self._render_ctx = None

    def closeEvent(self, event: QCloseEvent) -> None:  # noqa: N802
        self.shutdown()
        super().closeEvent(event)


# ---------------------------------------------------------------------------
# --wid-based widget (Linux / Windows)
# ---------------------------------------------------------------------------

class _WidVideoWidget(QWidget):
    """Plain QWidget that hands its native window handle to mpv."""

    def __init__(
        self,
        player: Any,
        theme_manager: ThemeManager,
        parent: QWidget | None = None,
    ) -> None:
        super().__init__(parent)
        self._player = player
        self._theme_manager = theme_manager
        self._mpv_initialized = False
        self.setObjectName("videoWidget")
        self.setMinimumSize(800, 500)
        # Force creation of a native window so winId() is valid for mpv.
        # Set here (not in showEvent) so the native handle is ready before
        # the widget is first shown — required on some Wayland compositors.
        self.setAttribute(Qt.WidgetAttribute.WA_NativeWindow)
        # Prevent Qt from painting over the embedded mpv child window.
        # The theme stylesheet sets background-color: transparent for
        # #videoWidget to avoid covering mpv's video frames during playback.
        self.setAttribute(Qt.WidgetAttribute.WA_NoSystemBackground)
        self.setAttribute(Qt.WidgetAttribute.WA_OpaquePaintEvent)

    def showEvent(self, event: QShowEvent) -> None:
        """Called when the widget is first shown; bind mpv here."""
        super().showEvent(event)
        if self._mpv_initialized:
            return
        self._mpv_initialized = True
        # winId() returns a platform-specific handle:
        #   Linux: X11 Window ID
        #   Windows: HWND
        wid = int(self.winId())
        logger.info("Binding mpv to wid=%s on platform=%s", wid, sys.platform)
        if wid:
            self._player.init_with_wid(wid)
        else:
            logger.warning(
                "winId() returned 0; mpv embedding may fail on this platform"
            )

    def shutdown(self) -> None:
        """No-op on wid-embedded platforms (Linux/Windows).

        Kept for API symmetry with _RenderVideoWidget so MainWindow can
        call video_widget.shutdown() unconditionally before mpv.terminate().
        """
        pass


# ---------------------------------------------------------------------------
# Public alias — choose the right implementation per platform
# ---------------------------------------------------------------------------

VideoWidget = _RenderVideoWidget if _USE_RENDER_API else _WidVideoWidget
