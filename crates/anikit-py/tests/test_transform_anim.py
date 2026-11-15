import re

import pytest

import anikit
from anikit import _anikit, ease
from anikit.transform import Matrix, mix_transforms, to_css

SPIN = ["rotateY(0deg)", "rotateY(180deg) translateZ(100px)"]


def m(css):
    return Matrix.from_css(css)


def test_transform_keyframes_write_matrix3d(engine):
    o = {}
    engine.animate(o, {"transform": ["rotateY(0deg)", "rotateY(90deg) translateZ(100px)"]}, duration=1, ease="linear")
    engine.tick(0)
    assert o["transform"].startswith("matrix3d(") and m(o["transform"]).isclose(Matrix(), 1e-6)
    engine.tick(0.5)
    expect = mix_transforms("rotateY(0deg)", "rotateY(90deg) translateZ(100px)", 0.5)
    assert m(o["transform"]).isclose(m(expect), 1e-5)
    # Decomposed: translation (rotated with the element) and rotation interpolate separately.
    assert m(o["transform"]).isclose(m("translate3d(50px, 0, 0) rotateY(45deg)"), 1e-5)
    engine.tick(0.6)
    assert m(o["transform"]).isclose(m("rotateY(90deg) translateZ(100px)"), 1e-5)


def test_half_turn_spin_matches_the_task_example(engine):
    o = {"transform": "none"}
    a = engine.animate(o, {"transform": SPIN}, duration=2, ease="inOutSine")
    engine.tick(2)
    assert a.finished
    assert m(o["transform"]).isclose(m(SPIN[1]), 1e-6)
    # The translation ends up behind (rotated with the element), like in browsers.
    assert m(o["transform"]).translation == pytest.approx((0, 0, -100), abs=1e-6)


def test_easing_is_applied(engine):
    o = {}
    a = engine.animate(o, {"transform": ["scale(1)", "scale(3)"]}, duration=1, ease="inQuad", autoplay=False)
    a.seek(0.5)
    engine.tick(0)
    s = ease("inQuad", 0.5)
    assert m(o["transform"])[0] == pytest.approx(1 + 2 * s, abs=1e-6)


def test_from_current_value(engine):
    o = {"transform": "translateX(100px)"}
    engine.animate(o, {"transform": "translateX(200px) rotate(90deg)"}, duration=1, ease="linear")
    engine.tick(0.5)
    got = m(o["transform"])
    assert got.translation[0] == pytest.approx(150, abs=1e-6)
    assert got.decompose()["rotate"][3] == pytest.approx(45, abs=1e-6)


def test_missing_start_is_identity(engine):
    starts = [{}, {"transform": None}, {"transform": ""}, {"transform": "none"}]
    engine.animate(starts, {"transform": "translateY(10px)"}, duration=1, ease="linear")
    engine.tick(0.5)
    for o in starts:
        assert m(o["transform"]).translation == pytest.approx((0, 5, 0))


def test_object_targets_and_matrix_values(engine):
    class Node:
        def __init__(self):
            self.matrix = Matrix()
            self.transform = "rotate(0deg)"

    n = Node()
    engine.animate(n, {"matrix": Matrix.rotate_z(90), "transform": "rotate(90deg)"}, duration=1, ease="linear")
    engine.tick(0.5)
    assert isinstance(n.matrix, Matrix) and n.matrix.isclose(Matrix.rotate_z(45), 1e-9)
    assert isinstance(n.transform, str) and m(n.transform).isclose(Matrix.rotate_z(45), 1e-6)
    # A Matrix keyframe makes even the `transform` key produce Matrix objects; strings mix in.
    o = {}
    engine.animate(o, {"transform": ["rotate(0deg)", Matrix.rotate_z(60)]}, duration=1, ease="linear")
    engine.tick(1)
    assert isinstance(o["transform"], Matrix) and o["transform"].isclose(Matrix.rotate_z(60), 1e-9)


def test_three_keyframes_offsets_and_per_segment_ease(engine):
    o = {}
    a = engine.animate(
        o,
        {
            "transform": [
                "rotate(0deg)",
                {"value": "rotate(90deg) scale(2)", "offset": 0.25, "ease": "linear"},
                {"value": "translateX(100px)", "ease": "linear"},
            ]
        },
        duration=1,
        autoplay=False,
    )
    a.seek(0.25)
    engine.tick(0)
    assert m(o["transform"]).isclose(m("rotate(90deg) scale(2)"), 1e-6)
    a.seek(0.625)
    engine.tick(0)
    assert m(o["transform"]).isclose(m(mix_transforms("rotate(90deg) scale(2)", "translateX(100px)", 0.5)), 1e-6)


def test_repeat_yoyo(engine):
    o = {}
    engine.animate(
        o, {"transform": ["rotateX(0deg)", "rotateX(60deg)"]}, duration=1, ease="linear", repeat=1, yoyo=True
    )
    engine.tick(1.5)
    assert m(o["transform"]).isclose(Matrix.rotate_x(30), 1e-6)
    engine.tick(0.6)
    assert m(o["transform"]).isclose(Matrix(), 1e-6)


def test_timeline_chains_transforms(engine):
    o = {}
    tl = engine.timeline(defaults={"duration": 1, "ease": "linear"})
    tl.to(o, {"transform": "rotateZ(90deg)"}).to(o, {"transform": "rotateZ(90deg) translateX(100px)"})
    engine.tick(1.5)
    # Second tween starts where the first ends (not from the identity).
    assert m(o["transform"]).isclose(m("rotateZ(90deg) translateX(50px)"), 1e-6)
    engine.tick(1)
    assert m(o["transform"]).isclose(m("rotateZ(90deg) translateX(100px)"), 1e-6)


def test_overwrite_takes_over_transform(engine):
    o = {}
    engine.animate(o, {"transform": "translateX(100px)"}, duration=1, ease="linear")
    engine.tick(0.5)
    engine.animate(o, {"transform": "translateX(0px)"}, duration=1, ease="linear")
    engine.tick(0.5)
    assert m(o["transform"]).translation[0] == pytest.approx(25, abs=1e-6)


def test_set_writes_matrix3d(engine):
    o = {}
    engine.set(o, {"transform": "rotate(45deg)"})
    assert o["transform"] == to_css("rotate(45deg)")
    anikit.set(o, {"transform": ["scale(1)", "scale(2)"]})
    assert m(o["transform"]).isclose(Matrix.scale(2))


def test_non_transform_strings_keep_generic_interpolation(engine):
    o = {"transform": "foo(0px)"}
    engine.animate(o, {"transform": "foo(10px)"}, duration=1, ease="linear")
    engine.tick(0.5)
    assert o["transform"] == "foo(5px)"
    # Other keys never become matrix tracks from strings.
    p = {"filter": "blur(0px)"}
    engine.animate(p, {"filter": "blur(4px)"}, duration=1, ease="linear")
    engine.tick(0.5)
    assert p["filter"] == "blur(2px)"


def test_bad_current_transform_raises(engine):
    with pytest.raises(ValueError, match="current value"):
        engine.animate({"transform": "rotate("}, {"transform": "rotate(10deg)"})
    with pytest.raises(ValueError, match="not a CSS transform or Matrix"):
        engine.animate({}, {"m": [Matrix(), "rotate(oops"]})


def test_css_export_of_transform(engine):
    o = {"opacity": 0}
    a = engine.animate(o, {"opacity": 1, "transform": SPIN}, duration=0.8, ease="outCubic", autoplay=False)
    css = a.to_css_keyframes("spin", ".card")
    lines = css.splitlines()
    assert lines[0] == "@keyframes spin {"
    assert re.search(r"0% \{ opacity: 0; transform: matrix3d\(1, 0, 0, 0, .*\); animation-timing-function: ", css)
    last = [ln for ln in lines if ln.strip().startswith("100%")][0]
    got = re.search(r"transform: (matrix3d\([^)]*\))", last).group(1)
    assert m(got).isclose(m(SPIN[1]), 1e-5)
    assert ".card {" in css and "animation: spin 0.8s" in css
    # Shorthands compose before the animated transform.
    o2 = {"x": 0, "transform": "rotate(0deg)"}
    a2 = engine.animate(o2, {"x": 100, "transform": "rotate(90deg)"}, duration=1, autoplay=False)
    frames, timing = a2.to_waapi()
    assert frames[0]["transform"] == "translateX(0px) " + to_css("rotate(0deg)")
    assert frames[-1]["transform"].startswith("translateX(100px) matrix3d(")
    assert timing["duration"] == 1000
    # Dense sampling also works (browsers interpolate matrix3d the same way).
    a3 = engine.animate({}, {"transform": SPIN}, duration=1, ease="outBack", autoplay=False)
    frames3, _ = a3.to_waapi(samples=4)
    assert len(frames3) == 5 and all(f["transform"].startswith("matrix3d(") for f in frames3)


def test_low_level_matrix_track():
    core = _anikit.Engine()
    a, b = Matrix(), Matrix.from_css("rotateY(90deg) translateZ(100px)")
    h = core.create_anim([(_anikit.TRACK_MATRIX, 16, list(a) + list(b), [], [], 0, None)], duration=100.0)
    core.play(h)
    core.tick(50.0)
    assert Matrix(core.values(h)).isclose(a.mix(b, 0.5), 1e-9)
    assert Matrix(core.sample(h, 1.0)).isclose(b, 1e-7)
    with pytest.raises(ValueError):
        core.create_anim([(_anikit.TRACK_MATRIX, 4, [0.0] * 8, [], [], 0, None)])
    # Singular keyframes (scale(0)) still animate.
    h2 = core.create_anim(
        [(_anikit.TRACK_MATRIX, 16, list(Matrix.scale(0)) + list(Matrix()), [], [], 0, None)], duration=100.0
    )
    assert Matrix(core.sample(h2, 1.0)).isclose(Matrix(), 1e-6)
    assert Matrix(core.sample(h2, 0.5))[0] == pytest.approx(0.5, abs=1e-6)
