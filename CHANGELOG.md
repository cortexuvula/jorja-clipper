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

## [0.1.0] - 2026-05-23

### Added
- Initial MVP release.
- PySide6 GUI with video playback via python-mpv.
- Instant clip extraction with ffmpeg stream-copy.
- Configurable buffer-before / buffer-after settings.
- Keyboard shortcuts (open, play/pause, clip, seek, quit).
- Cross-platform CI (Ubuntu, macOS, Windows) and PyInstaller packaging.
- Basic test suite covering clipper, player, settings, and GUI models.
