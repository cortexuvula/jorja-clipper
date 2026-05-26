"""Tests for settings module."""

import json
import sys
from pathlib import Path
from unittest.mock import patch

import pytest

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


def test_settings_save_no_temp_files_left(tmp_path):
    """Saving leaves no temporary files behind."""
    config = tmp_path / "config.json"
    s = Settings(config_path=config)
    s.buffer_before = 7.0
    s.theme = "light"
    s.save()

    # Config file has correct content
    data = json.loads(config.read_text())
    assert data["buffer_before"] == 7.0
    assert data["theme"] == "light"

    # No temp files left in the directory
    leftovers = list(tmp_path.glob(".config_*.tmp"))
    assert leftovers == []


def test_settings_save_atomic_failed_write_preserves_config(tmp_path):
    """A failed save leaves the existing config intact."""
    config = tmp_path / "config.json"

    # Write initial valid settings
    s = Settings(config_path=config)
    s.buffer_before = 12.0
    s.theme = "light"
    s.save()

    original_data = json.loads(config.read_text())
    assert original_data["buffer_before"] == 12.0

    # Prepare a second save that will fail during the write step
    s.buffer_before = 99.0
    s.theme = "broken"

    # Patch json.dump to simulate a write failure (e.g. disk full)
    with (
        patch("jorja_clipper.settings.json.dump", side_effect=OSError("disk full")),
        pytest.raises(RuntimeError, match="Failed to save settings"),
    ):
        s.save()

    # Original config is still intact
    assert config.exists()
    preserved_data = json.loads(config.read_text())
    assert preserved_data["buffer_before"] == 12.0
    assert preserved_data["theme"] == "light"

    # No temp files left behind
    leftovers = list(tmp_path.glob(".config_*.tmp"))
    assert leftovers == []


@pytest.mark.skipif(
    sys.platform == "win32", reason="os.fchmod not available on Windows"
)
def test_settings_save_preserves_permissions(tmp_path):
    """Saving preserves the original file permissions."""
    config = tmp_path / "config.json"

    # Create initial config with specific permissions
    s = Settings(config_path=config)
    s.save()

    # Set specific permissions (0644: rw-r--r--)
    config.chmod(0o644)
    original_mode = config.stat().st_mode & 0o7777
    assert original_mode == 0o644

    # Save again with different settings
    s.buffer_before = 15.0
    s.theme = "light"
    s.save()

    # Permissions should be preserved
    new_mode = config.stat().st_mode & 0o7777
    assert new_mode == 0o644

    # Content should be updated
    data = json.loads(config.read_text())
    assert data["buffer_before"] == 15.0
    assert data["theme"] == "light"
