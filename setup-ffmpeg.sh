#!/bin/bash
# Setup script to download FFmpeg binaries for sidecar bundling
# Run this before building release packages
#
# Usage:
#   ./setup-ffmpeg.sh          # Download binaries for current platform
#   ./setup-ffmpeg.sh --all    # Download binaries for ALL platforms (for CI/CD)
#   ./setup-ffmpeg.sh --force  # Re-download even if binaries exist
#
# For development: Creates symlinks to system FFmpeg if available
# For release: Downloads platform-specific binaries

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BINARIES_DIR="$SCRIPT_DIR/src-tauri/binaries"

# Create binaries directory
mkdir -p "$BINARIES_DIR"

DOWNLOAD_ALL=false
FORCE_DOWNLOAD=false

# Parse arguments
for arg in "$@"; do
    case "$arg" in
        --all)
            DOWNLOAD_ALL=true
            ;;
        --force)
            FORCE_DOWNLOAD=true
            ;;
    esac
done

if [ "$DOWNLOAD_ALL" = true ]; then
    echo "Setting up FFmpeg sidecar binaries for ALL platforms..."
else
    echo "Setting up FFmpeg sidecar binaries..."
fi

# Function to download macOS binaries
download_macos() {
    local ARCH=$1
    local SUFFIX

    if [ "$ARCH" = "arm64" ]; then
        echo "Downloading macOS ARM64 (Apple Silicon) binaries..."
        SUFFIX="aarch64-apple-darwin"
    else
        echo "Downloading macOS x86_64 binaries..."
        SUFFIX="x86_64-apple-darwin"
    fi

    # Check if binaries already exist
    if [ -f "$BINARIES_DIR/ffmpeg-$SUFFIX" ] && [ -f "$BINARIES_DIR/ffprobe-$SUFFIX" ]; then
        echo "✓ Binaries already exist, skipping download"
        if [ "$FORCE_DOWNLOAD" != true ]; then
            return 0
        fi
    fi

    # Download release binaries
    echo "Downloading from evermeet.cx..."
    TEMP_DIR=$(mktemp -d)
    cd "$TEMP_DIR"

    curl -L "https://evermeet.cx/ffmpeg/get/zip" -o ffmpeg.zip
    unzip -q ffmpeg.zip
    mv ffmpeg "$BINARIES_DIR/ffmpeg-$SUFFIX"
    chmod +x "$BINARIES_DIR/ffmpeg-$SUFFIX"

    curl -L "https://evermeet.cx/ffmpeg/get/ffprobe/zip" -o ffprobe.zip
    unzip -q ffprobe.zip
    mv ffprobe "$BINARIES_DIR/ffprobe-$SUFFIX"
    chmod +x "$BINARIES_DIR/ffprobe-$SUFFIX"

    cd "$SCRIPT_DIR"
    rm -rf "$TEMP_DIR"
    echo "✓ macOS binaries installed"
}

# Function to download Linux binaries
download_linux() {
    local SUFFIX="x86_64-unknown-linux-gnu"

    echo "Downloading Linux x86_64 binaries..."

    # Check if binaries already exist
    if [ -f "$BINARIES_DIR/ffmpeg-$SUFFIX" ] && [ -f "$BINARIES_DIR/ffprobe-$SUFFIX" ]; then
        echo "✓ Binaries already exist, skipping download"
        if [ "$FORCE_DOWNLOAD" != true ]; then
            return 0
        fi
    fi

    # Download release binaries
    echo "Downloading from johnvansickle.com..."
    TEMP_DIR=$(mktemp -d)
    cd "$TEMP_DIR"

    curl -L "https://johnvansickle.com/ffmpeg/releases/ffmpeg-release-amd64-static.tar.xz" -o ffmpeg.tar.xz
    tar xf ffmpeg.tar.xz
    FFMPEG_DIR=$(tar tf ffmpeg.tar.xz | head -1 | cut -f1 -d"/")
    mv "$FFMPEG_DIR/ffmpeg" "$BINARIES_DIR/ffmpeg-$SUFFIX"
    mv "$FFMPEG_DIR/ffprobe" "$BINARIES_DIR/ffprobe-$SUFFIX"
    chmod +x "$BINARIES_DIR/ffmpeg-$SUFFIX"
    chmod +x "$BINARIES_DIR/ffprobe-$SUFFIX"

    cd "$SCRIPT_DIR"
    rm -rf "$TEMP_DIR"
    echo "✓ Linux binaries installed"
}

# Function to download Windows binaries
download_windows() {
    local SUFFIX="x86_64-pc-windows-msvc"

    echo "Downloading Windows x86_64 binaries..."

    # Check if binaries already exist
    if [ -f "$BINARIES_DIR/ffmpeg-$SUFFIX.exe" ] && [ -f "$BINARIES_DIR/ffprobe-$SUFFIX.exe" ]; then
        echo "✓ Binaries already exist, skipping download"
        if [ "$FORCE_DOWNLOAD" != true ]; then
            return 0
        fi
    fi

    # Download release binaries
    echo "Downloading from gyan.dev..."
    TEMP_DIR=$(mktemp -d)
    cd "$TEMP_DIR"

    curl -L "https://www.gyan.dev/ffmpeg/builds/ffmpeg-release-essentials.zip" -o ffmpeg.zip
    unzip -q ffmpeg.zip
    # Find the directory containing ffmpeg.exe
    FFMPEG_DIR=$(unzip -l ffmpeg.zip | grep "ffmpeg.exe" | head -1 | awk '{print $4}' | sed 's|/bin/ffmpeg.exe||')
    mv "$FFMPEG_DIR/bin/ffmpeg.exe" "$BINARIES_DIR/ffmpeg-$SUFFIX.exe"
    mv "$FFMPEG_DIR/bin/ffprobe.exe" "$BINARIES_DIR/ffprobe-$SUFFIX.exe"

    cd "$SCRIPT_DIR"
    rm -rf "$TEMP_DIR"
    echo "✓ Windows binaries installed"
}

# Function to setup current platform (for development)
setup_current_platform() {
    local OS="$(uname -s)"
    local ARCH="$(uname -m)"

    case "$OS" in
        Darwin)
            # Try to use system FFmpeg for development first
            if command -v ffmpeg &> /dev/null && command -v ffprobe &> /dev/null; then
                if [ "$ARCH" = "arm64" ]; then
                    SUFFIX="aarch64-apple-darwin"
                else
                    SUFFIX="x86_64-apple-darwin"
                fi

                if [ ! -f "$BINARIES_DIR/ffmpeg-$SUFFIX" ] || [ "$FORCE_DOWNLOAD" = true ]; then
                    echo "Found system FFmpeg, creating symlinks for development..."
                    ln -sf "$(command -v ffmpeg)" "$BINARIES_DIR/ffmpeg-$SUFFIX"
                    ln -sf "$(command -v ffprobe)" "$BINARIES_DIR/ffprobe-$SUFFIX"
                    echo "✓ Symlinks created for development"
                    return 0
                fi
            fi

            # Download release binaries if no symlinks
            download_macos "$ARCH"
            ;;

        Linux)
            # Try to use system FFmpeg for development first
            if command -v ffmpeg &> /dev/null && command -v ffprobe &> /dev/null; then
                SUFFIX="x86_64-unknown-linux-gnu"
                if [ ! -f "$BINARIES_DIR/ffmpeg-$SUFFIX" ] || [ "$FORCE_DOWNLOAD" = true ]; then
                    echo "Found system FFmpeg, creating symlinks for development..."
                    ln -sf "$(command -v ffmpeg)" "$BINARIES_DIR/ffmpeg-$SUFFIX"
                    ln -sf "$(command -v ffprobe)" "$BINARIES_DIR/ffprobe-$SUFFIX"
                    echo "✓ Symlinks created for development"
                    return 0
                fi
            fi

            # Download release binaries if no symlinks
            download_linux
            ;;

        MINGW*|MSYS*|CYGWIN*)
            download_windows
            ;;

        *)
            echo "Unsupported platform: $OS"
            exit 1
            ;;
    esac
}

# Main logic
if [ "$DOWNLOAD_ALL" = true ]; then
    # Download for all platforms
    download_macos "arm64"
    download_macos "x86_64"
    download_linux
    download_windows
else
    # Download for current platform only
    setup_current_platform
fi

echo ""
echo "✓ FFmpeg sidecar binaries installed:"
ls -lh "$BINARIES_DIR"
echo ""
if [ "$DOWNLOAD_ALL" = true ]; then
    echo "Ready for cross-platform CI/CD builds!"
else
    echo "You can now build release packages with: npm run tauri build"
fi
