# Jorja Clipper Rust/Tauri Port Design

**Date:** 2026-05-26  
**Status:** Approved  
**Author:** Claude

## Problem Statement

The Python/PySide6 implementation suffers from persistent mpv window embedding issues on Linux Wayland systems. Despite multiple attempts (XCB platform forcing, stylesheet fixes, environment variable manipulation), the video fails to render embedded in the main window reliably.

**Decision:** Port to Rust/Tauri to leverage:
- More reliable native window management
- Better Wayland support via Tauri's windowing layer
- Stronger type system preventing entire classes of bugs
- Smaller binary size and faster startup
- No Python runtime dependency for end users

## Architecture Overview

### Three-Layer Architecture

```
┌─────────────────────────────────────┐
│   Svelte Frontend (Web)             │
│   - UI rendering                    │
│   - User input handling             │
│   - Reactive state management       │
└──────────────┬──────────────────────┘
               │ Tauri IPC (invoke/events)
┌──────────────▼──────────────────────┐
│   Rust Backend                      │
│   - Business logic                  │
│   - FFmpeg integration              │
│   - mpv process management          │
│   - SQLite persistence              │
└──────────────┬──────────────────────┘
               │ Unix socket / named pipe
┌──────────────▼──────────────────────┐
│   mpv Process                       │
│   - Video rendering                 │
│   - Audio playback                  │
│   - Codec support                   │
└─────────────────────────────────────┘
```

**Key principle:** The Rust backend has zero knowledge of the UI. It exposes pure functions that the frontend orchestrates. This separation enables:
- Independent backend testing
- Frontend framework flexibility
- Clear separation of concerns

### mpv Integration Strategy

mpv runs as a child process spawned by Rust with the following flags:
```
mpv --idle --input-ipc-server=/tmp/jorja-mpv-socket --wid=<native-window-id>
```

**Window management:**
1. Tauri creates a child window for mpv (initially hidden)
2. Frontend's `VideoPlayer` component renders a placeholder `<div>`
3. On mount/resize, frontend calls `invoke('position_mpv_window', { rect })`
4. Rust uses Tauri's `Window::set_position()` and `Window::set_size()` to align the mpv window over the placeholder
5. Rust shows the mpv window once positioned

**Why this works:** Tauri's windowing layer abstracts platform differences (X11, Wayland, Windows, macOS). The `--wid` parameter works reliably because Tauri provides a valid native window handle.

## Rust Backend Components

### Module Structure

```
src-tauri/src/
  main.rs              # Tauri app entry point
  commands.rs          # IPC command handlers
  controller.rs        # Orchestration layer
  player.rs            # mpv process management
  clipper.rs           # FFmpeg integration
  storage.rs           # SQLite persistence
  settings.rs          # Configuration management
  error.rs             # Unified error types
  events.rs            # Tauri event definitions
```

### Component Responsibilities

**`player.rs`** — mpv Process Lifecycle
- Spawns mpv with IPC socket
- Sends commands via JSON IPC (play, pause, seek, get_position, get_duration)
- Observes properties (position updates, pause state changes)
- Handles process crashes and restarts
- Uses `tokio::net::UnixStream` for async IPC

**`clipper.rs`** — FFmpeg Subprocess Management
- Calculates clip times with pre/post buffers
- Spawns FFmpeg with stream-copy flags: `ffmpeg -ss <start> -to <end> -c copy input.mp4 output.mp4`
- Runs in `tokio::spawn` to avoid blocking
- Emits progress events via Tauri's event system
- Handles FFmpeg errors and partial outputs

**`storage.rs`** — SQLite Persistence
- Schema: `clips(id, video_path, clip_path, start_time, end_time, created_at)`
- Schema: `settings(key, value)` for key-value config storage
- Uses `rusqlite` for type-safe queries
- Provides CRUD operations for clips and settings

**`controller.rs`** — Orchestration Layer
- Owns `Player`, `Clipper`, `Storage` instances
- Exposes high-level operations:
  - `open_video(path) -> Result<VideoInfo>`
  - `save_clip() -> Result<ClipResult>`
  - `toggle_pause() -> Result<()>`
  - `seek(seconds: f64) -> Result<()>`
  - `get_clips() -> Result<Vec<Clip>>`
- Manages state transitions (e.g., "no video loaded" → "video loaded")

**`commands.rs`** — Tauri Command Handlers
- Thin wrappers around controller methods
- Deserialize IPC arguments (serde)
- Handle async operations (tokio)
- Convert errors to `InvokeError` for frontend

**`error.rs`** — Unified Error Handling
```rust
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("FFmpeg failed: {0}")]
    FfmpegError(String),
    #[error("mpv IPC error: {0}")]
    MpvIpcError(String),
    #[error("Storage error: {0}")]
    StorageError(#[from] rusqlite::Error),
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

impl From<AppError> for tauri::InvokeError {
    fn from(err: AppError) -> Self {
        InvokeError::from(err.to_string())
    }
}
```

### State Management

The controller is wrapped in `Arc<Mutex<Controller>>` (or `RwLock` for read-heavy paths) and registered as Tauri state:

```rust
fn main() {
    let controller = Arc::new(Mutex::new(Controller::new()));
    
    tauri::Builder::default()
        .manage(controller)
        .invoke_handler(tauri::generate_handler![
            commands::open_video,
            commands::save_clip,
            commands::toggle_pause,
            // ...
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

Commands access state via:
```rust
#[tauri::command]
async fn open_video(
    state: State<'_, Arc<Mutex<Controller>>>,
    path: String,
) -> Result<VideoInfo, InvokeError> {
    let mut ctrl = state.lock().await;
    ctrl.open_video(&path).await.map_err(Into::into)
}
```

## Svelte Frontend Architecture

### Directory Structure

```
src/
  lib/
    components/
      VideoPlayer.svelte      # Placeholder div + controls overlay
      ClipButton.svelte       # Hotkey-aware clip trigger
      ClipList.svelte         # Shows saved clips
      SettingsDialog.svelte   # Buffer times, hotkey config
      ProgressBar.svelte      # FFmpeg progress indicator
    stores/
      player.ts               # Reactive player state (position, duration, paused)
      clips.ts                # Clip list, undo stack
      settings.ts             # User preferences
    api.ts                    # Tauri invoke() wrappers with TypeScript types
  routes/
    +layout.svelte            # Main window layout
    +page.svelte              # Main view (video + controls + clip list)
```

### Component Design

**`VideoPlayer.svelte`** — The most complex component
```svelte
<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { playerStore } from '$lib/stores/player';

  let container: HTMLDivElement;
  let rect: DOMRect;

  $: if (rect) {
    invoke('position_mpv_window', {
      x: Math.round(rect.x),
      y: Math.round(rect.y),
      width: Math.round(rect.width),
      height: Math.round(rect.height),
    });
  }

  onMount(() => {
    const observer = new ResizeObserver(() => {
      rect = container.getBoundingClientRect();
    });
    observer.observe(container);
    return () => observer.disconnect();
  });
</script>

<div bind:this={container} class="video-placeholder">
  {#if !$playerStore.videoLoaded}
    <p>No video loaded — press O to open</p>
  {/if}
</div>

<style>
  .video-placeholder {
    background: #1a1a1a;
    width: 100%;
    aspect-ratio: 16/9;
    display: flex;
    align-items: center;
    justify-content: center;
  }
</style>
```

**Key pattern:** The component only renders a placeholder. The actual mpv window is positioned over it by Rust. This avoids embedding complexity while giving the user a visual anchor.

### State Management with Svelte Stores

**`player.ts`** — Reactive player state
```typescript
import { writable, derived } from 'svelte/store';
import { listen } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';

interface PlayerState {
  videoLoaded: boolean;
  duration: number;
  position: number;
  paused: boolean;
}

const state = writable<PlayerState>({
  videoLoaded: false,
  duration: 0,
  position: 0,
  paused: true,
});

// Listen for position updates from Rust
listen<{ position: number }>('player-position-updated', (event) => {
  state.update(s => ({ ...s, position: event.payload.position }));
});

// Listen for pause state changes
listen<{ paused: boolean }>('player-pause-updated', (event) => {
  state.update(s => ({ ...s, paused: event.payload.paused }));
});

export const playerStore = {
  subscribe: state.subscribe,
  togglePause: () => invoke('toggle_pause'),
  seek: (seconds: number) => invoke('seek', { seconds }),
};
```

**`clips.ts`** — Clip list with undo
```typescript
import { writable } from 'svelte/store';
import { listen } from '@tauri-apps/api/event';

interface Clip {
  id: number;
  path: string;
  startTime: number;
  endTime: number;
  createdAt: string;
}

const clips = writable<Clip[]>([]);

listen<Clip>('clip-saved', (event) => {
  clips.update(list => [event.payload, ...list]);
});

export const clipStore = {
  subscribe: clips.subscribe,
};
```

### API Layer

**`api.ts`** — Type-safe IPC wrappers
```typescript
import { invoke } from '@tauri-apps/api/core';

export interface VideoInfo {
  duration: number;
  path: string;
}

export interface ClipResult {
  success: boolean;
  path?: string;
  startTime?: number;
  endTime?: number;
  error?: string;
}

export const api = {
  openVideo: (path: string) => invoke<VideoInfo>('open_video', { path }),
  saveClip: () => invoke<ClipResult>('save_clip'),
  togglePause: () => invoke<void>('toggle_pause'),
  seek: (seconds: number) => invoke<void>('seek', { seconds }),
  getClips: () => invoke<Clip[]>('get_clips'),
  getSettings: () => invoke<Settings>('get_settings'),
  updateSettings: (settings: Settings) => invoke<void>('update_settings', { settings }),
};
```

## Data Flow Examples

### Video Playback Flow

```
User clicks "Open Video"
  ↓
Svelte: api.openVideo(path)
  ↓
Tauri IPC: invoke('open_video', { path })
  ↓
Rust: commands::open_video
  ├─ Spawn mpv: --idle --input-ipc-server=/tmp/jorja-mpv-socket
  ├─ Create child window for mpv (hidden)
  ├─ Query mpv for duration via IPC
  └─ Return { duration: 120.5, path }
  ↓
Svelte: VideoPlayer.onMount
  ├─ Get container bounding rect
  └─ invoke('position_mpv_window', { x, y, width, height })
  ↓
Rust: commands::position_mpv_window
  ├─ Tauri Window::set_position()
  └─ Tauri Window::show()
  ↓
User clicks Play
  ↓
Svelte: api.togglePause()
  ↓
Rust: commands::toggle_pause
  └─ Send to mpv: {"command": ["cycle", "pause"]}
  ↓
mpv: Renders frames into window
  ↓
Rust: mpv property observer (time-pos)
  └─ Emit event: 'player-position-updated' { position: 5.2 }
  ↓
Svelte: playerStore updates
  └─ UI re-renders with new position
```

### Clipping Flow

```
User presses hotkey (e.g., 'C')
  ↓
Tauri: Global shortcut handler
  ↓
Rust: controller.save_clip()
  ├─ Query mpv: get_property('time-pos') → 45.3
  ├─ Calculate: start = 45.3 - 5s = 40.3, end = 45.3 + 5s = 50.3
  ├─ Spawn FFmpeg in tokio::spawn
  │   └─ ffmpeg -ss 40.3 -to 50.3 -c copy input.mp4 output.mp4
  └─ Emit event: 'clip-progress' { percent: 0 }
  ↓
FFmpeg: Processing
  ├─ Emit event: 'clip-progress' { percent: 50 }
  └─ Emit event: 'clip-progress' { percent: 100 }
  ↓
Rust: FFmpeg completes
  ├─ Save to SQLite: INSERT INTO clips (video_path, clip_path, ...)
  ├─ Emit event: 'clip-saved' { path, start_time, end_time }
  └─ Return ClipResult { success: true, ... }
  ↓
Svelte: clipStore receives event
  └─ UI updates with new clip in list
```

## Error Handling Strategy

### Backend Errors

All backend functions return `Result<T, AppError>`. Errors are categorized:

- **Recoverable errors** (e.g., FFmpeg not found) → Show dialog, suggest installing FFmpeg
- **Transient errors** (e.g., mpv process crashed) → Auto-restart mpv, notify user
- **Fatal errors** (e.g., SQLite corruption) → Log, show error, disable affected features

### Frontend Error Handling

```typescript
try {
  await api.saveClip();
} catch (error) {
  if (error.includes('FFmpeg not found')) {
    showErrorDialog('FFmpeg is required for clipping. Please install it.');
  } else if (error.includes('mpv process crashed')) {
    showNotification('Video player restarted. Please try again.');
  } else {
    showNotification(`Error: ${error}`);
  }
}
```

### Rust Error Conversion

```rust
impl From<AppError> for InvokeError {
    fn from(err: AppError) -> Self {
        match err {
            AppError::FfmpegNotFound => {
                InvokeError::from("FFmpeg not found. Please install FFmpeg.")
            }
            AppError::MpvCrashed => {
                InvokeError::from("Video player crashed. Attempting restart.")
            }
            _ => InvokeError::from(err.to_string()),
        }
    }
}
```

## Testing Strategy

### Backend Tests

**Unit tests** for pure functions:
- `clipper::calculate_times(position, duration, buffer_before, buffer_after)`
- `storage::ClipStore::add_clip()`, `get_clips_for_video()`
- `settings::Settings::load()`, `save()`

**Integration tests** for component interactions:
- Spawn real mpv process, send IPC commands, verify responses
- Spawn real FFmpeg, verify output file exists and is valid
- Test SQLite operations with in-memory database

**Example:**
```rust
#[tokio::test]
async fn test_player_seek() {
    let mut player = Player::new().await.unwrap();
    player.load("test.mp4").await.unwrap();
    player.seek(10.0).await.unwrap();
    
    let position = player.get_position().await.unwrap();
    assert!((position - 10.0).abs() < 0.1);
}
```

### Frontend Tests

**Component tests** using Vitest + Testing Library:
- `VideoPlayer` renders placeholder when no video loaded
- `ClipList` displays clips from store
- `SettingsDialog` validates input

**E2E tests** using Tauri's test harness:
- Open video → verify mpv window appears
- Click play → verify position updates
- Press hotkey → verify clip saved to database

## Platform Considerations

### Linux

**Wayland support:**
- Tauri's windowing layer handles Wayland natively
- mpv spawned with `--wid=<native-window-id>` works reliably
- No need for XCB/XWayland workarounds

**Dependencies:**
- `libmpv-dev` for mpv binary
- `ffmpeg` for clipping
- `libsqlite3-dev` for storage (bundled with rusqlite)

### Windows

**mpv window embedding:**
- Use `--wid=<HWND>` (Tauri provides valid HWND)
- No special handling needed

**FFmpeg:**
- Bundle FFmpeg binary in app resources
- Use `tauri::api::process::Command` to spawn

### macOS

**mpv integration:**
- Use mpv's render API (same as current Python implementation)
- Render into Metal/OpenGL texture
- Display in Tauri webview via `<canvas>`

**Code signing:**
- Sign mpv binary and FFmpeg binary
- Sign Tauri app bundle
- Notarize via Apple Developer portal

## Migration Path

### Phase 1: Backend Foundation (Week 1-2)
- Scaffold Tauri project
- Implement `player.rs` with mpv IPC
- Implement `clipper.rs` with FFmpeg integration
- Implement `storage.rs` with SQLite
- Write unit tests for all modules

### Phase 2: IPC Layer (Week 3)
- Define Tauri commands in `commands.rs`
- Implement `controller.rs` orchestration
- Test commands via `cargo tauri invoke`

### Phase 3: Frontend MVP (Week 4)
- Scaffold Svelte app
- Implement `VideoPlayer` with placeholder pattern
- Implement basic controls (play/pause, seek)
- Implement `ClipButton` with hotkey support

### Phase 4: Feature Parity (Week 5-6)
- Implement `ClipList` with undo
- Implement `SettingsDialog`
- Add theming support
- Polish UI/UX

### Phase 5: Testing & Packaging (Week 7-8)
- Write integration tests
- Write E2E tests
- Build packages for Linux, Windows, macOS
- Sign and notarize macOS build

## Success Criteria

The port is successful when:

1. **Video playback works reliably** on Linux Wayland (no embedding issues)
2. **Clipping is instant** (FFmpeg stream-copy, <1s for 10s clip)
3. **All current features work** (hotkeys, settings, clip list, undo)
4. **Binary size <50MB** (excluding FFmpeg)
5. **Startup time <2s** (cold start to interactive)
6. **Tests pass** on CI for Linux, Windows, macOS

## Open Questions

1. **Should we bundle FFmpeg or require system FFmpeg?**
   - Bundle: Larger binary, but works out-of-the-box
   - System: Smaller binary, but requires user to install FFmpeg
   - **Recommendation:** Bundle on Windows/macOS, use system on Linux (package managers)

2. **Should the plugin system be ported?**
   - Current system uses Python plugins (dynamic loading)
   - Rust equivalent: WebAssembly plugins or shared libraries
   - **Recommendation:** Defer to Phase 2, focus on core features first

3. **Should we support multiple video players?**
   - mpv is the current choice
   - Alternatives: GStreamer, FFplay
   - **Recommendation:** Stick with mpv, proven and feature-rich

## Conclusion

This Rust/Tauri port addresses the core mpv embedding issues while providing a modern, type-safe architecture. The three-layer separation (backend, IPC, frontend) enables independent development and testing. The Svelte frontend provides a reactive UI with minimal overhead.

**Next step:** Create implementation plan using `writing-plans` skill.
