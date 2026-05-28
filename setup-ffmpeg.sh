#!/bin/bash
# Setup script to download FFmpeg binaries for sidecar bundling
# Run this before building release packages
#
# For development: Creates symlinks to system FFmpeg if available
# For release: Downloads platform-specific binaries

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BINARIES_DIR="$SCRIPT_DIR/src-tauri/binaries"

# Create binaries directory
mkdir -p "$BINARIES_DIR"

echo "Setting up FFmpeg sidecar binaries..."

# Detect platform
OS="$(uname -s)"
ARCH="$(uname -m)"

case "$OS" in
    Darwin)
        if [ "$ARCH" = "arm64" ]; then
            echo "Detected macOS ARM64 (Apple Silicon)"
            SUFFIX="aarch64-apple-darwin"
        else
            echo "Detected macOS x86_64"
            SUFFIX="x86_64-apple-darwin"
        fi

        # Check if binaries already exist
        if [ -f "$BINARIES_DIR/ffmpeg-$SUFFIX" ] && [ -f "$BINARIES_DIR/ffprobe-$SUFFIX" ]; then
            echo "✓ Binaries already exist, skipping download"
            echo "Run with --force to re-download"
            if [ "$1" != "--force" ]; then
                exit 0
            fi
        fi

        # Try to use system FFmpeg for development
        if command -v ffmpeg &> /dev/null && command -v ffprobe &> /dev/null; then
            echo "Found system FFmpeg, creating symlinks for development..."
            ln -sf "$(command -v ffmpeg)" "$BINARIES_DIR/ffmpeg-$SUFFIX"
            ln -sf "$(command -v ffprobe)" "$BINARIES_DIR/ffprobe-$SUFFIX"
            echo "✓ Symlinks created for development"
        fi

        # Download release binaries
        echo "Downloading release binaries from evermeet.cx..."
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
        ;;

    Linux)
        echo "Detected Linux x86_64"
        SUFFIX="x86_64-unknown-linux-gnu"

        # Check if binaries already exist
        if [ -f "$BINARIES_DIR/ffmpeg-$SUFFIX" ] && [ -f "$BINARIES_DIR/ffprobe-$SUFFIX" ]; then
            echo "✓ Binaries already exist, skipping download"
            if [ "$1" != "--force" ]; then
                exit 0
            fi
        fi

        # Try to use system FFmpeg for development
        if command -v ffmpeg &> /dev/null && command -v ffprobe &> /dev/null; then
            echo "Found system FFmpeg, creating symlinks for development..."
            ln -sf "$(command -v ffmpeg)" "$BINARIES_DIR/ffmpeg-$SUFFIX"
            ln -sf "$(command -v ffprobe)" "$BINARIES_DIR/ffprobe-$SUFFIX"
            echo "✓ Symlinks created for development"
        fi

        # Download release binaries
        echo "Downloading release binaries from johnvansickle.com..."
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
        ;;

    MINGW*|MSYS*|CYGWIN*)
        echo "Detected Windows x86_64"
        SUFFIX="x86_64-pc-windows-msvc"

        # Check if binaries already exist
        if [ -f "$BINARIES_DIR/ffmpeg-$SUFFIX.exe" ] && [ -f "$BINARIES_DIR/ffprobe-$SUFFIX.exe" ]; then
            echo "✓ Binaries already exist, skipping download"
            if [ "$1" != "--force" ]; then
                exit 0
            fi
        fi

        # Download release binaries
        echo "Downloading release binaries from gyan.dev..."
        TEMP_DIR=$(mktemp -d)
        cd "$TEMP_DIR"

        curl -L "https://www.gyan.dev/ffmpeg/builds/ffmpeg-release-essentials.zip" -o ffmpeg.zip
        unzip -q ffmpeg.zip
        FFMPEG_DIR=$(unzip -l ffmpeg.zip | grep "ffmpeg.exe" | head -1 | awk '{print $4}' | cut -d'/' -f1-2)
        mv "$FFMPEG_DIR/bin/ffmpeg.exe" "$BINARIES_DIR/ffmpeg-$SUFFIX.exe"
        mv "$FFMPEG_DIR/bin/ffprobe.exe" "$BINARIES_DIR/ffprobe-$SUFFIX.exe"

        cd "$SCRIPT_DIR"
        rm -rf "$TEMP_DIR"
        ;;

    *)
        echo "Unsupported platform: $OS"
        exit 1
        ;;
esac

echo ""
echo "✓ FFmpeg sidecar binaries installed:"
ls -lh "$BINARIES_DIR"
echo ""
echo "You can now build release packages with: npm run tauri build"
