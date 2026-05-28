# macOS Video Embedding Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Embed mpv video rendering inside the main Tauri window on macOS using a child NSView, mirroring the Linux X11 child window pattern.

**Architecture:** Create a platform-specific `ns_view.rs` module that uses `objc2` to manage a child NSView. The module provides create/configure/show/hide/drop operations. Commands get macOS branches that delegate to this module. The controller stores the NSView reference.

**Tech Stack:** Rust, objc2, objc2-app-kit, objc2-foundation, Tauri 2, raw-window-handle

---

### Task 1: Add macOS Dependencies

**Files:**
- Modify: `src-tauri/Cargo.toml`

- [ ] **Step 1: Add objc2 dependencies for macOS**

Add this section after the existing `[target.'cfg(target_os = "linux")'.dependencies]` block:

```toml
[target.'cfg(target_os = "macos")'.dependencies]
objc2 = "0.5"
objc2-app-kit = { version = "0.2", features = ["NSView", "NSWindow", "NSColor", "NSResponder"] }
objc2-foundation = { version = "0.2", features = ["NSGeometry", "NSString"] }
```

- [ ] **Step 2: Verify dependencies compile**

Run: `cd src-tauri && cargo check`
Expected: Compiles successfully (may download crates)

- [ ] **Step 3: Commit**

```bash
git add src-tauri/Cargo.toml
git commit -m "chore: add objc2 dependencies for macOS embedding"
```

---

### Task 2: Create ns_view.rs Module

**Files:**
- Create: `src-tauri/src/ns_view.rs`

- [ ] **Step 1: Create the ns_view.rs file with complete implementation**

Create `src-tauri/src/ns_view.rs`:

```rust
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{msg_send, ClassType};
use objc2_app_kit::{NSColor, NSView, NSWindow};
use objc2_foundation::{CGFloat, NSPoint, NSRect, NSSize};

pub struct NsView {
    view: Retained<NSView>,
    parent: Retained<NSView>,
}

impl NsView {
    /// Create a child NSView inside the given parent NSView.
    /// `parent_ns_view` is the raw pointer from raw-window-handle's AppKitWindowHandle.
    ///
    /// # Safety
    /// The parent pointer must be a valid NSView pointer from the current application.
    pub fn create_child(parent_ns_view: *mut NSView) -> Result<Self, String> {
        if parent_ns_view.is_null() {
            return Err("Parent NSView pointer is null".to_string());
        }

        // Safety: We trust the caller to provide a valid parent pointer
        let parent = unsafe { Retained::retain(parent_ns_view) }
            .ok_or_else(|| "Failed to retain parent NSView".to_string())?;

        // Create child view with zero frame (will be configured later)
        let frame = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(1.0, 1.0));
        let view = unsafe {
            let allocated: Retained<NSView> = msg_send![NSView::class(), alloc];
            let initialized: Retained<NSView> = msg_send![allocated, initWithFrame: frame];
            initialized
        };

        // Set black background
        unsafe {
            let black = NSColor::blackColor();
            let _: () = msg_send![&view, setWantsLayer: true];
            let layer: *mut AnyObject = msg_send![&view, layer];
            if !layer.is_null() {
                let cg_black: *mut AnyObject = msg_send![&black, CGColor];
                let _: () = msg_send![layer, setBackgroundColor: cg_black];
            }
        }

        // Add as subview
        unsafe {
            let _: () = msg_send![&parent, addSubview: &view];
        }

        Ok(Self { view, parent })
    }

    /// Get the NSView pointer as u64 for mpv's --wid
    pub fn view_id(&self) -> u64 {
        Retained::as_ptr(&self.view) as u64
    }

    /// Reposition and resize the child view.
    /// x, y are in CSS pixels (top-left origin) — we flip to AppKit coords internally.
    /// width, height are in CSS pixels.
    pub fn configure(&self, x: i32, y: i32, width: u32, height: u32) -> Result<(), String> {
        // Get parent height for coordinate flipping
        let parent_frame: NSRect = unsafe { msg_send![&self.parent, frame] };
        let parent_height = parent_frame.size.height;

        // Convert from CSS (top-left origin) to AppKit (bottom-left origin)
        let appkit_x = x as CGFloat;
        let appkit_y = parent_height - (y as CGFloat) - (height as CGFloat);
        let appkit_width = width as CGFloat;
        let appkit_height = height as CGFloat;

        let frame = NSRect::new(
            NSPoint::new(appkit_x, appkit_y),
            NSSize::new(appkit_width, appkit_height),
        );

        unsafe {
            let _: () = msg_send![&self.view, setFrame: frame];
        }

        Ok(())
    }

    /// Show the view (setHidden:NO)
    pub fn show(&mut self) -> Result<(), String> {
        unsafe {
            let _: () = msg_send![&self.view, setHidden: false];
        }
        Ok(())
    }

    /// Hide the view (setHidden:YES)
    pub fn hide(&mut self) -> Result<(), String> {
        unsafe {
            let _: () = msg_send![&self.view, setHidden: true];
        }
        Ok(())
    }
}

impl Drop for NsView {
    fn drop(&mut self) {
        // Remove from parent to clean up
        unsafe {
            let _: () = msg_send![&self.view, removeFromSuperview];
        }
    }
}
```

- [ ] **Step 2: Verify the module compiles**

Run: `cd src-tauri && cargo check`
Expected: Compiles successfully (may show unused warnings — that's OK)

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/ns_view.rs
git commit -m "feat: add macOS NSView management module

Implements NsView struct for creating and managing a child NSView
inside the main Tauri window. Handles coordinate conversion from
CSS (top-left origin) to AppKit (bottom-left origin)."
```

---

### Task 3: Add macOS Field to Controller

**Files:**
- Modify: `src-tauri/src/controller.rs:1-30` (imports and struct definition)
- Modify: `src-tauri/src/controller.rs:35-53` (Controller::new)
- Modify: `src-tauri/src/controller.rs:180-190` (Controller::shutdown)

- [ ] **Step 1: Add macOS import at top of file**

After the existing `#[cfg(target_os = "linux")]` import block (line 4), add:

```rust
#[cfg(target_os = "macos")]
use crate::ns_view::NsView;
```

- [ ] **Step 2: Add macOS field to Controller struct**

In the `Controller` struct definition, after the `#[cfg(target_os = "linux")]` field (line 28), add:

```rust
    #[cfg(target_os = "macos")]
    pub mpv_ns_view: Option<NsView>,
```

- [ ] **Step 3: Initialize field in Controller::new**

In the `Ok(Self { ... })` block, after the `#[cfg(target_os = "linux")]` initialization (line 49-50), add:

```rust
            #[cfg(target_os = "macos")]
            mpv_ns_view: None,
```

- [ ] **Step 4: Clean up in shutdown method**

In the `shutdown` method, inside the `#[cfg(target_os = "macos")]` block (line 184), replace the existing block with:

```rust
        #[cfg(target_os = "macos")]
        {
            // Drop will remove the NSView from its parent
            self.mpv_ns_view.take();
        }
```

- [ ] **Step 5: Verify compilation**

Run: `cd src-tauri && cargo check`
Expected: Compiles successfully

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/controller.rs
git commit -m "feat: add macOS NSView field to Controller"
```

---

### Task 4: Add macOS Branches to Commands

**Files:**
- Modify: `src-tauri/src/commands.rs:17-67` (create_mpv_window)
- Modify: `src-tauri/src/commands.rs:71-93` (position_mpv_window)
- Modify: `src-tauri/src/commands.rs:96-118` (set_mpv_visible)

- [ ] **Step 1: Update create_mpv_window to handle macOS**

Replace the entire `create_mpv_window` function (lines 17-67) with:

```rust
/// Create an X11 child window inside the main Tauri window for mpv to render into.
/// Returns the native window ID (X11 window ID) for use with mpv's --wid.
#[tauri::command]
pub async fn create_mpv_window(
    app: tauri::AppHandle,
    state: State<'_, Arc<Mutex<Controller>>>,
) -> Result<u64, String> {
    #[cfg(target_os = "linux")]
    {
        use raw_window_handle::HasWindowHandle;
        use tauri::Manager;

        // Get the main window
        let main_window = app
            .get_webview_window("main")
            .ok_or("Main window not found")?;

        // Get the X11 window ID of the main window
        // Must extract before any .await since WindowHandle is !Send
        let parent_x11_id = {
            let handle = main_window
                .window_handle()
                .map_err(|e| format!("Failed to get window handle: {}", e))?;

            match handle.as_raw() {
                raw_window_handle::RawWindowHandle::Xlib(h) => h.window as u32,
                raw_window_handle::RawWindowHandle::Xcb(h) => h.window.get(),
                other => return Err(format!("Unsupported window handle type: {:?}", other)),
            }
        };

        // Close any existing mpv window
        {
            let mut ctrl = state.lock().await;
            ctrl.mpv_window.take(); // Drop will destroy the X11 window
        }

        // Create X11 child window
        let x11_window = X11Window::create_child(parent_x11_id)?;
        let wid = x11_window.window_id();

        // Store the X11 window reference
        let mut ctrl = state.lock().await;
        ctrl.mpv_window = Some(x11_window);
        ctrl.mpv_wid = Some(wid);

        Ok(wid)
    }

    #[cfg(target_os = "macos")]
    {
        use raw_window_handle::HasWindowHandle;
        use tauri::Manager;

        // Get the main window
        let main_window = app
            .get_webview_window("main")
            .ok_or("Main window not found")?;

        // Get the NSWindow pointer from raw-window-handle
        // Must extract before any .await since WindowHandle is !Send
        let ns_window_ptr = {
            let handle = main_window
                .window_handle()
                .map_err(|e| format!("Failed to get window handle: {}", e))?;

            match handle.as_raw() {
                raw_window_handle::RawWindowHandle::AppKit(h) => h.ns_window.as_ptr(),
                other => return Err(format!("Unsupported window handle type: {:?}", other)),
            }
        };

        // Get the contentView from the NSWindow
        let content_view = unsafe {
            let ns_window: *mut objc2_app_kit::NSWindow = ns_window_ptr as *mut _;
            let content_view: *mut objc2_app_kit::NSView = msg_send![ns_window, contentView];
            content_view
        };

        // Close any existing mpv window
        {
            let mut ctrl = state.lock().await;
            ctrl.mpv_ns_view.take(); // Drop will remove the NSView
        }

        // Create NSView child
        let ns_view = crate::ns_view::NsView::create_child(content_view)?;
        let wid = ns_view.view_id();

        // Store the NSView reference
        let mut ctrl = state.lock().await;
        ctrl.mpv_ns_view = Some(ns_view);
        ctrl.mpv_wid = Some(wid);

        Ok(wid)
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        Err("Video embedding only supported on Linux and macOS".to_string())
    }
}
```

- [ ] **Step 2: Update position_mpv_window to handle macOS**

Replace the entire `position_mpv_window` function (lines 71-93) with:

```rust
/// Reposition the mpv X11 child window to match the frontend's placeholder div.
/// Coordinates are in logical (CSS) pixels relative to the main window.
#[tauri::command]
pub async fn position_mpv_window(
    state: State<'_, Arc<Mutex<Controller>>>,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    {
        let mut ctrl = state.lock().await;
        if let Some(window) = &mut ctrl.mpv_window {
            window.configure(x as i32, y as i32, width as u32, height as u32)?;
            window.show()?;
        }
        Ok(())
    }

    #[cfg(target_os = "macos")]
    {
        let mut ctrl = state.lock().await;
        if let Some(ns_view) = &mut ctrl.mpv_ns_view {
            ns_view.configure(x as i32, y as i32, width as u32, height as u32)?;
            ns_view.show()?;
        }
        Ok(())
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        let _ = (x, y, width, height);
        Ok(())
    }
}
```

- [ ] **Step 3: Update set_mpv_visible to handle macOS**

Replace the entire `set_mpv_visible` function (lines 96-118) with:

```rust
/// Show or hide the mpv X11 child window (e.g. to let a dialog appear on top).
#[tauri::command]
pub async fn set_mpv_visible(
    state: State<'_, Arc<Mutex<Controller>>>,
    visible: bool,
) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    {
        let mut ctrl = state.lock().await;
        if let Some(window) = &mut ctrl.mpv_window {
            if visible {
                window.show()?;
            } else {
                window.hide()?;
            }
        }
        Ok(())
    }

    #[cfg(target_os = "macos")]
    {
        let mut ctrl = state.lock().await;
        if let Some(ns_view) = &mut ctrl.mpv_ns_view {
            if visible {
                ns_view.show()?;
            } else {
                ns_view.hide()?;
            }
        }
        Ok(())
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        let _ = visible;
        Ok(())
    }
}
```

- [ ] **Step 4: Verify compilation**

Run: `cd src-tauri && cargo check`
Expected: Compiles successfully

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/commands.rs
git commit -m "feat: add macOS branches to window management commands

Implements create_mpv_window, position_mpv_window, and set_mpv_visible
for macOS using NSView management."
```

---

### Task 5: Register ns_view Module in main.rs

**Files:**
- Modify: `src-tauri/src/main.rs:1-15` (module declarations)

- [ ] **Step 1: Add macOS module declaration**

After the `#[cfg(target_os = "linux")]` module declaration (line 12), add:

```rust
#[cfg(target_os = "macos")]
mod ns_view;
```

- [ ] **Step 2: Verify compilation**

Run: `cd src-tauri && cargo check`
Expected: Compiles successfully

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/main.rs
git commit -m "chore: register ns_view module for macOS"
```

---

### Task 6: Manual Testing

**Files:**
- No code changes

- [ ] **Step 1: Build and run the app**

Run: `cargo tauri dev`
Expected: App launches without errors

- [ ] **Step 2: Test video opening**

In the app:
1. Press 'O' to open a video file
2. Select a video file
3. Verify: Video renders inside the main window (not a separate window)
4. Verify: Video area has correct aspect ratio
5. Verify: No console errors

Expected: Video is embedded in the main window

- [ ] **Step 3: Test window resize**

In the app:
1. Resize the main window (make it larger and smaller)
2. Verify: Video area resizes proportionally
3. Verify: Video continues playing without interruption

Expected: Video area resizes correctly

- [ ] **Step 4: Test settings dialog**

In the app:
1. Click the "Settings" button
2. Verify: Settings dialog appears on top of the video
3. Verify: Video is hidden behind the dialog
4. Close the dialog
5. Verify: Video is visible again

Expected: Video hides/shows correctly with dialogs

- [ ] **Step 5: Test playback controls**

In the app:
1. Press Space to pause/play
2. Press Left/Right arrows to seek
3. Verify: Video responds to controls
4. Verify: Position updates in the status bar

Expected: All playback controls work correctly

- [ ] **Step 6: Test app restart**

In the app:
1. Close the app
2. Reopen with `cargo tauri dev`
3. Open a video again
4. Verify: No crashes or errors

Expected: App restarts cleanly

- [ ] **Step 7: Final commit (if any fixes were needed)**

If any fixes were made during testing:

```bash
git add -A
git commit -m "fix: address issues found during manual testing"
```

---

## Summary

**Total tasks:** 6
**Estimated time:** 30-45 minutes for implementation + 15-20 minutes for testing

**Key implementation points:**
- Uses `objc2` for type-safe Objective-C FFI
- Coordinate conversion handles CSS (top-left) vs AppKit (bottom-left) difference
- Proper cleanup via `Drop` trait
- Graceful fallback for unimplemented platforms

**Testing checklist:**
- Video embeds in main window (not separate window)
- Window resize works correctly
- Settings dialog overlays video correctly
- Playback controls work
- App restarts cleanly
