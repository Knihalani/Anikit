import gc

import pytest

import anikit


def starts(tl):
    """Start time (s) of every direct child, in insertion order."""
    core = tl.engine.core
    out = []
    for h in tl._tree.nodes:
        if core.is_alive(h) and core.parent(h) == tl.handle and h != tl._spacer:
            out.append(round(core.start(h) / 1000.0, 6))
    return out


def test_sequential_and_chaining(engine):
    o = {"x": 0}
    tl = engine.timeline(defaults={"ease": "linear", "duration": 1})
    tl.to(o, {"x": 100}).to(o, {"x": 50}).to(o, {"x": "+=25"})
    assert tl.duration == pytest.approx(3)
    engine.tick(0.5)
    assert o["x"] == pytest.approx(50)
    engine.tick(1.0)  # t=1.5: second tween chains from 100
    assert o["x"] == pytest.approx(75)
    engine.tick(1.0)  # t=2.5: relative to 50
    assert o["x"] == pytest.approx(62.5)
    engine.run_until_idle()
    assert o["x"] == 75 and tl.finished


def test_seek_backward_restores_earlier_values(engine):
    o = {"x": 0, "c": "red"}
    tl = engine.timeline(autoplay=False, defaults={"ease": "linear"})
    tl.add(o, {"x": 100, "c": "blue"}, duration=1)
    tl.add(o, {"x": 200}, duration=1)
    tl.seek(1.5)
    assert o["x"] == pytest.approx(150)
    tl.seek(0.5)
    assert o["x"] == pytest.approx(50)
    tl.seek(2)
    assert o["x"] == 200 and o["c"] == "rgb(0,0,255)"
    tl.seek(0)
    assert o["x"] == 0 and o["c"] == "rgb(255,0,0)"
    tl.progress = 0.75
    assert o["x"] == pytest.approx(150)


@pytest.mark.parametrize(
    "position,expected",
    [
        (None, 1.5),  # end of the timeline (GSAP semantics)
        (0.25, 0.25),
        ("0.75", 0.75),
        (">", 1.0),
        ("<", 0.5),
        (">0.5", 1.5),
        (">-0.1", 0.9),
        ("<0.2", 0.7),
        ("<+=0.2", 0.7),
        ("<-=0.2", 0.3),
        ("<50%", 0.75),
        ("+=0.5", 2.0),
        ("-=0.2", 1.3),
        ("intro", 0.3),
        ("intro+=0.3", 0.6),
        ("intro-=0.1", 0.2),
        ("newlabel", 1.5),
    ],
)
def test_positions(engine, position, expected):
    o = {"a": 0, "b": 0}
    tl = engine.timeline(autoplay=False)
    tl.add(o, {"a": 1}, duration=1.5)  # [0, 1.5] -> timeline end 1.5
    tl.label("intro", 0.3)
    tl.add(o, {"b": 1}, 0.5, duration=0.5)  # previous: [0.5, 1.0]
    tl.add(o, {"a": 2}, position, duration=0.1)
    assert starts(tl)[-1] == pytest.approx(expected)


def test_labels_and_negative_clamp(engine):
    tl = engine.timeline(autoplay=False)
    tl.add({"x": 0}, {"x": 1}, duration=1)
    tl.label("end")
    assert tl.labels == {"end": 1.0}
    tl.add({"x": 0}, {"x": 1}, "-=5", duration=1)
    assert starts(tl)[-1] == 0
    with pytest.raises(ValueError):
        tl.add({"x": 0}, {"x": 1}, "??", duration=1)
    with pytest.raises(ValueError):
        tl.label("1bad")
    with pytest.raises(TypeError):
        tl.add({"x": 0}, {"x": 1}, True)


def test_overlap_last_writer_wins(engine):
    o = {"x": 0}
    tl = engine.timeline(defaults={"ease": "linear"})
    tl.add(o, {"x": 100}, duration=1)
    tl.add(o, {"x": [0, 10]}, "<0.5", duration=1)
    engine.tick(0.75)
    assert o["x"] == pytest.approx(2.5)


def test_set_and_call(engine):
    o = {"x": 0, "visible": False}
    calls = []
    tl = engine.timeline(defaults={"ease": "linear"})
    tl.add(o, {"x": 10}, duration=1)
    tl.set(o, {"visible": True})
    tl.call(calls.append, 0.5, "half")
    tl.call(lambda: calls.append("end"), 1.0)
    engine.tick(0.6)
    assert calls == ["half"] and o["visible"] is False
    engine.tick(0.5)
    assert o["visible"] is True and calls == ["half", "end"]
    tl.seek(0.2)  # playing back past the set restores the value and re-fires calls
    assert o["visible"] is False
    assert calls[-2:] == ["end", "half"]


def test_add_callable_shortcut(engine):
    hits = []
    tl = engine.timeline()
    tl.add(lambda: hits.append(1), 0)
    engine.tick(0.01)
    assert hits == [1]


def test_timeline_callbacks(engine):
    log = []
    tl = engine.timeline(
        on_start=lambda: log.append("start"),
        on_update=lambda: log.append("update"),
        on_complete=lambda t: log.append(("complete", t)),
    )
    tl.add({"x": 0}, {"x": 1}, duration=0.5, on_complete=lambda t: log.append(("child", t)))
    engine.run_until_idle()
    assert log[0] == "start" and "update" in log
    assert ("child", tl) in log and log[-1] == ("complete", tl)


def test_timeline_repeat_yoyo_and_reverse(engine):
    o = {"x": 0}
    reps = []
    tl = engine.timeline(repeat=1, yoyo=True, on_repeat=lambda: reps.append(1), defaults={"ease": "linear"})
    tl.add(o, {"x": 100}, duration=1)
    assert tl.duration == pytest.approx(2)
    engine.tick(0.5)
    engine.tick(1.0)
    assert o["x"] == pytest.approx(50) and reps == [1]
    engine.run_until_idle()
    assert o["x"] == 0
    tl2 = engine.timeline(defaults={"ease": "linear"})
    tl2.add(o, {"x": 100}, duration=1)
    engine.tick(0.5)
    tl2.reverse()
    engine.tick(0.25)
    assert o["x"] == pytest.approx(25)


def test_timeline_repeat_delay(engine):
    o = {"x": 0}
    tl = engine.timeline(repeat=1, repeat_delay=0.5, defaults={"ease": "linear"})
    tl.add(o, {"x": 100}, duration=1)
    assert tl.duration == pytest.approx(2.5)
    engine.tick(1.25)
    assert o["x"] == 100
    engine.tick(0.5)
    assert o["x"] == pytest.approx(25)
    engine.run_until_idle()
    assert o["x"] == pytest.approx(100)


def test_nesting(engine):
    o = {"x": 0, "y": 0}
    inner = engine.timeline(autoplay=False, defaults={"ease": "linear"})
    inner.add(o, {"y": 10}, duration=1)
    anim = engine.animate(o, {"x": 10}, duration=1, ease="linear", autoplay=False)
    outer = engine.timeline()
    outer.add(anim).add(inner, "<0.5")
    assert outer.duration == pytest.approx(1.5)
    with pytest.raises(RuntimeError):
        anim.play()
    engine.tick(1.0)
    assert o["x"] == 10 and o["y"] == pytest.approx(5)
    del anim, inner
    gc.collect()
    engine.run_until_idle()
    assert o["y"] == 10
    n = engine.node_count
    outer.kill()
    assert n == 4  # outer group, anim, inner group, inner child
    assert engine.node_count == 0


def test_child_speed_and_stagger(engine):
    items = [{"v": 0} for _ in range(3)]
    tl = engine.timeline(defaults={"ease": "linear"})
    tl.add(items, {"v": 1}, duration=1, delay=anikit.stagger(3, 0.5))
    tl.add({"z": 0}, {"z": 1}, duration=1, speed=2)
    assert tl.duration == pytest.approx(2.5)
    engine.tick(1.0)
    assert [i["v"] for i in items] == pytest.approx([1, 0.5, 0])


def test_kill_releases(engine):
    tl = engine.timeline()
    tl.add({"x": 0}, {"x": 1}).add([{"x": 0}, {"x": 0}], {"x": 1}).call(print)
    assert engine.node_count == 6
    tl.kill()
    assert engine.node_count == 0
    with pytest.raises(RuntimeError):
        tl.add({"x": 0}, {"x": 1})
