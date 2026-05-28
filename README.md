# Jorja Clipper

Cross-platform desktop app for instant sports highlight extraction.

## Features

- **Instant clipping**: Press a hotkey during video playback to save a lossless clip
- **Configurable buffers**: Set pre/post buffers for perfect highlight timing
- **Stream copy**: Uses FFmpeg `-c copy` for instant, lossless clipping
- **Format conversion**: Automatically converts MKV/AVI/TS/MOV to MP4 with progress tracking
- **Bundled FFmpeg**: No need to install FFmpeg separately - it's included in the app
- **Cross-platform**: Works on macOS, Windows, and Linux with identical behavior
- **Native video controls**: HTML5 video player with play/pause, seek, volume, and fullscreen

## Tech Stack

- **Backend**: Rust (Tauri 2.0)
- **Frontend**: Svelte 5 + TypeScript
- **Video**: HTML5 `<video>` element (asset protocol)
- **Conversion**: FFmpeg (stream copy with transcode fallback)
- **Clipping**: FFmpeg (subprocess)
- **Storage**: SQLite

## Installation

### For Users

Download the installer for your platform from the [releases page](https://github.com/yourusername/jorja-clipper/releases):

- **macOS**: `.dmg` installer
- **Windows**: `.msi` or `.exe` installer
- **Linux**: `.deb`, `.rpm`, or `.AppImage`

FFmpeg is bundled with the app - no additional dependencies required.

### For Developers

#### Prerequisites

- Rust (1.70+)
- Node.js (18+)
- FFmpeg (for development mode)

**Install FFmpeg (development only):**

- **macOS:** `brew install ffmpeg`
- **Windows:** Download from https://ffmpeg.org/download.html or `choco install ffmpeg`
- **Linux:** 
  - Ubuntu/Debian: `sudo apt install ffmpeg`
  - Fedora: `sudo dnf install ffmpeg`
  - Arch: `sudo pacman -S ffmpeg`

#### Linux Dependencies

On Ubuntu/Debian, install the Tauri system dependencies:

```bash
sudo apt install libwebkit2gtk-4.1-dev libgtk-3-dev libsoup-3.0-dev libjavascriptcoregtk-4.1-dev libayatana-appindicator3-dev
```

#### Setup

```bash
npm install
```

#### Run Development Server

```bash
npm run tauri dev
```

#### Build Release

```bash
# Download FFmpeg binaries for sidecar bundling
./setup-ffmpeg.sh

# Build release packages
npm run tauri build
```

## Usage

1. Click "Open" or press `O` to load a video
2. Press `Space` to play/pause
3. Press `C` to save a clip at current position
4. Use arrow keys to seek (±5s, or ±1s with Shift)

## Architecture

The app follows a three-layer architecture:

1. **Rust Backend**: Business logic, FFmpeg integration, video format conversion
2. **Tauri IPC**: Type-safe command interface
3. **Svelte Frontend**: UI rendering, HTML5 video playback, user input

Video playback uses the HTML5 `<video>` element with Tauri's asset protocol (`convertFileSrc()`). Web-compatible formats (MP4, WebM, Ogg) play directly. Non-web formats (MKV, AVI, TS, MOV) are automatically converted to MP4 using FFmpeg, with real-time progress tracking. Converted files are cached for faster re-opening.

## FFmpeg Sidecar

FFmpeg and ffprobe are bundled as sidecar executables in release builds, so users don't need to install FFmpeg separately.

**How it works:**
- Run `./setup-ffmpeg.sh` to download platform-specific FFmpeg binaries
- Binaries are placed in `src-tauri/binaries/` with platform-specific names
- Tauri bundles them during `tauri build`
- At runtime, the app uses bundled binaries if available, falling back to system PATH
- On macOS, also checks homebrew paths for development convenience

**Supported platforms:**
- macOS (x86_64 and ARM64)
- Windows (x86_64)
- Linux (x86_64)

## Supported Video Formats

**Direct playback (no conversion):**
- MP4, M4V
- WebM
- Ogg, OGV

**Automatic conversion to MP4:**
- MKV
- AVI
- TS
- MOV

Conversion uses stream copy (fast, lossless) when possible, falling back to transcode (slower, high quality) when needed.

## License

MIT
