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
    # Check that the command is ffmpeg (could be full path or just "ffmpeg")
    # Case-insensitive check to handle Windows paths like "ffmpeg.EXE"
    ffmpeg_cmd = args[0].lower()
    is_ffmpeg = (
        ffmpeg_cmd.endswith("ffmpeg")
        or ffmpeg_cmd.endswith("ffmpeg.exe")
        or ffmpeg_cmd == "ffmpeg"
    )
    assert is_ffmpeg
    assert "-ss" in args
    assert "-c" in args
    assert "copy" in args


@patch("jorja_clipper.clipper.Clipper._find_ffmpeg")
def test_clipper_save_handles_ffmpeg_not_found(mock_find):
    """save_clip returns failure result when ffmpeg is not in PATH."""
    mock_find.return_value = None
    c = Clipper()
    result = c.save_clip(
        video_path=Path("/tmp/game.mp4"),
        current_pos=30.0,
        video_duration=120.0,
        clip_number=1,
    )
    assert result.success is False
    assert "ffmpeg" in result.error.lower() or "no such file" in result.error.lower()


@patch("jorja_clipper.clipper.subprocess.run")
def test_save_clip_cleans_up_partial_file_on_ffmpeg_failure(mock_run, tmp_path):
    """When ffmpeg exits non-zero, the partial output file is deleted."""
    mock_run.return_value = MagicMock(returncode=1, stderr="codec error")
    video = tmp_path / "game.mp4"
    video.touch()
    c = Clipper(buffer_before=5.0, buffer_after=5.0)
    result = c.save_clip(
        video_path=video,
        current_pos=30.0,
        video_duration=120.0,
        clip_number=1,
    )
    assert result.success is False
    assert result.path == ""
    # The clips/ directory may have been created, but the output file must be gone
    clips_dir = tmp_path / "clips"
    if clips_dir.exists():
        assert list(clips_dir.iterdir()) == []


@patch("jorja_clipper.clipper.subprocess.run")
def test_save_clip_cleans_up_when_partial_file_exists(mock_run, tmp_path):
    """Simulate ffmpeg creating a partial file before failing — it must be removed."""
    video = tmp_path / "game.mp4"
    video.touch()

    def fake_run(cmd, **kwargs):
        # Simulate ffmpeg: create the output file, then exit with error
        output = Path(cmd[-1])
        output.parent.mkdir(parents=True, exist_ok=True)
        output.write_bytes(b"\x00" * 64)  # partial/corrupt data
        return MagicMock(
            returncode=1,
            stderr="Invalid data found when processing input",
        )

    mock_run.side_effect = fake_run
    c = Clipper(buffer_before=5.0, buffer_after=5.0)
    result = c.save_clip(
        video_path=video,
        current_pos=30.0,
        video_duration=120.0,
        clip_number=1,
    )
    assert result.success is False
    assert result.path == ""
    # Verify the partial file was actually deleted
    clips_dir = tmp_path / "clips"
    if clips_dir.exists():
        assert list(clips_dir.iterdir()) == []


@patch("jorja_clipper.clipper.subprocess.run")
def test_save_clip_cleans_up_on_timeout(mock_run, tmp_path):
    """When ffmpeg times out, any partial output file is deleted."""
    video = tmp_path / "game.mp4"
    video.touch()

    import subprocess as sp

    def fake_run(cmd, **kwargs):
        output = Path(cmd[-1])
        output.parent.mkdir(parents=True, exist_ok=True)
        output.write_bytes(b"\x00" * 32)
        raise sp.TimeoutExpired(cmd=cmd, timeout=30)

    mock_run.side_effect = fake_run
    c = Clipper(buffer_before=5.0, buffer_after=5.0)
    result = c.save_clip(
        video_path=video,
        current_pos=30.0,
        video_duration=120.0,
        clip_number=1,
    )
    assert result.success is False
    assert "timed out" in result.error.lower()
    clips_dir = tmp_path / "clips"
    if clips_dir.exists():
        assert list(clips_dir.iterdir()) == []


# ---------------------------------------------------------------------------
# Property-based tests (hypothesis)
# ---------------------------------------------------------------------------

from hypothesis import given  # noqa: E402
from hypothesis import strategies as st  # noqa: E402


@given(
    current_pos=st.floats(min_value=0.0, max_value=1e6),
    video_duration=st.floats(min_value=0.0, max_value=1e6),
    buffer_before=st.floats(min_value=0.0, max_value=1e6),
    buffer_after=st.floats(min_value=0.0, max_value=1e6),
)
def test_calculate_times_clamps_inbounds(
    current_pos, video_duration, buffer_before, buffer_after
):
    """Start is always >= 0, end is always <= duration.

    When current_pos lies *inside* the video, start <= end.
    When current_pos > duration, start may exceed duration — this is an invalid
    playback position that ``save_clip`` guards against via ``end <= start``.
    """
    c = Clipper(buffer_before=buffer_before, buffer_after=buffer_after)
    start, end = c.calculate_times(current_pos, video_duration)
    assert start >= 0.0
    assert end <= video_duration or video_duration == 0.0
    if current_pos <= video_duration:
        assert start <= end or video_duration == 0.0


@given(
    current_pos=st.floats(min_value=-1e6, max_value=1e6),
    video_duration=st.floats(min_value=-1e6, max_value=1e6),
)
def test_calculate_times_never_raises(current_pos, video_duration):
    """calculate_times is total — it never throws for any float inputs."""
    c = Clipper()
    try:
        start, end = c.calculate_times(current_pos, video_duration)
        assert isinstance(start, float)
        assert isinstance(end, float)
    except Exception as exc:  # noqa: BLE001
        raise AssertionError("calculate_times should not raise") from exc


@patch("jorja_clipper.clipper.Clipper.build_output_path")
@patch("jorja_clipper.clipper.Clipper._find_ffmpeg")
def test_save_clip_catches_oserror_from_build_output_path(mock_find, mock_build):
    """OSError from build_output_path is caught and returned as a failure result."""
    mock_find.return_value = "/usr/bin/ffmpeg"
    mock_build.side_effect = PermissionError("Permission denied: '/tmp/clips'")
    c = Clipper()
    result = c.save_clip(
        video_path=Path("/tmp/game.mp4"),
        current_pos=30.0,
        video_duration=120.0,
        clip_number=1,
    )
    assert result.success is False
    assert "Permission denied" in result.error


@given(
    current_pos=st.floats(min_value=0.0, max_value=1e3),
    buffer_before=st.floats(min_value=1e3, max_value=1e6),
)
def test_buffer_larger_than_video_clamps_start_to_zero(current_pos, buffer_before):
    """When buffer_before exceeds current_pos, start clamps to 0."""
    c = Clipper(buffer_before=buffer_before, buffer_after=1.0)
    start, _ = c.calculate_times(current_pos, video_duration=current_pos + 2.0)
    assert start == 0.0


@given(
    current_pos=st.floats(min_value=0.0, max_value=1e3),
    buffer_after=st.floats(min_value=1e3, max_value=1e6),
)
def test_buffer_larger_than_video_clamps_end_to_duration(current_pos, buffer_after):
    """When buffer_after exceeds remaining duration, end clamps to duration."""
    c = Clipper(buffer_before=1.0, buffer_after=buffer_after)
    _, end = c.calculate_times(current_pos, video_duration=current_pos + 2.0)
    assert end == current_pos + 2.0
