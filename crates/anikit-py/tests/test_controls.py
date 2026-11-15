import gc

import pytest


def make(engine, **kw):
    o = {"x": 0}
    log = []
    kw.setdefault("duration", 1)
    kw.setdefault("ease", "linear")
    a = engine.animate(
        o,
        {"x": 100},
        on_start=lambda: log.append("start"),
        on_update=lambda: log.append("update"),
        on_complete=lambda: log.append("complete"),
        on_repeat=lambda: log.append("repeat"),
        on_reverse_complete=lambda: log.append("rcomplete"),
        **kw,
    )
    return o, a, log


def test_callbacks_order(engine):
    o, a, log = make(engine)
    engine.tick(0.5)
    assert log == ["start", "update"]
    engine.tick(0.6)
    assert log == ["start", "update", "update", "complete"]
    engine.tick(0.1)
    assert log[-1] == "complete" and log.count("complete") == 1


def test_callback_receives_animation(engine):
    got = []
    a = engine.animate({"x": 0}, {"x": 1}, duration=0.1, on_complete=lambda anim: got.append(anim))
    engine.run_until_idle()
    assert got == [a]


def test_on_update_fires_once_per_frame_for_many_targets(engine):
    n = []
    engine.animate([{"x": 0} for _ in range(10)], {"x": 1}, duration=1, on_update=lambda: n.append(1))
    engine.tick(0.1)
    assert len(n) == 1


def test_repeat_and_yoyo(engine):
    o, a, log = make(engine, repeat=2, yoyo=True)
    assert a.duration == pytest.approx(3)
    engine.tick(0.5)
    engine.tick(1.0)
    assert o["x"] == pytest.approx(50)
    assert log.count("repeat") == 1
    engine.tick(0.25)
    assert o["x"] == pytest.approx(25)
    engine.run_until_idle()
    assert o["x"] == 100 and log.count("repeat") == 2 and log[-1] == "complete"


def test_infinite_repeat(engine):
    o, a, log = make(engine, repeat=-1)
    assert a.duration == float("inf")
    engine.tick(10.25)
    assert o["x"] == pytest.approx(25)
    assert a.is_active
    assert engine.run_until_idle(max_time=1) == pytest.approx(1, abs=0.02)
    with pytest.raises(ValueError):
        a.progress = 0.5
    with pytest.raises(ValueError):
        a.complete()


def test_repeat_delay(engine):
    o, a, log = make(engine, repeat=1, repeat_delay=0.5)
    assert a.duration == pytest.approx(2.5)
    engine.tick(1.25)
    assert o["x"] == 100  # holds the end value during the gap
    engine.tick(0.5)
    assert o["x"] == pytest.approx(25)
    engine.run_until_idle()
    assert o["x"] == 100 and log[-1] == "complete" and a.finished


def test_repeat_delay_yoyo(engine):
    o, a, log = make(engine, repeat=1, yoyo=True, repeat_delay=0.4)
    assert a.duration == pytest.approx(2.4)
    engine.tick(1.2)
    assert o["x"] == 100
    engine.tick(0.6)  # 0.4s into the backward iteration
    assert o["x"] == pytest.approx(60)
    engine.run_until_idle()
    assert o["x"] == 0 and log[-1] == "complete"


def test_pause_resume_seek_progress(engine):
    o, a, log = make(engine)
    engine.tick(0.25)
    a.pause()
    assert a.state == "paused"
    engine.tick(1)
    assert o["x"] == pytest.approx(25)
    a.seek(0.6)
    assert o["x"] == pytest.approx(60) and a.time == pytest.approx(0.6)
    a.progress = 0.1
    assert o["x"] == pytest.approx(10) and a.progress == pytest.approx(0.1)
    a.time = 0.3
    assert o["x"] == pytest.approx(30)
    a.resume()
    engine.tick(0.2)
    assert o["x"] == pytest.approx(50)


def test_seek_to_end_completes(engine):
    o, a, log = make(engine, autoplay=False)
    assert a.state == "idle"
    a.complete()
    assert o["x"] == 100 and "complete" in log


def test_reverse(engine):
    o, a, log = make(engine)
    engine.tick(0.5)
    a.reverse()
    assert a.reversed
    engine.tick(0.25)
    assert o["x"] == pytest.approx(25)
    engine.run_until_idle()
    assert o["x"] == 0 and "rcomplete" in log and a.finished
    # Reverse from the start plays backwards from the end.
    o2, b, _ = make(engine, autoplay=False)
    b.reverse()
    engine.tick(0.25)
    assert o2["x"] == pytest.approx(75)


def test_play_after_finish_and_restart(engine):
    o, a, log = make(engine)
    engine.run_until_idle()
    a.play()
    engine.tick(0.5)
    assert o["x"] == pytest.approx(50)
    assert log.count("start") == 2
    a.restart()
    engine.tick(0.25)
    assert o["x"] == pytest.approx(25)
    assert log.count("start") == 3
    a.reverse()
    a.play()  # play() always goes forwards
    assert not a.reversed


def test_speed(engine):
    o, a, log = make(engine, speed=2)
    assert a.speed == 2
    engine.tick(0.25)
    assert o["x"] == pytest.approx(50)
    a.speed = 0.5
    engine.tick(0.5)
    assert o["x"] == pytest.approx(75)
    with pytest.raises(ValueError):
        a.speed = 0
    engine.speed = 2  # global time scale
    engine.tick(0.25)
    assert o["x"] == pytest.approx(100)


def test_kill_releases_nodes(engine):
    o, a, log = make(engine)
    multi = engine.animate([{"x": 0}, {"x": 0}], {"x": 1})
    assert engine.node_count == 4  # anim + group + 2 children
    a.kill()
    multi.kill()
    assert engine.node_count == 0
    assert not a.alive
    with pytest.raises(RuntimeError):
        a.play()
    a.kill()  # idempotent
    engine.tick(1)
    assert o["x"] == 0


def test_discarded_animations_are_released(engine):
    o = {"x": 0}
    for i in range(50):
        engine.animate(o, {"x": i}, duration=0.1)
    assert engine.node_count == 50  # still playing: kept alive by the engine
    engine.run_until_idle()
    gc.collect()
    assert engine.node_count == 0
    assert o["x"] == 49
    kept = engine.animate(o, {"x": 0}, duration=0.1)
    engine.run_until_idle()
    gc.collect()
    assert engine.node_count == 1  # finished but still referenced: replayable
    kept.play()
    engine.tick(0.05)
    assert 0 < o["x"] < 49
    del kept
    gc.collect()
    assert engine.node_count == 1  # playing: the engine keeps it alive...
    engine.run_until_idle()
    gc.collect()
    assert engine.node_count == 0  # ...until it finishes


def test_paused_unreferenced_animation_is_released(engine):
    engine.animate({"x": 0}, {"x": 1}, autoplay=False)
    gc.collect()
    assert engine.node_count == 0


def test_callback_exception_propagates_but_engine_survives(engine):
    def boom():
        raise RuntimeError("boom")

    o = {"x": 0}
    engine.animate(o, {"x": 1}, duration=0.1, on_complete=boom)
    with pytest.raises(RuntimeError):
        engine.run_until_idle()
    assert engine.is_idle
    gc.collect()
    assert engine.node_count == 0
