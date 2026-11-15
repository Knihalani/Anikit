import gc

import pytest

import anikit


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
