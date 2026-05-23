# Jorja Clipper

Cross-platform desktop application for instant sports highlight extraction.

## 🎯 Overview

Jorja Clipper lets you play a video and save a clip (with configurable pre/post-event buffers) with a single keystroke — without re-encoding.

## 🚀 Features

- Instant clip saving with configurable buffers
- Keyboard-driven playback control
- No re-encoding (fast `ffmpeg` stream-copy)
- Cross-platform: Linux, Windows, macOS

## ⌨️ Keyboard Shortcuts

| Key | Action |
| :--- | :--- |
| **Space** | Play / Pause |
| **Left / Right** | Seek ±5s |
| **Shift + Left/Right** | Seek ±1s |
| **C** | Save clip |
| **O** | Open file |
| **Q** | Quit |

## 🛠 Development

Install in editable mode with dev dependencies:

```bash
pip install -e '.[dev]'
```

Run tests:

```bash
pytest
```

## ⚖️ License

MIT License
