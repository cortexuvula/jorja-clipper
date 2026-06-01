"""Embeddable video widget for mpv using QOpenGLWidget."""

import ctypes.util
import locale
import sys
from pathlib import Path

# Patch ctypes.util.find_library for Homebrew paths
if sys.platform == "darwin":
    _original_find_library = ctypes.util.find_library

    def _patched_find_library(name):
        result = _original_find_library(name)
        if result is None and name == 'mpv':
            for lib_path in ["/opt/homebrew/lib/libmpv.dylib", "/usr/local/lib/libmpv.dylib"]:
                if Path(lib_path).exists():
                    return lib_path
        return result

    ctypes.util.find_library = _patched_find_library

import mpv
from PySide6.QtCore import Qt, Signal, Slot
from PySide6.QtOpenGLWidgets import QOpenGLWidget

# Load OpenGL framework and set up get_proc_address on macOS
_gl_lib = None
if sys.platform == "darwin":
    try:
        _gl_lib = ctypes.CDLL("/System/Library/Frameworks/OpenGL.framework/OpenGL")
    except OSError:
        pass

import ctypes

_GetProcAddressFn = ctypes.CFUNCTYPE(ctypes.c_void_p, ctypes.c_void_p, ctypes.c_char_p)


def _get_proc_address_impl(ctx, name):
    """Resolve OpenGL function addresses on macOS."""
    if _gl_lib is None or name is None:
        return 0
    try:
        func_name = name.decode('utf-8') if isinstance(name, bytes) else name
        func = getattr(_gl_lib, func_name, None)
        if func is not None:
            return ctypes.cast(func, ctypes.c_void_p).value or 0
    except Exception:
        pass
    return 0


_get_proc_address_fn = _GetProcAddressFn(_get_proc_address_impl)


class VideoWidget(QOpenGLWidget):
    """An OpenGL widget that embeds mpv video using the render API.

    On macOS, mpv's cocoa VO ignores wid and always opens its own window.
    We use vo=libmpv with an OpenGL render context backed by this widget's GL.

    Rendering happens on the GUI thread via paintGL() — mpv's update callback
    signals the main thread to schedule a repaint.
    """

    # Signal emitted from mpv's callback thread to trigger a repaint on the GUI thread
    _frame_ready = Signal()

    def __init__(self, player, parent=None):
        super().__init__(parent)
        self._player = player
        self._render_ctx = None
        self._mpv_initialized = False
        self.setMinimumSize(800, 500)
        self._frame_ready.connect(self.update)

    def showEvent(self, event):
        """Called when the widget is first shown; bind mpv here."""
        super().showEvent(event)
        if self._mpv_initialized:
            return
        self._mpv_initialized = True
        wid = int(self.winId())
        if wid:
            self._player.init_with_wid(wid, self)

    def initializeGL(self):
        """Called when the OpenGL context is ready — create mpv render context."""
        # Create the mpv instance with vo=libmpv
        saved_numeric = locale.setlocale(locale.LC_NUMERIC)
        locale.setlocale(locale.LC_NUMERIC, "C")
        try:
            self._player._mpv = mpv.MPV(
                input_default_bindings=False,
                input_vo_keyboard=False,
                osc=False,
                vo="libmpv",
                wid=int(self.winId()),
            )
        finally:
            locale.setlocale(locale.LC_NUMERIC, saved_numeric)

        # Create render context (GL context is current here)
        self._render_ctx = mpv.MpvRenderContext(
            self._player._mpv, 'opengl',
            opengl_init_params={'get_proc_address': _get_proc_address_fn},
        )

        # Set up update callback — mpv calls this from its own thread
        # when a new frame is available. We emit a signal to trigger
        # a repaint on the GUI thread.
        self._render_ctx.update_cb = self._on_mpv_update

        # Set up property observers
        @self._player._mpv.property_observer("duration")
        def _on_duration(_name, value):
            if value is not None:
                self._player._duration = float(value)

        @self._player._mpv.property_observer("time-pos")
        def _on_time_pos(_name, value):
            if value is not None:
                self._player._current_pos = float(value)

        self._player._render_ctx = self._render_ctx

    def _on_mpv_update(self):
        """Called from mpv's thread when a new frame is available."""
        # Emit signal to trigger repaint on GUI thread
        self._frame_ready.emit()

    def paintGL(self):
        """Called on the GUI thread when the widget needs to repaint."""
        if self._render_ctx is None:
            return
        try:
            if self._render_ctx.update():
                fbo_info = self.context().defaultFramebufferObject()
                w = self.width() * self.devicePixelRatio()
                h = self.height() * self.devicePixelRatio()
                self._render_ctx.render(
                    opengl_fbo={'fbo': fbo_info, 'w': int(w), 'h': int(h)},
                    flip_y=True,
                    block_for_target_time=False,
                )
        except Exception:
            pass

    def cleanup(self):
        """Clean up mpv render context."""
        if self._render_ctx is not None:
            self._render_ctx.update_cb = None
            self._render_ctx.free()
            self._render_ctx = None
