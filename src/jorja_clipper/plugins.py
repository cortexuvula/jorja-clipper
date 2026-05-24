"""Plugin system for custom clip engines and post-processing hooks.

The plugin loader scans ``~/.config/jorja-clipper/plugins/`` for ``*.py`` files,
imports them, and instantiates any class that subclasses :class:`ClipPlugin`.
"""

import importlib.util
import logging
from pathlib import Path

from jorja_clipper.clipper import ClipResult

__all__ = ["ClipPlugin", "PluginLoader"]

logger = logging.getLogger(__name__)


class ClipPlugin:
    """Base class for Jorja Clipper plugins.

    Subclass this and implement any of the hooks you care about.
    A plugin must provide a no-argument constructor.
    """

    @property
    def name(self) -> str:
        """Human-readable plugin name. Defaults to the class name."""
        return self.__class__.__name__

    def on_clip_start(  # noqa: B027
        self, video_path: Path, start_time: float, end_time: float
    ) -> None:
        """Called just before a clip extraction begins."""

    def on_clip_complete(self, result: ClipResult) -> None:  # noqa: B027
        """Called after a clip finishes successfully."""

    def on_clip_error(self, result: ClipResult) -> None:  # noqa: B027
        """When a clip extraction fails."""


class PluginLoader:
    """Discovers and instantiates plugins from a directory."""

    def __init__(self, plugins_dir: Path | None = None) -> None:
        if plugins_dir is None:
            plugins_dir = Path.home() / ".config" / "jorja-clipper" / "plugins"
        self._plugins_dir = plugins_dir
        self._plugins: list[ClipPlugin] = []

    # ------------------------------------------------------------------
    # Discovery
    # ------------------------------------------------------------------

    def scan(self) -> list[ClipPlugin]:
        """Scan *plugins_dir* for ``*.py`` files and instantiate any clips."""
        self._plugins.clear()
        if not self._plugins_dir.exists():
            logger.debug("Plugin directory does not exist: %s", self._plugins_dir)
            return self._plugins

        for py_file in sorted(self._plugins_dir.glob("*.py")):
            if py_file.name.startswith("_"):
                continue
            plugin = self._load_single(py_file)
            if plugin is not None:
                self._plugins.append(plugin)
                logger.info("Loaded plugin: %s (%s)", plugin.name, py_file.name)
        return self._plugins

    def _load_single(self, py_file: Path) -> ClipPlugin | None:
        """Import *py_file* and return the first ClipPlugin subclass found."""
        module_name = f"jorja_clipper.plugins.user.{py_file.stem}"
        spec = importlib.util.spec_from_file_location(module_name, py_file)
        if spec is None or spec.loader is None:
            logger.warning("Could not create spec for %s", py_file)
            return None
        module = importlib.util.module_from_spec(spec)
        try:
            spec.loader.exec_module(module)
        except Exception as exc:
            logger.warning("Failed to import plugin %s: %s", py_file, exc)
            return None

        for obj in vars(module).values():
            if (
                isinstance(obj, type)
                and issubclass(obj, ClipPlugin)
                and obj is not ClipPlugin
            ):
                try:
                    return obj()
                except Exception as exc:
                    logger.warning(
                        "Failed to instantiate %s from %s: %s",
                        obj,
                        py_file,
                        exc,
                    )
        return None

    # ------------------------------------------------------------------
    # Access
    # ------------------------------------------------------------------

    @property
    def plugins(self) -> list[ClipPlugin]:
        return list(self._plugins)

    # ------------------------------------------------------------------
    # Hook broadcasting
    # ------------------------------------------------------------------

    def broadcast_clip_start(
        self, video_path: Path, start_time: float, end_time: float
    ) -> None:
        for p in self._plugins:
            try:
                p.on_clip_start(video_path, start_time, end_time)
            except Exception:
                logger.exception("Plugin %s raised in on_clip_start", p.name)

    def broadcast_clip_complete(self, result: ClipResult) -> None:
        for p in self._plugins:
            try:
                p.on_clip_complete(result)
            except Exception:
                logger.exception("Plugin %s raised in on_clip_complete", p.name)

    def broadcast_clip_error(self, result: ClipResult) -> None:
        for p in self._plugins:
            try:
                p.on_clip_error(result)
            except Exception:
                logger.exception("Plugin %s raised in on_clip_error", p.name)
