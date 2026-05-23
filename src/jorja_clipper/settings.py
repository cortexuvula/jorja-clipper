"""Application settings with JSON persistence."""

import json
from pathlib import Path


class Settings:
    """Manages application configuration."""

    def __init__(self, config_path: Path | None = None):
        default = Path.home() / ".config" / "jorja-clipper" / "config.json"
        self.config_path = config_path or default
        self.buffer_before: float = 5.0
        self.buffer_after: float = 5.0
        self.clip_key: str = "C"
        self.output_dir: str = ""  # empty = clips/ next to video

    def save(self) -> None:
        """Save settings to JSON file."""
        self.config_path.parent.mkdir(parents=True, exist_ok=True)
        data = {
            "buffer_before": self.buffer_before,
            "buffer_after": self.buffer_after,
            "clip_key": self.clip_key,
            "output_dir": self.output_dir,
        }
        self.config_path.write_text(json.dumps(data, indent=2))

    def load(self) -> None:
        """Load settings from JSON file, using defaults for missing keys."""
        if not self.config_path.exists():
            return
        try:
            data = json.loads(self.config_path.read_text())
            self.buffer_before = data.get("buffer_before", self.buffer_before)
            self.buffer_after = data.get("buffer_after", self.buffer_after)
            self.clip_key = data.get("clip_key", self.clip_key)
            self.output_dir = data.get("output_dir", self.output_dir)
        except (json.JSONDecodeError, KeyError):
            pass  # Use defaults on corrupt file
