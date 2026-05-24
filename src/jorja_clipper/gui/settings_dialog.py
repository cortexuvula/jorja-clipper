"""Settings dialog for Jorja Clipper."""

import logging

from PySide6.QtCore import Qt
from PySide6.QtGui import QKeySequence
from PySide6.QtWidgets import (
    QComboBox,
    QDialog,
    QDialogButtonBox,
    QDoubleSpinBox,
    QFileDialog,
    QFormLayout,
    QHBoxLayout,
    QKeySequenceEdit,
    QLabel,
    QLineEdit,
    QMessageBox,
    QPushButton,
    QVBoxLayout,
    QWidget,
)

from jorja_clipper.gui.theme import THEMES
from jorja_clipper.settings import Settings

logger = logging.getLogger(__name__)

__all__ = ["SettingsDialog"]


class SettingsDialog(QDialog):
    """Dialog to edit application settings."""

    def __init__(self, settings: Settings, parent: QWidget | None = None) -> None:
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

        # Theme selector
        self._theme_combo = QComboBox()
        for name in THEMES:
            self._theme_combo.addItem(name.capitalize(), name)
        idx = self._theme_combo.findData(self._settings.theme)
        if idx >= 0:
            self._theme_combo.setCurrentIndex(idx)
        form.addRow("Theme:", self._theme_combo)

        # Output directory
        self._output_dir = QLineEdit()
        self._output_dir.setText(self._settings.output_dir)
        self._output_dir.setPlaceholderText("Default: clips/ next to video")
        browse_btn = QPushButton("Browse...")
        browse_btn.clicked.connect(self._browse_output_dir)
        output_dir_layout = QHBoxLayout()
        output_dir_layout.addWidget(self._output_dir)
        output_dir_layout.addWidget(browse_btn)
        form.addRow("Output directory:", output_dir_layout)

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

    def _browse_output_dir(self) -> None:
        """Open a folder dialog to select output directory."""
        path = QFileDialog.getExistingDirectory(self, "Select Output Directory")
        if path:
            self._output_dir.setText(path)

    def _on_save(self) -> None:
        key = self._key_clip.keySequence().toString()
        if not key:
            QMessageBox.warning(self, "Invalid Key", "Clip key cannot be empty.")
            return
        self._settings.buffer_before = self._spin_before.value()
        self._settings.buffer_after = self._spin_after.value()
        self._settings.clip_key = key.split(",")[0].strip()
        self._settings.output_dir = self._output_dir.text().strip()
        self._settings.theme = self._theme_combo.currentData()
        try:
            self._settings.save()
        except RuntimeError as exc:
            logger.error("Failed to save settings: %s", exc)
            QMessageBox.critical(self, "Save Failed", str(exc))
            return
        self.accept()
