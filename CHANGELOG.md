# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.2.23] - 2026-05-25

### Fixed
- **Test failure on Windows**: Fixed `test_clipper_save_calls_ffmpeg` to handle Windows FFmpeg paths with uppercase `.EXE` extension (e.g., `C:\ProgramData\Chocolatey\bin\ffmpeg.EXE`).

## [0.2.22] - 2026-05-25

### Fixed
- **Build failure**: Fixed lint error in clipper.py docstring (line too long).

## [0.2.21] - 2026-05-25

### Fixed
- **FFmpeg not found in packaged app**: The app couldn't find FFmpeg when running from the packaged build. Added FFmpeg binary and its dependencies (libav*, libsw*) to PyInstaller bundles for all platforms (macOS, Linux, Windows). Updated `clipper.py` to search for bundled FFmpeg first, then fall back to system PATH.

## [0.2.20] - 2026-05-25

### Fixed
- **macOS ZIP distribution signature error**: The ZIP archive was created using `zip -r` which doesn't preserve symlinks. This broke the code signature on the Python and Qt binaries (they were created as symlinks by PyInstaller). Now using `ditto` to preserve symlinks and macOS metadata, matching the DMG distribution.

## [0.2.19] - 2026-05-25

### Fixed
- **libmpv loading on macOS**: PyInstaller creates libmpv in a `libmpv__dot__2__dot__dylib/` directory (with dots escaped). The runtime hook now searches this path to find the bundled library.

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

## [0.2.17] - 2026-05-25

### Fixed
- **Clip list ordering**: Loaded clips were displayed in reverse order (newest at bottom instead of top). Removed incorrect `reversed()` call in `load_clips_for_current_video()`.
- **Queue clip count**: `queue_clip()` was incrementing `_clip_count` before the clip was saved, causing incorrect numbering if the queue was cleared. Now calculates clip number based on queue length instead.
- **Queue error handling**: Logic error in `_on_queue_clip()` - `isinstance(err, object)` was always True. Changed to `err is not None` check.
- **Thread safety**: Added missing lock around `_paused` updates in `player.py` for consistency with `_duration` and `_current_pos`.

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
