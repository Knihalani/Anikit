import gc

import pytest

import anikit
from anikit import _anikit


def test_render_frames(engine):
    o = {"x": 0}
    engine.animate(o, {"x": 100}, duration=1, ease="linear")
    seen = [(i, t, o["x"]) for i, t in engine.render(fps=4, duration=1)]
    assert [s[0] for s in seen] == [0, 1, 2, 3, 4]
    assert [s[1] for s in seen] == [0, 0.25, 0.5, 0.75, 1.0]
    assert [s[2] for s in seen] == pytest.approx([0, 25, 50, 75, 100])


def test_render_until_idle(engine):
    o = {"x": 0}
    engine.animate(o, {"x": 1}, duration=0.5)
    frames = list(engine.render(fps=10))
    assert frames[-1][1] == pytest.approx(0.5) and o["x"] == 1
    assert list(engine.render(fps=10)) == [(0, 0.0)]  # idle: one frame
    engine.animate(o, {"x": 2}, repeat=-1)
    assert len(list(engine.render(fps=10, max_duration=1))) == 11
    with pytest.raises(ValueError):
        next(engine.render(fps=0))


def test_run_until_idle_and_realtime(engine):
    engine.animate({"x": 0}, {"x": 1}, duration=0.05)
    assert engine.run_until_idle(fps=100, realtime=True) == pytest.approx(0.05, abs=0.011)
    assert engine.is_idle and engine.active_count == 0


def test_tick_validation_and_time(engine):
    with pytest.raises(ValueError):
        engine.tick(-1)
    with pytest.raises(ValueError):
        engine.tick(float("nan"))
    engine.tick(0.5)
    assert engine.time == pytest.approx(0.5)
    assert "Engine" in repr(engine)


def test_lag_smoothing_opt_in():
    e = anikit.Engine(lag_smoothing=(0.5, 0.033))
    o = {"x": 0}
    e.animate(o, {"x": 1000}, duration=1, ease="linear")
    e.tick(5)
    assert o["x"] == pytest.approx(33)
    e2 = anikit.Engine()
    o2 = {"x": 0}
    e2.animate(o2, {"x": 1000}, duration=1, ease="linear")
    e2.tick(5)
    assert o2["x"] == 1000


def test_clear(engine):
    a = engine.animate({"x": 0}, {"x": 1})
    s = engine.spring({"x": 0}, {"x": 1})
    engine.timeline().add({"x": 0}, {"x": 1})
    engine.clear()
    assert engine.node_count == 0 and engine.is_idle
    assert s.finished
    b = engine.animate({"x": 0}, {"x": 1})  # may reuse a released slot
    a.kill()  # stale controllers never touch reused slots
    del a
    gc.collect()
    assert engine.node_count == 1 and b.is_active


def test_many_animations_no_leak(engine):
    objs = [{"x": 0, "c": "red"} for _ in range(500)]
    for _ in range(3):
        engine.animate(objs, {"x": "+=1", "c": "blue"}, duration=0.1, delay=anikit.stagger(500, 0.0001))
        engine.run_until_idle()
    gc.collect()
    assert engine.node_count == 0
    assert all(o["x"] == 3 for o in objs)


def test_low_level_engine():
    core = _anikit.Engine()
    eid = core.easing("outQuad")
    assert core.easing("outQuad") == eid and core.ease(eid, 0.5) == pytest.approx(0.75)
    h = core.create_anim([(_anikit.TRACK_NUMBER, 1, [0, 10], [], [], 0, None)], duration=100)
    core.play(h)
    upd, vals, ev = core.step(50)
    assert upd == [h] and vals == [[5.0]]
    assert ev == [h, _anikit.EVENT_START]
    assert core.progress(h) == pytest.approx(0.5)
    assert core.state(h) == _anikit.STATE_PLAYING
    assert core.timing(h)[0] == 100
    core.set_timing(h, duration=200)
    assert core.duration(h) == 200
    assert core.sample(h, 1.0) == [10.0]
    g = core.create_group()
    core.group_add(g, h, 10)
    assert core.parent(h) == g and core.start(h) == 10
    with pytest.raises(RuntimeError):
        core.play(h)  # not a root
    core.release(g)
    assert not core.is_alive(h) and core.node_count == 0
    with pytest.raises(KeyError):
        core.values(h)
    with pytest.raises(ValueError):
        core.create_anim([(_anikit.TRACK_COLOR, 3, [0, 0, 0], [], [], 2, None)])
    with pytest.raises(ValueError):
        core.create_anim([(_anikit.TRACK_PATH, 1, [0, 1], [], [], 0, None)])
    with pytest.raises(ValueError):
        core.create_anim([(9, 1, [0, 1], [], [], 0, None)])
    with pytest.raises(ValueError):
        core.easing("bogus")
    c = core.create_callback()
    with pytest.raises(TypeError):
        core.values(c)
    s = core.create_spring([0], [1])
    with pytest.raises(TypeError):
        core.seek(s, 0)
    with pytest.raises(ValueError):
        core.create_spring([0, 0], [1])


def test_low_level_path_track():
    core = _anikit.Engine()
    p = _anikit.NativePath("M0 0 L100 0")
    h = core.create_anim([(_anikit.TRACK_PATH, 1, [0, 1], [], [], 0, p)], duration=100)
    core.seek(h, 50)
    assert core.values(h) == pytest.approx([50, 0, 0])
    assert core.track_offsets(h) == [0]


def test_version():
    assert anikit.__version__ == "0.1.0"
