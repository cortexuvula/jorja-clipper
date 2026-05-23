"""Main application window."""

from pathlib import Path

from PySide6.QtCore import Qt
from PySide6.QtGui import QKeySequence, QShortcut
from PySide6.QtWidgets import (
    QFileDialog,
    QHBoxLayout,
    QLabel,
    QListWidget,
    QMainWindow,
    QPushButton,
    QSplitter,
    QVBoxLayout,
    QWidget,
)

from jorja_clipper.clipper import Clipper
from jorja_clipper.gui.clip_list import ClipListModel


class MainWindow(QMainWindow):
    """Main Jorja Clipper window."""

    def __init__(self, player, clipper: Clipper):
        super().__init__()
        self._player = player
        self._clipper = clipper
        self._clip_count = 0
        self._current_video = None

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

        # Video widget placeholder (mpv will render here)
        self._video_container = QWidget()
        self._video_container.setMinimumSize(800, 500)
        self._video_container.setStyleSheet("background-color: #1a1a2e;")
        left_layout.addWidget(self._video_container)

        # Status bar
        self._status = QLabel("No video loaded \u2014 press O to open")
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

        left_layout.addLayout(controls)
        splitter.addWidget(left)

        # Right side: clip list
        right = QWidget()
        right_layout = QVBoxLayout(right)

        right_layout.addWidget(QLabel("Saved Clips"))
        self._clip_list = QListWidget()
        self._clip_model = ClipListModel()
        right_layout.addWidget(self._clip_list)

        splitter.addWidget(right)
        splitter.setSizes([900, 300])

    def _setup_shortcuts(self):
        """Set up keyboard shortcuts."""
        QShortcut(QKeySequence("C"), self, self._save_clip)
        QShortcut(QKeySequence("Space"), self, self._toggle_play)
        QShortcut(QKeySequence("O"), self, self._open_file)
        QShortcut(QKeySequence("Left"), self, lambda: self._player.seek(-5.0))
        QShortcut(QKeySequence("Right"), self, lambda: self._player.seek(5.0))
        QShortcut(QKeySequence("Shift+Left"), self, lambda: self._player.seek(-1.0))
        QShortcut(QKeySequence("Shift+Right"), self, lambda: self._player.seek(1.0))
        QShortcut(QKeySequence("Q"), self, self.close)

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
            self._player.load(self._current_video)
            self._status.setText(f"Loaded: {self._current_video.name}")
            self.setWindowTitle(f"Jorja Clipper \u2014 {self._current_video.name}")

    def _toggle_play(self):
        """Toggle play/pause."""
        self._player.toggle_pause()

    def _save_clip(self):
        """Save a clip at the current position."""
        if self._current_video is None:
            self._status.setText("No video loaded!")
            return

        self._clip_count += 1
        result = self._clipper.save_clip(
            video_path=self._current_video,
            current_pos=self._player.current_pos,
            video_duration=self._player.duration,
            clip_number=self._clip_count,
        )

        if result.success:
            name = Path(result.path).name
            self._status.setText(f"Clip saved: {name}")
            self._clip_model.add_clip(result.path, result.start_time, result.end_time)
            self._clip_list.addItem(
                f"{name}  [{result.start_time:.1f}s - {result.end_time:.1f}s]"
            )
        else:
            self._status.setText(f"Clip failed: {result.error[:80]}")
