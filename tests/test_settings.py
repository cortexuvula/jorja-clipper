"""Tests for settings module."""

from jorja_clipper.settings import Settings


def test_default_settings():
    """Settings have sensible defaults."""
    s = Settings()
    assert s.buffer_before == 5.0
    assert s.buffer_after == 5.0
    assert s.clip_key == "C"


def test_settings_save_load(tmp_path):
    """Settings persist to JSON including theme."""
    config = tmp_path / "config.json"
    s = Settings(config_path=config)
    s.buffer_before = 10.0
    s.theme = "light"
    s.save()

    s2 = Settings(config_path=config)
    s2.load()
    assert s2.buffer_before == 10.0
    assert s2.theme == "light"


def test_settings_load_missing_file(tmp_path):
    """Loading from missing file uses defaults."""
    config = tmp_path / "missing.json"
    s = Settings(config_path=config)
    s.load()
    assert s.buffer_before == 5.0
