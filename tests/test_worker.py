"""Tests for ClipWorker cancellation support."""

from pathlib import Path
from unittest.mock import MagicMock

from jorja_clipper.clipper import ClipResult
from jorja_clipper.worker import ClipWorker


def test_clip_worker_runs_and_emits_result():
    """ClipWorker emits finished with the result of save_clip."""
    clipper = MagicMock()
    clipper.save_clip.return_value = ClipResult(
        path="/tmp/out.mp4",
        start_time=25.0,
        end_time=35.0,
        success=True,
    )

    finished_results = []
    worker = ClipWorker(
        clipper=clipper,
        video_path=Path("/tmp/game.mp4"),
        current_pos=30.0,
        video_duration=120.0,
        clip_number=1,
    )
    worker.finished.connect(lambda r: finished_results.append(r))
    worker.run()

    assert len(finished_results) == 1
    assert finished_results[0].success is True
    clipper.save_clip.assert_called_once()


def test_clip_worker_interrupted_before_start():
    """ClipWorker does not run save_clip if cancelled before run()."""
    clipper = MagicMock()

    finished_results = []
    worker = ClipWorker(
        clipper=clipper,
        video_path=Path("/tmp/game.mp4"),
        current_pos=30.0,
        video_duration=120.0,
        clip_number=1,
    )
    worker.finished.connect(lambda r: finished_results.append(r))

    # Cancel before run
    worker.cancel()
    worker.run()

    clipper.save_clip.assert_not_called()
    assert len(finished_results) == 0


def test_clip_worker_interrupted_suppresses_signal():
    """ClipWorker suppresses the finished signal if cancelled mid-run."""
    clipper = MagicMock()
    clipper.save_clip.return_value = ClipResult(
        path="/tmp/out.mp4",
        start_time=25.0,
        end_time=35.0,
        success=True,
    )

    finished_results = []
    worker = ClipWorker(
        clipper=clipper,
        video_path=Path("/tmp/game.mp4"),
        current_pos=30.0,
        video_duration=120.0,
        clip_number=1,
    )
    worker.finished.connect(lambda r: finished_results.append(r))

    # Simulate cancellation happening during save_clip (after save_clip
    # returns but before the signal would be emitted). We do this by
    # having save_clip itself trigger the cancellation.
    def interrupt_during_save(*args, **kwargs):
        worker.cancel()
        return ClipResult(
            path="/tmp/out.mp4",
            start_time=25.0,
            end_time=35.0,
            success=True,
        )

    clipper.save_clip.side_effect = interrupt_during_save
    worker.run()

    # save_clip was called, but the finished signal was suppressed
    clipper.save_clip.assert_called_once()
    assert len(finished_results) == 0
