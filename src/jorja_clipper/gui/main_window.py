"""Main application window."""

from pathlib import Path

from PySide6.QtCore import QModelIndex, Qt, QUrl
from PySide6.QtGui import QCloseEvent, QDesktopServices, QKeySequence, QShortcut
from PySide6.QtWidgets import (
    QFileDialog,
    QHBoxLayout,
    QLabel,
    QListView,
    QMainWindow,
    QPushButton,
    QSplitter,
    QVBoxLayout,
    QWidget,
)

from jorja_clipper.controller import ClipController
from jorja_clipper.worker import ClipWorker

__all__ = ["MainWindow"]


class MainWindow(QMainWindow):
    """Main Jorja Clipper window."""

    def __init__(self, controller: ClipController) -> None:
        super().__init__()
        self._controller = controller
        self._shortcuts: list[QShortcut] = []

        self.setWindowTitle("Jorja Clipper")
        self.setMinimumSize(1200, 700)

        self._setup_ui()
        self._setup_shortcuts()

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
        self._video_widget = VideoWidget(self._controller.player, self)
        left_layout.addWidget(self._video_widget)

        # Status bar
        self._status = QLabel("No video loaded — press O to open")
        self._status.setAlignment(Qt.AlignmentFlag.AlignCenter)
        self._status.setStyleSheet("color: #888; padding: 8px;")
        left_layout.addWidget(self._status)

        # Controls
        controls = QHBoxLayout()

        self._btn_open = QPushButton("Open (O)")
        self._btn_open.clicked.connect(self._open_file_dialog)
        controls.addWidget(self._btn_open)

        self._btn_play = QPushButton("Play/Pause (Space)")
        self._btn_play.clicked.connect(self._controller.toggle_play)
        controls.addWidget(self._btn_play)

        self._btn_clip = QPushButton("Clip (C)")
        self._btn_clip.setStyleSheet(
            "QPushButton { background-color: #e94560; color: white; "
            "font-weight: bold; padding: 10px; border-radius: 5px; }"
            "QPushButton:hover { background-color: #c73e54; }"
            "QPushButton:disabled { background-color: #555; color: #aaa; }"
        )
        self._btn_clip.clicked.connect(self._on_clip_requested)
        controls.addWidget(self._btn_clip)

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
            QShortcut(QKeySequence("Q"), self, self.close)
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
