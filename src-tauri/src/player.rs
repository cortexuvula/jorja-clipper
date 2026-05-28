use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::Mutex;

#[cfg(unix)]
use tokio::net::UnixStream;

#[cfg(windows)]
use tokio::net::windows::named_pipe::ClientOptions;

use crate::error::{AppError, AppResult};

#[cfg(unix)]
const IPC_SOCKET: &str = "/tmp/jorja-mpv-socket";

#[cfg(windows)]
const IPC_PIPE: &str = r"\\.\pipe\jorja-mpv-socket";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MpvRequest {
    command: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MpvResponse {
    error: String,
    data: Option<serde_json::Value>,
}

#[cfg(unix)]
type IpcStream = UnixStream;

#[cfg(windows)]
type IpcStream = tokio::net::windows::named_pipe::NamedPipeClient;

pub struct Player {
    process: Option<Child>,
    socket: Mutex<Option<IpcStream>>,
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
        // Kill any existing mpv process first
        if let Some(mut old) = self.process.take() {
            let _ = old.kill().await;
        }
        *self.socket.lock().await = None;

        // Clean up stale socket from a previous run
        #[cfg(unix)]
        let _ = std::fs::remove_file(IPC_SOCKET);

        let mut cmd = Command::new("mpv");
        cmd.args([
            "--idle",
            "--force-window",
            #[cfg(unix)]
            &format!("--input-ipc-server={}", IPC_SOCKET),
            #[cfg(windows)]
            &format!("--input-ipc-server={}", IPC_PIPE),
        ]);

        if let Some(wid) = wid {
            cmd.arg(format!("--wid={}", wid));
            // Prevent mpv from opening its own Wayland window when embedding.
            // env_remove only affects the child's cloned environment — the
            // parent process keeps WAYLAND_DISPLAY untouched.
            #[cfg(unix)]
            cmd.env_remove("WAYLAND_DISPLAY");
        }

        let child = cmd.stdout(Stdio::null()).stderr(Stdio::null()).spawn()?;

        self.process = Some(child);

        // Give mpv a moment to create the IPC socket
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        // Connect to the IPC socket
        self.connect_ipc().await?;

        Ok(())
    }

    /// Retry connecting to the mpv IPC socket with exponential-ish backoff.
    #[cfg(unix)]
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

    /// Retry connecting to the mpv IPC named pipe with exponential-ish backoff.
    #[cfg(windows)]
    async fn connect_ipc(&self) -> AppResult<()> {
        let max_attempts = 10;

        for attempt in 1..=max_attempts {
            match ClientOptions::new().open(IPC_PIPE) {
                Ok(stream) => {
                    *self.socket.lock().await = Some(stream);
                    return Ok(());
                }
                Err(e) => {
                    if attempt >= max_attempts {
                        return Err(AppError::MpvIpc(format!(
                            "Failed to connect to mpv IPC pipe after {} attempts: {}",
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

        // mpv sends event notifications (like {"event":"playback-restart"}) mixed
        // with command responses. Command responses have an "error" field, events
        // have an "event" field. We need to skip events and find the response.
        let mut reader = BufReader::new(&mut *socket);
        let mut response_line = String::new();

        loop {
            response_line.clear();
            let bytes_read = reader.read_line(&mut response_line).await?;
            if bytes_read == 0 {
                return Err(AppError::MpvIpc("mpv IPC: connection closed".to_string()));
            }

            // Try to parse as a command response (has "error" field)
            match serde_json::from_str::<MpvResponse>(&response_line) {
                Ok(response) => {
                    // This is a command response
                    if response.error != "success" {
                        return Err(AppError::MpvIpc(response.error));
                    }
                    return Ok(response.data);
                }
                Err(_) => {
                    // This is likely an event notification, skip it
                    continue;
                }
            }
        }
    }

    /// Load a video file into mpv, replacing whatever was playing.
    /// Returns the duration of the loaded file in seconds.
    pub async fn load(&mut self, path: &Path) -> AppResult<f64> {
        let path_str = path
            .to_str()
            .ok_or_else(|| AppError::MpvIpc("Invalid path (not valid UTF-8)".to_string()))?;

        self.send_command(vec!["loadfile".into(), path_str.into(), "replace".into()])
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

        self.send_command(vec!["seek".into(), seconds.into(), mode.into()])
            .await?;

        Ok(())
    }

    /// Kill the mpv child process and remove the IPC socket file.
    pub async fn shutdown(&mut self) {
        if let Some(mut child) = self.process.take() {
            let _ = child.kill().await;
        }
        #[cfg(unix)]
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
        #[cfg(unix)]
        let _ = std::fs::remove_file(IPC_SOCKET);
    }
}
