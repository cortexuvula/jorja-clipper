# Jorja Clipper — Implementation Plan

> **For Hermes:** Use subagent-driven-development skill to implement this plan task-by-task.

**Goal:** Cross-platform (Linux/Windows/macOS) desktop app where you play a video, press a key when a highlight happens, and it instantly saves a clip (configurable seconds before + after).

**Architecture:** Python + PySide6 (Qt6) GUI wrapping libmpv for video playback. ffmpeg extracts clips via stream-copy (no re-encoding = instant). Single-window app with video player, transport controls, and a clip list sidebar.

**Tech Stack:** Python 3.10+, PySide6, python-mpv (ctypes wrapper for libmpv), ffmpeg (bundled or system), PyInstaller/briefcase for packaging.

---

## Task 1: Project scaffold + pyproject.toml

**Objective:** Create the project skeleton with build config, dependencies, and directory structure.

**Files:**
- Create: `pyproject.toml`
- Create: `README.md`
- Create: `LICENSE` (MIT)
- Create: `.gitignore`
- Create: `src/jorja_clipper/__init__.py`
- Create: `src/jorja_clipper/__main__.py`
- Create: `src/jorja_clipper/app.py`
- Create: `tests/__init__.py`
- Create: `.github/workflows/ci.yml`

**Steps:**

- [ ] **Step 1:** Create `pyproject.toml`:

```toml
[build-system]
requires = ["hatchling"]
build-backend = "hatchling.build"

[project]
name = "jorja-clipper"
version = "0.1.0"
description = "One-key video clipper for sports highlights"
readme = "README.md"
license = "MIT"
requires-python = ">=3.10"
dependencies = [
    "PySide6>=6.6",
    "python-mpv>=0.5.2",
]

[project.optional-dependencies]
dev = [
    "pytest>=7.0",
    "pytest-qt>=4.0",
    "ruff>=0.4",
]

[project.scripts]
jorja-clipper = "jorja_clipper.app:main"

[tool.hatch.build.targets.wheel]
packages = ["src/jorja_clipper"]

[tool.ruff]
line-length = 100
target-version = "py310"

[tool.ruff.lint]
select = ["E", "F", "I", "N", "W", "UP"]

[tool.pytest.ini_options]
testpaths = ["tests"]
```

- [ ] **Step 2:** Create `README.md`:

```markdown
# 🏀 Jorja Clipper

One-key video clipper for sports highlights.

Watch a game video. Press **C** when something awesome happens. Get a clip saved instantly — no re-encoding, no waiting.

## Features

- Play any video format (mpv backend)
- Press **C** to save a ±5 second clip (configurable)
- Instant save via ffmpeg stream-copy (no re-encoding)
- Clip list sidebar to review and play saved clips
- Cross-platform: Linux, macOS, Windows

## Install

```bash
pip install .
```

## Usage

```bash
jorja-clipper path/to/game-video.mp4
```

### Keyboard Shortcuts

| Key | Action |
|-----|--------|
| Space | Play / Pause |
| Left / Right | Seek ±5s |
| Shift+Left / Shift+Right | Seek ±1s |
| C | Save clip (±5s around current position) |
| O | Open file |
| Q | Quit |

## Build from Source

```bash
pip install -e ".[dev]"
pytest
```

## License

MIT
```

- [ ] **Step 3:** Create `.gitignore`:

```
__pycache__/
*.pyc
*.egg-info/
dist/
build/
.eggs/
*.egg
.venv/
venv/
.mypy_cache/
.ruff_cache/
.pytest_cache/
clips/
```

- [ ] **Step 4:** Create `LICENSE` (MIT, copyright Andre Hugo 2026)

- [ ] **Step 5:** Create package structure:

```python
# src/jorja_clipper/__init__.py
"""Jorja Clipper — One-key video clipper for sports highlights."""

__version__ = "0.1.0"
```

```python
# src/jorja_clipper/__main__.py
"""Allow running as `python -m jorja_clipper`."""

from jorja_clipper.app import main

main()
```

```python
# src/jorja_clipper/app.py
"""Main application entry point."""

import sys


def main():
    """Launch Jorja Clipper."""
    print("Jorja Clipper — not yet implemented")
    sys.exit(0)
```

```python
# tests/__init__.py
```

- [ ] **Step 6:** Create `.github/workflows/ci.yml`:

```yaml
name: CI

on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

jobs:
  lint:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-python@v5
        with:
          python-version: "3.12"
      - run: pip install ruff
      - run: ruff check src/ tests/
      - run: ruff format --check src/ tests/

  test:
    runs-on: ${{ matrix.os }}
    strategy:
      matrix:
        os: [ubuntu-latest, macos-latest, windows-latest]
        python-version: ["3.10", "3.12"]
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-python@v5
        with:
          python-version: ${{ matrix.python-version }}
      - name: Install system deps (Linux)
        if: runner.os == 'Linux'
        run: |
          sudo apt-get update
          sudo apt-get install -y libmpv-dev ffmpeg
      - name: Install system deps (macOS)
        if: runner.os == 'macOS'
        run: brew install mpv ffmpeg
      - name: Install system deps (Windows)
        if: runner.os == 'Windows'
        run: choco install mpv ffmpeg -y
      - run: pip install -e ".[dev]"
      - run: pytest -v
```

- [ ] **Step 7:** Commit and push:

```bash
git add -A
git commit -m "feat: project scaffold with pyproject.toml, CI, and README"
git push -u origin main
```

---

## Task 2: Core clip engine (headless, testable)

**Objective:** Build the clip-saving logic as a standalone module with no GUI dependency.

**Files:**
- Create: `src/jorja_clipper/clipper.py`
- Create: `tests/test_clipper.py`

**Steps:**

- [ ] **Step 1:** Write failing tests:

```python
# tests/test_clipper.py
"""Tests for the clip engine."""

import json
import subprocess
from pathlib import Path
from unittest.mock import patch, MagicMock

import pytest

from jorja_clipper.clipper import Clipper, ClipResult


def test_clip_result_fields():
    """ClipResult has the expected fields."""
    result = ClipResult(
        path="/tmp/test.mp4",
        start_time=10.0,
        end_time=20.0,
        success=True,
    )
    assert result.path == "/tmp/test.mp4"
    assert result.start_time == 10.0
    assert result.end_time == 20.0
    assert result.success is True


def test_clipper_default_config():
    """Clipper uses ±5 second buffer by default."""
    c = Clipper()
    assert c.buffer_before == 5.0
    assert c.buffer_after == 5.0


def test_clipper_custom_config():
    """Clipper accepts custom buffer durations."""
    c = Clipper(buffer_before=10.0, buffer_after=3.0)
    assert c.buffer_before == 10.0
    assert c.buffer_after == 3.0


def test_clipper_calculates_times():
    """Clipper correctly calculates start/end from current position."""
    c = Clipper(buffer_before=5.0, buffer_after=5.0)
    start, end = c.calculate_times(current_pos=30.0, video_duration=120.0)
    assert start == 25.0
    assert end == 35.0


def test_clipper_clamps_start_at_zero():
    """Start time clamps to 0 when near the beginning."""
    c = Clipper(buffer_before=5.0, buffer_after=5.0)
    start, end = c.calculate_times(current_pos=2.0, video_duration=120.0)
    assert start == 0.0
    assert end == 7.0


def test_clipper_clamps_end_at_duration():
    """End time clamps to video duration when near the end."""
    c = Clipper(buffer_before=5.0, buffer_after=5.0)
    start, end = c.calculate_times(current_pos=118.0, video_duration=120.0)
    assert start == 113.0
    assert end == 120.0


def test_clipper_builds_output_path(tmp_path):
    """Output path goes to clips/ folder next to the source video."""
    video = tmp_path / "game.mp4"
    video.touch()
    c = Clipper()
    out = c.build_output_path(video, clip_number=1)
    assert out.parent.name == "clips"
    assert out.name.startswith("game_clip_")
    assert out.suffix == ".mp4"


@patch("jorja_clipper.clipper.subprocess.run")
def test_clipper_save_calls_ffmpeg(mock_run):
    """save_clip invokes ffmpeg with correct -ss/-i/-t/-c copy args."""
    mock_run.return_value = MagicMock(returncode=0, stderr="")
    c = Clipper(buffer_before=5.0, buffer_after=5.0)
    result = c.save_clip(
        video_path=Path("/tmp/game.mp4"),
        current_pos=30.0,
        video_duration=120.0,
        clip_number=1,
    )
    assert result.success is True
    args = mock_run.call_args[0][0]
    assert args[0] == "ffmpeg"
    assert "-ss" in args
    assert "-c" in args
    assert "copy" in args


@patch("jorja_clipper.clipper.subprocess.run")
def test_clipper_save_handles_ffmpeg_failure(mock_run):
    """save_clip returns failure result when ffmpeg exits non-zero."""
    mock_run.return_value = MagicMock(returncode=1, stderr="error")
    mock_run.side_effect = subprocess.CalledProcessError(1, "ffmpeg", stderr="error")
    c = Clipper()
    result = c.save_clip(
        video_path=Path("/tmp/game.mp4"),
        current_pos=30.0,
        video_duration=120.0,
        clip_number=1,
    )
    assert result.success is False
```

- [ ] **Step 2:** Run tests to verify failure:

```bash
cd jorja-clipper
pip install -e ".[dev]"
pytest tests/test_clipper.py -v
```

Expected: all FAIL — `cannot import name 'Clipper'`

- [ ] **Step 3:** Implement `src/jorja_clipper/clipper.py`:

```python
"""Core clip engine — extracts clips via ffmpeg stream-copy."""

import subprocess
from dataclasses import dataclass
from datetime import datetime
from pathlib import Path


@dataclass
class ClipResult:
    """Result of a clip save operation."""

    path: str
    start_time: float
    end_time: float
    success: bool
    error: str = ""


class Clipper:
    """Extracts clips from video files using ffmpeg stream-copy."""

    def __init__(self, buffer_before: float = 5.0, buffer_after: float = 5.0):
        self.buffer_before = buffer_before
        self.buffer_after = buffer_after

    def calculate_times(
        self, current_pos: float, video_duration: float
    ) -> tuple[float, float]:
        """Calculate start/end times clamped to [0, duration]."""
        start = max(0.0, current_pos - self.buffer_before)
        end = min(video_duration, current_pos + self.buffer_after)
        return start, end

    def build_output_path(self, video_path: Path, clip_number: int) -> Path:
        """Build the output path in a clips/ folder next to the source video."""
        clips_dir = video_path.parent / "clips"
        clips_dir.mkdir(parents=True, exist_ok=True)
        timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
        stem = video_path.stem
        ext = video_path.suffix or ".mp4"
        name = f"{stem}_clip_{timestamp}_{clip_number:03d}{ext}"
        return clips_dir / name

    def save_clip(
        self,
        video_path: Path,
        current_pos: float,
        video_duration: float,
        clip_number: int,
    ) -> ClipResult:
        """Save a clip using ffmpeg stream-copy (no re-encoding)."""
        start, end = self.calculate_times(current_pos, video_duration)
        duration = end - start
        output_path = self.build_output_path(video_path, clip_number)

        cmd = [
            "ffmpeg",
            "-y",
            "-ss", str(start),
            "-i", str(video_path),
            "-t", str(duration),
            "-c", "copy",
            "-avoid_negative_ts", "make_zero",
            str(output_path),
        ]

        try:
            result = subprocess.run(
                cmd,
                capture_output=True,
                text=True,
                timeout=30,
            )
            if result.returncode == 0:
                return ClipResult(
                    path=str(output_path),
                    start_time=start,
                    end_time=end,
                    success=True,
                )
            return ClipResult(
                path="",
                start_time=start,
                end_time=end,
                success=False,
                error=result.stderr,
            )
        except Exception as e:
            return ClipResult(
                path="",
                start_time=start,
                end_time=end,
                success=False,
                error=str(e),
            )
```

- [ ] **Step 4:** Run tests to verify pass:

```bash
pytest tests/test_clipper.py -v
```

Expected: all PASS

- [ ] **Step 5:** Lint check:

```bash
ruff check src/ tests/
ruff format --check src/ tests/
```

- [ ] **Step 6:** Commit:

```bash
git add src/jorja_clipper/clipper.py tests/test_clipper.py
git commit -m "feat: core clip engine with ffmpeg stream-copy"
```

---

## Task 3: Player wrapper (python-mpv integration)

**Objective:** Wrap python-mpv into a clean interface for the GUI to use.

**Files:**
- Create: `src/jorja_clipper/player.py`
- Create: `tests/test_player.py`

**Steps:**

- [ ] **Step 1:** Write failing tests:

```python
# tests/test_player.py
"""Tests for the player wrapper."""

from unittest.mock import MagicMock, patch

import pytest

from jorja_clipper.player import Player


def test_player_initial_state():
    """Player starts with no file loaded."""
    p = Player.__new__(Player)
    p._mpv = MagicMock()
    p._duration = 0.0
    p._current_pos = 0.0
    p._paused = True
    assert p.duration == 0.0
    assert p.current_pos == 0.0
    assert p.paused is True


def test_player_toggle_pause():
    """toggle_pause flips the paused state."""
    p = Player.__new__(Player)
    p._mpv = MagicMock()
    p._paused = True
    p.toggle_pause()
    assert p._paused is False
    p._mpv.pause.assert_called_with("yes")


def test_player_seek():
    """seek calls mpv.command with the correct offset."""
    p = Player.__new__(Player)
    p._mpv = MagicMock()
    p.seek(10.0)
    p._mpv.command.assert_called_with("seek", 10.0, "relative")
```

- [ ] **Step 2:** Run tests to verify failure:

```bash
pytest tests/test_player.py -v
```

Expected: FAIL — `cannot import name 'Player'`

- [ ] **Step 3:** Implement `src/jorja_clipper/player.py`:

```python
"""Player wrapper around python-mpv."""

from pathlib import Path

import mpv


class Player:
    """Wraps mpv for video playback with clean interface."""

    def __init__(self):
        self._mpv = mpv.MPV(
            input_default_bindings=False,
            input_vo_keyboard=False,
            osc=False,
        )
        self._duration = 0.0
        self._current_pos = 0.0
        self._paused = True

        @self._mpv.property_observer("duration")
        def _on_duration(_name, value):
            if value is not None:
                self._duration = float(value)

        @self._mpv.property_observer("time-pos")
        def _on_time_pos(_name, value):
            if value is not None:
                self._current_pos = float(value)

    @property
    def duration(self) -> float:
        """Total video duration in seconds."""
        return self._duration

    @property
    def current_pos(self) -> float:
        """Current playback position in seconds."""
        return self._current_pos

    @property
    def paused(self) -> bool:
        """Whether playback is paused."""
        return self._paused

    def load(self, path: Path) -> None:
        """Load a video file."""
        self._mpv.play(str(path))
        self._mpv.pause = "yes"
        self._paused = True

    def toggle_pause(self) -> None:
        """Toggle play/pause."""
        self._paused = not self._paused
        self._mpv.pause = "yes" if self._paused else "no"

    def seek(self, offset: float) -> None:
        """Seek by relative offset in seconds."""
        self._mpv.command("seek", offset, "relative")

    def shutdown(self) -> None:
        """Clean up mpv instance."""
        self._mpv.terminate()
```

- [ ] **Step 4:** Run tests to verify pass:

```bash
pytest tests/test_player.py -v
```

Expected: PASS

- [ ] **Step 5:** Commit:

```bash
git add src/jorja_clipper/player.py tests/test_player.py
git commit -m "feat: player wrapper around python-mpv"
```

---

## Task 4: GUI main window

**Objective:** Build the PySide6 main window with video widget, transport bar, and clip list sidebar.

**Files:**
- Create: `src/jorja_clipper/gui/main_window.py`
- Create: `src/jorja_clipper/gui/clip_list.py`
- Create: `src/jorja_clipper/gui/__init__.py`
- Create: `tests/test_gui.py`

**Steps:**

- [ ] **Step 1:** Write failing tests:

```python
# tests/test_gui.py
"""Tests for GUI components."""

import pytest

from jorja_clipper.gui.clip_list import ClipListModel


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
```

- [ ] **Step 2:** Run tests to verify failure:

```bash
pytest tests/test_gui.py -v
```

Expected: FAIL

- [ ] **Step 3:** Create `src/jorja_clipper/gui/__init__.py` (empty)

- [ ] **Step 4:** Implement `src/jorja_clipper/gui/clip_list.py`:

```python
"""Clip list model and widget."""

from dataclasses import dataclass
from pathlib import Path

from PySide6.QtCore import QAbstractListModel, QModelIndex, Qt


@dataclass
class ClipEntry:
    """A saved clip entry."""

    path: str
    start_time: float
    end_time: float


class ClipListModel(QAbstractListModel):
    """Model for the list of saved clips."""

    def __init__(self):
        super().__init__()
        self._clips: list[ClipEntry] = []

    def rowCount(self, parent=QModelIndex()) -> int:
        return len(self._clips)

    def data(self, index: QModelIndex, role=Qt.ItemDataRole.DisplayRole):
        if not index.isValid() or index.row() >= len(self._clips):
            return None
        clip = self._clips[index.row()]
        if role == Qt.ItemDataRole.DisplayRole:
            name = Path(clip.path).name
            return f"{name}  [{clip.start_time:.1f}s - {clip.end_time:.1f}s]"
        if role == Qt.ItemDataRole.UserRole:
            return clip
        return None

    def add_clip(self, path: str, start_time: float, end_time: float):
        """Add a new clip to the model."""
        self.beginInsertRows(QModelIndex(), len(self._clips), len(self._clips))
        self._clips.append(ClipEntry(path=path, start_time=start_time, end_time=end_time))
        self.endInsertRows()
```

- [ ] **Step 5:** Implement `src/jorja_clipper/gui/main_window.py`:

```python
"""Main application window."""

from pathlib import Path

from PySide6.QtCore import Qt
from PySide6.QtGui import QAction, QKeySequence, QShortcut
from PySide6.QtWidgets import (
    QFileDialog,
    QHBoxLayout,
    QLabel,
    QListWidget,
    QMainWindow,
    QPushButton,
    QSlider,
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
        self._current_video: Path | None = None

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

        self._btn_clip = QPushButton("✂ Clip (C)")
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
            self.setWindowTitle(f"Jorja Clipper — {self._current_video.name}")

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
```

- [ ] **Step 6:** Update `src/jorja_clipper/app.py`:

```python
"""Main application entry point."""

import sys
from pathlib import Path

from PySide6.QtWidgets import QApplication

from jorja_clipper.clipper import Clipper
from jorja_clipper.gui.main_window import MainWindow
from jorja_clipper.player import Player


def main():
    """Launch Jorja Clipper."""
    app = QApplication(sys.argv)

    player = Player()
    clipper = Clipper(buffer_before=5.0, buffer_after=5.0)
    window = MainWindow(player, clipper)
    window.show()

    # If a video file was passed as argument, load it
    if len(sys.argv) > 1:
        video_path = Path(sys.argv[1])
        if video_path.exists():
            player.load(video_path)
            window._current_video = video_path
            window._status.setText(f"Loaded: {video_path.name}")
            window.setWindowTitle(f"Jorja Clipper — {video_path.name}")

    sys.exit(app.exec())


if __name__ == "__main__":
    main()
```

- [ ] **Step 7:** Run tests:

```bash
pytest tests/ -v
```

Expected: all PASS

- [ ] **Step 8:** Lint and format:

```bash
ruff check src/ tests/
ruff format src/ tests/
```

- [ ] **Step 9:** Commit:

```bash
git add -A
git commit -m "feat: GUI main window with video player, controls, and clip list"
```

---

## Task 5: Settings / configuration

**Objective:** Allow configuring buffer duration (seconds before/after), output directory, and default keybinding.

**Files:**
- Create: `src/jorja_clipper/settings.py`
- Create: `tests/test_settings.py`

**Steps:**

- [ ] **Step 1:** Write failing tests:

```python
# tests/test_settings.py
"""Tests for settings module."""

import json
from pathlib import Path

import pytest

from jorja_clipper.settings import Settings


def test_default_settings():
    """Settings have sensible defaults."""
    s = Settings()
    assert s.buffer_before == 5.0
    assert s.buffer_after == 5.0
    assert s.clip_key == "C"


def test_settings_save_load(tmp_path):
    """Settings persist to JSON."""
    config = tmp_path / "config.json"
    s = Settings(config_path=config)
    s.buffer_before = 10.0
    s.save()

    s2 = Settings(config_path=config)
    s2.load()
    assert s2.buffer_before == 10.0


def test_settings_load_missing_file(tmp_path):
    """Loading from missing file uses defaults."""
    config = tmp_path / "missing.json"
    s = Settings(config_path=config)
    s.load()
    assert s.buffer_before == 5.0
```

- [ ] **Step 2:** Run tests to verify failure.

- [ ] **Step 3:** Implement `src/jorja_clipper/settings.py`:

```python
"""Application settings with JSON persistence."""

import json
from pathlib import Path


class Settings:
    """Manages application configuration."""

    def __init__(self, config_path: Path | None = None):
        self.config_path = config_path or Path.home() / ".config" / "jorja-clipper" / "config.json"
        self.buffer_before: float = 5.0
        self.buffer_after: float = 5.0
        self.clip_key: str = "C"
        self.output_dir: str = ""  # empty = clips/ next to video

    def save(self) -> None:
        """Save settings to JSON file."""
        self.config_path.parent.mkdir(parents=True, exist_ok=True)
        data = {
            "buffer_before": self.buffer_before,
            "buffer_after": self.buffer_after,
            "clip_key": self.clip_key,
            "output_dir": self.output_dir,
        }
        self.config_path.write_text(json.dumps(data, indent=2))

    def load(self) -> None:
        """Load settings from JSON file, using defaults for missing keys."""
        if not self.config_path.exists():
            return
        try:
            data = json.loads(self.config_path.read_text())
            self.buffer_before = data.get("buffer_before", self.buffer_before)
            self.buffer_after = data.get("buffer_after", self.buffer_after)
            self.clip_key = data.get("clip_key", self.clip_key)
            self.output_dir = data.get("output_dir", self.output_dir)
        except (json.JSONDecodeError, KeyError):
            pass  # Use defaults on corrupt file
```

- [ ] **Step 4:** Run tests, verify pass.

- [ ] **Step 5:** Wire settings into `app.py`:

Update `app.py` to load settings and pass buffer durations to `Clipper` and the keybinding to `MainWindow`.

- [ ] **Step 6:** Commit:

```bash
git add -A
git commit -m "feat: configurable settings with JSON persistence"
```

---

## Task 6: Packaging and distribution

**Objective:** Set up PyInstaller configs for building native executables on all three platforms.

**Files:**
- Create: `packaging/linux.spec`
- Create: `packaging/macos.spec`
- Create: `packaging/windows.spec`
- Modify: `.github/workflows/ci.yml` (add release build job)

**Steps:**

- [ ] **Step 1:** Add PyInstaller to dev dependencies in `pyproject.toml`:

```toml
[project.optional-dependencies]
dev = [
    "pytest>=7.0",
    "pytest-qt>=4.0",
    "ruff>=0.4",
    "pyinstaller>=6.0",
]
```

- [ ] **Step 2:** Create `packaging/linux.spec`, `packaging/macos.spec`, `packaging/windows.spec` (PyInstaller spec files).

- [ ] **Step 3:** Add release build job to `.github/workflows/ci.yml` that builds executables for all three platforms and creates a GitHub release with the artifacts.

- [ ] **Step 4:** Commit:

```bash
git add -A
git commit -m "feat: cross-platform packaging with PyInstaller"
```

---

## Task 7: Integration tests + end-to-end smoke test

**Objective:** Verify the full flow: open video → play → press C → clip file exists.

**Files:**
- Create: `tests/test_integration.py`

**Steps:**

- [ ] **Step 1:** Create a small test video with ffmpeg:

```python
# tests/conftest.py
import subprocess
from pathlib import Path
import pytest

@pytest.fixture
def test_video(tmp_path):
    """Generate a 10-second test video."""
    video = tmp_path / "test.mp4"
    subprocess.run([
        "ffmpeg", "-y", "-f", "lavfi", "-i",
        "testsrc=duration=10:size=320x240:rate=25",
        "-c:v", "libx264", "-pix_fmt", "yuv420p",
        str(video),
    ], capture_output=True, check=True)
    return video
```

- [ ] **Step 2:** Write integration test:

```python
# tests/test_integration.py
"""Integration test for the full clip workflow."""

from pathlib import Path

from jorja_clipper.clipper import Clipper


def test_full_clip_workflow(test_video, tmp_path):
    """Create a clip from a test video and verify the file exists."""
    clipper = Clipper(buffer_before=2.0, buffer_after=2.0)
    result = clipper.save_clip(
        video_path=test_video,
        current_pos=5.0,
        video_duration=10.0,
        clip_number=1,
    )
    assert result.success is True
    assert Path(result.path).exists()
    assert Path(result.path).stat().st_size > 0
```

- [ ] **Step 3:** Run full test suite:

```bash
pytest tests/ -v
```

- [ ] **Step 4:** Commit:

```bash
git add -A
git commit -m "test: integration test for full clip workflow"
```

---

## Summary

| Task | Description | Est. Time |
|------|-------------|-----------|
| 1 | Project scaffold + CI | 5 min |
| 2 | Core clip engine | 10 min |
| 3 | Player wrapper | 10 min |
| 4 | GUI main window | 15 min |
| 5 | Settings/config | 5 min |
| 6 | Packaging | 10 min |
| 7 | Integration tests | 5 min |

**Total: ~60 minutes of agent time**
