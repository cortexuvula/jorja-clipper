# CLAUDE.md

This file provides guidance to Claude Code when working with this repository.

## Project

Jorja Clipper is a cross-platform desktop app built with **Tauri 2** (Rust backend) and **Svelte 5** (TypeScript frontend). It provides instant sports highlight extraction: press a hotkey during video playback to save a lossless clip using FFmpeg stream-copy, with configurable pre/post buffers.

## Commands

```bash
# Install dependencies
npm install

# Run the app in development mode
cargo tauri dev

# Build a release bundle
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
  ├── controller.rs    — Controller: orchestrates Player, Clipper, Settings, Storage
  ├── clipper.rs       — Clipper: runs ffmpeg -c copy via tokio::process
  ├── player.rs        — Player: mpv child process via IPC (stdin/stdout JSON)
  ├── commands.rs      — #[tauri::command] handlers exposed to the frontend
  ├── settings.rs      — Settings: JSON config (~/.config/jorja-clipper/config.json)
  ├── storage.rs       — Storage: SQLite via rusqlite (bundled) for clip history
  ├── error.rs         — AppError enum with thiserror
  └── x11_window.rs    — (Linux only) X11 child window creation for mpv --wid embedding

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

**Data flow:** Frontend calls `invoke('command_name', args)` → Tauri routes to `commands.rs` → `Controller` (shared state via `Arc<Mutex<Controller>>`) delegates to `Player`, `Clipper`, or `Storage`.

**mpv integration:** mpv runs as a child process with `--ipc` for JSON command/response. On Linux, a real X11 child window is created via `x11rb` for mpv's `--wid` embedding (required even under Wayland — `GDK_BACKEND=x11` is set in `main.rs` to force XWayland).

## Configuration

- **Settings:** `~/.config/jorja-clipper/config.json`
- **Clip database:** `~/.config/jorja-clipper/clips.db`
- **Log file:** `~/.config/jorja-clipper/jorja-clipper.log`

## Platform Notes

- **Linux:** Requires Tauri system deps (`libwebkit2gtk-4.1-dev`, `libgtk-3-dev`, `libsoup-3.0-dev`, `libjavascriptcoregtk-4.1-dev`, `libayatana-appindicator3-dev`). `GDK_BACKEND=x11` is forced in `main.rs` for mpv window embedding.
- **macOS / Windows:** mpv and FFmpeg must be on `PATH`.

## Testing

- Rust tests: `cd src-tauri && cargo test` — tests clipper logic, settings serialization, storage queries.
- Frontend: `npm run check` runs `svelte-check` for TypeScript validation.
- There are no headless CI environments configured yet (Python CI workflows were removed).
