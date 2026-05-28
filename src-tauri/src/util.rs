use std::path::PathBuf;

/// Resolve a binary name to a full path, checking common installation locations.
///
/// On macOS, GUI apps launched from Finder/Dock do not inherit the user's
/// shell PATH, so `Command::new("mpv")` fails with ENOENT even when the
/// binary is installed via Homebrew. This function checks well-known paths
/// and falls back to the bare name (which works when PATH is available).
pub fn resolve_binary(name: &str) -> PathBuf {
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
    PathBuf::from(name)
}
