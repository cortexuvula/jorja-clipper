# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

Jorja Clipper is a cross-platform desktop app (Python/PySide6) for instant sports highlight extraction. Press a hotkey during video playback to save a lossless clip (ffmpeg stream-copy) with configurable pre/post buffers. Designed for clipping basketball game highlights in real time.

## Commands

```bash
# Install dependencies (uv is the primary package manager)
uv sync --dev

# Run the app
uv run jorja-clipper              # interactive
uv run jorja-clipper /path/to.mp4 # with video argument

# Tests
uv run pytest                     # all tests
uv run pytest tests/test_clipper.py          # single file
uv run pytest tests/test_controller.py::test_controller_open_file_success  # single test

# Linting
uv run ruff check src/ tests/
uv run ruff format src/ tests/

# Build (requires packaging extras)
pip install -e ".[packaging]"
pyinstaller --clean packaging/linux.spec   # or macos.spec / windows.spec
```

## Architecture

The app follows a controller-mediated pattern separating GUI from backend:

```
app.py (composition root)
  ├── Creates: Settings, Player, Clipper, ClipListModel, ThemeManager, PluginLoader
  ├── Creates: ClipController (owns all backend components)
  └── Creates: MainWindow(controller, theme_manager) — GUI only talks to controller

MainWindow (gui/main_window.py)
  ├── User input → controller.save_clip(), controller.open_file(), etc.
  └── Receives: QThread worker signals (ClipWorker.finished, BatchWorker.progress/finished)
```

**Key modules and their roles:**

- **`controller.py`** — `ClipController` is the central orchestrator. Owns Player, Clipper, Settings, ClipStore, PluginLoader, and BatchQueue. All GUI actions go through it. The single-clip workflow returns either a `ClipWorker` (QThread) or a `ClipResult` with `success=False` on rejection (no video / already clipping). The caller connects to the worker's `finished` signal.

- **`clipper.py`** — `Clipper` is the pure clip engine. `save_clip()` runs `ffmpeg -c copy` via `subprocess.run` (30s timeout). Output goes to a `clips/` folder next to the source video. No Qt dependencies — can be called from any thread.

- **`worker.py`** — `ClipWorker(QThread)` wraps a single blocking `Clipper.save_clip()` call. Emits `finished(object)` with a `ClipResult`. The controller holds a reference in `_active_worker` and calls `deleteLater()` on completion.

- **`batch_queue.py`** — `ClipQueue` (FIFO of `ClipRequest` dataclasses) + `BatchWorker(QThread)` that dequeues and processes sequentially. Emits `progress(completed, total, result)` after each item and `finished(list[ClipResult])` at the end.

- **`player.py`** — `Player` wraps python-mpv. Lazy-init via `_ensure_mpv()` (deferred until `init_with_wid()` provides a native window handle). Uses property observers for duration/position/pause state, protected by a threading lock. **Locale quirk:** `LC_NUMERIC` must be "C" before mpv init — Qt resets it on Linux, so `app.py` sets it at startup AND `Player._ensure_mpv()` re-applies it.

- **`clip_store.py`** — `ClipStore` persists clip metadata to SQLite (`~/.config/jorja-clipper/clips.db`). Clips are loaded per-video when `open_file()` is called, enabling clip history across sessions.

- **`plugins.py`** — `PluginLoader` scans `~/.config/jorja-clipper/plugins/*.py` for `ClipPlugin` subclasses. Hooks: `on_clip_start`, `on_clip_complete`, `on_clip_error`. Plugins are loaded once at startup in `app.py`.

- **`gui/video_widget.py`** — `VideoWidget` provides a native window handle (`winId()`) for mpv embedding. The handle is platform-specific: X11 Window ID (Linux), NSView pointer (macOS), HWND (Windows). mpv is bound in `showEvent()` (not `__init__`) because the native window must exist first.

- **`gui/main_window.py`** — On Linux (`sys.platform == "linux"`), uses `FramelessWindowHint` + a custom `_TitleBar` widget to avoid double title bars on Wayland/X11. The title bar provides drag-to-move, minimize, maximize, and close.

## Data Flow: Clip Lifecycle

1. User presses hotkey → `MainWindow._on_clip_requested()` → `controller.save_clip()`
2. Controller validates (video loaded, no active worker), calculates times, broadcasts `on_clip_start` to plugins
3. Controller creates `ClipWorker`, connects `finished` signal, starts thread
4. Worker thread runs `Clipper.save_clip()` (blocking ffmpeg subprocess)
5. Worker emits `finished(ClipResult)` → Controller's `_on_clip_finished` handler:
   - Broadcasts plugin hook, updates clip model, persists to SQLite, stores undo info
6. MainWindow's `_on_clip_finished` handler: re-enables clip button, updates status bar

## Testing Patterns

- **`conftest.py`** mocks the `mpv` module globally before any test imports — python-mpv requires libmpv at import time, which CI runners don't have.
- **`test_gui.py`** uses `_needs_display` marker to skip widget tests in headless/CI environments (`$CI == "true"` or no `$DISPLAY` on Linux).
- **`test_integration.py`** is gated on `ffmpeg` being available (`shutil.which("ffmpeg")`).
- Controller tests use `MagicMock` for Player, Clipper, and Settings — no real mpv or ffmpeg needed.
- The `test_video` fixture generates a 10-second test video using ffmpeg's `testsrc` lavfi source.

## Platform-Specific Notes

- **macOS:** `Player._ensure_mpv()` sets `vo="libmpv"` for NSView rendering. CI signs + notarizes the .app bundle. The release workflow signs Mach-O binaries individually, then frameworks, then the app bundle (inside-out order).
- **Linux:** `LC_NUMERIC=C` is critical — set both in `app.py:14` and `Player._ensure_mpv()`. Custom titlebar via `FramelessWindowHint` avoids double decorations on Wayland/X11. The .deb package is built in CI from the PyInstaller artifact.
- **Windows:** Requires mpv and ffmpeg on PATH. PyInstaller bundles into a single .exe.

## Configuration

- **Settings file:** `~/.config/jorja-clipper/config.json` (JSON with `buffer_before`, `buffer_after`, `clip_key`, `output_dir`, `theme`)
- **Clip database:** `~/.config/jorja-clipper/clips.db` (SQLite)
- **Plugins directory:** `~/.config/jorja-clipper/plugins/`
- **Log file:** `~/.config/jorja-clipper/jorja-clipper.log` (rotating, 1MB max, 3 backups)
- **Theme:** `gui/theme.py` — `Theme` dataclass with color/font/spacing fields. Built-in: `THEME_DARK`, `THEME_LIGHT`. `ThemeManager` resolves theme by name.

## Entry Points

- `jorja-clipper` CLI → `jorja_clipper.app:main`
- `python -m jorja_clipper` → `__main__.py` → `app.main()`
- `app.main()` sets `LC_NUMERIC`, enables faulthandler, configures logging, then builds the object graph and enters the Qt event loop. Video CLI args are loaded via `QTimer.singleShot(100, ...)` to ensure the native widget is fully realized before mpv binds its render context.
