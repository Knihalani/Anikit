import math
import re

import pytest

import anikit
from anikit import Easing


def parse_keyframes(css):
    m = re.match(r"@keyframes ([\w-]+) \{\n(.*?)\n\}\n", css, re.S)
    assert m, css
    frames = []
    for line in m.group(2).splitlines():
        fm = re.match(r"^  ([\d.]+)% \{ (.*) \}$", line)
        assert fm, line
        decls = dict(d.split(": ", 1) for d in fm.group(2).rstrip(";").split("; "))
        frames.append((float(fm.group(1)), decls))
    return m.group(1), frames


def test_native_keyframes_with_easing(engine):
    o = {"opacity": 0, "x": 0, "background_color": "red"}
    a = engine.animate(o, {"opacity": 1, "x": 100, "background_color": "blue"}, duration=0.5, ease="ease")
    css = a.to_css_keyframes("fade-in", ".box")
    name, frames = parse_keyframes(css)
    assert name == "fade-in"
    assert [f[0] for f in frames] == [0, 100]
    first = frames[0][1]
    assert first["opacity"] == "0"
    assert first["transform"] == "translateX(0px)"
    assert first["background-color"] == "rgb(255,0,0)"
    assert first["animation-timing-function"] == "cubic-bezier(0.25,0.1,0.25,1)"
    assert "animation-timing-function" not in frames[1][1]
    assert ".box {\n  animation: fade-in 0.5s linear 0s 1 normal both;\n}" in css
    assert css.count("{") == css.count("}")


def test_keyframe_offsets_and_spring_easing(engine):
    o = {"scale": 1}
    a = engine.animate(o, {"scale": [1, 1.2, 1]}, duration=1, ease="spring(200, 15)", repeat=-1, yoyo=True, delay=0.2)
    name, frames = parse_keyframes(a.to_css_keyframes("pulse", ".dot"))
    assert [f[0] for f in frames] == [0, 50, 100]
    assert frames[1][1]["transform"] == "scale(1.2)"
    tf = frames[0][1]["animation-timing-function"]
    assert tf.startswith("linear(")
    # The exported easing reproduces the spring.
    sp = Easing("spring(200, 15)")
    back = Easing(tf)
    assert max(abs(back(i / 100) - sp(i / 100)) for i in range(101)) < 0.01
    css = a.to_css_keyframes("pulse", ".dot")
    assert "infinite alternate both" in css and " 0.2s " in css


def test_dense_sampling_when_tracks_differ(engine):
    o = {"opacity": 0, "x": 0}
    a = engine.animate(o, {"opacity": [0, 1, 0], "x": 100}, duration=1, ease="outQuad")
    name, frames = parse_keyframes(a.to_css_keyframes("mix"))
    assert len(frames) > 10
    offsets = [f[0] for f in frames]
    assert offsets == sorted(offsets) and offsets[0] == 0 and offsets[-1] == 100
    assert 50 in offsets
    assert all("animation-timing-function" not in d for _, d in frames)
    mid = dict(frames)[50]
    assert mid["opacity"] == "1"
    explicit = a.to_css_keyframes("mix", samples=4)
    assert len(parse_keyframes(explicit)[1]) == 5


def test_waapi(engine):
    o = {"opacity": 0, "rotate": 0, "border_radius": "0px"}
    a = engine.animate(
        o, {"opacity": 1, "rotate": 90, "border_radius": "8px"}, duration=0.4, delay=0.1, ease="outBack", repeat=2
    )
    frames, timing = a.to_waapi()
    assert frames[0] == {
        "offset": 0.0,
        "opacity": 0.0,
        "borderRadius": "0px",
        "transform": "rotate(0deg)",
        "easing": Easing("outBack").css(),
    }
    assert frames[-1]["transform"] == "rotate(90deg)" and "easing" not in frames[-1]
    assert timing == {
        "duration": pytest.approx(400),
        "delay": pytest.approx(100),
        "endDelay": 0,
        "iterations": 3,
        "direction": "normal",
        "fill": "both",
        "easing": "linear",
    }
    inf = engine.animate({"x": 0}, {"x": 1}, repeat=-1).to_waapi()[1]
    assert math.isinf(inf["iterations"])


def test_export_motion_and_errors(engine):
    o = {"x": 0, "y": 0}
    a = engine.animate(o, {"m": anikit.motion_path("M0 0 L100 0", rotate="rotation")}, duration=1, ease="linear")
    _, frames = parse_keyframes(a.to_css_keyframes("move"))
    assert frames[-1][1]["transform"] == "translateX(100px) translateY(0px) rotate(0deg)"
    with pytest.raises(ValueError):
        a.to_css_keyframes("bad name")
    tl = engine.timeline()
    tl.add(o, {"x": 1})
    with pytest.raises(TypeError):
        anikit.export.css_keyframes(tl, "x")


def test_export_repeat_delay_and_speed(engine):
    o = {"opacity": 0}
    a = engine.animate(o, {"opacity": 1}, duration=1, repeat=1, repeat_delay=1, ease="linear", speed=2)
    css = a.to_css_keyframes("blink", ".b")
    _, frames = parse_keyframes(css)
    assert [f[0] for f in frames] == [0, 50, 100]  # motion, then the held gap
    assert "animation: blink 1s linear 0s 2 normal both;" in css


def test_css_property_names():
    from anikit.export import css_property_name

    assert css_property_name("background_color") == "background-color"
    assert css_property_name("backgroundColor") == "background-color"
    assert css_property_name("opacity") == "opacity"
