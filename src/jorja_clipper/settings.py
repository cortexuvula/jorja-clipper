"""Application settings with JSON persistence."""

import json
import os
import tempfile
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
        """Save settings to JSON file atomically.

        Writes to a temporary file in the same directory, then renames it
        over the target using os.replace(), which is atomic on POSIX systems.
        A crash during the write leaves the original config intact.
        """
        self.config_path.parent.mkdir(parents=True, exist_ok=True)
        data = {
            "buffer_before": self.buffer_before,
            "buffer_after": self.buffer_after,
            "clip_key": self.clip_key,
            "output_dir": self.output_dir,
            "theme": self.theme,
        }
        try:
            fd, tmp_path = tempfile.mkstemp(
                dir=self.config_path.parent,
                prefix=".config_",
                suffix=".tmp",
            )
            try:
                # Preserve original file permissions if it exists
                if self.config_path.exists():
                    original_mode = os.stat(self.config_path).st_mode
                    os.fchmod(fd, original_mode & 0o7777)
                # else: keep mkstemp's secure 0600 for new files

                with os.fdopen(fd, "w") as f:
                    json.dump(data, f, indent=2)
                os.replace(tmp_path, self.config_path)
            except BaseException:
                Path(tmp_path).unlink(missing_ok=True)
                raise
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
