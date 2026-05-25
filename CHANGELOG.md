# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- `CONTRIBUTING.md` with development setup instructions for macOS, Linux, and Windows.
- `CHANGELOG.md` to track release history.
- Dependency lockfile (`uv.lock`) for reproducible installs.
- Type hints across all GUI modules, `controller.py`, and `worker.py`.

### Changed
- Pinned runtime and dev dependency versions in `pyproject.toml` with upper bounds.
  - `PySide6>=6.11,<6.12`
  - `python-mpv>=1.0.7,<2.0`
  - `pytest>=7.0,<9.0`
  - `pytest-qt>=4.2,<5.0`
  - `ruff>=0.1.0,<1.0`
  - `pyinstaller>=6.0,<7.0`
- Split `pyproject.toml` dev extras into `dev` and `packaging` groups.

## [0.2.16] - 2026-05-25

### Fixed
- **macOS launch failure (root cause)**: PyInstaller creates Qt and Python binaries in two places:
  flat binaries at `/Contents/Frameworks/X` and framework bundles at
  `/Contents/Frameworks/PySide6/Qt/lib/X.framework`. The flat binaries get signed as standalone
  Mach-O files (without Info.plist binding), causing `dlopen` to reject them at runtime with
  "code signature invalid". Fix: replace flat binaries with symlinks to the framework versions,
  so `dlopen` follows the symlink and loads the binary from inside the framework where the
  signature IS valid (Info.plist bound).
- **CLI video loading bug**: When launching with a video path argument (`jorja-clipper game.mp4`),
  `controller.open_file()` was called but not `window.load_video()`, leaving the status bar at
  "No video loaded" and window title at "Jorja Clipper". Now calls both.

## [0.2.10] - 2026-05-25

### Fixed
- **macOS video playback**: Replace broken `--wid` mpv embedding with render API (`MpvRenderContext` + `QOpenGLWidget`). The `--wid` approach no longer works with modern mpv's Swift cocoa-cb backend on macOS, causing videos to not display. Now uses platform-specific implementations: render API on macOS, `--wid` on Linux/Windows.
- **macOS shutdown crash**: Fix abort in `mp_clients_destroy` by freeing the mpv render context before terminating the mpv instance.

## [0.1.0] - 2026-05-23

### Added
- Initial MVP release.
- PySide6 GUI with video playback via python-mpv.
- Instant clip extraction with ffmpeg stream-copy.
- Configurable buffer-before / buffer-after settings.
- Keyboard shortcuts (open, play/pause, clip, seek, quit).
- Cross-platform CI (Ubuntu, macOS, Windows) and PyInstaller packaging.
- Basic test suite covering clipper, player, settings, and GUI models.
