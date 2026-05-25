"""Main application window."""

import sys
from pathlib import Path

from PySide6.QtCore import QModelIndex, Qt, QUrl
from PySide6.QtGui import QCloseEvent, QDesktopServices, QKeySequence, QShortcut
from PySide6.QtWidgets import (
    QFileDialog,
    QHBoxLayout,
    QLabel,
    QListView,
    QMainWindow,
    QProgressBar,
    QPushButton,
    QSplitter,
    QVBoxLayout,
    QWidget,
)

from jorja_clipper.batch_queue import BatchWorker
from jorja_clipper.clipper import ClipResult
from jorja_clipper.controller import ClipController
from jorja_clipper.gui.theme import ThemeManager
from jorja_clipper.worker import ClipWorker

__all__ = ["MainWindow"]


def _remove_wm_decorations(window: QMainWindow) -> None:
    """On Linux/X11, tell the WM to skip its own title bar (SSD).

    Qt already draws client-side decorations (CSD), so the WM's frame
    is redundant and causes the double-title-bar bug on GNOME/KDE.
    Uses the _MOTIF_WM_HINTS X11 property which most WMs honour.
    On Wayland this is a safe no-op — XOpenDisplay returns None.
    """
    if sys.platform != "linux":
        return
    try:
        import ctypes
        import ctypes.util

        xlib = ctypes.CDLL(ctypes.util.find_library("X11"))
        display = xlib.XOpenDisplay(None)
        if display is None:
            return  # Wayland or no X11 — nothing to do
        w_id = int(window.winId())
        # Motif WmHints: flags, functions, decorations, input_mode, status
        # flags=2 (MWM_HINT_DECORATIONS), decorations=0 → no WM decorations
        hints = (ctypes.c_ulong * 5)(2, 0, 0, 0, 0)
        atom = xlib.XInternAtom(display, b"_MOTIF_WM_HINTS", False)
        xlib.XChangeProperty(
            display, w_id, atom, atom, 32, 0, ctypes.cast(hints, ctypes.c_char_p), 5
        )
        xlib.XFlush(display)
        xlib.XCloseDisplay(display)
    except Exception:
        pass  # Worst case: no fix, still usable


class MainWindow(QMainWindow):
    """Main Jorja Clipper window."""

    def __init__(self, controller: ClipController, theme_manager: ThemeManager) -> None:
        super().__init__()
        self._controller = controller
        self._theme_manager = theme_manager
        self._shortcuts: list[QShortcut] = []

        self.setWindowTitle("Jorja Clipper")
        self.setMinimumSize(1200, 700)
        _remove_wm_decorations(self)
        self._apply_theme()

        self._setup_ui()
        self._setup_shortcuts()

    # ------------------------------------------------------------------
    # Theme helpers
    # ------------------------------------------------------------------

    def _apply_theme(self) -> None:
        self.setStyleSheet(self._theme_manager.stylesheet())

    def _rebuild_theme(self) -> None:
        self._theme_manager.theme_name = self._controller.settings.theme
        self._apply_theme()
        # Re-apply video widget background
        t = self._theme_manager.theme
        self._video_widget.setStyleSheet(f"background-color: {t.video_bg};")

    # ------------------------------------------------------------------
    # UI construction
    # ------------------------------------------------------------------

    def _setup_ui(self) -> None:
        """Build the UI layout."""
        from jorja_clipper.gui.video_widget import VideoWidget

        central = QWidget()
        self.setCentralWidget(central)
        layout = QHBoxLayout(central)

        # Splitter: video on left, clip list on right
        splitter = QSplitter(Qt.Orientation.Horizontal)
        layout.addWidget(splitter)

        # Left side: video area + controls
        left = QWidget()
        left_layout = QVBoxLayout(left)

        # Video widget (mpv renders here)
        self._video_widget = VideoWidget(
            self._controller.player, self._theme_manager, self
        )
        left_layout.addWidget(self._video_widget)

        # Status bar
        self._status = QLabel("No video loaded — press O to open")
        self._status.setObjectName("statusLabel")
        self._status.setAlignment(Qt.AlignmentFlag.AlignCenter)
        left_layout.addWidget(self._status)

        # Batch progress
        self._batch_progress = QProgressBar()
        self._batch_progress.setRange(0, 100)
        self._batch_progress.setValue(0)
        self._batch_progress.setTextVisible(True)
        self._batch_progress.setVisible(False)
        left_layout.addWidget(self._batch_progress)

        # Controls
        controls = QHBoxLayout()

        self._btn_open = QPushButton("Open (O)")
        self._btn_open.clicked.connect(self._open_file_dialog)
        controls.addWidget(self._btn_open)

        self._btn_play = QPushButton("Play/Pause (Space)")
        self._btn_play.clicked.connect(self._controller.toggle_play)
        controls.addWidget(self._btn_play)

        self._btn_clip = QPushButton("Clip (C)")
        self._btn_clip.setObjectName("clipButton")
        self._btn_clip.clicked.connect(self._on_clip_requested)
        controls.addWidget(self._btn_clip)

        self._btn_queue = QPushButton("Queue Clip (Q)")
        self._btn_queue.clicked.connect(self._on_queue_clip)
        controls.addWidget(self._btn_queue)

        self._btn_process = QPushButton("Process Batch")
        self._btn_process.clicked.connect(self._on_process_batch)
        self._btn_process.setEnabled(False)
        controls.addWidget(self._btn_process)

        self._btn_settings = QPushButton("Settings")
        self._btn_settings.clicked.connect(self._open_settings)
        controls.addWidget(self._btn_settings)

        self._btn_undo = QPushButton("Undo (U)")
        self._btn_undo.clicked.connect(self._on_undo_requested)
        self._btn_undo.setEnabled(False)
        controls.addWidget(self._btn_undo)

        left_layout.addLayout(controls)
        splitter.addWidget(left)

        # Right side: clip list
        right = QWidget()
        right_layout = QVBoxLayout(right)

        right_layout.addWidget(QLabel("Saved Clips"))
        self._clip_list = QListView()
        self._clip_list.setModel(self._controller.clip_model)
        self._clip_list.doubleClicked.connect(self._preview_clip)
        right_layout.addWidget(self._clip_list)

        splitter.addWidget(right)
        splitter.setSizes([900, 300])

    # ------------------------------------------------------------------
    # Shortcuts
    # ------------------------------------------------------------------

    def _setup_shortcuts(self) -> None:
        """Set up keyboard shortcuts."""
        self._shortcuts.clear()
        self._shortcuts.append(
            QShortcut(
                QKeySequence(self._controller.settings.clip_key),
                self,
                self._on_clip_requested,
            )
        )
        self._shortcuts.append(
            QShortcut(QKeySequence("Space"), self, self._controller.toggle_play)
        )
        self._shortcuts.append(
            QShortcut(QKeySequence("O"), self, self._open_file_dialog)
        )
        self._shortcuts.append(
            QShortcut(QKeySequence("Left"), self, lambda: self._controller.seek(-5.0))
        )
        self._shortcuts.append(
            QShortcut(QKeySequence("Right"), self, lambda: self._controller.seek(5.0))
        )
        self._shortcuts.append(
            QShortcut(
                QKeySequence("Shift+Left"),
                self,
                lambda: self._controller.seek(-1.0),
            )
        )
        self._shortcuts.append(
            QShortcut(
                QKeySequence("Shift+Right"),
                self,
                lambda: self._controller.seek(1.0),
            )
        )
        self._shortcuts.append(
            QShortcut(QKeySequence("Q"), self, self._on_queue_clip)
        )
        self._shortcuts.append(
            QShortcut(QKeySequence("U"), self, self._on_undo_requested)
        )

    def update_shortcuts(self) -> None:
        """Recreate keyboard shortcuts after settings change."""
        for sc in self._shortcuts:
            sc.setEnabled(False)
            sc.deleteLater()
        self._shortcuts.clear()
        self._setup_shortcuts()

    # ------------------------------------------------------------------
    # Status / title helpers
    # ------------------------------------------------------------------

    def load_video(self, video_path: Path) -> None:
        """Notify UI that a video is loaded."""
        self._status.setText(f"Loaded: {video_path.name}")
        self.setWindowTitle(f"Jorja Clipper — {video_path.name}")

    def set_status(self, message: str) -> None:
        """Update the status bar label."""
        self._status.setText(message)

    # ------------------------------------------------------------------
    # Actions
    # ------------------------------------------------------------------

    def _open_file_dialog(self) -> None:
        """Open a video file dialog."""
        path, _ = QFileDialog.getOpenFileName(
            self,
            "Open Video",
            "",
            "Video Files (*.mp4 *.mkv *.avi *.mov *.webm *.ts);;All Files (*)",
        )
        if path:
            video_path = Path(path)
            if self._controller.open_file(video_path):
                self.load_video(video_path)
            else:
                self.set_status(f"Failed to load: {video_path.name}")

    def _on_clip_requested(self) -> None:
        """Handle clip save request from button or shortcut."""
        result = self._controller.save_clip()
        if not isinstance(result, ClipWorker):
            # Immediate rejection (no video loaded or already clipping)
            self.set_status(f"Clip failed: {result.error[:80]}")
            return

        # Disable the clip button and show busy state
        self._btn_clip.setEnabled(False)
        self.set_status("Clipping…")
        result.finished.connect(self._on_clip_finished)

    def _on_clip_finished(self, result: object) -> None:
        """Update UI after the background clip worker finishes."""
        self._btn_clip.setEnabled(True)
        # result is a ClipResult, but Signal passes it as object
        if getattr(result, "success", False):
            name = Path(getattr(result, "path", "")).name
            self.set_status(f"Clip saved: {name}")
            self._btn_undo.setEnabled(True)
        else:
            error = getattr(result, "error", "unknown error")
            self.set_status(f"Clip failed: {error[:80]}")

    # ------------------------------------------------------------------
    # Batch queue actions
    # ------------------------------------------------------------------

    def _on_queue_clip(self) -> None:
        """Add the current position to the batch queue."""
        err = self._controller.queue_clip()
        if isinstance(err, object) and getattr(err, "success", True) is False:
            self.set_status(f"Queue failed: {getattr(err, 'error', 'unknown')[:80]}")
            return
        total = len(self._controller.batch_queue)
        self.set_status(f"Queued clip at current position ({total} in queue)")
        self._btn_process.setEnabled(total > 0)

    def _on_process_batch(self) -> None:
        """Start processing the batch queue."""
        total = len(self._controller.batch_queue)
        if total == 0:
            self.set_status("Batch queue is empty.")
            return

        worker_or_err = self._controller.process_batch()
        # When process_batch returns a ClipResult it means the batch was rejected
        if isinstance(worker_or_err, ClipResult) and not worker_or_err.success:
            self.set_status(f"Batch failed: {worker_or_err.error[:80]}")
            return

        # It is a BatchWorker (QThread subclass)
        if not isinstance(worker_or_err, BatchWorker):
            self.set_status("Batch failed: unexpected result from process_batch")
            return
        worker = worker_or_err
        self._batch_progress.setRange(0, total)
        self._batch_progress.setValue(0)
        self._batch_progress.setVisible(True)
        self._btn_process.setEnabled(False)
        self._btn_queue.setEnabled(False)
        self.set_status("Processing batch…")

        worker.progress.connect(self._on_batch_progress)
        worker.finished.connect(self._on_batch_finished)

    def _on_batch_progress(self, completed: int, total: int, result: object) -> None:
        """Update progress bar during batch processing."""
        self._batch_progress.setValue(completed)
        self._batch_progress.setFormat(f"Clipping {completed}/{total}…")
        if getattr(result, "success", False):
            name = Path(getattr(result, "path", "")).name
            self.set_status(f"Batch {completed}/{total}: saved {name}")
            self._btn_undo.setEnabled(True)
        else:
            error = getattr(result, "error", "unknown error")
            self.set_status(f"Batch {completed}/{total}: failed {error[:60]}")

    def _on_batch_finished(self, results: list[object]) -> None:
        """Hide progress bar and re-enable controls after batch finishes."""
        self._batch_progress.setVisible(False)
        self._batch_progress.setValue(0)
        self._btn_process.setEnabled(False)
        self._btn_queue.setEnabled(True)
        success_count = sum(1 for r in results if getattr(r, "success", False))
        total = len(results)
        self.set_status(f"Batch complete — {success_count}/{total} clips saved.")

    def _on_undo_requested(self) -> None:
        """Undo the last clip and update UI state."""
        undone = self._controller.undo_last_clip()
        if undone:
            self.set_status("Last clip undone.")
            self._btn_undo.setEnabled(False)
        else:
            self.set_status("Nothing to undo.")

    def _open_settings(self) -> None:
        """Open the settings dialog."""
        from jorja_clipper.gui.settings_dialog import SettingsDialog

        dialog = SettingsDialog(self._controller.settings, self)
        if dialog.exec() == SettingsDialog.DialogCode.Accepted:
            self._controller.apply_settings()
            self.update_shortcuts()
            self._rebuild_theme()
            self.set_status(
                f"Settings saved: before={self._controller.settings.buffer_before}s, "
                f"after={self._controller.settings.buffer_after}s, "
                f"key={self._controller.settings.clip_key}"
            )

    def _preview_clip(self, index: QModelIndex) -> None:
        """Open the clip with the system default player on double-click."""
        clip = self._controller.clip_model.clip_at(index.row())
        if clip is not None and Path(clip.path).exists():
            QDesktopServices.openUrl(QUrl.fromLocalFile(str(clip.path)))
        else:
            self.set_status("Clip file not found.")

    def closeEvent(self, event: QCloseEvent) -> None:  # noqa: N802
        """Shut down the player on window close."""
        self._controller.shutdown()
        event.accept()
