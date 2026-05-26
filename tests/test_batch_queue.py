"""Tests for the batch queue module."""

from pathlib import Path
from unittest.mock import MagicMock

from jorja_clipper.batch_queue import BatchWorker, ClipQueue, ClipRequest
from jorja_clipper.clipper import ClipResult


def test_clip_request_dataclass():
    """ClipRequest stores its fields."""
    req = ClipRequest(
        video_path=Path("/tmp/game.mp4"),
        current_pos=30.0,
        video_duration=120.0,
        clip_number=1,
    )
    assert req.video_path == Path("/tmp/game.mp4")
    assert req.current_pos == 30.0
    assert req.video_duration == 120.0
    assert req.clip_number == 1


def test_clip_queue_enqueue_dequeue():
    """Queue respects FIFO ordering."""
    q = ClipQueue()
    q.enqueue(ClipRequest(Path("/tmp/a.mp4"), 1.0, 10.0, 1))
    q.enqueue(ClipRequest(Path("/tmp/b.mp4"), 2.0, 10.0, 2))
    assert len(q) == 2
    first = q.dequeue()
    assert first is not None
    assert first.clip_number == 1
    assert len(q) == 1
    second = q.dequeue()
    assert second is not None
    assert second.clip_number == 2
    assert q.dequeue() is None


def test_clip_queue_clear():
    """Clear empties the queue."""
    q = ClipQueue()
    q.enqueue(ClipRequest(Path("/tmp/a.mp4"), 1.0, 10.0, 1))
    q.clear()
    assert len(q) == 0
    assert q.dequeue() is None


def test_batch_worker_runs_queue_items():
    """BatchWorker processes all items in the queue."""
    clipper = MagicMock()
    clipper.save_clip.return_value = ClipResult(
        path="/tmp/out.mp4",
        start_time=0.0,
        end_time=1.0,
        success=True,
    )
    q = ClipQueue()
    q.enqueue(ClipRequest(Path("/tmp/v.mp4"), 5.0, 10.0, 1))
    q.enqueue(ClipRequest(Path("/tmp/v.mp4"), 8.0, 10.0, 2))

    worker = BatchWorker(clipper, q)
    # Do NOT start the thread; just run synchronously for the test
    worker.run()

    assert len(q) == 0
    assert clipper.save_clip.call_count == 2
    # Both results are successful
    assert all(r.success for r in worker._results)


def test_batch_worker_stops_on_interruption():
    """BatchWorker stops processing when cancel() is called."""
    clipper = MagicMock()
    clipper.save_clip.return_value = ClipResult(
        path="/tmp/out.mp4",
        start_time=0.0,
        end_time=1.0,
        success=True,
    )
    q = ClipQueue()
    q.enqueue(ClipRequest(Path("/tmp/v.mp4"), 5.0, 10.0, 1))
    q.enqueue(ClipRequest(Path("/tmp/v.mp4"), 8.0, 10.0, 2))
    q.enqueue(ClipRequest(Path("/tmp/v.mp4"), 9.0, 10.0, 3))

    worker = BatchWorker(clipper, q)
    # Cancel before running — should process nothing
    worker.cancel()
    worker.run()

    assert clipper.save_clip.call_count == 0
    assert len(worker._results) == 0


def test_batch_worker_interrupted_no_finished_signal():
    """BatchWorker does not emit finished signal when cancelled."""
    clipper = MagicMock()
    clipper.save_clip.return_value = ClipResult(
        path="/tmp/out.mp4",
        start_time=0.0,
        end_time=1.0,
        success=True,
    )
    q = ClipQueue()
    q.enqueue(ClipRequest(Path("/tmp/v.mp4"), 5.0, 10.0, 1))

    finished_calls = []
    worker = BatchWorker(clipper, q)
    worker.finished.connect(lambda results: finished_calls.append(results))

    # Cancel before running
    worker.cancel()
    worker.run()

    assert len(finished_calls) == 0


def test_batch_worker_emits_progress_and_finished():
    """BatchWorker signals are emitted during and after processing."""
    clipper = MagicMock()
    clipper.save_clip.return_value = ClipResult(
        path="/tmp/out.mp4",
        start_time=0.0,
        end_time=1.0,
        success=True,
    )
    q = ClipQueue()
    q.enqueue(ClipRequest(Path("/tmp/v.mp4"), 5.0, 10.0, 1))

    progress_calls = []
    finished_calls = []

    worker = BatchWorker(clipper, q)
    worker.progress.connect(lambda c, t, r: progress_calls.append((c, t, r)))
    worker.finished.connect(lambda results: finished_calls.append(results))
    worker.run()

    assert len(progress_calls) == 1
    assert progress_calls[0][0] == 1  # completed
    assert progress_calls[0][1] == 1  # total
    assert len(finished_calls) == 1
    assert len(finished_calls[0]) == 1
