"""Shared fixtures, and a collection guard: the numpy-backed modules (and the
example using them) need numpy to import.

Kept at the crate root (not in ``tests/``) so ``pytest crates/anikit-py``
from the repository root and plain ``pytest`` here both see one conftest.
"""

import gc

import pytest

import anikit

try:
    import numpy  # noqa: F401
except ImportError:  # pragma: no cover - depends on the environment
    collect_ignore = ["python/anikit/raster.py", "python/anikit/video.py", "examples/knot_mirror.py"]


@pytest.fixture
def engine():
    """A fresh engine per test (the default engine is reset too)."""
    e = anikit.Engine()
    anikit.set_default_engine(None)
    yield e
    anikit.set_default_engine(None)
    gc.collect()


def approx(x, rel=1e-6, abs=1e-6):
    return pytest.approx(x, rel=rel, abs=abs)
