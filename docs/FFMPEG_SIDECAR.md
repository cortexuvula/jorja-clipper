# FFmpeg Sidecar Integration

This document describes how FFmpeg is bundled as a sidecar binary with Jorja Clipper.

## Overview

FFmpeg and ffprobe are bundled as sidecar executables in release builds, eliminating the need for users to install FFmpeg separately.

## How It Works

### Development Mode

In development, the app falls back to system FFmpeg if available:
- macOS: Checks `/opt/homebrew/bin/` and `/usr/local/bin/`
- Windows/Linux: Uses system PATH

### Release Mode

In release builds, the app uses bundled sidecar binaries:
- Binaries are downloaded by `setup-ffmpeg.sh`
- Placed in `src-tauri/binaries/` with platform-specific names
- Tauri bundles them during `cargo tauri build`
- At runtime, `util::resolve_binary()` returns the sidecar path

## Setup

### For Developers

Before building release packages, run:

```bash
./setup-ffmpeg.sh
```

This script:
1. Detects your platform (macOS/Windows/Linux)
2. Creates symlinks to system FFmpeg for development
3. Downloads platform-specific release binaries
4. Places them in `src-tauri/binaries/`

### For Users

No setup required - FFmpeg is bundled with the app.

## Platform Support

| Platform | Binary Name | Source |
|----------|-------------|--------|
| macOS (Intel) | `ffmpeg-x86_64-apple-darwin` | evermeet.cx |
| macOS (Apple Silicon) | `ffmpeg-aarch64-apple-darwin` | evermeet.cx |
| Windows | `ffmpeg-x86_64-pc-windows-msvc.exe` | gyan.dev |
| Linux | `ffmpeg-x86_64-unknown-linux-gnu` | johnvansickle.com |

## Implementation Details

### Path Resolution (`src/util.rs`)

```rust
pub fn resolve_binary(name: &str) -> PathBuf {
    // 1. Check sidecar cache (initialized at startup)
    if let Some(sidecar_path) = get_sidecar_path(name) {
        if sidecar_path.exists() {
            return sidecar_path;
        }
    }

    // 2. macOS: Check Homebrew paths
    #[cfg(target_os = "macos")]
    { /* check /opt/homebrew/bin, /usr/local/bin */ }

    // 3. Fallback to system PATH
    PathBuf::from(name)
}
```

### Initialization (`src/main.rs`)

```rust
fn main() {
    tauri::Builder::default()
        .setup(|app| {
            util::init_sidecar_paths(app.handle());
            Ok(())
        })
        // ...
}
```

### Configuration (`tauri.conf.json`)

```json
{
  "bundle": {
    "externalBin": [
      "binaries/ffmpeg",
      "binaries/ffprobe"
    ]
  }
}
```

## Binary Sizes

- macOS (Intel): ~80 MB
- macOS (Apple Silicon): ~75 MB
- Windows: ~90 MB
- Linux: ~70 MB

## Updating FFmpeg

To update to a newer FFmpeg version:

1. Update URLs in `setup-ffmpeg.sh`
2. Run `./setup-ffmpeg.sh --force`
3. Rebuild release packages

## Troubleshooting

### "FFmpeg not found" in Development

Install FFmpeg on your system:
```bash
# macOS
brew install ffmpeg

# Ubuntu/Debian
sudo apt install ffmpeg

# Windows
choco install ffmpeg
```

### Sidecar Not Found in Release Build

1. Run `./setup-ffmpeg.sh` before building
2. Verify binaries exist in `src-tauri/binaries/`
3. Check `tauri.conf.json` includes `externalBin` config
4. Rebuild with `cargo tauri build`

### Permission Denied on macOS/Linux

Ensure binaries are executable:
```bash
chmod +x src-tauri/binaries/ffmpeg-*
chmod +x src-tauri/binaries/ffprobe-*
```

## Security Considerations

- Binaries are downloaded from official sources over HTTPS
- SHA256 checksums should be verified (TODO: implement)
- Binaries are not modified after download
- Users can verify binaries match official releases

## Future Improvements

- [ ] Add SHA256 checksum verification
- [ ] Support for additional architectures (ARM64 on Windows/Linux)
- [ ] Automatic FFmpeg updates
- [ ] Compression to reduce bundle size
