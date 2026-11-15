import pytest

from anikit import Stagger, stagger


def test_linear():
    assert stagger(4, 0.1) == pytest.approx([0, 0.1, 0.2, 0.3])
    assert stagger(4, 1, start=2) == [2, 3, 4, 5]
    assert stagger(3, total=1.0) == [0, 0.5, 1]
    assert stagger(0, 1) == []
    assert stagger(["a", "b", "c"], 1) == [0, 1, 2]


def test_origins():
    assert stagger(5, 1, from_="center") == [2, 1, 0, 1, 2]
    assert stagger(5, 1, from_="edges") == [0, 1, 2, 1, 0]
    assert stagger(3, 1, from_="last") == [2, 1, 0]
    assert stagger(5, 1, from_=1) == [1, 0, 1, 2, 3]
    r = stagger(10, 1, from_="random", seed=7)
    assert sorted(r) == list(range(10)) and r != list(range(10))
    assert r == stagger(10, 1, from_="random", seed=7)


def test_grid_axis_ease():
    g = stagger(9, 1, grid=(3, 3), from_="center")
    assert g[4] == 0 and g[1] == 1 and g[0] == pytest.approx(2**0.5)
    gx = stagger(9, 1, grid=(3, 3), from_="center", axis="x")
    assert gx[:3] == [1, 0, 1]
    eased = stagger(5, total=1, ease="inQuad")
    assert eased == pytest.approx([0, 0.0625, 0.25, 0.5625, 1])


def test_lazy_stagger():
    s = stagger(each=0.5)
    assert isinstance(s, Stagger)
    assert [s(None, i, 3) for i in range(3)] == [0, 0.5, 1.0]


def test_errors():
    with pytest.raises(ValueError):
        stagger(3, 1, from_="middle")
    with pytest.raises(ValueError):
        stagger(3, 1, axis="z")
    with pytest.raises(TypeError):
        stagger(3, 1, from_=True)
