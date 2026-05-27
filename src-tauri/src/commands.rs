use std::path::PathBuf;
use std::sync::Arc;

use tauri::State;
use tokio::sync::Mutex;

use crate::clipper::ClipResult;
use crate::controller::Controller;
use crate::storage::Clip;

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
