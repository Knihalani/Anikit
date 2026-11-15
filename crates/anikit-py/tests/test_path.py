import math

import pytest

from anikit import Morph, Path, morph


def test_measure_line():
    p = Path("M0 0 L100 0")
    assert p.length == pytest.approx(100)
    assert p.bounds == pytest.approx((0, 0, 100, 0))
    x, y, a = p.point_at(0.5)
    assert (x, y, a) == pytest.approx((50, 0, 0), abs=1e-6)
    pts = p.points(5)
    assert [round(pt[0]) for pt in pts] == [0, 25, 50, 75, 100]


def test_relative_arc_and_tangent():
    p = Path("M0 0 l100 0 l0 100")
    assert p.length == pytest.approx(200)
    assert p.point_at(0.75)[:2] == pytest.approx((100, 50), abs=1e-3)
    assert p.point_at(0.75)[2] == pytest.approx(90)
    c = Path.circle(0, 0, 50)
    assert c.length == pytest.approx(2 * math.pi * 50, rel=1e-3)
    assert c.bounds == pytest.approx((-50, -50, 50, 50), abs=1e-6)
    arc = Path("M0 0 A50 50 0 0 1 100 0")
    assert arc.length == pytest.approx(math.pi * 50, rel=1e-3)


def test_shapes():
    r = Path.rect(10, 20, 100, 50)
    assert r.length == pytest.approx(300)
    assert r.bounds == pytest.approx((10, 20, 110, 70))
    rr = Path.rect(0, 0, 100, 100, rx=10)
    assert rr.length < 400
    e = Path.ellipse(0, 0, 100, 50)
    assert e.bounds == pytest.approx((-100, -50, 100, 50), abs=1e-6)
    poly = Path.polygon([(0, 0), (10, 0), (10, 10)])
    assert poly.length == pytest.approx(20 + math.sqrt(200))
    flat = Path.polygon([0, 0, 10, 0, 10, 10], closed=False)
    assert flat.length == pytest.approx(20)
    star = Path.star(0, 0, 5, 50, 20)
    assert star.segment_count == 10
    assert star.bounds[1] == pytest.approx(-50)


def test_svg_roundtrip_and_transforms():
    p = Path("M10 10 Q50 50 90 10 T170 10")
    d = p.to_svg()
    assert d.startswith("M10 10C") and "Q" not in d
    assert Path(d).length == pytest.approx(p.length, rel=1e-4)
    assert str(p) == d
    assert p.normalize().length == pytest.approx(p.length, rel=1e-6)
    t = Path("M0 0 L10 0").translate(5, 5)
    assert t.bounds == pytest.approx((5, 5, 15, 5))
    s = Path("M0 0 L10 0").scale(2)
    assert s.length == pytest.approx(20)
    rot = Path("M0 0 L10 0").rotate(90)
    assert rot.point_at(1)[:2] == pytest.approx((0, 10), abs=1e-9)
    assert Path("M0 0 L10 0").reversed().point_at(0)[:2] == pytest.approx((10, 0))
    assert Path("M0 0 L1 1") == Path("M0 0 L1 1")


def test_path_errors():
    with pytest.raises(ValueError):
        Path("hello")
    with pytest.raises(TypeError):
        Path(42)
    with pytest.raises(ValueError):
        Path("M0 0 L1 1").points(0)


def test_morph():
    m = morph(Path.rect(0, 0, 100, 100), Path.circle(50, 50, 50))
    assert isinstance(m, Morph)
    assert m.channels == len(m.from_values) == len(m.to_values)
    assert Path(m.at(0)).length == pytest.approx(400, rel=1e-3)
    assert Path(m.at(1)).length == pytest.approx(math.pi * 100, rel=1e-3)
    frames = m.frames(5)
    assert len(frames) == 5 and frames[0] == m.at(0) and frames[-1] == m.at(1)
    eased = m.frames(3, ease="inQuad")
    assert eased[1] == m.at(0.25)
    assert m.template[0][1] is True  # closed
    assert m.write(m.to_values) == m.at(1)
    with pytest.raises(ValueError):
        m.write([1, 2, 3])


def test_morph_subpath_mismatch():
    m = morph("M0 0 L10 0 L10 10 Z M20 20 L30 20 L30 30 Z", "M0 0 L10 0 L10 10 L0 10 Z")
    assert len(m.template) == 2
    assert Path(m.at(0.5)).subpath_count == 2
