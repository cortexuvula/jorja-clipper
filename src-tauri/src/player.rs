use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use tokio::process::{Child, Command};
use tokio::sync::Mutex;

use crate::error::{AppError, AppResult};

const IPC_SOCKET: &str = "/tmp/jorja-mpv-socket";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MpvRequest {
    command: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MpvResponse {
    error: String,
    data: Option<serde_json::Value>,
}

pub struct Player {
    process: Option<Child>,
    socket: Mutex<Option<UnixStream>>,
}

impl Player {
    pub fn new() -> Self {
        Self {
            process: None,
            socket: Mutex::new(None),
        }
    }

    /// Returns true if an mpv child process is currently running.
    pub fn is_running(&self) -> bool {
        self.process.is_some()
    }

    /// Spawn mpv as a child process with IPC server enabled.
    ///
    /// When `wid` is provided, mpv renders into the given native window handle.
    /// WAYLAND_DISPLAY is temporarily removed from the child environment when
    /// embedding, so mpv uses X11 rendering through the provided window id.
    pub async fn spawn(&mut self, wid: Option<u64>) -> AppResult<()> {
        // Clean up stale socket from a previous run
        let _ = std::fs::remove_file(IPC_SOCKET);

        let mut cmd = Command::new("mpv");
        cmd.args(&[
            "--idle",
            "--force-window",
            "--input-ipc-server=/tmp/jorja-mpv-socket",
        ]);

        if let Some(wid) = wid {
            cmd.arg(format!("--wid={}", wid));
        }

        // Prevent mpv from opening its own Wayland window when embedding
        let wayland_display = std::env::var("WAYLAND_DISPLAY").ok();
        if wid.is_some() {
            cmd.env_remove("WAYLAND_DISPLAY");
        }

        let child = cmd
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;

        self.process = Some(child);

        // Restore WAYLAND_DISPLAY in our own process so the rest of the app
        // (and future mpv spawns) still see it.
        if let Some(display) = wayland_display {
            // SAFETY: set_var is unsafe in Rust 2024+ editions due to UB with
            // concurrent reads. In 2021 edition this is fine. If the project
            // moves to 2024, wrap this in an unsafe block or use a mutex.
            std::env::set_var("WAYLAND_DISPLAY", display);
        }

        // Give mpv a moment to create the IPC socket
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        // Connect to the IPC socket
        self.connect_ipc().await?;

        Ok(())
    }

    /// Retry connecting to the mpv IPC socket with exponential-ish backoff.
    async fn connect_ipc(&self) -> AppResult<()> {
        let max_attempts = 10;

        for attempt in 1..=max_attempts {
            match UnixStream::connect(IPC_SOCKET).await {
                Ok(stream) => {
                    *self.socket.lock().await = Some(stream);
                    return Ok(());
                }
                Err(e) => {
                    if attempt >= max_attempts {
                        return Err(AppError::MpvIpc(format!(
                            "Failed to connect to mpv IPC socket after {} attempts: {}",
                            max_attempts, e
                        )));
                    }
                    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                }
            }
        }

        unreachable!()
    }

    /// Send a JSON IPC command to mpv and return the optional data payload.
    ///
    /// Note: This borrows the socket exclusively for the duration of the
    /// request/response cycle. The BufReader is created locally and dropped
    /// before the next call so we don't lose buffered bytes from a later
    /// response.
    async fn send_command(
        &self,
        command: Vec<serde_json::Value>,
    ) -> AppResult<Option<serde_json::Value>> {
        let mut socket_guard = self.socket.lock().await;
        let socket = socket_guard
            .as_mut()
            .ok_or_else(|| AppError::MpvIpc("Not connected to mpv".to_string()))?;

        let request = MpvRequest { command };
        let request_json =
            serde_json::to_string(&request).map_err(|e| AppError::MpvIpc(e.to_string()))?;

        socket.write_all(request_json.as_bytes()).await?;
        socket.write_all(b"\n").await?;
        socket.flush().await?;

        // Read exactly one line of response. We create the BufReader here and
        // drop it at the end of the function. Since mpv sends one JSON object
        // per line in response to synchronous commands, no bytes should be
        // left in the buffer for the next caller.
        let mut reader = BufReader::new(&mut *socket);
        let mut response_line = String::new();
        reader.read_line(&mut response_line).await?;

        let response: MpvResponse =
            serde_json::from_str(&response_line).map_err(|e| AppError::MpvIpc(e.to_string()))?;

        if response.error != "success" {
            return Err(AppError::MpvIpc(response.error));
        }

        Ok(response.data)
    }

    /// Load a video file into mpv, replacing whatever was playing.
    /// Returns the duration of the loaded file in seconds.
    pub async fn load(&mut self, path: &Path) -> AppResult<f64> {
        let path_str = path
            .to_str()
            .ok_or_else(|| AppError::MpvIpc("Invalid path (not valid UTF-8)".to_string()))?;

        self.send_command(vec![
            "loadfile".into(),
            path_str.into(),
            "replace".into(),
        ])
        .await?;

        // Wait for the file to finish loading before querying duration
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        let duration = self.get_duration().await?;
        Ok(duration)
    }

    /// Toggle the pause state.
    pub async fn toggle_pause(&self) -> AppResult<()> {
        self.send_command(vec!["cycle".into(), "pause".into()])
            .await?;
        Ok(())
    }

    /// Get the current playback position in seconds.
    pub async fn get_position(&self) -> AppResult<f64> {
        let data = self
            .send_command(vec!["get_property".into(), "time-pos".into()])
            .await?;

        data.and_then(|v| v.as_f64())
            .ok_or_else(|| AppError::MpvIpc("Invalid position response".to_string()))
    }

    /// Get the duration of the currently loaded file in seconds.
    pub async fn get_duration(&self) -> AppResult<f64> {
        let data = self
            .send_command(vec!["get_property".into(), "duration".into()])
            .await?;

        data.and_then(|v| v.as_f64())
            .ok_or_else(|| AppError::MpvIpc("Invalid duration response".to_string()))
    }

    /// Seek to a position. If `relative` is true, seek by `seconds` from the
    /// current position; otherwise seek to the absolute position.
    pub async fn seek(&self, seconds: f64, relative: bool) -> AppResult<()> {
        let mode = if relative { "relative" } else { "absolute" };

        self.send_command(vec![
            "seek".into(),
            seconds.into(),
            mode.into(),
        ])
        .await?;

        Ok(())
    }

    /// Kill the mpv child process and remove the IPC socket file.
    pub async fn shutdown(&mut self) {
        if let Some(mut child) = self.process.take() {
            let _ = child.kill().await;
        }
        let _ = std::fs::remove_file(IPC_SOCKET);
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        // Best-effort cleanup on drop. We can't await here, so use start_kill()
        // which sends SIGKILL without waiting.
        if let Some(mut child) = self.process.take() {
            let _ = child.start_kill();
        }
        let _ = std::fs::remove_file(IPC_SOCKET);
    }
}
