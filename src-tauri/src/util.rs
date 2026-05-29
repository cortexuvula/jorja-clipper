use std::path::PathBuf;
use std::sync::OnceLock;
use tauri::Manager;

/// Cache for resolved binary paths
static FFMPEG_PATH: OnceLock<PathBuf> = OnceLock::new();
static FFPROBE_PATH: OnceLock<PathBuf> = OnceLock::new();

/// Resolve a binary name to a full path.
///
/// Resolution order:
/// 1. Sidecar binary (bundled with the app)
/// 2. System PATH (with macOS homebrew path checks)
///
/// The sidecar binary is expected to be at the location returned by
/// `tauri::AppHandle::path_resolver().resolve_resource()`.
///
/// Falls back to looking up the binary in the system PATH if the sidecar
/// is not found (useful during development).
///
/// On macOS, GUI apps launched from Finder/Dock do not inherit the user's
/// shell PATH, so `Command::new("ffmpeg")` fails with ENOENT even when the
/// binary is installed via Homebrew. This function checks well-known paths
/// and falls back to the bare name (which works when PATH is available).
pub fn resolve_binary(name: &str) -> PathBuf {
    // First, try to get the sidecar path from the app handle
    if let Some(sidecar_path) = get_sidecar_path(name) {
        if sidecar_path.exists() {
            return sidecar_path;
        }
    }

    // Fall back to system PATH with macOS homebrew checks
    #[cfg(target_os = "macos")]
    {
        let candidates = [
            format!("/opt/homebrew/bin/{}", name),
            format!("/usr/local/bin/{}", name),
        ];
        for candidate in &candidates {
            let path = PathBuf::from(candidate);
            if path.exists() {
                return path;
            }
        }
    }

    // Final fallback: bare name (works when PATH is available)
    PathBuf::from(name)
}

/// Get the sidecar path for a binary.
///
/// This checks if we've initialized the sidecar paths during app startup.
fn get_sidecar_path(name: &str) -> Option<PathBuf> {
    match name {
        "ffmpeg" => FFMPEG_PATH.get().cloned(),
        "ffprobe" => FFPROBE_PATH.get().cloned(),
        _ => None,
    }
}

/// Initialize sidecar paths during app startup.
///
/// Call this from main.rs with the app handle to resolve sidecar paths.
/// This should be called once during app initialization.
pub fn init_sidecar_paths(app: &tauri::AppHandle) {
    let resolver = app.path();

    // Resolve ffmpeg sidecar path
    if let Ok(resource_dir) = resolver.resource_dir() {
        let binaries_dir = resource_dir.join("binaries");
        let ffmpeg_path = binaries_dir.join(ffmpeg_sidecar_name());
        let _ = FFMPEG_PATH.set(ffmpeg_path);

        // Resolve ffprobe sidecar path
        let ffprobe_path = binaries_dir.join(ffprobe_sidecar_name());
        let _ = FFPROBE_PATH.set(ffprobe_path);
    }
}

/// Get the sidecar binary name for a given tool (ffmpeg or ffprobe).
fn sidecar_name(tool: &str) -> String {
    #[cfg(target_os = "macos")]
    { format!("{}-x86_64-apple-darwin", tool) }

    #[cfg(target_os = "windows")]
    { format!("{}-x86_64-pc-windows-msvc.exe", tool) }

    #[cfg(target_os = "linux")]
    { format!("{}-x86_64-unknown-linux-gnu", tool) }
}

/// Get the FFmpeg sidecar binary name.
pub fn ffmpeg_sidecar_name() -> String {
    sidecar_name("ffmpeg")
}

/// Get the ffprobe sidecar binary name.
pub fn ffprobe_sidecar_name() -> String {
    sidecar_name("ffprobe")
}

/// Get the application config directory.
pub fn app_config_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("jorja-clipper")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_binary_fallback() {
        // Without initialization, should fall back to system PATH or homebrew paths
        let path = resolve_binary("ffmpeg");
        // On macOS, it might resolve to a homebrew path, so just check it's not empty
        assert!(!path.as_os_str().is_empty());
    }

    #[test]
    fn test_sidecar_names() {
        let ffmpeg_name = ffmpeg_sidecar_name();
        let ffprobe_name = ffprobe_sidecar_name();

        assert!(ffmpeg_name.starts_with("ffmpeg-"));
        assert!(ffprobe_name.starts_with("ffprobe-"));

        #[cfg(target_os = "macos")]
        {
            assert!(ffmpeg_name.contains("apple-darwin"));
            assert!(ffprobe_name.contains("apple-darwin"));
        }
        #[cfg(target_os = "windows")]
        {
            assert!(ffmpeg_name.ends_with(".exe"));
            assert!(ffprobe_name.ends_with(".exe"));
        }
        #[cfg(target_os = "linux")]
        {
            assert!(ffmpeg_name.contains("linux"));
            assert!(ffprobe_name.contains("linux"));
        }
    }
}
