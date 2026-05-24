# Jorja Clipper

Cross-platform desktop application for instant sports highlight extraction.

## Overview

Jorja Clipper lets you play a video and save a lossless clip (with configurable pre/post-event buffers) with a single keystroke — no re-encoding. Designed for clipping basketball game highlights in real time.

## Features

- **Instant clip saving** — press a hotkey during playback to grab the surrounding seconds
- **No re-encoding** — fast `ffmpeg` stream-copy preserves original quality
- **Batch queue** — queue multiple clips and process them in one run
- **Clip persistence** — clips are saved to a SQLite database per video, restored on reopen
- **Undo** — reverse the last clip (deletes file + DB entry, restores playback position)
- **Themes** — dark (default) and light theme
- **Plugins** — drop Python scripts in `~/.config/jorja-clipper/plugins/` to extend behaviour
- **Cross-platform** — Linux (.deb), macOS (.dmg / .zip), Windows (.exe)

## Installation

Download the latest release from [GitHub Releases](https://github.com/cortexuvula/jorja-clipper/releases).

### Linux (Debian/Ubuntu)

```bash
sudo apt install ./jorja-clipper-*-amd64.deb
```

Dependencies: `mpv`, `ffmpeg` (installed automatically via apt).

### macOS

1. Download the `.dmg` or `.zip`
2. Drag **Jorja Clipper** to Applications
3. On first launch, right-click → Open to bypass Gatekeeper (the app is signed and notarized)

### Windows

Run the `.exe` installer. Requires `mpv` and `ffmpeg` on your PATH.

## Keyboard Shortcuts

| Key | Action |
| :--- | :--- |
| **O** | Open video file |
| **Space** | Play / Pause |
| **Left / Right** | Seek ±5 s |
| **Shift + Left/Right** | Seek ±1 s |
| **C** | Save clip (hotkey is configurable) |
| **Q** | Queue clip for batch processing |
| **U** | Undo last clip |

## Settings

Click **Settings** in the app or edit `~/.config/jorja-clipper/config.json`:

| Key | Default | Description |
| :--- | :--- | :--- |
| `buffer_before` | `5.0` | Seconds to include before the clip point |
| `buffer_after` | `5.0` | Seconds to include after the clip point |
| `clip_key` | `"C"` | Hotkey for saving a clip |
| `theme` | `"dark"` | UI theme (`"dark"` or `"light"`) |

## Plugins

Create a Python file in `~/.config/jorja-clipper/plugins/`. Plugins receive hooks for clip start and clip complete events. See `jorja_clipper/plugins.py` for the hook interface.

## Development

Requires Python 3.10+.

```bash
git clone https://github.com/cortexuvula/jorja-clipper.git
cd jorja-clipper
pip install -e '.[dev]'
```

Run tests:

```bash
pytest
```

Lint:

```bash
ruff check src tests
```

### Tech Stack

Python, PySide6 (Qt 6), SQLite, ffmpeg, mpv (python-mpv), uv, pytest, ruff

## License

MIT License
