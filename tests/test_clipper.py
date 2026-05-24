"""Tests for the clip engine."""

from pathlib import Path
from unittest.mock import MagicMock, patch

from jorja_clipper.clipper import Clipper, ClipResult


def test_clip_result_fields():
    """ClipResult has the expected fields."""
    result = ClipResult(
        path="/tmp/test.mp4",
        start_time=10.0,
        end_time=20.0,
        success=True,
    )
    assert result.path == "/tmp/test.mp4"
    assert result.start_time == 10.0
    assert result.end_time == 20.0
    assert result.success is True


def test_clipper_default_config():
    """Clipper uses ±5 second buffer by default."""
    c = Clipper()
    assert c.buffer_before == 5.0
    assert c.buffer_after == 5.0


def test_clipper_custom_config():
    """Clipper accepts custom buffer durations."""
    c = Clipper(buffer_before=10.0, buffer_after=3.0)
    assert c.buffer_before == 10.0
    assert c.buffer_after == 3.0


def test_clipper_calculates_times():
    """Clipper correctly calculates start/end from current position."""
    c = Clipper(buffer_before=5.0, buffer_after=5.0)
    start, end = c.calculate_times(current_pos=30.0, video_duration=120.0)
    assert start == 25.0
    assert end == 35.0


def test_clipper_clamps_start_at_zero():
    """Start time clamps to 0 when near the beginning."""
    c = Clipper(buffer_before=5.0, buffer_after=5.0)
    start, end = c.calculate_times(current_pos=2.0, video_duration=120.0)
    assert start == 0.0
    assert end == 7.0


def test_clipper_clamps_end_at_duration():
    """End time clamps to video duration when near the end."""
    c = Clipper(buffer_before=5.0, buffer_after=5.0)
    start, end = c.calculate_times(current_pos=118.0, video_duration=120.0)
    assert start == 113.0
    assert end == 120.0


def test_clipper_builds_output_path(tmp_path):
    """Output path goes to clips/ folder next to the source video."""
    video = tmp_path / "game.mp4"
    video.touch()
    c = Clipper()
    out = c.build_output_path(video, clip_number=1)
    assert out.parent.name == "clips"
    assert out.name.startswith("game_clip_")
    assert out.suffix == ".mp4"


@patch("jorja_clipper.clipper.subprocess.run")
def test_clipper_save_calls_ffmpeg(mock_run):
    """save_clip invokes ffmpeg with correct -ss/-i/-t/-c copy args."""
    mock_run.return_value = MagicMock(returncode=0, stderr="")
    c = Clipper(buffer_before=5.0, buffer_after=5.0)
    result = c.save_clip(
        video_path=Path("/tmp/game.mp4"),
        current_pos=30.0,
        video_duration=120.0,
        clip_number=1,
    )
    assert result.success is True
    args = mock_run.call_args[0][0]
    assert args[0] == "ffmpeg"
    assert "-ss" in args
    assert "-c" in args
    assert "copy" in args


@patch("jorja_clipper.clipper.subprocess.run")
def test_clipper_save_handles_ffmpeg_not_found(mock_run):
    """save_clip returns failure result when ffmpeg is not in PATH."""
    mock_run.side_effect = FileNotFoundError("[Errno 2] No such file or directory: 'ffmpeg'")
    c = Clipper()
    result = c.save_clip(
        video_path=Path("/tmp/game.mp4"),
        current_pos=30.0,
        video_duration=120.0,
        clip_number=1,
    )
    assert result.success is False
    assert "ffmpeg" in result.error.lower() or "no such file" in result.error.lower()
