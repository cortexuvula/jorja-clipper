"""Integration tests for the full clip workflow."""

import shutil
from pathlib import Path

import pytest

from jorja_clipper.clipper import Clipper

pytestmark = pytest.mark.skipif(
    shutil.which("ffmpeg") is None,
    reason="ffmpeg not installed",
)


def test_full_clip_workflow(test_video, tmp_path):
    """Create a clip from a test video and verify the file exists."""
    clipper = Clipper(buffer_before=2.0, buffer_after=2.0)
    result = clipper.save_clip(
        video_path=test_video,
        current_pos=5.0,
        video_duration=10.0,
        clip_number=1,
    )
    assert result.success is True
    assert Path(result.path).exists()
    assert Path(result.path).stat().st_size > 0
    assert result.start_time == 3.0
    assert result.end_time == 7.0


def test_clip_near_start(test_video):
    """Clip near the start clamps to 0."""
    clipper = Clipper(buffer_before=5.0, buffer_after=5.0)
    result = clipper.save_clip(
        video_path=test_video,
        current_pos=1.0,
        video_duration=10.0,
        clip_number=1,
    )
    assert result.success is True
    assert result.start_time == 0.0
    assert result.end_time == 6.0


def test_clip_near_end(test_video):
    """Clip near the end clamps to duration."""
    clipper = Clipper(buffer_before=5.0, buffer_after=5.0)
    result = clipper.save_clip(
        video_path=test_video,
        current_pos=9.0,
        video_duration=10.0,
        clip_number=1,
    )
    assert result.success is True
    assert result.start_time == 4.0
    assert result.end_time == 10.0


def test_multiple_clips(test_video):
    """Multiple clips create distinct files."""
    clipper = Clipper(buffer_before=2.0, buffer_after=2.0)
    results = []
    for i, pos in enumerate([3.0, 5.0, 7.0], start=1):
        r = clipper.save_clip(
            video_path=test_video,
            current_pos=pos,
            video_duration=10.0,
            clip_number=i,
        )
        assert r.success is True
        results.append(r)

    paths = [r.path for r in results]
    assert len(set(paths)) == 3  # All unique filenames
    for p in paths:
        assert Path(p).exists()


def test_clip_output_in_clips_folder(test_video):
    """Clip is saved in a clips/ folder next to the source video."""
    clipper = Clipper(buffer_before=2.0, buffer_after=2.0)
    result = clipper.save_clip(
        video_path=test_video,
        current_pos=5.0,
        video_duration=10.0,
        clip_number=1,
    )
    assert result.success is True
    assert "clips" in Path(result.path).parts
