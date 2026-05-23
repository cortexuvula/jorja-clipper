"""Pytest fixtures and configuration."""

import sys
from unittest.mock import MagicMock

# Mock python-mpv if libmpv is not available on the system.
try:
    import mpv  # noqa: F401
except OSError:
    # libmpv not installed on system — inject sufficient mock so imports work.
    _mock_spec = MagicMock()
    _mock_spec.MPV = MagicMock(return_value=MagicMock())
    _mock_spec.Property = MagicMock()
    _mock_spec.Event = MagicMock()
    sys.modules["mpv"] = _mock_spec
