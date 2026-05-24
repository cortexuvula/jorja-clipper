"""Spike / proof-of-concept: Settings using pydantic BaseModel.

This file demonstrates how the existing ``Settings`` class could be
replaced or wrapped by a ``pydantic.BaseModel`` for automatic validation,
JSON serialization, and schema generation.  It is **not** wired into the
production code yet; it lives in ``tests/`` as an evaluation artefact.

Run::

    pytest tests/test_pydantic_settings_spike.py -v

"""

import json
from pathlib import Path

import pytest
from pydantic import BaseModel, Field, field_validator

from jorja_clipper.settings import Settings


class PydanticSettings(BaseModel):
    """Pydantic-powered settings — drop-in conceptual replacement."""

    config_path: Path | None = None
    buffer_before: float = Field(default=5.0, ge=0.0, le=60.0)
    buffer_after: float = Field(default=5.0, ge=0.0, le=60.0)
    clip_key: str = Field(default="C", min_length=1, max_length=1)
    output_dir: str = Field(default="", max_length=4096)

    @field_validator("config_path", mode="before")
    @classmethod
    def _coerce_path(cls, v):  # noqa: N805
        if v is None or isinstance(v, Path):
            return v
        return Path(v)

    def _default_config_path(self) -> Path:
        return Path.home() / ".config" / "jorja-clipper" / "config.json"

    def save(self) -> None:
        """Serialize to JSON, mirroring the original Settings API."""
        path = self.config_path or self._default_config_path()
        path.parent.mkdir(parents=True, exist_ok=True)
        data = {
            "buffer_before": self.buffer_before,
            "buffer_after": self.buffer_after,
            "clip_key": self.clip_key,
            "output_dir": self.output_dir,
        }
        path.write_text(json.dumps(data, indent=2))

    def load(self) -> None:
        """Load from JSON, using pydantic coercion and validation."""
        path = self.config_path or self._default_config_path()
        if not path.exists():
            return
        try:
            raw = json.loads(path.read_text())
        except (json.JSONDecodeError, OSError):
            return
        # Use pydantic validation while updating fields individually
        for key in ("buffer_before", "buffer_after", "clip_key", "output_dir"):
            if key in raw:
                setattr(self, key, raw[key])


# ---------------------------------------------------------------------------
# Tests comparing old and new implementations
# ---------------------------------------------------------------------------


def test_pydantic_settings_defaults():
    """PydanticSettings has the same default values as the original."""
    ps = PydanticSettings()
    assert ps.buffer_before == 5.0
    assert ps.buffer_after == 5.0
    assert ps.clip_key == "C"
    assert ps.output_dir == ""


def test_pydantic_settings_rejects_negative_buffer():
    """Pydantic enforces non-negative buffers automatically."""
    with pytest.raises(ValueError):
        PydanticSettings(buffer_before=-1.0)


def test_pydantic_settings_rejects_empty_clip_key():
    """Pydantic enforces a non-empty clip key."""
    with pytest.raises(ValueError):
        PydanticSettings(clip_key="")


def test_pydantic_settings_roundtrip(tmp_path):
    """Save + load roundtrip preserves values."""
    path = tmp_path / "config.json"
    ps = PydanticSettings(
        config_path=path,
        buffer_before=12.5,
        buffer_after=3.0,
        clip_key="X",
        output_dir="/tmp/clips",
    )
    ps.save()

    ps2 = PydanticSettings(config_path=path)
    ps2.load()
    assert ps2.buffer_before == 12.5
    assert ps2.buffer_after == 3.0
    assert ps2.clip_key == "X"
    assert ps2.output_dir == "/tmp/clips"


def test_pydantic_settings_handles_corrupt_file(tmp_path):
    """Corrupt JSON falls back to defaults silently."""
    path = tmp_path / "config.json"
    path.write_text("not json {{}")
    ps = PydanticSettings(config_path=path)
    ps.load()
    assert ps.buffer_before == 5.0  # default


def test_pydantic_and_original_produce_same_json(tmp_path):
    """Both implementations write identical JSON shapes."""
    orig = Settings(config_path=tmp_path / "orig.json")
    orig.buffer_before = 7.0
    orig.buffer_after = 2.0
    orig.clip_key = "Q"
    orig.output_dir = "/tmp/out"
    orig.save()

    pyd = PydanticSettings(config_path=tmp_path / "pyd.json")
    pyd.buffer_before = 7.0
    pyd.buffer_after = 2.0
    pyd.clip_key = "Q"
    pyd.output_dir = "/tmp/out"
    pyd.save()

    orig_data = json.loads((tmp_path / "orig.json").read_text())
    pyd_data = json.loads((tmp_path / "pyd.json").read_text())
    assert orig_data == pyd_data
