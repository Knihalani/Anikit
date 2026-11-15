import gc

import pytest


def test_spring_settles(engine):
    o = {"x": 0.0}
    done = []
    s = engine.spring(o, {"x": 100}, stiffness=170, damping=26, on_complete=lambda: done.append(1))
    t = engine.run_until_idle()
    assert o["x"] == 100 and done == [1] and s.finished
    assert 0.3 < t < 3
    gc.collect()
    assert engine.node_count == 0


def test_spring_overshoots_when_underdamped(engine):
    o = {"x": 0.0}
    engine.spring(o, {"x": 100}, stiffness=200, damping=5)
    peak = 0.0
    for _ in range(120):
        engine.tick(1 / 60)
        peak = max(peak, o["x"])
    assert peak > 110


def test_spring_retarget_preserves_velocity(engine):
    o = {"x": 0.0}
    first_done = []
    s1 = engine.spring(o, {"x": 100}, stiffness=170, damping=26, on_complete=lambda: first_done.append(1))
    for _ in range(10):
        engine.tick(1 / 60)
    v = s1.velocity("x")
    pos = o["x"]
    assert v > 0
    s2 = engine.spring(o, {"x": -100}, stiffness=170, damping=26)
    assert s2.velocity("x") == pytest.approx(v)
    assert s1.finished and not s2.finished  # the property moved to s2
    assert engine.node_count == 1  # same simulation, retargeted
    engine.tick(1 / 60)
    assert o["x"] > pos  # momentum carries it on before turning around
    engine.run_until_idle()
    assert o["x"] == -100 and first_done == []
    assert s2.finished


def test_spring_relative_to_current_target(engine):
    o = {"x": 0.0}
    engine.spring(o, {"x": "+=10"})
    engine.spring(o, {"x": "+=10"})
    engine.run_until_idle()
    assert o["x"] == 20


def test_spring_units_colours_from_and_velocity(engine):
    o = {"w": "0px", "c": "red", "x": 5}
    s = engine.spring(o, {"w": "50px", "c": "#0000ff", "x": [0, 10]}, bounce=0, duration=0.3, velocity=0)
    engine.tick(1 / 60)
    assert o["x"] < 5  # restarted from the explicit `from`
    assert s.target_value("w") == "50px"
    engine.run_until_idle()
    assert o == {"w": "50px", "c": "#0000ff", "x": 10}
    o2 = {"x": 0.0}
    engine.spring(o2, {"x": 0}, velocity=500)
    engine.tick(0.05)
    assert o2["x"] > 5


def test_spring_retarget_method_and_stop(engine):
    o = {"x": 0.0}
    s = engine.spring(o, {"x": 100})
    engine.tick(0.1)
    s2 = s.retarget({"x": 50})
    engine.run_until_idle()
    assert o["x"] == 50 and s2.finished
    s3 = engine.spring(o, {"x": 500})
    engine.tick(0.1)
    x = o["x"]
    s3.stop()
    engine.tick(0.1)
    assert o["x"] == x and engine.node_count == 0


def test_spring_vs_tween_overwrite(engine):
    o = {"x": 0.0}
    engine.spring(o, {"x": 1000}, stiffness=10, damping=1)
    engine.tick(0.1)
    engine.animate(o, {"x": 5}, duration=0.2)
    engine.run_until_idle()
    assert o["x"] == 5
    a = engine.animate(o, {"x": 100}, duration=1)
    engine.tick(0.1)
    engine.spring(o, {"x": -5})
    engine.run_until_idle()
    assert o["x"] == -5
    assert a.finished


def test_spring_multiple_targets(engine):
    objs = [{"x": 0.0} for _ in range(3)]
    s = engine.spring(objs, {"x": lambda t, i, n: i * 10})
    engine.run_until_idle()
    assert [o["x"] for o in objs] == [0, 10, 20]
    assert s.finished


def test_spring_errors(engine):
    with pytest.raises(TypeError):
        engine.spring({"x": True}, {"x": False})
    with pytest.raises(ValueError):
        engine.spring({"x": 0}, {"x": [0, 1, 2]})
    with pytest.raises(ValueError):
        engine.spring({"x": 0}, {})


def test_inertia_bounds(engine):
    card = {"x": 100.0}
    done = []
    i = engine.inertia(card, {"x": {"velocity": 2000, "min": 0, "max": 500}}, on_complete=lambda: done.append(1))
    peak = 0.0
    for _ in range(600):
        engine.tick(1 / 60)
        peak = max(peak, card["x"])
    assert 500 < peak < 600  # overshoots the bound, springs back
    assert card["x"] == 500 and i.finished and done == [1]


def test_inertia_natural_end_and_snapping(engine):
    o = {"x": 0.0}
    engine.inertia(o, {"x": 1000}, power=0.8, time_constant=0.35)
    engine.run_until_idle()
    assert o["x"] == pytest.approx(280)
    o2 = {"x": 0.0}
    engine.inertia(o2, {"x": {"velocity": 1000, "end": [0, 100, 200, 300]}})
    engine.run_until_idle()
    assert o2["x"] == 300
    o3 = {"x": 0.0}
    engine.inertia(o3, {"x": {"velocity": 1000, "end": lambda n: round(n / 50) * 50}})
    engine.run_until_idle()
    assert o3["x"] == 300
    o4 = {"x": "0px"}
    engine.inertia(o4, {"x": {"velocity": -1000, "min": -100}})
    engine.run_until_idle()
    assert o4["x"] == "-100px"


def test_inertia_inherits_velocity(engine):
    o = {"x": 0.0}
    engine.spring(o, {"x": 1000}, stiffness=50, damping=5)
    engine.tick(0.2)
    i = engine.inertia(o, {"x": {}})
    assert i.velocity("x") != 0
    with pytest.raises(TypeError):
        engine.inertia(o, {"x": {"bogus": 1}})
    with pytest.raises(TypeError):
        engine.inertia({"c": "red"}, {"c": 10})


def test_spring_on_update_receives_controller(engine):
    seen = []
    s = engine.spring({"x": 0.0}, {"x": 10}, on_update=lambda c: seen.append(c))
    engine.tick(1 / 60)
    engine.tick(1 / 60)
    assert seen == [s, s]
