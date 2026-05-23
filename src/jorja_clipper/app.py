"""Main application entry point."""

import ctypes
import locale
import sys
from pathlib import Path


# Fix libmpv locale crash on Linux — must run before any mpv/Qt imports
if sys.platform == "linux":
    try:
        libc = ctypes.CDLL("libc.so.6")
        libc.setlocale(ctypes.c_int(0), b"C")  # LC_NUMERIC = 0
    except Exception:
        locale.setlocale(locale.LC_NUMERIC, "C")

from PySide6.QtWidgets import QApplication

from jorja_clipper.clipper import Clipper
from jorja_clipper.gui.main_window import MainWindow
from jorja_clipper.player import Player
from jorja_clipper.settings import Settings


def main():
    """Launch Jorja Clipper."""
    app = QApplication(sys.argv)

    settings = Settings()
    settings.load()

    player = Player()
    clipper = Clipper(
        buffer_before=settings.buffer_before,
        buffer_after=settings.buffer_after,
    )
    window = MainWindow(player, clipper, settings)
    window.show()

    # If a video file was passed as argument, load it
    if len(sys.argv) > 1:
        video_path = Path(sys.argv[1])
        if video_path.exists():
            player.load(video_path)
            window._current_video = video_path
            window._status.setText(f"Loaded: {video_path.name}")
            window.setWindowTitle(f"Jorja Clipper — {video_path.name}")

    sys.exit(app.exec())


if __name__ == "__main__":
    main()
