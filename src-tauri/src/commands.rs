use std::path::PathBuf;
use std::sync::Arc;

#[cfg(target_os = "linux")]
use crate::x11_window::X11Window;

use tauri::State;
use tokio::sync::Mutex;

use crate::clipper::ClipResult;
use crate::controller::Controller;
use crate::storage::Clip;

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

        // Get the NSView pointer from raw-window-handle and extract contentView
        // Must extract before any .await since WindowHandle is !Send
        let content_view_ptr = {
            let main_window = app
                .get_webview_window("main")
                .ok_or("Main window not found")?;

            let ns_view_ptr = {
                let handle = main_window
                    .window_handle()
                    .map_err(|e| format!("Failed to get window handle: {}", e))?;

                match handle.as_raw() {
                    raw_window_handle::RawWindowHandle::AppKit(h) => h.ns_view.as_ptr(),
                    other => return Err(format!("Unsupported window handle type: {:?}", other)),
                }
            };

            // Get the contentView from the NSWindow
            unsafe {
                let ns_view: *mut objc2_app_kit::NSView = ns_view_ptr as *mut _;
                let window: *mut objc2_app_kit::NSWindow = objc2::msg_send![ns_view, window];
                let content_view: *mut objc2_app_kit::NSView = objc2::msg_send![window, contentView];
                content_view as u64 // Convert to u64 to make it Send
            }
        };

        // Close any existing mpv window
        {
            let mut ctrl = state.lock().await;
            ctrl.mpv_ns_view.take(); // Drop will remove the NSView
        }

        // Create NSView child
        let content_view = content_view_ptr as *mut objc2_app_kit::NSView;
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

#[tauri::command]
pub async fn open_video(
    state: State<'_, Arc<Mutex<Controller>>>,
    path: String,
    wid: Option<u64>,
) -> Result<f64, String> {
    let mut ctrl = state.lock().await;
    ctrl.open_video(PathBuf::from(&path), wid)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn toggle_pause(state: State<'_, Arc<Mutex<Controller>>>) -> Result<(), String> {
    let ctrl = state.lock().await;
    ctrl.toggle_pause().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn seek(state: State<'_, Arc<Mutex<Controller>>>, seconds: f64) -> Result<(), String> {
    let ctrl = state.lock().await;
    ctrl.seek(seconds).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_position(state: State<'_, Arc<Mutex<Controller>>>) -> Result<f64, String> {
    let ctrl = state.lock().await;
    ctrl.get_position().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn save_clip(state: State<'_, Arc<Mutex<Controller>>>) -> Result<ClipResult, String> {
    let mut ctrl = state.lock().await;
    ctrl.save_clip().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_clips(state: State<'_, Arc<Mutex<Controller>>>) -> Result<Vec<Clip>, String> {
    let ctrl = state.lock().await;
    ctrl.get_clips().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_clip(
    state: State<'_, Arc<Mutex<Controller>>>,
    id: i64,
    clip_path: String,
) -> Result<(), String> {
    let ctrl = state.lock().await;
    ctrl.delete_clip(id, &clip_path).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn shutdown(state: State<'_, Arc<Mutex<Controller>>>) -> Result<(), String> {
    let mut ctrl = state.lock().await;
    ctrl.shutdown().await;
    Ok(())
}
