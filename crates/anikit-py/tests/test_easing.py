import pytest

import anikit
from anikit import Easing, ease

NAMES = [
    "linear",
    "ease",
    "ease-in",
    "ease-out",
    "ease-in-out",
    "outCubic",
    "easeInOutQuad",
    "inQuart",
    "power2.inOut",
    "power4.out",
    "sine.in",
    "expo.inOut",
    "circ.out",
    "back.out(1.7)",
    "elastic.out(1, 0.3)",
    "bounce.out",
    "cubic-bezier(.2, .8, .2, 1)",
    "spring",
    "spring(200, 20)",
    "spring(bounce: 0.3, duration: 0.6)",
    "anticipate",
    "slowmo(0.7, 0.7)",
    "pow(3).in",
    "steps(4)",
    "linear(0, 0.25 75%, 1)",
]


@pytest.mark.parametrize("name", NAMES)
def test_endpoints(name):
    e = Easing(name)
    assert e(0.0) == pytest.approx(0.0, abs=1e-9)
    assert e(1.0) == pytest.approx(1.0, abs=1e-9)
    assert ease(name, 1.0) == pytest.approx(1.0, abs=1e-9)


def test_known_values():
    assert ease("outQuad", 0.5) == pytest.approx(0.75)
    assert ease("inQuad", 0.5) == pytest.approx(0.25)
    assert ease("inOutQuad", 0.25) == pytest.approx(0.125)
    assert ease("linear", 0.3) == pytest.approx(0.3)
    assert ease(None, 0.3) == pytest.approx(0.3)
    assert ease("steps(4)", 0.3) == 0.25
    assert ease("steps(4, jump-start)", 0.0) == 0.25
    assert Easing("power2.inOut") == Easing("easeInOutCubic")


def test_callable_easing_is_sampled():
    e = Easing(lambda t: t * t)
    assert e.kind == "table"
    assert e(0.5) == pytest.approx(0.25, abs=1e-5)
    assert ease(lambda t: 1 - t, 0.25) == 0.75
    with pytest.raises(ValueError):
        Easing(lambda t: float("nan"))


def test_sample():
    s = Easing("linear").sample(5)
    assert s == pytest.approx([0, 0.25, 0.5, 0.75, 1])
    assert len(Easing("outBack").sample(100)) == 100
    assert max(Easing("back.out(2)").sample(200)) > 1.05  # overshoot
    with pytest.raises(ValueError):
        Easing("linear").sample(0)


def test_css_export():
    assert Easing("ease").css() == "cubic-bezier(0.25,0.1,0.25,1)"
    assert Easing("linear").css() == "linear"
    assert Easing("steps(3, jump-none)").css() == "steps(3,jump-none)"
    css = Easing("spring(200, 12)").css()
    assert css.startswith("linear(0,") and css.endswith(")")
    # Round-trips through the parser within tolerance.
    back = Easing(css)
    for i in range(101):
        t = i / 100
        assert abs(back(t) - Easing("spring(200, 12)")(t)) < 0.01
    loose = Easing("outBounce").css(tolerance=0.05)
    tight = Easing("outBounce").css(tolerance=0.001)
    assert len(loose) < len(tight)


def test_spring_duration():
    e = Easing("spring(bounce: 0.3, duration: 0.6)")
    assert e.kind == "spring"
    assert 0.3 < e.duration < 5
    assert Easing("outQuad").duration is None
    s = Easing.spring(170, 26)
    assert s.duration == pytest.approx(anikit._anikit.spring_settle_time(170, 26, 1, 0, 0.0005), rel=0.05)
    assert Easing.spring(bounce=0.2, duration=0.4).kind == "spring"


def test_constructors_and_repr():
    assert Easing.bezier(0.42, 0, 1, 1) == Easing("ease-in")
    p = Easing.points([(0, 0), (0.5, 0.8), (1, 1)])
    assert p(0.25) == pytest.approx(0.4)
    assert Easing.steps(2)(0.6) == 0.5
    assert repr(Easing("outCubic")) == "Easing('outCubic')"
    assert Easing(Easing("outCubic")).spec == "outCubic"
    assert hash(Easing("outCubic")) == hash(Easing("easeOutCubic"))


def test_errors():
    with pytest.raises(ValueError):
        Easing("nonsense")
    with pytest.raises(ValueError):
        ease("cubic-bezier(1, 2)", 0.5)
    with pytest.raises(TypeError):
        Easing(42)
    with pytest.raises(TypeError):
        ease(42, 0.5)
