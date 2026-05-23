"""Tests for GUI components."""

from jorja_clipper.gui.clip_list import ClipListModel


def test_clip_list_model_empty():
    """Model starts empty."""
    model = ClipListModel()
    assert model.rowCount() == 0


def test_clip_list_model_add_clip():
    """Adding a clip increases row count."""
    model = ClipListModel()
    model.add_clip("/tmp/test.mp4", 25.0, 35.0)
    assert model.rowCount() == 1


def test_clip_list_model_clip_data():
    """Model returns correct data for a clip."""
    model = ClipListModel()
    model.add_clip("/tmp/test.mp4", 25.0, 35.0)
    index = model.index(0, 0)
    assert "test" in model.data(index)
