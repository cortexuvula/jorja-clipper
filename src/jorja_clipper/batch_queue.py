"""Batch clip queue — buffers clip requests and processes them sequentially."""

from dataclasses import dataclass, field
from pathlib import Path

from PySide6.QtCore import QObject, QThread, Signal

from jorja_clipper.clipper import Clipper, ClipResult

__all__ = ["ClipRequest", "ClipQueue", "BatchWorker"]


@dataclass
class ClipRequest:
    """A single pending clip request in the batch queue."""

    video_path: Path
    current_pos: float
    video_duration: float
    clip_number: int


@dataclass
class ClipQueue:
    """FIFO buffer for clip requests."""

    _items: list[ClipRequest] = field(default_factory=list)

    def enqueue(self, request: ClipRequest) -> None:
        self._items.append(request)

    def dequeue(self) -> ClipRequest | None:
        try:
            return self._items.pop(0)
        except IndexError:
            return None

    def clear(self) -> None:
        self._items.clear()

    def __len__(self) -> int:
        """Return the number of items in the queue."""
        return len(self._items)

    @property
    def pending(self) -> list[ClipRequest]:
        return list(self._items)


class BatchWorker(QThread):
    """Processes a :class:`ClipQueue` sequentially in a background thread.

    Emits *progress* after every item with ``(completed, total, result)``,
    and *finished* when the queue is exhausted.
    """

    progress = Signal(int, int, object)  # completed, total, ClipResult
    finished = Signal(list)  # list[ClipResult]

    def __init__(
        self,
        clipper: Clipper,
        queue: ClipQueue,
        parent: QObject | None = None,
    ) -> None:
        super().__init__(parent)
        self._clipper = clipper
        self._queue = queue
        self._results: list[ClipResult] = []
        self._cancelled = False

    def cancel(self) -> None:
        """Request cancellation of this batch worker.

        Sets an internal flag checked at the top of each loop iteration
        and before emitting the final ``finished`` signal.
        """
        self._cancelled = True

    def _is_cancelled(self) -> bool:
        """Check both the custom flag and Qt's interruption request."""
        return self._cancelled or self.isInterruptionRequested()

    def run(self) -> None:
        total = len(self._queue)
        completed = 0
        while True:
            if self._is_cancelled():
                break
            request = self._queue.dequeue()
            if request is None:
                break
            result = self._clipper.save_clip(
                video_path=request.video_path,
                current_pos=request.current_pos,
                video_duration=request.video_duration,
                clip_number=request.clip_number,
            )
            self._results.append(result)
            completed += 1
            self.progress.emit(completed, total, result)
        if not self._is_cancelled():
            self.finished.emit(self._results)
