"""Main application window."""

from pathlib import Path

from PySide6.QtCore import Qt, QUrl
from PySide6.QtGui import QDesktopServices, QKeySequence, QShortcut
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

from jorja_clipper.clipper import Clipper
from jorja_clipper.gui.clip_list import ClipListModel
from jorja_clipper.gui.settings_dialog import SettingsDialog
from jorja_clipper.gui.video_widget import VideoWidget
from jorja_clipper.settings import Settings


class MainWindow(QMainWindow):
    """Main Jorja Clipper window."""

    def __init__(self, player, clipper: Clipper, settings: Settings):
        super().__init__()
        self._player = player
        self._clipper = clipper
        self._settings = settings
        self._clip_count = 0
        self._current_video = None
        self._shortcuts: list[QShortcut] = []

        self.setWindowTitle("Jorja Clipper")
        self.setMinimumSize(1200, 700)

        self._setup_ui()
        self._setup_shortcuts()

    def _setup_ui(self):
        """Build the UI layout."""
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
        self._video_widget = VideoWidget(self._player, self)
        left_layout.addWidget(self._video_widget)

        # Status bar
        self._status = QLabel("No video loaded — press O to open")
        self._status.setAlignment(Qt.AlignmentFlag.AlignCenter)
        self._status.setStyleSheet("color: #888; padding: 8px;")
        left_layout.addWidget(self._status)

        # Controls
        controls = QHBoxLayout()

        self._btn_open = QPushButton("Open (O)")
        self._btn_open.clicked.connect(self._open_file)
        controls.addWidget(self._btn_open)

        self._btn_play = QPushButton("Play/Pause (Space)")
        self._btn_play.clicked.connect(self._toggle_play)
        controls.addWidget(self._btn_play)

        self._btn_clip = QPushButton("Clip (C)")
        self._btn_clip.setStyleSheet(
            "QPushButton { background-color: #e94560; color: white; "
            "font-weight: bold; padding: 10px; border-radius: 5px; }"
            "QPushButton:hover { background-color: #c73e54; }"
        )
        self._btn_clip.clicked.connect(self._save_clip)
        controls.addWidget(self._btn_clip)

        self._btn_settings = QPushButton("Settings")
        self._btn_settings.clicked.connect(self._open_settings)
        controls.addWidget(self._btn_settings)

        left_layout.addLayout(controls)
        splitter.addWidget(left)

        # Right side: clip list
        right = QWidget()
        right_layout = QVBoxLayout(right)

        right_layout.addWidget(QLabel("Saved Clips"))
        self._clip_list = QListView()
        self._clip_model = ClipListModel()
        self._clip_list.setModel(self._clip_model)
        self._clip_list.doubleClicked.connect(self._preview_clip)
        right_layout.addWidget(self._clip_list)

        splitter.addWidget(right)
        splitter.setSizes([900, 300])

    def _setup_shortcuts(self):
        """Set up keyboard shortcuts."""
        self._shortcuts.clear()
        self._shortcuts.append(QShortcut(QKeySequence(self._settings.clip_key), self, self._save_clip))
        self._shortcuts.append(QShortcut(QKeySequence("Space"), self, self._toggle_play))
        self._shortcuts.append(QShortcut(QKeySequence("O"), self, self._open_file))
        self._shortcuts.append(QShortcut(QKeySequence("Left"), self, lambda: self._player.seek(-5.0)))
        self._shortcuts.append(QShortcut(QKeySequence("Right"), self, lambda: self._player.seek(5.0)))
        self._shortcuts.append(QShortcut(QKeySequence("Shift+Left"), self, lambda: self._player.seek(-1.0)))
        self._shortcuts.append(QShortcut(QKeySequence("Shift+Right"), self, lambda: self._player.seek(1.0)))
        self._shortcuts.append(QShortcut(QKeySequence("Q"), self, self.close))

    def update_shortcuts(self):
        """Recreate keyboard shortcuts after settings change."""
        for sc in self._shortcuts:
            sc.setEnabled(False)
            sc.deleteLater()
        self._shortcuts.clear()
        self._setup_shortcuts()
    def load_video(self, video_path: Path) -> None:
        """Load a video into the player and update UI."""
        self._current_video = video_path
        self._status.setText(f"Loaded: {video_path.name}")
        self.setWindowTitle(f"Jorja Clipper — {video_path.name}")

    def set_status(self, message: str) -> None:
        """Update the status bar label."""
        self._status.setText(message)

    def _open_file(self):
        """Open a video file dialog."""
        path, _ = QFileDialog.getOpenFileName(
            self,
            "Open Video",
            "",
            "Video Files (*.mp4 *.mkv *.avi *.mov *.webm *.ts);;All Files (*)",
        )
        if path:
            self._current_video = Path(path)
            if self._player.load(self._current_video):
                self._status.setText(f"Loaded: {self._current_video.name}")
                self.setWindowTitle(f"Jorja Clipper — {self._current_video.name}")
            else:
                self._status.setText(f"Failed to load: {self._current_video.name}")

    def _toggle_play(self):
        """Toggle play/pause."""
        self._player.toggle_pause()

    def _save_clip(self):
        """Save a clip at the current position."""
        if self._current_video is None:
            self._status.setText("No video loaded!")
            return

        result = self._clipper.save_clip(
            video_path=self._current_video,
            current_pos=self._player.current_pos,
            video_duration=self._player.duration,
            clip_number=self._clip_count + 1,
        )

        if result.success:
            self._clip_count += 1
            name = Path(result.path).name
            self._status.setText(f"Clip saved: {name}")
            self._clip_model.add_clip(result.path, result.start_time, result.end_time)
        else:
            self._status.setText(f"Clip failed: {result.error[:80]}")

    def _open_settings(self):
        """Open the settings dialog."""
        dialog = SettingsDialog(self._settings, self)
        if dialog.exec() == SettingsDialog.DialogCode.Accepted:
            # Propagate updated buffer values to clipper
            self._clipper.buffer_before = self._settings.buffer_before
            self._clipper.buffer_after = self._settings.buffer_after
            self.update_shortcuts()
            self._status.setText(
                f"Settings saved: before={self._settings.buffer_before}s, "
                f"after={self._settings.buffer_after}s, key={self._settings.clip_key}"
            )

    def _preview_clip(self, index):
        """Open the clip with the system default player on double-click."""
        clip = self._clip_model.clip_at(index.row())
        if clip is not None and Path(clip.path).exists():
            QDesktopServices.openUrl(QUrl.fromLocalFile(str(clip.path)))
        else:
            self._status.setText("Clip file not found.")

    def closeEvent(self, event):
        """Shut down the player on window close."""
        self._player.shutdown()
        event.accept()
