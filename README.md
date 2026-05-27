# Jorja Clipper

Cross-platform desktop app for instant sports highlight extraction.

## Features

- **Instant clipping**: Press a hotkey during video playback to save a lossless clip
- **Configurable buffers**: Set pre/post buffers for perfect highlight timing
- **Stream copy**: Uses FFmpeg `-c copy` for instant, lossless clipping
- **Cross-platform**: Works on Linux (Wayland/X11), Windows, and macOS

## Tech Stack

- **Backend**: Rust (Tauri 2.0)
- **Frontend**: Svelte 5 + TypeScript
- **Video**: mpv (via IPC)
- **Clipping**: FFmpeg (subprocess)
- **Storage**: SQLite

## Development

### Prerequisites

- Rust (1.70+)
- Node.js (18+)
- mpv
- FFmpeg

### Linux Dependencies

On Ubuntu/Debian, install the Tauri system dependencies:

```bash
sudo apt install libwebkit2gtk-4.1-dev libgtk-3-dev libsoup-3.0-dev libjavascriptcoregtk-4.1-dev libayatana-appindicator3-dev
```

### Setup

```bash
npm install
```

### Run Development Server

```bash
npm run tauri dev
```

### Build Release

```bash
npm run tauri build
```

## Usage

1. Click "Open" or press `O` to load a video
2. Press `Space` to play/pause
3. Press `C` to save a clip at current position
4. Use arrow keys to seek (±5s, or ±1s with Shift)

## Architecture

The app follows a three-layer architecture:

1. **Rust Backend**: Business logic, FFmpeg integration, mpv process management
2. **Tauri IPC**: Type-safe command interface
3. **Svelte Frontend**: UI rendering, user input

mpv runs as a child process with `--wid` embedding managed by Tauri's windowing layer, providing reliable video embedding on all platforms including Linux Wayland.

## License

MIT
