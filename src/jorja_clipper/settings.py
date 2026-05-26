"""Application settings with JSON persistence."""

import json
from pathlib import Path

__all__ = ["Settings"]


class Settings:
    """Manages application configuration."""

    def __init__(self, config_path: Path | None = None) -> None:
        default = Path.home() / ".config" / "jorja-clipper" / "config.json"
        self.config_path = config_path or default
        self.buffer_before: float = 5.0
        self.buffer_after: float = 5.0
        self.clip_key: str = "C"
        self.output_dir: str = ""  # empty = clips/ next to video
        self.theme: str = "dark"

    def save(self) -> None:
        """Save settings to JSON file."""
        self.config_path.parent.mkdir(parents=True, exist_ok=True)
        data = {
            "buffer_before": self.buffer_before,
            "buffer_after": self.buffer_after,
            "clip_key": self.clip_key,
            "output_dir": self.output_dir,
            "theme": self.theme,
        }
        try:
            self.config_path.write_text(json.dumps(data, indent=2))
        except OSError as exc:
            raise RuntimeError(f"Failed to save settings: {exc}") from exc

    def load(self) -> None:
        """Load settings from JSON file, using defaults for missing keys."""
        if not self.config_path.exists():
            return
        try:
            data = json.loads(self.config_path.read_text())
            if not isinstance(data, dict):
                return  # use defaults
            self.buffer_before = float(data.get("buffer_before", self.buffer_before))
            self.buffer_after = float(data.get("buffer_after", self.buffer_after))
            self.clip_key = data.get("clip_key", self.clip_key)
            self.output_dir = data.get("output_dir", self.output_dir)
            self.theme = data.get("theme", self.theme)
            if self.buffer_before < 0 or self.buffer_after < 0:
                raise ValueError("negative buffer")
        except (json.JSONDecodeError, KeyError, UnicodeDecodeError, OSError):
            pass  # Use defaults on corrupt file
        except (ValueError, TypeError):
            # Reset to defaults on invalid numeric values
            self.buffer_before = 5.0
            self.buffer_after = 5.0
