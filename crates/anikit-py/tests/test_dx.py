"""Error messages and warnings that tell you how to fix the mistake."""

import pytest

import anikit


def test_unknown_easing_suggests_the_nearest_name():
    with pytest.raises(ValueError, match='Did you mean "outCubic"'):
        anikit.animate({"x": 0}, {"x": 1}, ease="outCubik")
    with pytest.raises(ValueError, match='Did you mean "power2.out"'):
        anikit.Easing("power2.outt")


def test_unknown_timeline_option_suggests_the_nearest_option():
    tl = anikit.timeline()
    with pytest.raises(TypeError, match="Did you mean 'duration'"):
        tl.add({"x": 0}, {"x": 1}, durration=1)


def test_milliseconds_look_alike_warns():
    with pytest.warns(UserWarning, match=r"did you mean duration=0\.6"):
        anikit.animate({"x": 0}, {"x": 1}, duration=600)
    anikit.kill_all()


def test_stagger_takes_seconds_first_like_typescript():
    s = anikit.stagger(0.1, from_="last")
    assert isinstance(s, anikit.Stagger)
    assert s.values(3) == pytest.approx([0.2, 0.1, 0.0])
    assert anikit.stagger(3, 0.1) == pytest.approx([0.0, 0.1, 0.2])
