use std::path::PathBuf;
use std::sync::Arc;

use tauri::State;
use tokio::sync::Mutex;

use crate::clipper::ClipResult;
use crate::controller::Controller;
use crate::storage::Clip;

/// Create a child window for mpv to render into.
/// Returns the native window ID (X11 window ID on Linux) for use with mpv's --wid.
#[tauri::command]
pub async fn create_mpv_window(
    app: tauri::AppHandle,
    state: State<'_, Arc<Mutex<Controller>>>,
) -> Result<u64, String> {
    use raw_window_handle::HasWindowHandle;
    use tauri::WindowBuilder;

    // Close any existing mpv window
    {
        let mut ctrl = state.lock().await;
        if let Some(win) = ctrl.mpv_window.take() {
            let _ = win.close();
        }
    }

    // Create a plain window (not WebviewWindow) for mpv rendering.
    // This avoids the WebKitGTK child window that would obscure mpv's rendering surface.
    // Position off-screen initially; position_mpv_window will move it to the correct location.
    let window = WindowBuilder::new(&app, "mpv-window")
        .title("mpv")
        .inner_size(1.0, 1.0)
        .position(-100.0, -100.0)  // off-screen initially
        .visible(true)
        .decorations(false)
        .build()
        .map_err(|e| format!("Failed to create mpv window: {}", e))?;

    // Keep it on top of the main window
    window
        .set_always_on_top(true)
        .map_err(|e| format!("Failed to set always on top: {}", e))?;

    // Get the native window handle for mpv's --wid parameter
    let wid = {
        let handle = window
            .window_handle()
            .map_err(|e| format!("Failed to get window handle: {}", e))?;

        match handle.as_raw() {
            raw_window_handle::RawWindowHandle::Xlib(h) => h.window as u64,
            raw_window_handle::RawWindowHandle::Xcb(h) => h.window.get() as u64,
            #[cfg(target_os = "windows")]
            raw_window_handle::RawWindowHandle::Win32(h) => h.hwnd.get() as u64,
            other => return Err(format!("Unsupported window handle type: {:?}", other)),
        }
    };

    // Store the window reference
    let mut ctrl = state.lock().await;
    ctrl.mpv_window = Some(window.clone());
    ctrl.mpv_wid = Some(wid);

    Ok(wid)
}

/// Reposition the mpv overlay window to match the frontend's placeholder div.
/// Coordinates are in logical (CSS) pixels.
#[tauri::command]
pub async fn position_mpv_window(
    state: State<'_, Arc<Mutex<Controller>>>,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
) -> Result<(), String> {
    let ctrl = state.lock().await;

    if let Some(window) = &ctrl.mpv_window {
        window
            .set_position(tauri::Position::Logical(tauri::LogicalPosition::new(x, y)))
            .map_err(|e| format!("Failed to set position: {}", e))?;
        window
            .set_size(tauri::Size::Logical(tauri::LogicalSize::new(width, height)))
            .map_err(|e| format!("Failed to set size: {}", e))?;
        if !window.is_visible().unwrap_or(false) {
            window
                .show()
                .map_err(|e| format!("Failed to show window: {}", e))?;
        }
    }

    Ok(())
}

#[tauri::command]
pub async fn open_video(
    state: State<'_, Arc<Mutex<Controller>>>,
    path: String,
    wid: Option<u64>,
) -> Result<f64, String> {
    let mut ctrl = state.lock().await;
    ctrl.open_video(PathBuf::from(path), wid)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn toggle_pause(
    state: State<'_, Arc<Mutex<Controller>>>,
) -> Result<(), String> {
    let ctrl = state.lock().await;
    ctrl.toggle_pause().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn seek(
    state: State<'_, Arc<Mutex<Controller>>>,
    seconds: f64,
) -> Result<(), String> {
    let ctrl = state.lock().await;
    ctrl.seek(seconds).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_position(
    state: State<'_, Arc<Mutex<Controller>>>,
) -> Result<f64, String> {
    let ctrl = state.lock().await;
    ctrl.get_position().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn save_clip(
    state: State<'_, Arc<Mutex<Controller>>>,
) -> Result<ClipResult, String> {
    let mut ctrl = state.lock().await;
    ctrl.save_clip().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_clips(
    state: State<'_, Arc<Mutex<Controller>>>,
) -> Result<Vec<Clip>, String> {
    let ctrl = state.lock().await;
    ctrl.get_clips().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn shutdown(
    state: State<'_, Arc<Mutex<Controller>>>,
) -> Result<(), String> {
    let mut ctrl = state.lock().await;
    ctrl.shutdown().await;
    Ok(())
}
