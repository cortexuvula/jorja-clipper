# Jorja Clipper — Technical Debt Review

**Review date:** 2026-05-23
**Version reviewed:** v0.1.0
**Scope:** All Python source (~640 SLOC), tests (~321 SLOC), packaging specs, CI, and project metadata.

---

## Executive Summary

Jorja Clipper is a lean, functional MVP with a clear value proposition. The codebase is small enough to remain manageable, but several structural decisions made during rapid prototyping will compound maintenance cost as features are added. The most significant risks are: (1) UI freezes during clip extraction due to synchronous ffmpeg calls on the main thread, (2) tight coupling between the GUI and backend modules preventing testability, (3) zero persistence for clip metadata, and (4) packaging fragility caused by unmanaged external binaries (ffmpeg, libmpv). The recent bug review fixed 28 issues; this review focuses on the deeper structural and strategic debt that remains.

**Overall health score:** 6.5 / 10
- Stability: 7 (core logic is sound after bug fixes)
- Testability: 4 (heavy GUI coupling, missing abstractions)
- Maintainability: 6 (small codebase, but growing complexity in MainWindow)
- Packaging: 5 (platform-specific specs with duplication)
- Scalability: 5 (single-video, single-threaded design)

---

## 1. Code Architecture Issues

### A1. `MainWindow` is a God Class
- **File:** `src/jorja_clipper/gui/main_window.py` (203 lines)
- **Issue:** `MainWindow` orchestrates file dialogs, shortcut registration, clip saving, settings propagation, video loading, status updates, and clip preview. It directly calls `self._clipper.save_clip(...)`, `self._player.seek(...)`, and `self._settings.save()` with no intermediary layer. This violates the Single Responsibility Principle and makes the window impossible to unit-test in isolation.
- **Impact:** Any change to the clip workflow requires touching UI code. Refactoring the player backend forces changes in the window class.
- **Fix:** Introduce an `AppController` or `ApplicationFacade` that owns `Player`, `Clipper`, and `Settings`. The window should emit signals (e.g., `clip_requested`, `file_opened`) and the controller should execute the workflow. This also makes headless/scripted usage possible.

### A2. No Layered Architecture
- **Issue:** The project mixes presentation, domain, and infrastructure concerns in a flat module structure:
  - `app.py` — application bootstrap + locale fix
  - `clipper.py` — domain logic + subprocess infrastructure
  - `player.py` — domain wrapper + low-level mpv embedding
  - `settings.py` — data class + filesystem I/O
  - `gui/*.py` — presentation layer
- **Impact:** There is no clear boundary between "what the app does" and "how it does it." Swapping ffmpeg for another clipping engine would require changes inside `clipper.py` that could leak into the GUI.
- **Fix:** Adopt a minimal layered structure:
  ```
  src/jorja_clipper/
    core/          — domain models (Clip, Timeline, Config)
    services/      — business workflows (ClipService, PlaybackService)
    infrastructure/ — ffmpeg runner, mpv player, filesystem settings store
    gui/           — presentation (unchanged)
    app.py         — composition root (wires everything together)
  ```

### A3. Composition Root is Implicit
- **File:** `src/jorja_clipper/app.py`
- **Issue:** `app.py` manually instantiates `Settings`, `Player`, `Clipper`, and `MainWindow` with hardcoded constructor arguments. There is no dependency injection or factory pattern.
- **Impact:** Testing requires monkey-patching imports or complex mocking. Integration tests cannot substitute a fake player or clipper.
- **Fix:** A simple factory function `create_app(player_factory, clipper_factory, settings_store)` would suffice at this scale.

---

## 2. Missing Abstractions & Poor Separation of Concerns

### S1. No Abstract Player Interface
- **File:** `src/jorja_clipper/player.py`
- **Issue:** `Player` is a concrete wrapper around `python-mpv`. There is no `AbstractPlayer` protocol or interface. The GUI directly depends on `Player` properties (`duration`, `current_pos`, `paused`).
- **Impact:** Replacing mpv with VLC, AVFoundation, or a custom renderer would require rewriting `MainWindow` and `VideoWidget`.
- **Fix:** Define a `PlayerProtocol` (or `abc.ABC`) with `load(path)`, `play()`, `pause()`, `seek(offset)`, `position`, `duration`. Make `MainWindow` depend only on the protocol.

### S2. No Abstract Clip Engine Interface
- **File:** `src/jorja_clipper/clipper.py`
- **Issue:** `Clipper` is tightly bound to ffmpeg command-line invocation. The `save_clip` method constructs the `ffmpeg` arg list inline and calls `subprocess.run` directly.
- **Impact:** Adding a second backend (e.g., `mkvextract` for MKV files, or a future re-encode option) requires forking the class.
- **Fix:** Extract a `ClipEngine` protocol with `extract(video_path, start, end, output_path) -> ClipResult`. Provide `FfmpegClipEngine` as the default implementation.

### S3. Settings is Both Domain Model and Persistence Layer
- **File:** `src/jorja_clipper/settings.py`
- **Issue:** `Settings` knows its file path, JSON format, and default values. It is not a pure domain object.
- **Fix:** Split into `Settings` (dataclass with validation) and `SettingsStore` (JSON file persistence). This allows swapping to QSettings, SQLite, or a registry later without touching the domain class.

### S4. `ClipResult` Lacks Rich Metadata
- **File:** `src/jorja_clipper/clipper.py`
- **Issue:** `ClipResult` only stores path, times, success flag, and error string. It does not capture file size, codec info, or creation timestamp.
- **Impact:** Future features (clip library, thumbnails, export) will need to probe the output file after creation.
- **Fix:** Add optional fields: `file_size`, `codec`, `created_at`. This is a small, backward-compatible change.

### S5. `VideoWidget` Platform Logic Leaked into `Player`
- **File:** `src/jorja_clipper/player.py` (lines 24–33), `src/jorja_clipper/gui/video_widget.py`
- **Issue:** `Player._ensure_mpv()` contains platform-specific `vo=libmpv` logic for macOS. `VideoWidget` also handles platform embedding. The responsibility for "how to render" is split across two classes.
- **Fix:** Move all platform-specific initialization into a `PlatformPlayerFactory` or keep it inside `VideoWidget` (which already knows it is a Qt widget).

---

## 3. Test Coverage Gaps

### T1. No Unit Tests for `MainWindow` Workflow Logic
- **Issue:** `test_gui.py` only tests `ClipListModel`, `SettingsDialog` field population, and `VideoWidget` initialization. The core interactions (open file → load video → save clip → update list) are untested.
- **Impact:** regressions in `_save_clip`, `_open_file`, or `_open_settings` will not be caught by CI.
- **Fix:** Extract controller logic from `MainWindow` so it can be tested without Qt. Where Qt is required, use `pytest-qt` to simulate clicks and verify model state.

### T2. No Tests for `app.py`
- **Issue:** The bootstrap sequence (locale fix, argument parsing, object wiring) is untested.
- **Fix:** Add an `test_app.py` that verifies `main()` exits cleanly and parses arguments correctly.

### T3. `VideoWidget` Embedding Untested
- **Issue:** The critical `showEvent` → `winId()` → `init_with_wid()` chain has no test coverage. Platform-specific embedding bugs (especially on macOS) are only discovered manually.
- **Fix:** Mock `winId()` and verify `player.init_with_wid()` is called exactly once.

### T4. No Tests for `__main__.py`
- **Issue:** The module-level entry point is a single line, but it should at least be smoke-tested.

### T5. Low Branch Coverage in `Clipper`
- **Issue:** `test_clipper.py` tests happy path and missing ffmpeg, but does not cover:
  - `end <= start` guard
  - `subprocess.TimeoutExpired`
  - Generic `Exception` fallback
  - `build_output_path` edge cases (missing suffix, special characters in filename)
- **Fix:** Add parameterized tests for these branches.

### T6. No Property-Based or Fuzz Testing
- **Issue:** Edge cases like `buffer_before=0`, `current_pos=NaN`, very long filenames, or filenames with Unicode are not exercised.
- **Fix:** Introduce `hypothesis` for a small number of property tests on `calculate_times` and `build_output_path`.

### T7. Test Suite Relies on ffmpeg Being Installed
- **Issue:** `test_integration.py` is gated behind a `skipif`, but `conftest.py`'s `test_video` fixture also calls ffmpeg and is used by integration tests only. The unit tests (`test_clipper.py`) mock subprocess, which is good.
- **Note:** This is acceptable but should be documented clearly.

---

## 4. Outdated Dependencies & Patterns

### D1. Dependency Version Ranges are Too Broad
- **File:** `pyproject.toml` (lines 29–32)
- **Issue:**
  - `PySide6>=6.6` — PySide6 6.6 and 6.11 have significant API differences (e.g., `QEnum` behavior, deprecated shiboken signatures).
  - `python-mpv>=1.0.1` — The mpv binding has had breaking changes in observer APIs across minor versions.
- **Impact:** CI may pass on one version while users on a newer/older version experience crashes.
- **Fix:** Pin to a tested range: `PySide6>=6.11,<6.12` and `python-mpv>=1.0.7,<2.0`. Maintain a lockfile (`requirements.lock` or `uv.lock`).

### D2. No Type Hints on Several Public Methods
- **Files:** `gui/main_window.py`, `gui/settings_dialog.py`
- **Issue:** Methods like `_open_file()`, `_save_clip()`, `_toggle_play()` have no return type annotations. The project already uses `|` union syntax (Python 3.10+), so full typing is feasible.
- **Fix:** Add `-> None` and parameter types everywhere. Enable `mypy --strict` in CI.

### D3. Settings Could Use a Dataclass with Validation
- **File:** `src/jorja_clipper/settings.py`
- **Issue:** `Settings` is a hand-written class with manual JSON serialization. It does not use `dataclasses`, `pydantic`, or `attrs`.
- **Fix:** Convert to `@dataclass` with `__post_init__` validation, or adopt `pydantic.BaseModel` for automatic JSON serialization and schema validation.

### D4. `main.py` is a Stale Placeholder
- **File:** `src/jorja_clipper/main.py`
- **Issue:** The file delegates to `app.main()` but adds no value. The real entry point is `__main__.py`.
- **Fix:** Remove `main.py` to reduce confusion.

### D5. `.spec.new` Files are Artifacts
- **Files:** `packaging/linux.spec.new`, `packaging/macos.spec.new`
- **Issue:** These appear to be backup/artefact files from editing. They should not be in version control.
- **Fix:** Delete them and add `*.spec.new` to `.gitignore`.

---

## 5. Hardcoded Values That Should Be Configurable

| Value | Location | Should Be In |
|-------|----------|------------|
| `buffer_before=5.0`, `buffer_after=5.0` | `clipper.py`, `settings.py` defaults | Already in Settings, but defaults duplicated |
| `1200x700` min window size | `main_window.py` | Settings or `ui_config.json` |
| `800x500` video widget min size | `video_widget.py` | Settings |
| `#1a1a2e` background color | `video_widget.py` | Theme config |
| `#e94560` clip button color | `main_window.py` | Theme config |
| `30` second ffmpeg timeout | `clipper.py` | Settings (advanced) |
| `±5.0s` / `±1.0s` seek steps | `main_window.py` shortcuts | Settings |
| `"clips"` output folder name | `clipper.py` | Settings (already partially supported) |
| `"%Y%m%d_%H%M%S"` timestamp format | `clipper.py` | Settings |
| `[900, 300]` splitter sizes | `main_window.py` | Settings (persisted) |
| `*.mp4 *.mkv *.avi *.mov *.webm *.ts` filters | `main_window.py` | Configurable or derived from ffmpeg capabilities |

---

## 6. Missing Error Handling Patterns

### E1. Synchronous ffmpeg Call Blocks the GUI Thread
- **File:** `src/jorja_clipper/gui/main_window.py` (lines 158–177)
- **Issue:** `self._clipper.save_clip(...)` calls `subprocess.run(..., timeout=30)` directly from the Qt main thread. If ffmpeg takes 5–30 seconds (large file, slow disk), the entire UI freezes.
- **Impact:** User sees a hung window; macOS may show the spinning beach ball.
- **Fix:** Move ffmpeg execution to a `QThread` or use `asyncio.create_subprocess_exec` with a callback. Show a progress indicator or at least a busy cursor.

### E2. No Centralized Error Reporting
- **Issue:** Errors are surfaced ad-hoc via `window.set_status()` or `QMessageBox`. There is no consistent error taxonomy (user error, system error, external tool error).
- **Fix:** Introduce a lightweight `ErrorHandler` that routes errors to the status bar, a toast notification, or a modal dialog based on severity.

### E3. `Player.load()` Failure Handled Inconsistently
- **File:** `src/jorja_clipper/player.py` (lines 74–83), `src/jorja_clipper/app.py` (lines 40–45)
- **Issue:** `Player.load()` catches `mpv.MPVError` and returns a boolean, but `MainWindow._open_file()` also catches failure and sets status text. The error message is not detailed enough to diagnose codec issues.
- **Fix:** Return a rich result object (or raise a domain exception) with an error category and suggestion.

### E4. No Recovery Path for Missing External Dependencies
- **Issue:** If ffmpeg or libmpv is missing, the app starts but fails silently when the user tries to clip or play.
- **Fix:** Perform a dependency health check at startup and show a "Setup Wizard" or warning dialog before the main window appears.

### E5. No Logging Configuration
- **Issue:** `logging.getLogger(__name__)` is used in `settings_dialog.py` and `video_widget.py`, but there is no root logger configuration. Logs may go to the void or to stderr unpredictably across platforms.
- **Fix:** Configure a rotating file handler and a platform-appropriate stderr/console handler in `app.py`.

---

## 7. Documentation Gaps

| Gap | Impact | Priority |
|-----|--------|----------|
| No `CONTRIBUTING.md` | New contributors don't know how to set up the project | Medium |
| No `CHANGELOG.md` | Users can't track what changed between releases | Medium |
| No architecture / module diagram | Maintainers must read all files to understand boundaries | High |
| No inline docstrings on private GUI methods | IDE hover help is missing | Low |
| No platform-specific setup guide | macOS/Windows users struggle with mpv/ffmpeg installation | High |
| No packaging / release playbook | Releasing v0.1.1 requires tribal knowledge | Medium |
| README lacks screenshot / GIF | GitHub page is plain; lowers user trust | Low |
| No API docs for `Clipper` / `Player` | Scripting/hacking is harder | Low |

---

## 8. Build & Packaging Issues

### B1. Three Duplicated PyInstaller Specs
- **Files:** `packaging/{linux,macos,windows}.spec`
- **Issue:** ~70% of each spec is identical (Analysis, PYZ, EXE, hiddenimports). Only the binary discovery path and BUNDLE step differ.
- **Impact:** Updating shared options (e.g., adding a hidden import) requires editing three files.
- **Fix:** Create a shared `packaging/common.py` with `make_analysis()`, and have each `.spec` import it.

### B2. No Code Signing or Notarization
- **Issue:** macOS `.app` bundles built by CI are unsigned. On modern macOS, Gatekeeper will block them unless the user right-clicks → Open. Windows executables are also unsigned, triggering SmartScreen.
- **Fix:** Add CI steps for:
  - macOS: `codesign` + `xcrun notarytool submit`
  - Windows: AzureSignTool or signtool with a certificate
  Document that this requires secrets not available in forks.

### B3. PyInstaller Spec Uses Fragile Relative Paths
- **Issue:** `../src/jorja_clipper/app.py` assumes the spec is always run from the `packaging/` directory.
- **Fix:** Resolve paths relative to the spec file itself: `os.path.join(os.path.dirname(SPECPATH), 'src', 'jorja_clipper', 'app.py')`.

### B4. No ARM64-Specific macOS Build
- **Issue:** `macos-latest` on GitHub Actions is now Apple Silicon, but the spec searches both `/opt/homebrew` and `/usr/local`. This works, but there is no explicit `macos-13` (Intel) runner matrix entry for a universal build.
- **Fix:** Add an explicit Intel runner and create a universal binary, or ship separate `arm64` and `x86_64` artifacts.

### B5. Linux Build is a Folder, Not a Self-Contained Package
- **Issue:** The Linux artifact is a `dist/jorja-clipper` directory. Users must extract and run a binary inside a folder. There is no `.AppImage`, `.deb`, or `.rpm`.
- **Fix:** Add an `appimage-builder` step or `fpm` packaging step to CI.

### B6. CI Release Job Downloads All Artifacts Blindly
- **File:** `.github/workflows/ci.yml` (lines 127–138)
- **Issue:** `action-gh-release` uploads all artifacts. If one platform build fails, the release may still proceed with missing assets.
- **Fix:** Add an explicit `needs: build` job that verifies all artifacts exist before releasing.

---

## 9. Dependency Management Concerns

### M1. External Binaries are Not Version-Pinned
- **Issue:** CI installs `ffmpeg` and `mpv` via `apt`, `brew`, and `choco`. The versions vary across runners and time. A future ffmpeg release could change CLI behavior (e.g., default encoder) and break clipping.
- **Fix:** Pin to known-good versions in CI. Document the minimum tested versions in README.

### M2. No Lockfile for Python Dependencies
- **Issue:** `pip install -e '.[dev]'` resolves the latest compatible versions at install time. Two developers may have different dependency trees.
- **Fix:** Generate a `requirements.txt` or `uv.lock` from `pyproject.toml` and check it into version control.

### M3. `dev` Extras Mix Concerns
- **File:** `pyproject.toml` (lines 34–40)
- **Issue:** `dev` includes `pytest`, `pytest-qt`, `ruff`, and `pyinstaller`. Packaging tools (`pyinstaller`) are needed for release builds, not day-to-day development.
- **Fix:** Split into `dev = ["pytest", "pytest-qt", "ruff"]` and `packaging = ["pyinstaller"]`.

### M4. `packaging/runtime_hook_mpv.py` Shadows Variables
- **File:** `packaging/runtime_hook_mpv.py` (lines 10, 16, 33)
- **Issue:** The function parameter `name` is shadowed by the loop variable `candidate`. This was noted in the bug review (M10) and is a mild but real code-smell.
- **Fix:** Rename the loop variable (already backlog from bug review).

---

## 10. Scalability Limitations

### L1. Single-Video, Single-Session Design
- **Issue:** The app only tracks one `self._current_video` at a time. There is no playlist, recent files, or multi-tab support.
- **Impact:** Users analyzing a full game (e.g., 90-minute soccer match) must open the file, clip, close, and repeat for the next game.
- **Roadmap:** Add a `RecentFiles` store and a playlist sidebar.

### L2. Clip Metadata is Volatile
- **Issue:** `ClipListModel` stores clips in a Python list. Closing the app loses the clip list. There is no library, tagging, or search.
- **Impact:** Power users who create 50+ clips per session cannot organize or revisit them.
- **Roadmap:** Persist clips to a SQLite database with fields: `id`, `source_video_path`, `clip_path`, `start`, `end`, `tags`, `notes`, `created_at`.

### L3. No Batch / Background Processing
- **Issue:** Each clip triggers a single ffmpeg process. There is no queue for bulk export or background processing.
- **Impact:** Saving 10 clips sequentially means 10 UI-blocking calls.
- **Roadmap:** Implement a `ClipQueue` with a worker thread and a progress panel.

### L4. No Undo / Redo
- **Issue:** Accidentally clicking "Clip" creates a file immediately. There is no undo.
- **Impact:** Users may litter their disk with unwanted clips.
- **Roadmap:** Add a "Confirm clip" option in Settings, or implement undo by moving the file to a trash folder.

### L5. Settings are Not Reactive
- **Issue:** Changing a setting in the dialog writes to disk, but other components only see the change when manually notified (`update_shortcuts()`). There is no observer or pub-sub system.
- **Impact:** Adding a new setting requires threading the value through multiple classes manually.
- **Roadmap:** Use Qt signals (`settings_changed`) or a lightweight event bus.

### L6. No Plugin or Extension System
- **Issue:** The architecture is closed. Users cannot add custom post-processing (e.g., upload to cloud, add watermark, generate GIF).
- **Roadmap:** Not urgent for v0.1.x, but keep the `ClipEngine` protocol open so power users can inject custom engines.

---

## Prioritized Backlog

### 🔴 P0 — Critical (Fix Before Next Release)

| ID | Item | Effort | Owner Suggestion |
|----|------|--------|------------------|
| P0-1 | Move ffmpeg execution to a background thread (`QThread`) | **M** | `clipper.py` + `main_window.py` |
| P0-2 | Introduce `AppController` to decouple `MainWindow` from `Clipper`/`Player` | **M** | New `services/controller.py` |
| P0-3 | Pin dependency versions and commit a lockfile | **S** | `pyproject.toml` + CI |
| P0-4 | Remove stale `main.py` and `.spec.new` files | **S** | Cleanup |

### 🟠 P1 — High (Address in v0.2.0)

| ID | Item | Effort | Owner Suggestion |
|----|------|--------|------------------|
| P1-1 | Define `PlayerProtocol` and `ClipEngine` ABCs | **M** | `core/protocols.py` |
| P1-2 | Add `CONTRIBUTING.md` and platform setup guides | **S** | Docs |
| P1-3 | Implement clip persistence (SQLite) and a clip library view | **L** | `services/clip_store.py` + `gui/clip_library.py` |
| P1-4 | Add dependency health check at startup | **S** | `app.py` |
| P1-5 | Add proper logging configuration | **S** | `app.py` |
| P1-6 | Refactor Settings into dataclass + `SettingsStore` | **M** | `settings.py` |
| P1-7 | Unify PyInstaller specs with a shared module | **M** | `packaging/common.py` |

### 🟡 P2 — Medium (Backlog for v0.3.0)

| ID | Item | Effort | Owner Suggestion |
|----|------|--------|------------------|
| P2-1 | Add batch clip queue with progress UI | **L** | `services/queue.py` |
| P2-2 | Add recent files / playlist sidebar | **M** | `gui/playlist.py` |
| P2-3 | Add undo / trash for accidental clips | **M** | `services/clip_store.py` |
| P2-4 | Add `mypy --strict` to CI and type-hint all public APIs | **M** | CI + all `.py` |
| P2-5 | Add code signing / notarization to release pipeline | **L** | `.github/workflows/ci.yml` |
| P2-6 | Add Linux `.AppImage` packaging | **M** | CI + `packaging/` |
| P2-7 | Add theme / color config and remove hardcoded styles | **M** | `gui/theme.py` |

### 🟢 P3 — Low (Nice to Have)

| ID | Item | Effort | Owner Suggestion |
|----|------|--------|------------------|
| P3-1 | README screenshot / demo GIF | **S** | Docs |
| P3-2 | Property-based tests for `calculate_times` | **S** | `tests/test_clipper.py` |
| P3-3 | Add `__all__` exports to all modules | **S** | Cleanup |
| P3-4 | Add a `CHANGELOG.md` | **S** | Docs |
| P3-5 | Evaluate `pydantic` for settings validation | **S** | Spike |

---

## Recommended Roadmap

### Phase 1 — Foundation (v0.2.0, ~4–6 weeks)
1. **Merge bug fixes** from the bug review (already completed).
2. **Decouple GUI from backend** via `AppController` and protocols.
3. **Move ffmpeg to a worker thread** to eliminate UI freezes.
4. **Add dependency pinning** and a lockfile.
5. **Add startup health checks** and centralized error handling.
6. **Write `CONTRIBUTING.md`** and platform setup docs.

### Phase 2 — Data Layer (v0.3.0, ~4–6 weeks)
1. **Refactor Settings** into dataclass + store.
2. **Add SQLite-backed clip library** with search and tags.
3. **Add recent files** and a simple playlist.
4. **Add undo / trash** for clips.
5. **Improve packaging**: `.AppImage`, signed macOS bundle, signed Windows binary.

### Phase 3 — Scale (v0.4.0+, ~6–8 weeks)
1. **Batch clip queue** with background processing.
2. **Theme system** and configurable UI.
3. **Plugin interface** for custom clip engines.
4. **Optional re-encode profiles** (e.g., social-media formats).
5. **Comprehensive test suite** targeting >80% branch coverage.

---

## Appendix: Metrics

| Metric | Value |
|--------|-------|
| Total Python source lines | ~640 (excluding packaging specs) |
| Total test lines | ~321 |
| Test-to-source ratio | 0.50 : 1 |
| Number of modules | 9 source + 5 GUI |
| Number of test files | 5 |
| External dependencies | ffmpeg, libmpv, PySide6, python-mpv |
| CI platforms | Ubuntu, macOS, Windows |
| Packaging systems | PyInstaller (3 platform specs) |

---

*End of Technical Debt Review.*
