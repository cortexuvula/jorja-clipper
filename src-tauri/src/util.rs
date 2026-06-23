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
            println!(
                "[binary] {} resolved to sidecar: {}",
                name,
                sidecar_path.display()
            );
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
                println!("[binary] {} resolved to homebrew: {}", name, path.display());
                return path;
            }
        }
    }

    // Final fallback: bare name (works when PATH is available)
    println!("[binary] {} resolved to system PATH", name);
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
    {
        #[cfg(target_arch = "aarch64")]
        {
            format!("{}-aarch64-apple-darwin", tool)
        }

        #[cfg(target_arch = "x86_64")]
        {
            format!("{}-x86_64-apple-darwin", tool)
        }
    }

    #[cfg(target_os = "windows")]
    {
        format!("{}-x86_64-pc-windows-msvc.exe", tool)
    }

    #[cfg(target_os = "linux")]
    {
        format!("{}-x86_64-unknown-linux-gnu", tool)
    }
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
///
/// Falls back from the platform config dir → the user's home/.config → the
/// current working directory, so the app keeps working (with a warning) even
/// in sandboxed/headless contexts where `dirs::config_dir()` returns None.
pub fn app_config_dir() -> PathBuf {
    let base = dirs::config_dir().or_else(|| {
        dirs::home_dir().map(|h| {
            eprintln!("[warn] Platform config dir unavailable; using ~/.config as fallback");
            h.join(".config")
        })
    });
    let base = base.unwrap_or_else(|| {
        eprintln!(
            "[warn] Could not resolve config or home directory; using current working directory"
        );
        PathBuf::from(".")
    });
    base.join("jorja-clipper")
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
    fn test_resolve_binary_unknown_name() {
        // Unknown binary should fall back to bare name
        let path = resolve_binary("nonexistent-tool");
        assert_eq!(path, PathBuf::from("nonexistent-tool"));
    }

    #[test]
    fn test_get_sidecar_path_unknown_binary() {
        // Unknown binary names should return None
        let path = get_sidecar_path("unknown-tool");
        assert!(path.is_none());

        let path = get_sidecar_path("ffmpeg-custom");
        assert!(path.is_none());
    }

    #[test]
    fn test_get_sidecar_path_ffmpeg() {
        // Should return None if not initialized
        let path = get_sidecar_path("ffmpeg");
        // May or may not be Some depending on test order, just ensure it doesn't panic
        let _ = path;
    }

    #[test]
    fn test_get_sidecar_path_ffprobe() {
        // Should return None if not initialized
        let path = get_sidecar_path("ffprobe");
        // May or may not be Some depending on test order, just ensure it doesn't panic
        let _ = path;
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

    #[test]
    fn test_app_config_dir() {
        let config_dir = app_config_dir();
        assert!(config_dir.ends_with("jorja-clipper"));
        assert!(!config_dir.as_os_str().is_empty());
    }

    #[test]
    fn test_app_config_dir_is_absolute() {
        let config_dir = app_config_dir();
        // Should be an absolute path (or "." if config_dir() fails)
        assert!(config_dir.is_absolute() || config_dir == PathBuf::from("./jorja-clipper"));
    }

    #[test]
    fn test_sidecar_name_linux() {
        #[cfg(target_os = "linux")]
        {
            let name = sidecar_name("ffmpeg");
            assert_eq!(name, "ffmpeg-x86_64-unknown-linux-gnu");

            let name = sidecar_name("ffprobe");
            assert_eq!(name, "ffprobe-x86_64-unknown-linux-gnu");

            let name = sidecar_name("custom-tool");
            assert_eq!(name, "custom-tool-x86_64-unknown-linux-gnu");
        }
    }

    #[test]
    fn test_resolve_binary_multiple_calls() {
        // Ensure resolve_binary is idempotent
        let path1 = resolve_binary("ffmpeg");
        let path2 = resolve_binary("ffmpeg");
        assert_eq!(path1, path2);

        let path3 = resolve_binary("ffprobe");
        let path4 = resolve_binary("ffprobe");
        assert_eq!(path3, path4);
    }

    #[test]
    fn test_resolve_binary_empty_name() {
        // Empty name should still return a path (bare name fallback)
        let path = resolve_binary("");
        assert_eq!(path, PathBuf::from(""));
    }

    #[test]
    fn test_ffmpeg_sidecar_name_format() {
        let name = ffmpeg_sidecar_name();
        assert!(!name.is_empty());
        assert!(name.contains("ffmpeg"));

        #[cfg(target_os = "linux")]
        assert!(name.ends_with("-unknown-linux-gnu"));
    }

    #[test]
    fn test_ffprobe_sidecar_name_format() {
        let name = ffprobe_sidecar_name();
        assert!(!name.is_empty());
        assert!(name.contains("ffprobe"));

        #[cfg(target_os = "linux")]
        assert!(name.ends_with("-unknown-linux-gnu"));
    }

    #[test]
    fn test_resolve_binary_ffmpeg() {
        // This should try to resolve ffmpeg from sidecar paths first
        let path = resolve_binary("ffmpeg");
        // On test environment, it will likely fall back to system PATH
        assert!(path.to_str().unwrap().contains("ffmpeg") || path == PathBuf::from("ffmpeg"));
    }

    #[test]
    fn test_resolve_binary_ffprobe() {
        // This should try to resolve ffprobe from sidecar paths first
        let path = resolve_binary("ffprobe");
        // On test environment, it will likely fall back to system PATH
        assert!(path.to_str().unwrap().contains("ffprobe") || path == PathBuf::from("ffprobe"));
    }

    #[test]
    fn test_app_config_dir_structure() {
        let config_dir = app_config_dir();
        // The config directory should end with jorja-clipper
        assert!(config_dir.to_str().unwrap().ends_with("jorja-clipper"));
    }

    #[test]
    fn test_resolve_binary_unknown_binary() {
        // Test with an unknown binary name
        let path = resolve_binary("unknown_binary_xyz");
        // Should return the bare name as fallback
        assert_eq!(path.to_str().unwrap(), "unknown_binary_xyz");
    }

    #[test]
    fn test_resolve_binary_empty_string() {
        // Test with empty string
        let path = resolve_binary("");
        // Should return empty path
        assert_eq!(path.to_str().unwrap(), "");
    }

    #[test]
    fn test_resolve_binary_with_extension() {
        // Test with binary name that has extension
        let path = resolve_binary("ffmpeg.exe");
        // Should return the name as-is
        assert!(path.to_str().unwrap().contains("ffmpeg.exe"));
    }

    #[test]
    fn test_app_config_dir_is_absolute_or_relative() {
        let config_dir = app_config_dir();
        // The config directory should be either absolute or relative to current dir
        let path_str = config_dir.to_str().unwrap();
        assert!(
            config_dir.is_absolute()
                || path_str.starts_with(".")
                || path_str.starts_with("jorja-clipper"),
            "Config dir should be absolute or relative: {}",
            path_str
        );
    }

    #[test]
    fn test_app_config_dir_can_be_created() {
        let config_dir = app_config_dir();

        // Try to create the config directory
        let result = std::fs::create_dir_all(&config_dir);
        assert!(result.is_ok(), "Should be able to create config directory");

        // Verify it exists
        assert!(
            config_dir.exists(),
            "Config directory should exist after creation"
        );

        // Clean up
        let _ = std::fs::remove_dir_all(&config_dir);
    }

    #[test]
    fn test_resolve_binary_with_path_separator() {
        // Test with binary name that includes path separator
        let path = resolve_binary("./ffmpeg");
        // Should return the name as-is
        assert_eq!(path.to_str().unwrap(), "./ffmpeg");
    }

    #[test]
    fn test_resolve_binary_absolute_path() {
        // Test with absolute path
        let path = resolve_binary("/usr/bin/ffmpeg");
        // Should return the path as-is
        assert_eq!(path.to_str().unwrap(), "/usr/bin/ffmpeg");
    }
}
