"""Tests for the theme system."""

from jorja_clipper.gui.theme import THEME_DARK, THEME_LIGHT, THEMES, ThemeManager


def test_theme_dark_fields():
    """Dark theme has dark background and light foreground."""
    t = THEME_DARK
    assert t.window_bg == "#1a1a2e"
    assert t.window_fg == "#e0e0e0"
    assert t.accent == "#e94560"


def test_theme_light_fields():
    """Light theme has light background and dark foreground."""
    t = THEME_LIGHT
    assert t.window_bg == "#f5f5f5"
    assert t.window_fg == "#1a1a2e"
    assert t.accent == "#e94560"


def test_themes_dict():
    """THEMES contains at least dark and light."""
    assert "dark" in THEMES
    assert "light" in THEMES


def test_theme_manager_default():
    """ThemeManager defaults to the dark theme."""
    tm = ThemeManager()
    assert tm.theme_name == "dark"
    assert tm.theme == THEME_DARK


def test_theme_manager_switch():
    """ThemeManager can switch themes."""
    tm = ThemeManager("light")
    assert tm.theme_name == "light"
    assert tm.theme == THEME_LIGHT
    tm.theme_name = "dark"
    assert tm.theme == THEME_DARK


def test_theme_manager_unknown_falls_back():
    """An unknown theme name falls back to dark."""
    tm = ThemeManager("nonexistent")
    assert tm.theme == THEME_DARK


def test_stylesheet_contains_colors():
    """The generated stylesheet references theme colors."""
    tm = ThemeManager("dark")
    ss = tm.stylesheet()
    assert "#1a1a2e" in ss
    assert "#e0e0e0" in ss
    assert "QPushButton" in ss
    assert "QListView" in ss


def test_light_stylesheet_contains_colors():
    """The generated light stylesheet references light colors."""
    tm = ThemeManager("light")
    ss = tm.stylesheet()
    assert "#f5f5f5" in ss
    assert "#1a1a2e" in ss
    assert "QPushButton" in ss


def test_clip_button_stylesheet():
    """The stylesheet styles the special clipButton ID."""
    tm = ThemeManager()
    ss = tm.stylesheet()
    assert "QPushButton#clipButton" in ss
    assert tm.theme.accent in ss
