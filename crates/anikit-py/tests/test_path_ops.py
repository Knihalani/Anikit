"""Path operations (flatten, area, trim, simplify, splines, distance queries)
and multi-shape morphing."""

import math

import pytest

import anikit
from anikit import Morph, MorphSequence, Path, morph, morph_many


def test_flatten():
    c = Path.circle(0, 0, 50)
    fine = c.flatten(0.01)
    coarse = c.flatten(2)
    assert len(fine) == 1 and len(fine[0]) > len(coarse[0]) > 4
    for x, y in fine[0]:
        assert math.hypot(x, y) == pytest.approx(50, abs=0.05)
    assert fine[0][0] == pytest.approx((50, 0), abs=1e-9)
    # One polyline per subpath; straight segments stay single segments.
    two = Path("M0 0 L10 0 M20 0 L30 0 L30 10").flatten()
    assert two == [[(0, 0), (10, 0)], [(20, 0), (30, 0), (30, 10)]]
    assert Path("").flatten() == []
    with pytest.raises(ValueError):
        c.flatten(0)
    with pytest.raises(ValueError):
        c.flatten(-1)


def test_area():
    assert Path("M0 0 H10 V10 H0 Z").area == pytest.approx(100)  # clockwise on screen
    assert Path("M0 0 V10 H10 V0 Z").area == pytest.approx(-100)  # counter-clockwise
    assert abs(Path.circle(0, 0, 10).area) == pytest.approx(math.pi * 100, rel=1e-3)
    assert Path.rect(0, 0, 30, 20).area == pytest.approx(600)
    assert Path.rect(0, 0, 30, 20).reversed().area == pytest.approx(-600)
    # Open paths are treated as closed.
    assert Path("M0 0 L10 0 L10 10").area == pytest.approx(50)
    assert Path("M0 0 L100 0").area == 0
    # Subpaths add up (a hole drawn the other way subtracts).
    donut = Path(Path.rect(0, 0, 10, 10).d + " " + Path.rect(2, 2, 6, 6).reversed().d)
    assert donut.area == pytest.approx(100 - 36)


def test_trim():
    line = Path("M0 0 L100 0")
    t = line.trim(0.25, 0.75)
    assert t.length == pytest.approx(50)
    assert t.point_at(0)[:2] == pytest.approx((25, 0))
    assert t.point_at(1)[:2] == pytest.approx((75, 0))
    assert line.trim(0.75, 0.25).length == pytest.approx(50)  # swapped
    assert line.trim(-1, 2).length == pytest.approx(100)  # clamped
    assert line.trim().length == pytest.approx(100)
    assert line.trim(0.5, 0.5).length == pytest.approx(0)
    c = Path.circle(0, 0, 10)
    half = c.trim(0, 0.5)
    assert half.length == pytest.approx(c.length / 2, rel=1e-4)
    assert half.point_at(1)[:2] == pytest.approx((-10, 0), abs=1e-3)
    # Across subpaths (jumps between subpaths do not count as length).
    m = Path("M0 0 L10 0 M20 0 L30 0").trim(0.25, 0.75)
    assert m.subpath_count == 2 and m.length == pytest.approx(10)
    assert m.bounds == pytest.approx((5, 0, 25, 0))


def test_draw_on_effect(engine):
    p = Path("M0 0 L100 0 L100 100")
    o = {"end": 0.0, "d": ""}
    engine.animate(o, {"end": 1}, duration=1, ease="linear", on_update=lambda: o.update(d=p.trim(0, o["end"]).d))
    engine.tick(0.25)
    assert Path(o["d"]).length == pytest.approx(50)
    engine.tick(0.75)
    assert Path(o["d"]).length == pytest.approx(200)


def test_through_points():
    pts = [(0, 0), (50, 50), (100, 0)]
    p = Path.through(pts)
    assert p.segment_count == 2 and p.subpath_count == 1
    # Passes through every point (closest_point is accurate to the lookup table's resolution).
    for x, y in pts:
        assert p.closest_point(x, y)[3] == pytest.approx(0, abs=0.02)
    assert p.closest_point(50, 55)[3] == pytest.approx(5, abs=0.02)  # above the smooth peak
    x, y, a = p.point_at(0.5)
    assert (x, y, a) == pytest.approx((50, 50, 0), abs=0.1)  # smooth peak
    straight = Path.through(pts, curviness=0)
    assert straight.length == pytest.approx(2 * math.hypot(50, 50))
    assert p.length > straight.length
    closed = Path.through([(0, 0), (100, 0), (100, 100), (0, 100)], closed=True)
    assert closed.segment_count == 4
    assert closed.to_svg().endswith("Z")
    assert abs(closed.area) > 100 * 100  # bulges outwards
    assert Path.through([0, 0, 10, 0]).length == pytest.approx(10)  # flat input
    with pytest.raises(ValueError):
        Path.through([(0, 0)])
    with pytest.raises(ValueError):
        Path.through([0, 0, 1])


def test_simplify():
    c = Path.circle(0, 0, 100)
    s = c.simplify(1.0)
    assert 8 <= s.segment_count < len(c.flatten(0.25)[0])
    assert s.length == pytest.approx(c.length, rel=0.01)
    assert s.to_svg().endswith("Z")  # stays closed
    coarse = c.simplify(10)
    assert coarse.segment_count < s.segment_count
    smooth = c.simplify(10, smooth=True)
    assert smooth.segment_count == coarse.segment_count
    # The spline hugs the circle much better than the polygon.
    assert abs(smooth.length - c.length) < abs(coarse.length - c.length)
    # Collinear points collapse.
    zig = Path.polygon([(0, 0), (10, 0.01), (20, 0), (30, 0.01), (40, 0)], closed=False)
    assert zig.simplify(0.1).segment_count == 1
    with pytest.raises(ValueError):
        c.simplify(0)


def test_distance_queries():
    line = Path("M0 0 L100 0")
    assert line.point_at_length(25) == pytest.approx((25, 0, 0))
    assert line.point_at_length(-10) == pytest.approx((0, 0, 0))
    assert line.point_at_length(1000) == pytest.approx((100, 0, 0))
    p, x, y, d = line.closest_point(30, 5)
    assert (p, x, y, d) == pytest.approx((0.3, 30, 0, 5))
    assert line.closest_point(-20, 0) == pytest.approx((0, 0, 0, 20))
    assert line.closest_point(150, 0) == pytest.approx((1, 100, 0, 50))
    c = Path.circle(0, 0, 10)
    prog, cx, cy, dist = c.closest_point(0, 30)
    assert (cx, cy, dist) == pytest.approx((0, 10, 20), abs=0.05)
    assert c.point_at(prog)[:2] == pytest.approx((cx, cy), abs=1e-6)
    l_shape = Path("M0 0 L100 0 L100 100")
    assert l_shape.point_at_length(150)[:2] == pytest.approx((100, 50))
    pts = l_shape.sample_points(5)
    expected = [(0, 0), (50, 0), (100, 0), (100, 50), (100, 100)]
    # (the lookup table interpolates across the sharp corner, cutting it by < 1 sample spacing)
    assert [c for pt in pts for c in pt] == pytest.approx([c for pt in expected for c in pt], abs=0.15)
    assert line.sample_points(1) == [(0, 0)]
    with pytest.raises(ValueError):
        line.sample_points(0)


def test_morph_many_shapes():
    shapes = [Path.rect(0, 0, 100, 100), Path.circle(50, 50, 50), Path.star(50, 50, 5, 50, 20)]
    seq = morph(*shapes)
    assert isinstance(seq, MorphSequence) and len(seq) == 3
    assert isinstance(morph(shapes[0], shapes[1]), Morph)  # two shapes keep the classic Morph
    assert morph_many(shapes).keyframes == seq.keyframes
    assert len(seq.keyframes) == 3 and all(len(k) == seq.channels for k in seq.keyframes)
    assert seq.template == [(seq.template[0][0], True)]
    # Passes through every shape.
    for t, shape in zip((0, 0.5, 1), shapes):
        assert Path(seq.at(t)).length == pytest.approx(shape.length, rel=1e-3)
        assert Path(seq.at(t)).bounds == pytest.approx(shape.bounds, abs=1e-2)
    assert seq.at(-1) == seq.at(0) and seq.at(2) == seq.at(1)  # clamped
    frames = seq.frames(5)
    assert len(frames) == 5 and frames[0] == seq.at(0) and frames[2] == seq.at(0.5)
    assert seq.frames(3, ease="inQuad")[1] == seq.at(0.25)
    assert seq.frames(1) == [seq.at(1)]
    assert seq.write(seq.keyframes[1]) == seq.at(0.5)
    assert seq.at(0.5, decimals=0).count(".") == 0
    assert "shapes=3" in repr(seq)
    with pytest.raises(ValueError):
        seq.write([1.0])
    with pytest.raises(ValueError):
        seq.frames(0)
    with pytest.raises(ValueError):
        morph_many([shapes[0]])


def test_morph_many_subpaths_and_strings():
    seq = morph_many(["M0 0 L10 0 L10 10 Z", "M0 0 L10 0 Z M50 50 L60 60 L50 60 Z", Path.circle(5, 5, 5)])
    assert len(seq.template) == 2
    assert Path(seq.at(0.5)).subpath_count == 2
    assert Path(seq.at(0.5)).length == pytest.approx(Path("M0 0 L10 0 Z M50 50 L60 60 L50 60 Z").length, rel=1e-3)


def test_animate_through_several_shapes(engine):
    shapes = [Path.rect(0, 0, 100, 100), Path.circle(50, 50, 50), Path.star(50, 50, 5, 50, 20)]
    o = {"d": ""}
    a = engine.animate(o, {"d": shapes}, duration=1, ease="linear")
    engine.tick(0.5)
    assert Path(o["d"]).length == pytest.approx(shapes[1].length, rel=1e-3)
    engine.tick(0.25)
    mid = morph_many(shapes).at(0.75)
    assert o["d"] == mid
    engine.tick(0.25)
    assert Path(o["d"]).length == pytest.approx(shapes[2].length, rel=1e-3)
    assert a.finished
    # A MorphSequence object works as a value, and so do keyframes starting at the current shape.
    o2 = {"d": Path.rect(0, 0, 10, 10).d}
    engine.animate(o2, {"d": [None, Path.circle(5, 5, 5), "M0 0 L10 0 L5 10 Z"]}, duration=1, ease="linear")
    engine.tick(0.5)
    assert Path(o2["d"]).length == pytest.approx(Path.circle(5, 5, 5).length, rel=1e-3)
    o3 = {"d": ""}
    engine.animate(o3, {"d": morph(*shapes)}, duration=1, ease="linear")
    engine.tick(0.5)
    assert Path(o3["d"]).length == pytest.approx(shapes[1].length, rel=1e-3)


def test_animate_shape_keyframes_with_offsets_and_eases(engine):
    shapes = [Path.rect(0, 0, 100, 100), Path.circle(50, 50, 50), Path.star(50, 50, 5, 50, 20)]
    o = {"d": ""}
    engine.animate(
        o,
        {"d": [shapes[0], {"value": shapes[1], "offset": 0.2}, {"value": shapes[2], "ease": "inQuad"}]},
        duration=1,
        ease="linear",
    )
    engine.tick(0.2)
    assert Path(o["d"]).length == pytest.approx(shapes[1].length, rel=1e-3)
    engine.tick(0.4)  # half-way through the second (inQuad) segment → a quarter of the way
    seq = morph_many(shapes)
    assert o["d"] == seq.at(0.5 + 0.5 * 0.25)


def test_shape_morph_errors(engine):
    with pytest.raises(ValueError):
        engine.animate({}, {"d": [None, Path.circle(0, 0, 1), Path.circle(0, 0, 2)]})
    with pytest.raises(TypeError):
        engine.animate({"d": ""}, {"d": [Path.circle(0, 0, 1), 5, Path.circle(0, 0, 2)]})


def test_multi_morph_in_timeline_and_export(engine):
    shapes = [Path.rect(0, 0, 10, 10), Path.circle(5, 5, 5), Path.star(5, 5, 5, 5, 2)]
    o = {"d": ""}
    tl = engine.timeline()
    tl.to(o, {"d": shapes}, duration=1, ease="linear")
    tl.to(o, {"d": [None, Path.rect(0, 0, 10, 10)]}, duration=1, ease="linear")
    engine.tick(1.0)
    assert Path(o["d"]).length == pytest.approx(shapes[2].length, rel=1e-3)
    engine.tick(1.0)
    assert Path(o["d"]).length == pytest.approx(40, rel=1e-3)
    a = anikit.Engine().animate({"d": ""}, {"d": shapes}, duration=1, autoplay=False)
    css = a.to_css_keyframes("shape")
    assert "50%" in css and css.count("d:") >= 3
