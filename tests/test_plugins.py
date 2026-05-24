"""Tests for the plugin system."""

import textwrap
from pathlib import Path
from unittest.mock import MagicMock

from jorja_clipper.clipper import ClipResult
from jorja_clipper.plugins import ClipPlugin, PluginLoader


class DummyPlugin(ClipPlugin):
    """A plugin that records hook calls."""

    def __init__(self) -> None:
        self.calls: list[tuple[str, tuple]] = []

    def on_clip_start(self, video_path, start_time, end_time) -> None:
        self.calls.append(("start", (video_path, start_time, end_time)))

    def on_clip_complete(self, result) -> None:
        self.calls.append(("complete", (result,)))

    def on_clip_error(self, result) -> None:
        self.calls.append(("error", (result,)))


def test_clip_plugin_hooks_are_noops_by_default():
    """Base ClipPlugin provides empty implementations."""
    p = ClipPlugin()
    # should not raise
    p.on_clip_start(Path("/tmp/v.mp4"), 0.0, 1.0)
    p.on_clip_complete(ClipResult("/tmp/out.mp4", 0.0, 1.0, True))
    p.on_clip_error(ClipResult("", 0.0, 0.0, False, error="fail"))


def test_plugin_name_defaults_to_class_name():
    """name returns the class name by default."""
    p = DummyPlugin()
    assert p.name == "DummyPlugin"


def test_plugin_loader_finds_plugin(tmp_path):
    """PluginLoader discovers a valid plugin file."""
    plugins_dir = tmp_path / "plugins"
    plugins_dir.mkdir()
    py_file = plugins_dir / "dummy.py"
    py_file.write_text(textwrap.dedent("""
        from jorja_clipper.plugins import ClipPlugin

        class MyPlugin(ClipPlugin):
            pass
    """))

    loader = PluginLoader(plugins_dir)
    plugins = loader.scan()
    assert len(plugins) == 1
    assert isinstance(plugins[0], ClipPlugin)
    assert plugins[0].name == "MyPlugin"


def test_plugin_loader_skips_private_files(tmp_path):
    """Files starting with underscore are ignored."""
    plugins_dir = tmp_path / "plugins"
    plugins_dir.mkdir()
    py_file = plugins_dir / "_hidden.py"
    py_file.write_text(textwrap.dedent("""
        from jorja_clipper.plugins import ClipPlugin
        class HiddenPlugin(ClipPlugin):
            pass
    """))
    loader = PluginLoader(plugins_dir)
    assert loader.scan() == []


def test_plugin_loader_handles_broken_file(tmp_path):
    """A file with a syntax error is skipped gracefully."""
    plugins_dir = tmp_path / "plugins"
    plugins_dir.mkdir()
    py_file = plugins_dir / "broken.py"
    py_file.write_text("this is not valid python!!!")

    loader = PluginLoader(plugins_dir)
    assert loader.scan() == []


def test_broadcast_clip_start():
    """broadcast_clip_start delegates to all loaded plugins."""
    p1 = DummyPlugin()
    p2 = DummyPlugin()
    loader = PluginLoader()
    loader._plugins = [p1, p2]
    loader.broadcast_clip_start(Path("/tmp/v.mp4"), 10.0, 20.0)
    assert len(p1.calls) == 1
    assert p1.calls[0] == ("start", (Path("/tmp/v.mp4"), 10.0, 20.0))
    assert len(p2.calls) == 1


def test_broadcast_clip_complete():
    """broadcast_clip_complete delegates to all loaded plugins."""
    p1 = DummyPlugin()
    loader = PluginLoader()
    loader._plugins = [p1]
    result = ClipResult("/tmp/out.mp4", 0.0, 1.0, True)
    loader.broadcast_clip_complete(result)
    assert p1.calls == [("complete", (result,))]


def test_broadcast_clip_error():
    """broadcast_clip_error delegates to all loaded plugins."""
    p1 = DummyPlugin()
    loader = PluginLoader()
    loader._plugins = [p1]
    result = ClipResult("", 0.0, 0.0, False, error="fail")
    loader.broadcast_clip_error(result)
    assert p1.calls == [("error", (result,))]


def test_broadcast_swallows_plugin_exceptions():
    """A misbehaving plugin does not crash the broadcast."""
    bad = DummyPlugin()
    bad.on_clip_start = lambda *a: 1 / 0  # type: ignore[method-assign]
    good = DummyPlugin()
    loader = PluginLoader()
    loader._plugins = [bad, good]
    # must not raise
    loader.broadcast_clip_start(Path("/tmp/v.mp4"), 0.0, 1.0)
    assert len(good.calls) == 1


def test_plugin_loader_returns_plugins_property():
    """plugins returns the scanned list."""
    loader = PluginLoader()
    assert loader.plugins == []
