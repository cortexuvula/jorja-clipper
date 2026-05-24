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

## 🖼 Demo

> A demo GIF or screenshot will be added here before the next release.
> To generate one, record a short clip workflow and place the file in
> `docs/assets/demo.gif`, then update the line below.
>
> `![Jorja Clipper Demo](docs/assets/demo.gif)`

### UI Layout (ASCII Mockup)

```
+-------------------------------------------------------------+
|  Jorja Clipper — game.mp4                                   |
+-------------------------------------------------------------+
|                                                             |
|  +-------------------------+   +------------------------+  |
|  |                         |   | Saved Clips            |  |
|  |    Video Player Area    |   | ---------------------- |  |
|  |    (mpv renders here)   |   | game_clip_... [25-35s] |  |
|  |                         |   | game_clip_... [45-55s] |  |
|  |                         |   |                        |  |
|  +-------------------------+   +------------------------+  |
|                                                             |
|  [Open (O)] [Play/Pause (Space)] [Clip (C)] [Settings] [Undo (U)]
|                                                             |
|  Loaded: game.mp4                                           |
+-------------------------------------------------------------+
```

## ⚖️ License

MIT License
