import pytest

import anikit
from anikit import Path, motion_path, stagger


class Sprite:
    def __init__(self, **kw):
        self.__dict__.update(kw)


def test_dict_number_tween(engine):
    o = {"x": 0}
    a = engine.animate(o, {"x": 100}, duration=1, ease="linear")
    engine.tick(0.25)
    assert o["x"] == pytest.approx(25)
    engine.tick(1)
    assert o["x"] == 100
    assert a.finished and a.state == "finished"


def test_object_attributes_and_ease(engine):
    s = Sprite(x=0.0, y=10.0)
    engine.animate(s, {"x": 100, "y": [0, 50]}, duration=1, ease="outQuad")
    engine.tick(0.5)
    assert s.x == pytest.approx(75)
    assert s.y == pytest.approx(37.5)


def test_lists_of_targets_and_stagger(engine):
    items = [{"y": 0} for _ in range(4)]
    a = engine.animate(items, {"y": 10}, duration=1, delay=stagger(4, 0.5), ease="linear")
    assert a.duration == pytest.approx(2.5)
    engine.tick(1.0)
    assert [i["y"] for i in items] == pytest.approx([10, 5, 0, 0])
    engine.tick(2)
    assert all(i["y"] == 10 for i in items)
    mixed = [{"v": 0}, Sprite(v=0)]
    engine.animate(mixed, {"v": 1}, duration=0.1)
    engine.run_until_idle()
    assert mixed[0]["v"] == 1 and mixed[1].v == 1


def test_units_and_relative(engine):
    o = {"w": "10px", "r": "0deg", "n": 5, "m": 8, "q": 6, "pct": 0}
    engine.animate(
        o,
        {"w": "+=90", "r": "1turn", "n": "-=5", "m": "*=2", "q": "/=3", "pct": "50%"},
        duration=1,
        ease="linear",
    )
    engine.tick(0.5)
    assert o["w"] == "55px"
    assert o["pct"] == "25%"
    engine.tick(1)
    assert o == {"w": "100px", "r": "1turn", "n": 0, "m": 16, "q": 2, "pct": "50%"}
    assert isinstance(o["n"], float)


def test_numeric_strings_keep_type(engine):
    o = {"a": "0", "b": 0}
    engine.animate(o, {"a": 10, "b": "10px"}, duration=0.1)
    engine.run_until_idle()
    assert o == {"a": "10", "b": "10px"}


def test_colours(engine):
    o = {"c": "#ff0000", "d": "red", "e": "rgba(0,0,255,0)"}
    engine.animate(o, {"c": "#0000ff", "d": "blue", "e": "blue"}, duration=1, ease="linear", color_space="srgb")
    engine.tick(0.5)
    assert o["d"] == "rgb(128,0,128)"
    assert o["c"] == "#800080"  # hex in → hex out
    assert o["e"] == "rgba(0,0,255,0.5)"
    engine.tick(1)
    assert o["d"] == "rgb(0,0,255)" and o["c"] == "#0000ff"


def test_colour_space_oklab_default(engine):
    o = {"c": "red"}
    engine.animate(o, {"c": "blue"}, duration=1, ease="linear")
    engine.tick(0.5)
    r, g, b, a = anikit.color.parse(o["c"])
    assert o["c"] != "rgb(128,0,128)" and r > 100 and b > 100


def test_template_strings(engine):
    o = {"shadow": "0px 0px 10px rgba(0,0,0,.5)", "tf": "translate(0px, 0px) rotate(0deg)"}
    engine.animate(
        o,
        {"shadow": "10px 4px 20px rgba(0,0,0,1)", "tf": "translate(100px, 50px) rotate(90deg)"},
        duration=1,
        ease="linear",
    )
    engine.tick(0.5)
    assert o["shadow"] == "5px 2px 15px rgba(0,0,0,0.75)"
    assert o["tf"] == "translate(50px, 25px) rotate(45deg)"


def test_template_with_named_colours(engine):
    o = {"b": "1px solid red"}
    engine.animate(o, {"b": "3px solid blue"}, duration=1, ease="linear")
    engine.tick(0.5)
    assert o["b"].startswith("2px solid rgba(")


def test_missing_values_default_to_zero(engine):
    o = {}
    engine.animate(o, {"x": 10, "w": "5px", "c": "red"}, duration=1, ease="linear")
    engine.tick(0.5)
    assert o["x"] == 5 and o["w"] == "2.5px"
    assert o["c"].startswith("rgba(255,0,0,")  # fades in from transparent red


def test_keyframes_offsets_and_per_keyframe_ease(engine):
    o = {"v": 0}
    engine.animate(o, {"v": [0, {"value": 10, "offset": 0.25, "ease": "inQuad"}, 0]}, duration=1, ease="linear")
    engine.tick(0.125)
    assert o["v"] == pytest.approx(2.5)  # inQuad(0.5) * 10
    engine.tick(0.5)  # t=0.625: half-way back down
    assert o["v"] == pytest.approx(5)
    o2 = {"v": 1}
    engine.animate(o2, {"v": [None, 5, 3]}, duration=1, ease="linear")
    engine.tick(0.25)
    assert o2["v"] == pytest.approx(3)
    o3 = {"v": 0}
    engine.animate(o3, {"v": {"from": 10, "to": 20, "ease": "linear"}}, duration=1)
    engine.tick(0.5)
    assert o3["v"] == pytest.approx(15)


def test_callable_values_and_options(engine):
    objs = [{"x": 0} for _ in range(3)]
    engine.animate(
        objs,
        {"x": lambda t, i, n: (i + 1) * 10},
        duration=lambda t, i, n: 0.1 * (i + 1),
        delay=[0, 0.1, 0.2],
    )
    engine.run_until_idle()
    assert [o["x"] for o in objs] == [10, 20, 30]


def test_discrete_values(engine):
    o = {"flag": False, "name": "a"}
    engine.animate(o, {"flag": True, "name": ["a", "b", "c"]}, duration=1, ease="linear")
    engine.tick(0.2)
    assert o["flag"] is False and o["name"] == "a"
    engine.tick(0.4)
    assert o["flag"] is True and o["name"] == "b"
    engine.tick(1)
    assert o["name"] == "c"


def test_motion_path_and_morph(engine):
    s = Sprite(x=0, y=0, angle=0, pos=None, d=None)
    p = Path("M0 0 L100 0 L100 100")
    engine.animate(
        s,
        {
            "motion": motion_path(p, rotate="angle"),
            "pos": p,
            "d": [Path.rect(0, 0, 10, 10), Path.circle(5, 5, 5)],
        },
        duration=1,
        ease="linear",
    )
    engine.tick(0.75)
    assert (s.x, s.y, s.angle) == pytest.approx((100, 50, 90), abs=1e-6)
    assert s.pos == pytest.approx((100, 50, 90), abs=1e-6)
    assert isinstance(s.d, str) and s.d.startswith("M")
    engine.tick(1)
    assert Path(s.d).length == pytest.approx(Path.circle(5, 5, 5).length, rel=1e-3)


def test_partial_motion_path(engine):
    s = {"x": 0, "y": 0}
    engine.animate(s, {"m": motion_path("M0 0 L100 0", start=0.5, end=1)}, duration=1, ease="linear")
    engine.tick(0.5)
    assert s["x"] == pytest.approx(75)


def test_morph_object_and_from_current(engine):
    o = {"d": "M0 0 L10 0 L10 10 Z"}
    engine.animate(o, {"d": [None, Path("M0 0 L20 0 L20 20 Z")]}, duration=1, ease="linear")
    engine.tick(1)
    assert Path(o["d"]).bounds == pytest.approx((0, 0, 20, 20), abs=1e-6)
    o2 = {"d": ""}
    engine.animate(o2, {"d": anikit.morph("M0 0 L1 1", "M0 0 L2 2")}, duration=0.1)
    engine.run_until_idle()
    assert Path(o2["d"]).length == pytest.approx(2 * 2**0.5, rel=1e-3)


def test_custom_python_easing(engine):
    o = {"x": 0}
    engine.animate(o, {"x": 100}, duration=1, ease=lambda t: t * t)
    engine.tick(0.5)
    assert o["x"] == pytest.approx(25, abs=0.05)


def test_values_only_written_when_updated(engine):
    writes = []

    class Spy:
        def __setattr__(self, k, v):
            writes.append((k, v))
            object.__setattr__(self, k, v)

    s = Spy()
    object.__setattr__(s, "x", 0)
    engine.animate(s, {"x": 1}, duration=0.1, delay=0.5)
    engine.tick(0.25)  # still in delay: nothing rendered
    assert writes == []
    engine.run_until_idle()
    n = len(writes)
    engine.tick(1)  # finished: nothing written
    engine.tick(1)
    assert len(writes) == n and writes[-1] == ("x", 1.0)


def test_overwrite_auto(engine):
    o = {"x": 0}
    first = engine.animate(o, {"x": 100}, duration=1, ease="linear")
    engine.tick(0.5)
    engine.animate(o, {"x": 0}, duration=1, ease="linear")
    engine.tick(0.5)
    assert o["x"] == pytest.approx(25)  # from 50 towards 0
    engine.tick(0.6)
    assert o["x"] == 0  # the first tween (ended at 1s) no longer writes
    assert first.finished


def test_immediate_render(engine):
    o = {"x": 0}
    engine.animate(o, {"x": [50, 100]}, duration=1, delay=1, immediate_render=True)
    engine.tick(0)
    assert o["x"] == 50
    o2 = {"x": 0}
    engine.animate(o2, {"x": [50, 100]}, duration=1, delay=1)
    engine.tick(0.5)
    assert o2["x"] == 0


def test_module_level_default_engine():
    anikit.set_default_engine(None)
    o = {"x": 0}
    anikit.animate(o, {"x": 10}, duration=0.5)
    anikit.tick(0.25)
    assert 0 < o["x"] < 10
    assert anikit.run_until_idle() > 0
    assert o["x"] == 10
    anikit.set(o, {"x": "+=5", "c": "red"})
    assert o["x"] == 15 and o["c"] == "rgb(255,0,0)"
    anikit.kill_all()
    assert anikit.default_engine().node_count == 0


def test_set_motion(engine):
    o = {}
    engine.set(o, {"m": motion_path("M0 0 L10 0"), "v": [1, 2, 3]})
    assert o == {"x": 10, "y": 0, "v": 3}


def test_errors(engine):
    with pytest.raises(ValueError):
        engine.animate({}, {})
    with pytest.raises(ValueError):
        engine.animate([], {"x": 1})
    with pytest.raises(TypeError):
        engine.animate(5, {"x": 1})
    with pytest.raises(ValueError):
        engine.animate({"x": 0}, {"x": 1}, duration=-1)
    with pytest.raises(ValueError):
        engine.animate({"x": 0}, {"x": 1}, ease="bogus")
    with pytest.raises(ValueError):
        engine.animate({"x": 0}, {"x": []})
    with pytest.raises(ValueError):
        engine.animate({"x": 0}, {"x": [0, None, 1]})
    with pytest.raises(ValueError):
        engine.animate({"x": 0}, {"x": [0, {"value": 1, "offset": 2}]})
    with pytest.raises(ValueError):
        engine.animate({"x": 0}, {"x": 1}, color_space="cmyk")
    with pytest.raises(TypeError):
        anikit.animate({"x": 0}, {"x": 1}, engine=engine, bogus=1)  # type: ignore[call-arg]
    # Failed creations leak nothing.
    assert engine.node_count == 0
