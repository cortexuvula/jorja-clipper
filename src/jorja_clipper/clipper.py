"""Core clip engine — extracts clips via ffmpeg stream-copy."""

import subprocess
from dataclasses import dataclass
from datetime import datetime
from pathlib import Path


@dataclass
class ClipResult:
    """Result of a clip save operation."""

    path: str
    start_time: float
    end_time: float
    success: bool
    error: str = ""


class Clipper:
    """Extracts clips from video files using ffmpeg stream-copy."""

    def __init__(self, buffer_before: float = 5.0, buffer_after: float = 5.0):
        self.buffer_before = buffer_before
        self.buffer_after = buffer_after

    def calculate_times(
        self, current_pos: float, video_duration: float
    ) -> tuple[float, float]:
        """Calculate start/end times clamped to [0, duration]."""
        start = max(0.0, current_pos - self.buffer_before)
        end = min(video_duration, current_pos + self.buffer_after)
        return start, end

    def build_output_path(self, video_path: Path, clip_number: int) -> Path:
        """Build the output path in a clips/ folder next to the source video."""
        clips_dir = video_path.parent / "clips"
        clips_dir.mkdir(parents=True, exist_ok=True)
        timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
        stem = video_path.stem
        ext = video_path.suffix or ".mp4"
        name = f"{stem}_clip_{timestamp}_{clip_number:03d}{ext}"
        return clips_dir / name

    def save_clip(
        self,
        video_path: Path,
        current_pos: float,
        video_duration: float,
        clip_number: int,
    ) -> ClipResult:
        """Save a clip using ffmpeg stream-copy (no re-encoding)."""
        start, end = self.calculate_times(current_pos, video_duration)
        duration = end - start
        output_path = self.build_output_path(video_path, clip_number)

        cmd = [
            "ffmpeg",
            "-y",
            "-ss", str(start),
            "-i", str(video_path),
            "-t", str(duration),
            "-c", "copy",
            "-avoid_negative_ts", "make_zero",
            str(output_path),
        ]

        try:
            result = subprocess.run(
                cmd,
                capture_output=True,
                text=True,
                timeout=30,
            )
            if result.returncode == 0:
                return ClipResult(
                    path=str(output_path),
                    start_time=start,
                    end_time=end,
                    success=True,
                )
            return ClipResult(
                path="",
                start_time=start,
                end_time=end,
                success=False,
                error=result.stderr,
            )
        except Exception as e:
            return ClipResult(
                path="",
                start_time=start,
                end_time=end,
                success=False,
                error=str(e),
            )
