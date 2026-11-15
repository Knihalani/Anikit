import pytest

from anikit import color


def test_parse_forms():
    assert color.parse("#f00") == (255, 0, 0, 1)
    assert color.parse("#ff000080")[3] == pytest.approx(128 / 255)
    assert color.parse("red") == (255, 0, 0, 1)
    assert color.parse("rgb(0 128 255 / 50%)") == pytest.approx((0, 128, 255, 0.5))
    assert color.parse("hsl(120, 100%, 50%)") == pytest.approx((0, 255, 0, 1))
    r, g, b, a = color.parse("oklch(62.8% 0.2577 29.23)")
    assert r == pytest.approx(255, abs=1.5) and g < 2 and b < 2
    assert color.parse((10, 20, 30)) == (10, 20, 30, 1)
    assert color.try_parse("nope") is None
    assert color.is_color("blue") and not color.is_color("10px") and not color.is_color(3)


def test_parse_errors():
    with pytest.raises(ValueError):
        color.parse("not-a-colour")
    with pytest.raises(ValueError):
        color.parse((1, 2))
    with pytest.raises(TypeError):
        color.parse(42)


def test_mix_spaces():
    assert color.mix("red", "blue", 0) == "rgb(255,0,0)"
    assert color.mix("red", "blue", 1) == "rgb(0,0,255)"
    assert color.mix("black", "white", 0.5, space="srgb") == "rgb(128,128,128)"
    # OKLab midpoint of black/white is perceptually lighter than sRGB's.
    ok = color.parse(color.mix("black", "white", 0.5))
    assert ok[0] < 128 or ok[0] > 90
    for space in color.SPACES:
        assert color.mix("#123456", "#abcdef", 0.5, space=space).startswith("rgb")
    with pytest.raises(ValueError):
        color.mix("red", "blue", 0.5, space="cmyk")


def test_mix_premultiplied_alpha():
    # Fading red to transparent never goes through black.
    r, g, b, a = color.mix_rgba("red", "transparent", 0.5)
    assert r == pytest.approx(255, abs=1)
    assert a == pytest.approx(0.5)


def test_formatting():
    assert color.to_hex("rgb(255, 128, 0)") == "#ff8000"
    assert color.to_hex((255, 0, 0, 0.5)) == "#ff000080"
    assert color.to_css("#00ff00") == "rgb(0,255,0)"
    assert color.to_css((0, 0, 0, 0.25)) == "rgba(0,0,0,0.25)"
