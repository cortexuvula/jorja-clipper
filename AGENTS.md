# AGENTS.md

## Project

Jorja Clipper — cross-platform desktop app (Tauri 2 + Svelte 5 + TypeScript). Instant sports highlight extraction via hotkey, using FFmpeg stream-copy with configurable pre/post buffers.

## Commands

```bash
npm install                          # install all deps
./setup-ffmpeg.sh                    # download FFmpeg sidecar binaries (required before cargo tauri build)
cargo tauri dev                      # run app in dev mode
cargo tauri build                    # release bundle (run setup-ffmpeg.sh first)

# Rust
cd src-tauri && cargo test                                              # all Rust tests
cd src-tauri && cargo test -- test_get_clips_logic_empty                # single test
cd src-tauri && cargo fmt -- --check                                    # formatting check
cd src-tauri && cargo clippy -- -D warnings                             # clippy lint (treat warnings as errors)

# Frontend
npx svelte-kit sync                  # generate .svelte-kit types (must run before first check, CI step)
npm run check                        # svelte-check type validation
```

## Architecture

```
src-tauri/src/
  main.rs          — entry point, Tauri Builder, command registration, tokio::sync::Mutex
  lib.rs           — module declarations only
  controller.rs    — Controller struct + update_settings/validation, get_clips (stale-file cleanup)
  commands.rs      — #[tauri::command] + *_logic functions separated for testability
  clipper.rs       — FFmpeg clip extraction via tokio::process
  converter.rs     — non-web-format → MP4 conversion + ffprobe duration
  settings.rs      — JSON config (~/.config/jorja-clipper/config.json)
  storage.rs       — SQLite via rusqlite (bundled feature), Clip + ClipStore
  video_server.rs  — local HTTP server for range-request video streaming (Linux WebKitGTK workaround)
  cleanup.rs       — background task to prune stale converted MP4 files
  util.rs          — binary path resolution + sidecar init
  error.rs         — AppError enum (thiserror)

src/
  app.html              — Tauri entry HTML
  app.d.ts              — GENERATED Tauri types (do not edit)
  routes/+page.svelte   — main UI page
  lib/
    api.ts              — invoke() wrappers matching commands.rs
    types.ts            — shared TypeScript types
    components/         — Svelte 5 components
    stores/             — Svelte 5 stores
```

**Data flow:** Frontend calls `invoke('command_name', args)` → Tauri routes to `commands.rs` → `Controller` (shared via `Arc<tokio::sync::Mutex<Controller>>`) delegates to `Clipper`, `Converter`, `Storage`.

**Lock discipline (critical):** Both commands `open_video` and `save_clip` use 3-phase patterns:
1. Acquire lock briefly, compute what's needed, drop lock
2. Do slow work (FFmpeg conversion/clipping) without any lock held
3. Re-acquire lock, update state

`save_clip` uses a `ClippingGuard` RAII struct that sets `is_clipping = false` on drop; it's explicitly `std::mem::forget`-ed on success to keep the flag true through Phase 2.

## Tests

- Rust tests live inline as `#[cfg(test)] mod tests` in the same source files (commands.rs, controller.rs, etc.) — no separate `tests/` directory.
- Many tests create real video files using FFmpeg (`Command::new("ffmpeg")`). If FFmpeg isn't available, those tests will fail.
- Frontend has no test framework — only `npm run check` for type validation.
- CI runs `cargo fmt -- --check`, `cargo clippy -- -D warnings`, and `npm run check`. No `cargo test` in CI currently.

## Video playback quirks

- Web-compatible formats (MP4, WebM, Ogg, OGV, M4V) play directly via Tauri's `asset://` protocol using `convertFileSrc()`.
- Non-web formats (MKV, AVI, TS, MOV) are converted to MP4 via FFmpeg before playback, cached in `~/.config/jorja-clipper/clips/`, and cleaned up after 7 days.
- Linux (WebKitGTK) requires a local HTTP video server (`VideoServer`) because `asset://` doesn't support range requests.

## Configuration

| Path | Purpose |
|---|---|
| `~/.config/jorja-clipper/config.json` | Settings (buffers, clip key, output_dir, theme) |
| `~/.config/jorja-clipper/clips.db` | SQLite clip history |
| `~/.config/jorja-clipper/jorja-clipper.log` | Application log |
| `~/.config/jorja-clipper/clips/` | Converted MP4 cache (auto-cleaned) |

## FFmpeg sidecar

Binaries live in `src-tauri/binaries/` with target-triple suffixes (e.g. `ffmpeg-aarch64-apple-darwin`). Resolution order: sidecar → system PATH → macOS Homebrew fallback (`/opt/homebrew/bin`, `/usr/local/bin`). `setup-ffmpeg.sh` downloads platform-specific binaries; `--all` flag downloads all platforms for CI/CD.

## Settings validation

- `buffer_before` / `buffer_after`: 0.0–60.0 seconds
- `clip_key`: must be exactly one character
- `output_dir`: must exist and be writable if specified; empty string normalizes to `None`
