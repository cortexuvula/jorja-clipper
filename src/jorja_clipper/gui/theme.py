"""Configurable theme system for Jorja Clipper.

Provides a :class:`Theme` dataclass that bundles all colors, fonts, and spacing,
and a :class:`ThemeManager` that can apply the current theme as a Qt stylesheet.
"""

from dataclasses import dataclass

__all__ = ["Theme", "THEMES", "ThemeManager"]


@dataclass(frozen=True)
class Theme:
    """A complete UI theme specification."""

    # Window
    window_bg: str
    window_fg: str
    accent: str

    # Buttons
    button_bg: str
    button_fg: str
    button_hover_bg: str
    button_disabled_bg: str
    button_disabled_fg: str

    # Input / spin boxes
    input_bg: str
    input_fg: str
    input_border: str

    # Lists
    list_bg: str
    list_fg: str
    list_selected_bg: str
    list_selected_fg: str
    list_alternate_bg: str

    # Status / labels
    status_fg: str
    placeholder_fg: str

    # Video widget
    video_bg: str

    # Fonts (point sizes)
    font_base: int = 13
    font_small: int = 11
    font_large: int = 15

    # Spacing
    padding: int = 8
    border_radius: int = 5


# ---------------------------------------------------------------------------
# Built-in themes
# ---------------------------------------------------------------------------

THEME_DARK = Theme(
    window_bg="#1a1a2e",
    window_fg="#e0e0e0",
    accent="#e94560",
    button_bg="#16213e",
    button_fg="#e0e0e0",
    button_hover_bg="#0f3460",
    button_disabled_bg="#555555",
    button_disabled_fg="#aaaaaa",
    input_bg="#16213e",
    input_fg="#e0e0e0",
    input_border="#0f3460",
    list_bg="#16213e",
    list_fg="#e0e0e0",
    list_selected_bg="#0f3460",
    list_selected_fg="#e0e0e0",
    list_alternate_bg="#1a1a2e",
    status_fg="#888888",
    placeholder_fg="#888888",
    video_bg="#1a1a2e",
)

THEME_LIGHT = Theme(
    window_bg="#f5f5f5",
    window_fg="#1a1a2e",
    accent="#e94560",
    button_bg="#ffffff",
    button_fg="#1a1a2e",
    button_hover_bg="#e0e0e0",
    button_disabled_bg="#cccccc",
    button_disabled_fg="#666666",
    input_bg="#ffffff",
    input_fg="#1a1a2e",
    input_border="#cccccc",
    list_bg="#ffffff",
    list_fg="#1a1a2e",
    list_selected_bg="#d0d0d0",
    list_selected_fg="#1a1a2e",
    list_alternate_bg="#f5f5f5",
    status_fg="#666666",
    placeholder_fg="#999999",
    video_bg="#1a1a2e",
)

THEMES: dict[str, Theme] = {
    "dark": THEME_DARK,
    "light": THEME_LIGHT,
}


class ThemeManager:
    """Loads / persists the active theme name and builds Qt stylesheets."""

    def __init__(self, theme_name: str = "dark") -> None:
        self._theme_name = theme_name
        self._theme = THEMES.get(theme_name, THEME_DARK)

    @property
    def theme_name(self) -> str:
        return self._theme_name

    @theme_name.setter
    def theme_name(self, value: str) -> None:
        self._theme_name = value
        self._theme = THEMES.get(value, THEME_DARK)

    @property
    def theme(self) -> Theme:
        return self._theme

    def stylesheet(self) -> str:
        """Build a global Qt stylesheet from the current theme."""
        t = self._theme
        return f"""
            QMainWindow, QWidget {{
                background-color: {t.window_bg};
                color: {t.window_fg};
                font-size: {t.font_base}pt;
            }}
            /* Video widget must NOT have a painted background — it would cover
               the embedded mpv child window during playback. */
            QWidget#videoWidget {{
                background-color: transparent;
            }}
            QPushButton {{
                background-color: {t.button_bg};
                color: {t.button_fg};
                padding: {t.padding}px;
                border-radius: {t.border_radius}px;
                border: none;
            }}
            QPushButton:hover {{
                background-color: {t.button_hover_bg};
            }}
            QPushButton:disabled {{
                background-color: {t.button_disabled_bg};
                color: {t.button_disabled_fg};
            }}
            QPushButton#clipButton {{
                background-color: {t.accent};
                color: white;
                font-weight: bold;
            }}
            QPushButton#clipButton:hover {{
                background-color: {t.accent};
                filter: brightness(120%);
            }}
            QPushButton#clipButton:disabled {{
                background-color: {t.button_disabled_bg};
                color: {t.button_disabled_fg};
            }}
            QLineEdit, QDoubleSpinBox, QKeySequenceEdit {{
                background-color: {t.input_bg};
                color: {t.input_fg};
                border: 1px solid {t.input_border};
                padding: {t.padding}px;
                border-radius: {t.border_radius}px;
            }}
            QListView {{
                background-color: {t.list_bg};
                color: {t.list_fg};
                border: none;
                padding: {t.padding}px;
            }}
            QListView::item:selected {{
                background-color: {t.list_selected_bg};
                color: {t.list_selected_fg};
            }}
            QListView::item:alternate {{
                background-color: {t.list_alternate_bg};
            }}
            QLabel {{
                color: {t.window_fg};
            }}
            QLabel#statusLabel {{
                color: {t.status_fg};
                padding: {t.padding}px;
            }}
            QDialog {{
                background-color: {t.window_bg};
                color: {t.window_fg};
            }}
            QFormLayout QLabel {{
                color: {t.window_fg};
            }}
            QSplitter::handle {{
                background-color: {t.input_border};
            }}
        """
