# Jorja Clipper — Bug Report & Code Review

**Review date:** 2026-05-23  
**Scope:** All Python source files, tests, and packaging specs.  
**Focus areas:** Logic errors, edge-case crashes, thread safety, resource leaks, ffmpeg behavior, mpv integration, GUI bugs.

---

## Critical

### C1. `QDesktopServices.fromLocalFile` does not exist — double-clicking a clip crashes the app
- **File:** `src/jorja_clipper/gui/main_window.py`  
- **Line:** 177  
- **Issue:** `QDesktopServices` has no static method `fromLocalFile`. The correct API is `QUrl.fromLocalFile(path)`. Double-clicking any saved clip raises `AttributeError` and terminates the application.  
- **Fix:**
  ```python
  from PySide6.QtCore import QUrl   # add import
  QDesktopServices.openUrl(QUrl.fromLocalFile(str(clip.path)))
  ```

### C2. Main window never shuts down mpv — resource leak and potential hang on exit
- **File:** `src/jorja_clipper/gui/main_window.py`  
- **Line:** 26–179 (class body)  
- **Issue:** `MainWindow` does not override `closeEvent()`. When the user closes the window, `Player.shutdown()` is never called, leaving the mpv event thread and renderer alive. On macOS this often causes the app to hang in the Dock and require Force Quit.  
- **Fix:**
  ```python
  def closeEvent(self, event):
      self._player.shutdown()
      event.accept()
  ```

### C3. ffmpeg `-ss` placed before `-i` with `-c copy` produces inaccurate clips (keyframes only)
- **File:** `src/jorja_clipper/clipper.py`  
- **Line:** 65–79 (`save_clip` cmd construction)  
- **Issue:** `-ss` before `-i` is an **input seek**; with stream-copy (`-c copy`) it snaps to the nearest keyframe, which can be several seconds away from the requested time. For a sports-highlight tool that promises ±5 s around the playhead, this silently produces wrong clips. Output seek (`-ss` after `-i`) is frame-accurate.  
- **Fix:** Move `-ss` after the input for frame-accurate cuts, or at minimum document the keyframe limitation. If performance is a concern, offer a precision toggle.
  ```python
  cmd = [
      "ffmpeg", "-y",
      "-i", str(video_path),
      "-ss", str(start),
      "-t", str(duration),
      "-c", "copy",
      "-avoid_negative_ts", "make_zero",
      str(output_path),
  ]
  ```

### C4. Player property observers write shared state from mpv’s background thread without synchronization
- **File:** `src/jorja_clipper/player.py`  
- **Line:** 34–42 (`property_observer` closures)  
- **Issue:** `_on_duration` and `_on_time_pos` run on mpv’s internal event thread, mutating `self._duration` and `self._current_pos`. The Qt main thread reads these values in `_save_clip()` (via `MainWindow`). Under CPython the GIL makes this mostly safe for float references, but there is **no memory barrier**; with PyPy or free-threaded CPython this is a genuine data race. It is also brittle design.  
- **Fix:** Use a `threading.Lock` around writes and reads, or marshal the values to the main thread with `QMetaObject.invokeMethod` / signals. Minimal fix:
  ```python
  import threading
  self._lock = threading.Lock()
  # in observers:
  with self._lock:
      self._duration = float(value)
  # in properties:
  with self._lock:
      return self._duration
  ```

---

## High

### H1. Settings load crashes on binary/corrupt config or permission errors
- **File:** `src/jorja_clipper/settings.py`  
- **Line:** 29–40 (`load`)  
- **Issue:** The `except` block catches only `json.JSONDecodeError` and `KeyError`. A binary/corrupt file raises `UnicodeDecodeError`; a permission-denied file raises `PermissionError` / `OSError`. Both propagate and crash the app on startup.  
- **Fix:**
  ```python
  except (json.JSONDecodeError, KeyError, UnicodeDecodeError, OSError):
      pass
  ```

### H2. `sys.argv` video path check only verifies existence, not that it is a file
- **File:** `src/jorja_clipper/app.py`  
- **Line:** 41–47  
- **Issue:** `video_path.exists()` is `True` for directories. Passing a directory to `player.load()` will raise an `mpv.MPVError` and crash the app.  
- **Fix:** Change `video_path.exists()` to `video_path.is_file()`.

### H3. QApplication mutates `sys.argv` in-place, breaking post-init argument parsing
- **File:** `src/jorja_clipper/app.py`  
- **Line:** 27, 41  
- **Issue:** `QApplication(sys.argv)` removes Qt-specific flags from `sys.argv`. If a user passes a video path after a Qt flag (e.g., `jorja-clipper -style fusion video.mp4`), the path may be removed before the `len(sys.argv) > 1` check.  
- **Fix:** Copy `sys.argv` before passing to `QApplication`, or parse the video path first.
  ```python
  video_args = [a for a in sys.argv[1:] if not a.startswith("-")]
  app = QApplication(sys.argv)
  # later use video_args
  ```

### H4. `subprocess.TimeoutExpired` not handled specifically; user gets raw traceback on long clips
- **File:** `src/jorja_clipper/clipper.py`  
- **Line:** 81–109  
- **Issue:** `timeout=30` is fine for short clips, but on large files or slow systems `subprocess.run` raises `subprocess.TimeoutExpired`. It is caught by the generic `except Exception`, but the error message is a raw Python exception string rather than a user-friendly "Clip extraction timed out" message.  
- **Fix:** Add an explicit `except subprocess.TimeoutExpired` branch with a clear message.

### H5. `player.load()` raises unhandled `mpv.MPVError` for corrupted/unsupported files
- **File:** `src/jorja_clipper/player.py`  
- **Line:** 63–68  
- **Issue:** `self._mpv.play(str(path))` will raise `mpv.MPVError` if the file cannot be loaded. The caller (`MainWindow._open_file` and `app.py`) does not catch it, crashing the app.  
- **Fix:** Wrap in `try/except mpv.MPVError` and return a boolean or raise a custom domain exception that the GUI can handle gracefully.

### H6. `ClipListModel` is orphaned — never attached to the QListWidget view
- **File:** `src/jorja_clipper/gui/main_window.py`  
- **Line:** 99–101, 153  
- **Issue:** `self._clip_model` is instantiated and populated, but `self._clip_list` is a `QListWidget` (convenience widget with its own internal model). The model signals are emitted into the void; the list widget is populated manually via `addItem()`. `ClipListModel` is effectively dead code.  
- **Fix:** Either (a) replace `QListWidget` with `QListView` and `setModel(self._clip_model)`, or (b) remove `ClipListModel` entirely and manage clips inside the widget. Option (a) is cleaner.

### H7. Settings dialog does not expose `output_dir` even though `Settings` stores it
- **File:** `src/jorja_clipper/gui/settings_dialog.py`  
- **Line:** 18–69  
- **Issue:** The `Settings` dataclass has an `output_dir` field (empty = default to `clips/` next to video), but the dialog has no UI for it. Users cannot change the output directory.  
- **Fix:** Add a `QLineEdit` + `QPushButton` (browse) for `output_dir` and persist it in `_on_save`.

### H8. Locale fix sets `LC_ALL` (category 0) instead of `LC_NUMERIC`, and only on Linux
- **File:** `src/jorja_clipper/app.py`  
- **Line:** 10–15  
- **Issue:** `ctypes.c_int(0)` is `LC_ALL` on glibc, not `LC_NUMERIC` (which is `1`). This overwrites *all* locale categories, potentially messing up date/time formatting, collation, etc. The fallback `locale.setlocale(locale.LC_NUMERIC, "C")` is correct but only reached on exception. Additionally, the same libmpv locale crash is known to affect macOS (and sometimes Windows); limiting the fix to `sys.platform == "linux""` is too narrow.  
- **Fix:**
  ```python
  import locale
  # run on all Unix-like platforms
  if sys.platform in ("linux", "darwin", "freebsd", "openbsd"):
      try:
          locale.setlocale(locale.LC_NUMERIC, "C")
      except locale.Error:
          pass
  ```

---

## Medium

### M1. Clip key shortcut is hardcoded and never updated after settings change
- **File:** `src/jorja_clipper/gui/main_window.py`  
- **Line:** 107–116 (`_setup_shortcuts`)  
- **Issue:** `QShortcut(QKeySequence("C"), self, self._save_clip)` is registered once at startup. If the user changes the clip key in Settings, the old shortcut remains and the new one is never registered. The user must restart the app for the new key to work.  
- **Fix:** Extract shortcut creation into a `update_shortcuts()` method. Call it from `__init__` and from `_open_settings` after the dialog is accepted. Keep references to `QShortcut` objects in a list so old ones can be deleted.

### M2. `_save_clip` increments `_clip_count` before verifying success
- **File:** `src/jorja_clipper/gui/main_window.py`  
- **Line:** 142  
- **Issue:** `self._clip_count += 1` happens unconditionally. If ffmpeg fails, the number is still consumed, leaving gaps in clip filenames (e.g., `_001`, `_003`).  
- **Fix:** Increment only after `result.success is True`.

### M3. `main_window.py` accesses private attributes from `app.py`
- **File:** `src/jorja_clipper/app.py`  
- **Line:** 45–46  
- **Issue:** `window._current_video = video_path` and `window._status.setText(...)` access private members (single-underscore convention) from outside the class. This breaks encapsulation and makes refactoring dangerous.  
- **Fix:** Add public setters or properties on `MainWindow`, e.g. `window.load_video(video_path)`.

### M4. `toggle_pause` mutates local state before issuing mpv command
- **File:** `src/jorja_clipper/player.py`  
- **Line:** 74–75  
- **Issue:** `self._paused = not self._paused` runs before `self._mpv.pause = ...`. If the mpv command raises (e.g., mpv is in a broken state), the internal boolean is now wrong and there is no way to recover without restarting the player.  
- **Fix:** Toggle the mpv property first, then mirror the state locally:
  ```python
  new_state = not self._paused
  self._mpv.pause = "yes" if new_state else "no"
  self._paused = new_state
  ```

### M5. `Player.seek` raises unhandled `mpv.MPVError`
- **File:** `src/jorja_clipper/player.py`  
- **Line:** 77–81  
- **Issue:** `self._mpv.command("seek", ...)` raises if no file is loaded or if mpv is terminating. The shortcut lambdas in `MainWindow` do not catch this.  
- **Fix:** Wrap in `try/except mpv.MPVError: pass` (silently ignore seek when nothing is loaded).

### M6. `video_widget.py` embedding may fail silently on macOS
- **File:** `src/jorja_clipper/gui/video_widget.py`  
- **Line:** 17–31 (`showEvent`)  
- **Issue:** On macOS, `vo=libmpv` with a raw `wid` integer does **not** reliably embed into an arbitrary `NSView`. The Cocoa backend (`cocoa-cb`) requires special API (`--macos-app-activation-policy`, `cocoa-cb` render context) that simple `wid` mapping does not provide. Users may see a black widget or a detached window. This is a known upstream limitation.  
- **Fix:** At minimum, log the `wid` and platform. Consider a platform-specific embed path: on macOS, use `vo=libmpv` without `wid` and let mpv create its own Cocoa window, or document that embedding is best-effort on macOS.

### M7. `settings.save()` can raise `OSError` (disk full, no permissions) uncaught
- **File:** `src/jorja_clipper/settings.py`  
- **Line:** 18–27  
- **Issue:** `write_text()` propagates `OSError`, `PermissionError`, etc. The settings dialog and app startup have no protection.  
- **Fix:** Wrap in `try/except OSError` and surface a message to the status bar or a `QMessageBox`.

### M8. `test_clipper.py` tests an impossible exception (`CalledProcessError` without `check=True`)
- **File:** `tests/test_clipper.py`  
- **Line:** 92–105  
- **Issue:** `clipper.py` uses `subprocess.run(...)` **without** `check=True`, so `CalledProcessError` is never raised in real code. The test injects it via `side_effect`, giving a false sense of coverage. The actual failure branch (`returncode != 0`) is already tested in the preceding test, making this one redundant and misleading.  
- **Fix:** Remove the redundant test, or change it to test a realistic failure such as `ffmpeg` not found in `$PATH` (`FileNotFoundError`).

### M9. `main.py` is a stale placeholder that does nothing
- **File:** `src/jorja_clipper/main.py`  
- **Line:** 1–14  
- **Issue:** The file prints a message and exits. The real entry point is `app.py`. This is confusing for new contributors and may break certain import expectations.  
- **Fix:** Either delete the file or make it delegate to `app.main()`.

### M10. `runtime_hook_mpv.py` shadowing of parameter `name`
- **File:** `packaging/runtime_hook_mpv.py`  
- **Line:** 10, 16, 33  
- **Issue:** The outer function parameter `name` is shadowed by the loop variable `for name in candidates:`. If the original `name` was something other than `"mpv"` / `"libmpv"`, the loop still executes and could return an unrelated library path.  
- **Fix:** Rename the loop variable:
  ```python
  for candidate in candidates:
      path = os.path.join(bundle_dir, candidate)
      ...
  ```

---

## Low

### L1. `Player._paused` not synced with actual mpv pause state after load
- **File:** `src/jorja_clipper/player.py`  
- **Line:** 66–68  
- **Issue:** `self._paused = True` is set unconditionally after `load()`. If mpv’s default pause state differs (e.g., `pause=no` in `mpv.conf`), the wrapper is out of sync.  
- **Fix:** Query `self._mpv.pause` after load, or observe the `pause` property.

### L2. `calculate_times` can receive `None` duration and crash with `TypeError`
- **File:** `src/jorja_clipper/clipper.py`  
- **Line:** 29–33  
- **Issue:** If `video_duration` is `None` (live stream, metadata not loaded), `min(None, ...)` raises `TypeError`.  
- **Fix:** Guard against `None`:
  ```python
  if video_duration is None or current_pos is None:
      return 0.0, 0.0
  ```

### L3. No check for `ffmpeg` availability at runtime
- **File:** `src/jorja_clipper/clipper.py`  
- **Line:** 65  
- **Issue:** If `ffmpeg` is not installed, `subprocess.run` raises `FileNotFoundError`, caught generically, but the user only sees a cryptic error string.  
- **Fix:** Check `shutil.which("ffmpeg")` during app startup or in `save_clip`, and surface a clear message like "ffmpeg not found in PATH".

### L4. `QShortcut` objects not stored as instance attributes
- **File:** `src/jorja_clipper/gui/main_window.py`  
- **Line:** 109–116  
- **Issue:** `QShortcut` instances are created as temporaries. While Qt’s parent-child system usually keeps them alive (parent=`self`), this is fragile. If Python GC ever collects them before Qt’s C++ side notices, shortcuts disappear.  
- **Fix:** Store in a list: `self._shortcuts = []` and `self._shortcuts.append(QShortcut(...))`.

### L5. `ClipListModel.rowCount` default argument is `None` instead of `QModelIndex()`
- **File:** `src/jorja_clipper/gui/clip_list.py`  
- **Line:** 25  
- **Issue:** `parent=None` works in PySide6 due to forgiving argument handling, but the Qt API contract expects a `QModelIndex()`. Some Qt versions or bindings may warn or misbehave.  
- **Fix:** Use `parent=QModelIndex()` as the default.

### L6. `settings_dialog.py` allows empty clip key
- **File:** `src/jorja_clipper/gui/settings_dialog.py`  
- **Line:** 65–67  
- **Issue:** If the user clears the `QKeySequenceEdit`, `key` is an empty string. `if key:` guards against it, but the old shortcut is already registered. The result is a broken hotkey configuration.  
- **Fix:** Reject empty keys in the dialog, e.g. show a red status label and do not accept.

### L7. Integration tests assume `ffmpeg` exists without skip guard
- **File:** `tests/test_integration.py`  
- **Line:** 1–83  
- **Issue:** If `ffmpeg` is not installed, every integration test fails with an opaque error instead of being skipped.  
- **Fix:** Add a `pytest.mark.skipif(shutil.which("ffmpeg") is None, reason="ffmpeg not installed")` decorator.

### L8. `settings.py` does not validate loaded numeric values
- **File:** `src/jorja_clipper/settings.py`  
- **Line:** 34–38  
- **Issue:** A hand-edited JSON with `"buffer_before": "abc"` or a negative number will be loaded and later cause crashes in `calculate_times` or ffmpeg.  
- **Fix:** Validate types after loading, falling back to defaults on mismatch.
  ```python
  self.buffer_before = float(data.get("buffer_before", self.buffer_before))
  if self.buffer_before < 0:
      self.buffer_before = 5.0
  ```

---

## Summary Table

| ID  | Severity | File | Line(s) | Category |
|-----|----------|------|---------|----------|
| C1  | Critical | `gui/main_window.py` | 177 | GUI crash |
| C2  | Critical | `gui/main_window.py` | 26–179 | Resource leak |
| C3  | Critical | `clipper.py` | 65–79 | ffmpeg logic |
| C4  | Critical | `player.py` | 34–42 | Thread safety |
| H1  | High | `settings.py` | 29–40 | Startup crash |
| H2  | High | `app.py` | 41–47 | Edge-case crash |
| H3  | High | `app.py` | 27, 41 | Argument parsing |
| H4  | High | `clipper.py` | 81–109 | UX / error handling |
| H5  | High | `player.py` | 63–68 | mpv error |
| H6  | High | `gui/main_window.py` | 99–101, 153 | Dead code / UI mismatch |
| H7  | High | `gui/settings_dialog.py` | 18–69 | Missing feature |
| H8  | High | `app.py` | 10–15 | Locale fix incomplete |
| M1  | Medium | `gui/main_window.py` | 107–116 | Shortcut staleness |
| M2  | Medium | `gui/main_window.py` | 142 | Logic error |
| M3  | Medium | `app.py` | 45–46 | Encapsulation |
| M4  | Medium | `player.py` | 74–75 | State inconsistency |
| M5  | Medium | `player.py` | 77–81 | mpv error |
| M6  | Medium | `gui/video_widget.py` | 17–31 | macOS embedding |
| M7  | Medium | `settings.py` | 18–27 | OSError uncaught |
| M8  | Medium | `tests/test_clipper.py` | 92–105 | Test quality |
| M9  | Medium | `main.py` | 1–14 | Dead code |
| M10 | Medium | `packaging/runtime_hook_mpv.py` | 10, 16, 33 | Variable shadowing |
| L1  | Low | `player.py` | 66–68 | State sync |
| L2  | Low | `clipper.py` | 29–33 | None guard |
| L3  | Low | `clipper.py` | 65 | Missing dependency check |
| L4  | Low | `gui/main_window.py` | 109–116 | GC risk |
| L5  | Low | `gui/clip_list.py` | 25 | API contract |
| L6  | Low | `gui/settings_dialog.py` | 65–67 | Empty key |
| L7  | Low | `tests/test_integration.py` | 1–83 | Test portability |
| L8  | Low | `settings.py` | 34–38 | Input validation |

---

*End of report.*
