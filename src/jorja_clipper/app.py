"""Main application entry point."""

import contextlib
import locale
import logging
import logging.handlers
import sys
from pathlib import Path

# Fix libmpv locale crash on Unix-like platforms — must run before any mpv/Qt imports
if sys.platform in ("linux", "darwin", "freebsd", "openbsd"):
    with contextlib.suppress(locale.Error):
        locale.setlocale(locale.LC_NUMERIC, "C")

from PySide6.QtWidgets import QApplication

from jorja_clipper.clipper import Clipper
from jorja_clipper.controller import ClipController
from jorja_clipper.gui.clip_list import ClipListModel
from jorja_clipper.gui.main_window import MainWindow
from jorja_clipper.gui.theme import ThemeManager
from jorja_clipper.player import Player
from jorja_clipper.plugins import PluginLoader
from jorja_clipper.settings import Settings

logger = logging.getLogger(__name__)

__all__ = ["main", "_setup_logging"]


def _setup_logging() -> None:
    """Configure root logger with stderr and rotating file handlers."""
    log_dir = Path.home() / ".config" / "jorja-clipper"
    log_dir.mkdir(parents=True, exist_ok=True)
    log_file = log_dir / "jorja-clipper.log"

    formatter = logging.Formatter(
        "%(asctime)s [%(levelname)s] %(name)s: %(message)s",
        datefmt="%Y-%m-%d %H:%M:%S",
    )

    # Console handler
    console_handler = logging.StreamHandler(sys.stderr)
    console_handler.setLevel(logging.INFO)
    console_handler.setFormatter(formatter)

    # Rotating file handler (max 1 MB, keep 3 backups)
    file_handler = logging.handlers.RotatingFileHandler(
        log_file, maxBytes=1_048_576, backupCount=3, encoding="utf-8"
    )
    file_handler.setLevel(logging.DEBUG)
    file_handler.setFormatter(formatter)

    root = logging.getLogger()
    root.setLevel(logging.DEBUG)
    root.addHandler(console_handler)
    root.addHandler(file_handler)

    logger.info("Logging configured — file: %s", log_file)


def main() -> None:
    """Launch Jorja Clipper."""
    _setup_logging()
    logger.info("Jorja Clipper starting")

    video_args = [a for a in sys.argv[1:] if not a.startswith("-")]
    app = QApplication(sys.argv)

    settings = Settings()
    settings.load()
    logger.debug("Settings loaded from %s", settings.config_path)

    # Load plugins early so they are ready before any clips fire
    plugin_loader = PluginLoader()
    loaded = plugin_loader.scan()
    logger.info("Plugins loaded: %d", len(loaded))

    player = Player()
    clipper = Clipper(
        buffer_before=settings.buffer_before,
        buffer_after=settings.buffer_after,
    )
    clip_model = ClipListModel()
    theme_manager = ThemeManager(theme_name=settings.theme)
    controller = ClipController(
        player, clipper, settings, clip_model, plugin_loader=plugin_loader
    )
    window = MainWindow(controller, theme_manager)
    window.show()

    # If a video file was passed as argument, load it
    if video_args:
        video_path = Path(video_args[0])
        if video_path.is_file():
            controller.open_file(video_path)
        else:
            logger.warning("Argument path not found: %s", video_path)
            window.set_status(f"File not found: {video_path.name}")

    logger.info("Entering Qt event loop")
    sys.exit(app.exec())


if __name__ == "__main__":
    main()
