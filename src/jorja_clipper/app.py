"""Main application entry point."""

import locale
import sys
from pathlib import Path

# Fix libmpv locale crash on Unix-like platforms — must run before any mpv/Qt imports
if sys.platform in ("linux", "darwin", "freebsd", "openbsd"):
    try:
        locale.setlocale(locale.LC_NUMERIC, "C")
    except locale.Error:
        pass

from PySide6.QtWidgets import QApplication

from jorja_clipper.clipper import Clipper
from jorja_clipper.gui.main_window import MainWindow
from jorja_clipper.player import Player
from jorja_clipper.settings import Settings


def main():
    """Launch Jorja Clipper."""
    video_args = [a for a in sys.argv[1:] if not a.startswith("-")]
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
    if video_args:
        video_path = Path(video_args[0])
        if video_path.is_file():
            if player.load(video_path):
                window.load_video(video_path)
            else:
                window.set_status(f"Failed to load: {video_path.name}")

    sys.exit(app.exec())


if __name__ == "__main__":
    main()
