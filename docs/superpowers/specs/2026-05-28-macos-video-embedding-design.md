# macOS Video Embedding Design

**Date:** 2026-05-28
**Status:** Approved
**Scope:** macOS only (Windows to follow in a separate spec)

## Problem

On macOS, opening a video launches mpv in a separate standalone window instead of embedding the video inside the main application window. This happens because the video embedding logic is Linux-only (X11 child windows via `x11_window.rs`). On macOS, `create_mpv_window` returns an error, `mpvWid` stays `undefined`, and mpv spawns without `--wid`.

## Solution

Add a macOS-specific native module (`ns_view.rs`) that creates a child `NSView` inside the main Tauri window, mirroring the Linux `x11_window.rs` pattern. Pass the `NSView` pointer to mpv's `--wid` flag so mpv renders into the child view.

## Architecture

### File Structure

```
src-tauri/src/
  ├── ns_view.rs (NEW) — macOS: creates/manages child NSView for mpv
  ├── commands.rs (MODIFY) — add #[cfg(target_os = "macos")] branches
  ├── controller.rs (MODIFY) — add macOS ns_view field
  ├── player.rs (UNCHANGED) — already handles wid correctly
  └── main.rs (MODIFY) — register ns_view module
```

### Data Flow

1. `VideoPlayer.svelte` mounts and calls `create_mpv_window`
2. `commands.rs` gets the main window's `NSWindow` handle via `raw-window-handle`
3. `ns_view.rs` creates a child `NSView`, adds it as subview of the window's `contentView`
4. Returns the `NSView` pointer as `u64` (mpv's `--wid` format)
5. When the user opens a video, mpv gets `--wid=<nsview_ptr>` and renders into the child view
6. `position_mpv_window` converts CSS coords (top-left origin) to AppKit coords (bottom-left origin) and calls `setFrame:`

## The `ns_view.rs` Module

### API

```rust
pub struct NsView {
    view: *mut NSView,      // our child view
    parent: *mut NSView,    // the window's contentView
}

impl NsView {
    /// Create a child NSView inside the given parent NSView.
    /// `parent_ns_view` is the raw pointer from raw-window-handle's AppKitHandle.
    pub fn create_child(parent_ns_view: *mut NSView) -> Result<Self, String>

    /// Get the NSView pointer as u64 for mpv's --wid
    pub fn view_id(&self) -> u64

    /// Reposition and resize the child view.
    /// x, y are in CSS pixels (top-left origin) — we flip to AppKit coords internally.
    pub fn configure(&self, x: i32, y: i32, width: u32, height: u32) -> Result<(), String>

    /// Show the view (setHidden:NO)
    pub fn show(&mut self) -> Result<(), String>

    /// Hide the view (setHidden:YES)
    pub fn hide(&mut self) -> Result<(), String>
}

impl Drop for NsView {
    // Call removeFromSuperview to clean up
}
```

### Coordinate System

AppKit uses bottom-left origin; CSS uses top-left origin. Conversion:

```
appkit_y = parent_height - css_y - css_height
```

We get `parent_height` from `[parent frame].size.height` on each call (it can change on window resize), so no caching needed.

### Retina/HiDPI

Tauri's `position_mpv_window` sends CSS pixels. AppKit's `setFrame:` uses "points" (logical pixels). On most macOS displays these are 1:1. We handle this as a known simplification — if retina scaling issues emerge later, we can multiply by `backingScaleFactor`. For now, direct mapping.

### Thread Safety

AppKit UI calls must happen on the main thread. We'll use `dispatch_async` to the main queue for `show`/`hide`/`configure` calls, or use `objc2`'s `MainThreadMarker` where available. Tauri commands run on the main thread by default, so this should work without extra dispatch in most cases.

### View Styling

The child view gets a black background (`NSColor.blackColor`) so the video area looks correct before mpv starts rendering.

### Cleanup

When `NsView` is dropped (on shutdown or when creating a new window), we call `removeFromSuperview` on the child view.

## Changes to Existing Files

### `Cargo.toml` — Add macOS dependencies

```toml
[target.'cfg(target_os = "macos")'.dependencies]
objc2 = "0.5"
objc2-app-kit = { version = "0.2", features = ["NSView", "NSWindow", "NSColor"] }
objc2-foundation = "0.2"
```

### `main.rs` — Register the module

```rust
#[cfg(target_os = "macos")]
mod ns_view;
```

No `GDK_BACKEND` equivalent needed on macOS — AppKit is the native toolkit.

### `commands.rs` — Add `#[cfg(target_os = "macos")]` branches

Each of the three window management commands (`create_mpv_window`, `position_mpv_window`, `set_mpv_visible`) gets a macOS branch alongside the existing Linux one.

#### `create_mpv_window` on macOS

1. Get main window via `app.get_webview_window("main")`
2. Get `raw-window-handle` and match on `AppKitWindowHandle` to extract the `NSWindow` pointer
3. Get the window's `contentView` via `[window contentView]`
4. Call `NsView::create_child(content_view)`
5. Store in `ctrl.mpv_ns_view` and `ctrl.mpv_wid`
6. Return `view_id()` as u64

#### `position_mpv_window` on macOS

Delegate to `ctrl.mpv_ns_view.configure(x, y, w, h)` — coordinate flipping happens inside `NsView`.

#### `set_mpv_visible` on macOS

Delegate to `ctrl.mpv_ns_view.show()` or `.hide()`.

### `controller.rs` — Add macOS field

```rust
#[cfg(target_os = "macos")]
pub mpv_ns_view: Option<NsView>,
```

Initialized to `None` in `Controller::new()`. Cleaned up in `shutdown()` via `take()`.

### `player.rs` — No changes

The `spawn()` method already handles `wid: Option<u64>` correctly. On macOS, mpv's `--wid` accepts an `NSView` pointer, which is exactly what we pass. The `WAYLAND_DISPLAY` env_remove is already gated behind `#[cfg(unix)]` — on macOS this is harmless (env var doesn't exist anyway).

## Error Handling

| Scenario | Behavior |
|----------|----------|
| mpv not found | Already handled — `AppError::MpvNotFound` from `player.rs` |
| NSView creation fails | `create_mpv_window` returns error string; frontend logs it, mpv opens standalone (graceful fallback) |
| Position called before view exists | `ctrl.mpv_ns_view` is `None` — command silently succeeds (no-op), same as Linux |
| Unexpected window handle type | Returns descriptive error — shouldn't happen on macOS but handled defensively |
| AppKit calls off main thread | `objc2` will panic; mitigated by Tauri running commands on main thread by default |

## Unimplemented Platforms

Windows falls back to current behavior: `create_mpv_window` returns an error, mpv opens in its own standalone window. Windows embedding will be addressed in a follow-up spec.

## Testing

Manual testing only (no automated GUI tests):

1. Run `cargo tauri dev` on macOS
2. Open a video — confirm it renders inside the main window, not a separate window
3. Resize the window — confirm the video area resizes correctly
4. Open settings dialog — confirm the video hides behind the dialog
5. Close and reopen — confirm no crashes or leaks

## Out of Scope

- Windows embedding (separate follow-up spec)
- Retina/HiDPI scaling (known simplification; address if issues emerge)
- Unit tests for coordinate math (trivial logic, tested via manual verification)
