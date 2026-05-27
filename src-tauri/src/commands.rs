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
        let main_window = app.get_webview_window("main")
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

    #[cfg(not(target_os = "linux"))]
    {
        Err("X11 windows only supported on Linux".to_string())
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

    #[cfg(not(target_os = "linux"))]
    {
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
