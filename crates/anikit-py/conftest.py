"""Doctest collection guard: the numpy-backed modules need numpy to import."""

try:
    import numpy  # noqa: F401
except ImportError:  # pragma: no cover - depends on the environment
    collect_ignore = ["python/anikit/raster.py", "python/anikit/video.py"]
