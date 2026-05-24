# Contributing to Jorja Clipper

Thanks for your interest in contributing! This document covers how to set up the project for local development and our workflow conventions.

## Development Setup

### Prerequisites

- **Python** 3.10 or newer
- **ffmpeg** installed and available in `PATH`
- **libmpv** installed (for video playback)

Platform-specific install tips:

- **macOS:** `brew install ffmpeg mpv`
- **Ubuntu/Debian:** `sudo apt update && sudo apt install ffmpeg libmpv-dev`
- **Windows:** Use `winget install Gyan.FFmpeg` and install mpv via the official installer.

### Clone & Install

```bash
git clone https://github.com/cortexuvula/jorja-clipper.git
cd jorja-clipper
```

We use **uv** for dependency management. If you don't have uv, install it from [astral.sh/uv](https://astral.sh/uv).

```bash
# Sync locked dependencies and install the package in editable mode
uv sync --dev
```

If you prefer pip, you can still install in a virtual environment:

```bash
python -m venv .venv
source .venv/bin/activate  # Windows: .venv\Scripts\activate
pip install -e ".[dev]"
```

### Run the App

```bash
uv run jorja-clipper
# or with a video file argument
uv run jorja-clipper /path/to/game.mp4
```

### Run Tests

```bash
uv run pytest tests/ -v
```

If you installed with pip inside a virtual environment:

```bash
pytest tests/ -v
```

### Linting

We use **ruff** for linting and formatting.

```bash
uv run ruff check src/ tests/
uv run ruff format src/ tests/
```

## Dependency Management

- Runtime and dev dependencies are declared in `pyproject.toml`.
- A `uv.lock` lockfile is committed to version control so all developers and CI use the exact same dependency tree.
- If you update `pyproject.toml`, regenerate the lockfile:
  ```bash
  uv lock
  ```

## Submitting Changes

1. Create a feature branch: `git checkout -b feature/my-change`
2. Make your changes and add tests when applicable.
3. Ensure tests pass: `uv run pytest tests/ -v`
4. Run the linter: `uv run ruff check src/ tests/`
5. Commit with a descriptive message.
6. Open a Pull Request on GitHub.

## Release Notes

See `CHANGELOG.md` for a history of changes.
