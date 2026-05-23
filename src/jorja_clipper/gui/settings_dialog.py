"""Settings dialog for Jorja Clipper."""

from PySide6.QtCore import Qt
from PySide6.QtGui import QKeySequence
from PySide6.QtWidgets import (
    QDialog,
    QDialogButtonBox,
    QDoubleSpinBox,
    QFormLayout,
    QKeySequenceEdit,
    QLabel,
    QVBoxLayout,
)

from jorja_clipper.settings import Settings


class SettingsDialog(QDialog):
    """Dialog to edit application settings."""

    def __init__(self, settings: Settings, parent=None):
        super().__init__(parent)
        self._settings = settings
        self.setWindowTitle("Settings")
        self.setMinimumWidth(350)

        layout = QVBoxLayout(self)

        form = QFormLayout()
        self._spin_before = QDoubleSpinBox()
        self._spin_before.setRange(0.0, 60.0)
        self._spin_before.setSingleStep(0.5)
        self._spin_before.setDecimals(1)
        self._spin_before.setValue(self._settings.buffer_before)
        form.addRow("Buffer before (seconds):", self._spin_before)

        self._spin_after = QDoubleSpinBox()
        self._spin_after.setRange(0.0, 60.0)
        self._spin_after.setSingleStep(0.5)
        self._spin_after.setDecimals(1)
        self._spin_after.setValue(self._settings.buffer_after)
        form.addRow("Buffer after (seconds):", self._spin_after)

        self._key_clip = QKeySequenceEdit()
        self._key_clip.setKeySequence(QKeySequence(self._settings.clip_key))
        form.addRow("Clip key:", self._key_clip)

        layout.addLayout(form)

        self._buttons = QDialogButtonBox(
            QDialogButtonBox.StandardButton.Save
            | QDialogButtonBox.StandardButton.Cancel
        )
        self._buttons.accepted.connect(self._on_save)
        self._buttons.rejected.connect(self.reject)
        layout.addWidget(self._buttons)

        self._status = QLabel("")
        self._status.setAlignment(Qt.AlignmentFlag.AlignCenter)
        layout.addWidget(self._status)

    def _on_save(self):
        self._settings.buffer_before = self._spin_before.value()
        self._settings.buffer_after = self._spin_after.value()
        key = self._key_clip.keySequence().toString()
        if key:
            self._settings.clip_key = key.split(",")[0].strip()
        self._settings.save()
        self.accept()
