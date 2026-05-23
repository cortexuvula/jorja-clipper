"""Clip list model and widget."""

from dataclasses import dataclass
from pathlib import Path

from PySide6.QtCore import QAbstractListModel, QModelIndex, Qt


@dataclass
class ClipEntry:
    """A saved clip entry."""

    path: str
    start_time: float
    end_time: float


class ClipListModel(QAbstractListModel):
    """Model for the list of saved clips."""

    def __init__(self):
        super().__init__()
        self._clips: list[ClipEntry] = []

    def rowCount(self, parent=None) -> int:  # noqa: ARG002
        return len(self._clips)

    def data(self, index: QModelIndex, role=Qt.ItemDataRole.DisplayRole):
        if not index.isValid() or index.row() >= len(self._clips):
            return None
        clip = self._clips[index.row()]
        if role == Qt.ItemDataRole.DisplayRole:
            name = Path(clip.path).name
            return f"{name}  [{clip.start_time:.1f}s - {clip.end_time:.1f}s]"
        if role == Qt.ItemDataRole.UserRole:
            return clip
        return None

    def add_clip(self, path: str, start_time: float, end_time: float):
        """Add a new clip to the model."""
        self.beginInsertRows(QModelIndex(), len(self._clips), len(self._clips))
        entry = ClipEntry(path=path, start_time=start_time, end_time=end_time)
        self._clips.append(entry)
        self.endInsertRows()

    def clip_at(self, index: int) -> ClipEntry | None:
        """Return the clip entry at the given row index."""
        if 0 <= index < len(self._clips):
            return self._clips[index]
        return None
