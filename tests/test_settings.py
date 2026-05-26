"""Tests for settings module."""

from pathlib import Path

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


def _write_config(tmp_path, content: str) -> "Path":
    config = tmp_path / "config.json"
    config.write_text(content)
    return config


def test_settings_load_non_dict_json_array(tmp_path):
    """Loading a JSON array uses defaults instead of crashing."""
    config = _write_config(tmp_path, "[1, 2, 3]")
    s = Settings(config_path=config)
    s.load()
    assert s.buffer_before == 5.0
    assert s.buffer_after == 5.0
    assert s.clip_key == "C"
    assert s.theme == "dark"


def test_settings_load_non_dict_json_string(tmp_path):
    """Loading a JSON string uses defaults instead of crashing."""
    config = _write_config(tmp_path, '"hello"')
    s = Settings(config_path=config)
    s.load()
    assert s.buffer_before == 5.0
    assert s.theme == "dark"


def test_settings_load_non_dict_json_number(tmp_path):
    """Loading a JSON number uses defaults instead of crashing."""
    config = _write_config(tmp_path, "42")
    s = Settings(config_path=config)
    s.load()
    assert s.buffer_before == 5.0
    assert s.theme == "dark"


def test_settings_load_non_dict_json_null(tmp_path):
    """Loading a JSON null uses defaults instead of crashing."""
    config = _write_config(tmp_path, "null")
    s = Settings(config_path=config)
    s.load()
    assert s.buffer_before == 5.0
    assert s.theme == "dark"
