# CLAUDE.md

This file provides guidance to Claude Code when working with this repository.

## Project

Jorja Clipper is a cross-platform desktop app built with **Tauri 2** (Rust backend) and **Svelte 5** (TypeScript frontend). It provides instant sports highlight extraction: press a hotkey during video playback to save a lossless clip using FFmpeg stream-copy, with configurable pre/post buffers.

## Commands

```bash
# Install dependencies
npm install

# Setup FFmpeg sidecar binaries (required before building releases)
./setup-ffmpeg.sh

# Run the app in development mode
cargo tauri dev

# Build a release bundle (run setup-ffmpeg.sh first)
cargo tauri build

# Run Rust tests
cd src-tauri && cargo test

# Run frontend type checks
npm run check

# Lint frontend code
npx svelte-check --tsconfig ./tsconfig.json
```

## Architecture

```
src-tauri/src/
  ├── main.rs          — entry point, Tauri Builder, command registration
  ├── controller.rs    — Controller: orchestrates Clipper, Converter, Settings, Storage
  ├── clipper.rs       — Clipper: runs ffmpeg -c copy via tokio::process
  ├── converter.rs     — Converter: converts non-web video formats to MP4 with progress tracking
  ├── commands.rs      — #[tauri::command] handlers exposed to the frontend
  ├── settings.rs      — Settings: JSON config (~/.config/jorja-clipper/config.json)
  ├── storage.rs       — Storage: SQLite via rusqlite (bundled) for clip history
  ├── util.rs          — Binary path resolution (sidecar + system PATH + macOS homebrew fallback)
  └── error.rs         — AppError enum with thiserror

src/
  ├── app.html         — Tauri entry HTML
  ├── app.d.ts         — generated Tauri types
  ├── routes/+page.svelte — main UI page
  └── lib/
      ├── api.ts       — invoke() wrappers for Tauri commands
      ├── stores/      — Svelte stores for app state
      ├── components/  — Svelte components
      └── types.ts     — shared TypeScript types
```

**Data flow:** Frontend calls `invoke('command_name', args)` → Tauri routes to `commands.rs` → `Controller` (shared state via `Arc<Mutex<Controller>>`) delegates to `Clipper`, `Converter`, or `Storage`.

**Video playback:** Uses HTML5 `<video>` element in the webview. Web-compatible formats (MP4, WebM, Ogg, OGV, M4V) play directly via Tauri's asset protocol (`convertFileSrc()`). Non-web formats (MKV, AVI, TS, MOV) are converted to MP4 using FFmpeg with real-time progress tracking. Converted files are cached in `~/.config/jorja-clipper/clips/` for faster re-opening.

**FFmpeg sidecar:** FFmpeg and ffprobe binaries are bundled as sidecar executables (see `setup-ffmpeg.sh`). The `util` module resolves binary paths: sidecar first, then system PATH with macOS homebrew fallback.

## Configuration

- **Settings:** `~/.config/jorja-clipper/config.json`
- **Clip database:** `~/.config/jorja-clipper/clips.db`
- **Log file:** `~/.config/jorja-clipper/jorja-clipper.log`

## Platform Notes

**FFmpeg is bundled as a sidecar binary** - users don't need to install it separately. Run `./setup-ffmpeg.sh` before building release packages to download platform-specific binaries.

**Development requirements:**
- **macOS:** No additional dependencies needed (FFmpeg will be bundled)
- **Windows:** No additional dependencies needed (FFmpeg will be bundled)
- **Linux:** Requires Tauri system deps for building (`libwebkit2gtk-4.1-dev`, `libgtk-3-dev`, `libsoup-3.0-dev`, `libjavascriptcoregtk-4.1-dev`, `libayatana-appindicator3-dev`)

**Runtime behavior:**
- App uses bundled FFmpeg sidecar if available
- Falls back to system PATH if sidecar not found (useful during development)
- On macOS, also checks homebrew paths (`/opt/homebrew/bin`, `/usr/local/bin`)

## Testing

- Rust tests: `cd src-tauri && cargo test` — tests clipper logic, settings serialization, storage queries.
- Frontend: `npm run check` runs `svelte-check` for TypeScript validation.
- There are no headless CI environments configured yet (Python CI workflows were removed).
