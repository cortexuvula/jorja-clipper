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
