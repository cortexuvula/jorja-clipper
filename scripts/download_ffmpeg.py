#!/usr/bin/env python3
"""
Download FFmpeg binaries for sidecar bundling.
Downloads platform-specific builds and places them in src-tauri/binaries/
"""

import os
import sys
import shutil
import zipfile
import tarfile
import urllib.request
from pathlib import Path

# FFmpeg build URLs (using gyan.dev for Windows, evermeet.cx for macOS, johnvansickle.com for Linux)
FFMPEG_VERSION = "7.1"

DOWNLOAD_URLS = {
    "windows": {
        "ffmpeg": f"https://www.gyan.dev/ffmpeg/builds/ffmpeg-{FFMPEG_VERSION}-essentials_build.zip",
        "ffprobe": f"https://www.gyan.dev/ffmpeg/builds/ffmpeg-{FFMPEG_VERSION}-essentials_build.zip",  # Same zip
    },
    "macos": {
        "ffmpeg": f"https://evermeet.cx/ffmpeg/ffmpeg-{FFMPEG_VERSION}.zip",
        "ffprobe": f"https://evermeet.cx/ffmpeg/ffprobe-{FFMPEG_VERSION}.zip",
    },
    "linux": {
        "ffmpeg": f"https://johnvansickle.com/ffmpeg/releases/ffmpeg-release-amd64-static.tar.xz",
        "ffprobe": f"https://johnvansickle.com/ffmpeg/releases/ffmpeg-release-amd64-static.tar.xz",  # Same tarball
    },
}

def download_file(url: str, dest: Path) -> None:
    """Download a file with progress indication."""
    print(f"Downloading {url}...")
    urllib.request.urlretrieve(url, dest)
    print(f"  Saved to {dest}")

def extract_windows(src: Path, dest_dir: Path) -> None:
    """Extract FFmpeg from Windows zip."""
    print(f"Extracting {src.name}...")
    with zipfile.ZipFile(src, 'r') as zip_ref:
        # Find ffmpeg.exe and ffprobe.exe in the archive
        for member in zip_ref.namelist():
            if member.endswith('bin/ffmpeg.exe'):
                # Extract to temp location first
                zip_ref.extract(member, dest_dir)
                extracted = dest_dir / member
                shutil.move(str(extracted), str(dest_dir / "ffmpeg-x86_64-pc-windows-msvc.exe"))
            elif member.endswith('bin/ffprobe.exe'):
                zip_ref.extract(member, dest_dir)
                extracted = dest_dir / member
                shutil.move(str(extracted), str(dest_dir / "ffprobe-x86_64-pc-windows-msvc.exe"))

def extract_macos(src: Path, binary_name: str, dest_dir: Path) -> None:
    """Extract FFmpeg from macOS zip."""
    print(f"Extracting {src.name}...")
    with zipfile.ZipFile(src, 'r') as zip_ref:
        zip_ref.extractall(dest_dir)
        # The binary is directly in the zip
        if binary_name == "ffmpeg":
            shutil.move(str(dest_dir / "ffmpeg"), str(dest_dir / "ffmpeg-x86_64-apple-darwin"))
        else:
            shutil.move(str(dest_dir / "ffprobe"), str(dest_dir / "ffprobe-x86_64-apple-darwin"))

def extract_linux(src: Path, dest_dir: Path) -> None:
    """Extract FFmpeg from Linux tarball."""
    print(f"Extracting {src.name}...")
    with tarfile.open(src, 'r:xz') as tar_ref:
        # Find ffmpeg and ffprobe in the archive
        for member in tar_ref.getmembers():
            if member.name.endswith('/ffmpeg') and not member.name.endswith('/ffprobe'):
                tar_ref.extract(member, dest_dir)
                extracted = dest_dir / member.name
                shutil.move(str(extracted), str(dest_dir / "ffmpeg-x86_64-unknown-linux-gnu"))
            elif member.name.endswith('/ffprobe'):
                tar_ref.extract(member, dest_dir)
                extracted = dest_dir / member.name
                shutil.move(str(extracted), str(dest_dir / "ffprobe-x86_64-unknown-linux-gnu"))

def setup_binaries_dir() -> Path:
    """Create binaries directory if it doesn't exist."""
    binaries_dir = Path("src-tauri/binaries")
    binaries_dir.mkdir(parents=True, exist_ok=True)
    return binaries_dir

def download_for_platform(platform: str, binaries_dir: Path) -> None:
    """Download FFmpeg binaries for a specific platform."""
    print(f"\n{'='*60}")
    print(f"Setting up FFmpeg for {platform}")
    print(f"{'='*60}\n")

    if platform == "windows":
        zip_path = binaries_dir / "ffmpeg-windows.zip"
        download_file(DOWNLOAD_URLS["windows"]["ffmpeg"], zip_path)
        extract_windows(zip_path, binaries_dir)
        zip_path.unlink()

    elif platform == "macos":
        # Download ffmpeg
        ffmpeg_zip = binaries_dir / "ffmpeg-macos.zip"
        download_file(DOWNLOAD_URLS["macos"]["ffmpeg"], ffmpeg_zip)
        extract_macos(ffmpeg_zip, "ffmpeg", binaries_dir)
        ffmpeg_zip.unlink()

        # Download ffprobe
        ffprobe_zip = binaries_dir / "ffprobe-macos.zip"
        download_file(DOWNLOAD_URLS["macos"]["ffprobe"], ffprobe_zip)
        extract_macos(ffprobe_zip, "ffprobe", binaries_dir)
        ffprobe_zip.unlink()

    elif platform == "linux":
        tar_path = binaries_dir / "ffmpeg-linux.tar.xz"
        download_file(DOWNLOAD_URLS["linux"]["ffmpeg"], tar_path)
        extract_linux(tar_path, binaries_dir)
        tar_path.unlink()

    print(f"\n✓ FFmpeg binaries ready for {platform}")

def main():
    """Main entry point."""
    if len(sys.argv) < 2:
        print("Usage: python download_ffmpeg.py <platform>")
        print("Platforms: windows, macos, linux, all")
        sys.exit(1)

    platform = sys.argv[1].lower()
    binaries_dir = setup_binaries_dir()

    if platform == "all":
        for p in ["windows", "macos", "linux"]:
            try:
                download_for_platform(p, binaries_dir)
            except Exception as e:
                print(f"✗ Failed to download for {p}: {e}")
    elif platform in ["windows", "macos", "linux"]:
        try:
            download_for_platform(platform, binaries_dir)
        except Exception as e:
            print(f"✗ Failed: {e}")
            sys.exit(1)
    else:
        print(f"Unknown platform: {platform}")
        sys.exit(1)

    print("\n" + "="*60)
    print("All done! FFmpeg binaries are in src-tauri/binaries/")
    print("="*60)

if __name__ == "__main__":
    main()
