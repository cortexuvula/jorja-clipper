"""Shared test fixtures — mpv mock and test video generation."""

import sys
import types
from unittest.mock import MagicMock

import pytest

# Mock the mpv module BEFORE any test imports player.py.
# python-mpv requires libmpv at import time; CI runners on macOS/Windows
# don't have it. We inject a fake module so collection never hits the OSError.
_mpv_mock = types.ModuleType("mpv")
_mpv_mock.MPV = MagicMock
sys.modules.setdefault("mpv", _mpv_mock)


@pytest.fixture
def test_video(tmp_path):
    """Generate a 10-second test video using ffmpeg."""
    import subprocess

    video = tmp_path / "test.mp4"
    subprocess.run(
        [
            "ffmpeg",
            "-y",
            "-f",
            "lavfi",
            "-i",
            "testsrc=duration=10:size=320x240:rate=25",
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
            str(video),
        ],
        capture_output=True,
        check=True,
    )
    return video
