"""Shared test fixtures."""

import subprocess

import pytest


@pytest.fixture
def test_video(tmp_path):
    """Generate a 10-second test video using ffmpeg."""
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
