"""Easings added on top of the classic set: CustomEase paths, modifiers,
rough, expoScale and spring presets."""

import pytest

import anikit
from anikit import Easing, ease

PRESETS = ["gentle", "wobbly", "stiff", "slow", "molasses", "snappy", "bouncy"]

EXTENDED = (
    [
        "custom(M0,0 C0.5,0 0.5,1 1,1)",
        "custom(M0,0 L1,1)",
        "reverse(inQuad)",
        "reverse(back.out(2))",
        "blend(inQuad, outQuad, 0.5)",
        "blend(linear, elastic.out(1, 0.3))",
        "rough",
        "rough(30, 2, 7, outQuad)",
        "expoScale(1, 10)",
        "expoScale(0.5, 7)",
    ]
    + [f"spring.{p}" for p in PRESETS]
    + PRESETS
)


@pytest.mark.parametrize("name", EXTENDED)
def test_extended_endpoints(name):
    e = Easing(name)
    assert e(0) == pytest.approx(0, abs=1e-6)
    assert e(1) == pytest.approx(1, abs=1e-6)
    assert ease(name, 0.5) == e(0.5)
    assert Easing(e.css())(0.5) == pytest.approx(e(0.5), abs=0.02)


def test_custom_ease_from_path():
    s = Easing("custom(M0,0 C0.5,0 0.5,1 1,1)")
    assert s.kind == "table"
    assert s(0.25) < 0.25 and s(0.75) > 0.75
    assert s(0.5) == pytest.approx(0.5, abs=1e-3)
    assert Easing("custom(M0,0 L1,1)")(0.3) == pytest.approx(0.3, abs=1e-3)
    # Overshooting curve.
    o = Easing.from_path("M0,0 C0.2,0 0.3,1.2 0.6,1 0.8,0.9 1,1")
    assert max(o.sample(101)) > 1.0
    # SVG-space (y down, arbitrary range) paths are normalised by their end points.
    assert Easing.from_path("M0,100 L200,0")(0.5) == pytest.approx(0.5, abs=1e-3)
    # Classmethod and string form agree.
    f = Easing.from_path("M0,0 C0.5,0 0.5,1 1,1")
    assert f == s and f.spec == "custom(M0,0 C0.5,0 0.5,1 1,1)"
    assert Easing(f.spec) == f
    # Aliases.
    assert Easing("path(M0,0 L1,1)")(0.4) == pytest.approx(0.4, abs=1e-3)


def test_custom_ease_errors():
    with pytest.raises(ValueError):
        Easing.from_path("M0,0 L0,1")  # no horizontal extent
    with pytest.raises(ValueError):
        Easing.from_path("")
    with pytest.raises(ValueError):
        Easing("custom(nonsense)")
    with pytest.raises(TypeError):
        Easing.from_path(3)  # type: ignore[arg-type]


def test_reverse_mirror_blend_strings():
    r = Easing("reverse(inQuad)")
    assert r.kind == "reverse"
    assert r(0.5) == pytest.approx(0.75)
    for t in (0.1, 0.4, 0.9):
        assert r(t) == pytest.approx(ease("outQuad", t))
    m = Easing("mirror(linear)")
    assert m.kind == "mirror"
    assert m(0.25) == pytest.approx(0.5) and m(0.5) == pytest.approx(1) and m(1) == pytest.approx(0)
    assert Easing("yoyo(linear)") == m
    assert Easing("mirror(back.out(2))")(0.25) > 0.5  # nested arguments
    b = Easing("blend(inQuad, outQuad, 0.25)")
    assert b.kind == "blend"
    assert b(0.3) == pytest.approx(0.75 * 0.09 + 0.25 * ease("outQuad", 0.3))
    assert Easing("blend(inQuad, outQuad)")(0.5) == pytest.approx(0.5)  # weight defaults to 0.5
    with pytest.raises(ValueError):
        Easing("blend(inQuad)")
    with pytest.raises(ValueError):
        Easing("reverse(nonsense)")


def test_modifier_methods():
    e = Easing("inCubic")
    assert e.reversed()(0.3) == pytest.approx(ease("outCubic", 0.3))
    assert e.reversed() == Easing("reverse(inCubic)")
    assert e.reversed().reversed()(0.3) == pytest.approx(e(0.3))
    assert e.mirrored()(0.75) == pytest.approx(e(0.5))
    py = Easing(lambda t: t * t)  # works on any easing, including Python callables
    assert py.reversed()(0.5) == pytest.approx(0.75, abs=1e-3)
    b = Easing.blend("linear", lambda t: t**2, weight=1.0)
    assert b(0.5) == pytest.approx(0.25, abs=1e-3)
    assert Easing.blend("inQuad", "outQuad", 0.25) == Easing("blend(inQuad, outQuad, 0.25)")


def test_rough():
    a = Easing.rough(points=30, strength=2, seed=7, template="outQuad")
    assert a == Easing("rough(30, 2, 7, outQuad)")
    assert a.kind == "points"
    assert a(0) == 0 and a(1) == 1
    smooth = Easing("outQuad")
    devs = [abs(a(i / 200) - smooth(i / 200)) for i in range(201)]
    assert 0.01 < max(devs) <= 0.2 + 1e-9  # jitter is at most 0.1 * strength
    # Deterministic per seed; different seeds differ.
    assert Easing.rough(seed=3).sample(50) == Easing.rough(seed=3).sample(50)
    assert Easing.rough(seed=3).sample(50) != Easing.rough(seed=4).sample(50)
    # Zero strength reproduces the template at the sample points.
    flat = Easing.rough(points=10, strength=0)
    assert flat(0.5) == pytest.approx(0.5)
    with pytest.raises(ValueError):
        Easing.rough(points=0)
    with pytest.raises(ValueError):
        Easing.rough(seed=-1)
    with pytest.raises(ValueError):
        Easing.rough(strength=float("nan"))


def test_expo_scale():
    e = Easing("expoScale(1, 10)")
    assert e(0.5) < 0.5
    # Each step multiplies the scale by the same ratio: scale(0.5) is the geometric mean.
    scale = 1 + (10 - 1) * e(0.5)
    assert scale == pytest.approx(10**0.5, rel=1e-3)
    assert Easing.expo_scale(1, 10) == e
    assert Easing("expoScale(2, 2)").kind == "linear"
    with pytest.raises(ValueError):
        Easing.expo_scale(0, 10)


@pytest.mark.parametrize("name", PRESETS)
def test_spring_presets(name):
    e = Easing(f"spring.{name}")
    assert e.kind == "spring"
    assert e.duration is not None and e.duration > 0
    assert Easing(name) == e
    assert e.css().startswith("linear(")


def test_spring_presets_differ():
    wobbly, molasses = Easing("spring.wobbly"), Easing("spring.molasses")
    assert max(wobbly.sample(200)) > 1.05  # under-damped: overshoots
    assert max(molasses.sample(200)) <= 1.0 + 1e-9  # over-damped: never overshoots
    assert len({Easing(f"spring.{p}").sample(20)[5] for p in PRESETS}) == len(PRESETS)


def test_extended_easings_in_animations(engine):
    o = {"a": 0.0, "b": 0.0}
    engine.animate(o, {"a": 100}, duration=1, ease="mirror(linear)")
    engine.animate(o, {"b": 100}, duration=1, ease=Easing.from_path("M0,0 C0.5,0 0.5,1 1,1"))
    engine.tick(0.5)
    assert o["a"] == pytest.approx(100) and o["b"] == pytest.approx(50, abs=0.2)
    engine.tick(0.5)
    assert o["a"] == pytest.approx(0) and o["b"] == pytest.approx(100)


def test_extended_easings_export():
    a = anikit.Engine().animate({"x": 0}, {"x": 100}, duration=1, ease="reverse(inQuad)", autoplay=False)
    assert "linear(" in a.to_css_keyframes("k") or "cubic-bezier(" in a.to_css_keyframes("k")
