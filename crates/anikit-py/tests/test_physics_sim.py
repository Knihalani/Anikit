"""Ballistic physics: anikit.physics() and the low-level create_physics()."""

import gc
import math

import pytest

import anikit
from anikit import PhysicsAnimation, SpringAnimation
from anikit import _anikit as ak


class Ball:
    def __init__(self, x=0.0, y=0.0):
        self.x = x
        self.y = y


def test_constant_velocity_and_free_fall(engine):
    o = {"x": 0.0, "y": 0.0}
    c = engine.physics(o, {"x": 100, "y": {"velocity": 0, "acceleration": 100}}, life=10)
    assert isinstance(c, PhysicsAnimation) and isinstance(c, SpringAnimation)
    engine.tick(0.5)
    assert o["x"] == pytest.approx(50)
    assert o["y"] == pytest.approx(12.5)  # ½ g t²
    assert c.velocity("x") == pytest.approx(100)
    assert c.velocity("y") == pytest.approx(50)
    assert c.acceleration("y") == 100 and c.acceleration("x") == 0
    # Exact integration: the frame rate does not matter.
    o2 = {"y": 0.0}
    engine.physics(o2, {"y": {"velocity": -30, "acceleration": 100}}, friction=0.7, life=10)
    for _ in range(30):
        engine.tick(1 / 60)
    o3 = {"y": 0.0}
    e2 = anikit.Engine()
    e2.physics(o3, {"y": {"velocity": -30, "acceleration": 100}}, friction=0.7, life=10)
    e2.tick(0.5)
    assert o2["y"] == pytest.approx(o3["y"], rel=1e-9)


def test_friction_drag(engine):
    o = {"x": 0.0}
    c = engine.physics(o, {"x": 200}, friction=2)
    engine.tick(0.5)
    assert c.velocity("x") == pytest.approx(200 * math.exp(-1))
    assert o["x"] == pytest.approx(100 * (1 - math.exp(-1)))
    t = engine.run_until_idle()
    assert c.finished and o["x"] == pytest.approx(100, abs=0.05)  # v0 / friction
    assert t < 10
    # Terminal velocity under constant acceleration: a / friction.
    f = {"y": 0.0}
    d = engine.physics(f, {"y": {"acceleration": 100}}, friction=5, life=10)
    for _ in range(200):
        engine.tick(1 / 60)
    assert d.velocity("y") == pytest.approx(20, rel=1e-4)


def test_bouncing_ball_comes_to_rest(engine):
    ball = Ball()
    ys = []
    done = []
    c = engine.physics(
        ball,
        {"x": {"velocity": 50, "max": 120}, "y": {"velocity": 0, "acceleration": 980, "max": 300}},
        friction=0.5,
        restitution=0.3,
        on_update=lambda: ys.append(ball.y),
        on_complete=lambda ctrl: done.append(ctrl),
    )
    t = engine.run_until_idle()
    assert done == [c] and c.finished and not c.is_active
    assert ball.y == 300 and max(ys) <= 300
    # It bounced: y went back up (decreased) at least twice before resting.
    ups = sum(1 for i in range(2, len(ys)) if ys[i - 1] >= ys[i - 2] and ys[i] < ys[i - 1])
    assert ups >= 2
    first_bounce = min(i for i in range(1, len(ys)) if ys[i] < ys[i - 1])
    assert min(ys[first_bounce:]) < 290  # rebound ≈ 0.3 * impact speed → ~25 units high
    assert 13 < t < 15  # x glides until 50·e^(-0.5t) < 0.05: t = ln(1000) / 0.5
    gc.collect()
    assert engine.node_count == 0


def test_bouncing_ball_rests_at_60fps_with_high_restitution(engine):
    ball = {"y": 0.0}
    c = engine.physics(ball, {"y": {"acceleration": 980, "max": 300}}, friction=0.5, restitution=0.6)
    engine.run_until_idle(max_time=60)
    assert c.finished


def test_restitution_and_bounds(engine):
    o = {"x": 50.0}
    c = engine.physics(o, {"x": {"velocity": 1000, "min": 0, "max": 100}}, restitution=1.0, life=1.0)
    lo = hi = 50.0
    while not c.finished:
        engine.tick(1 / 120)
        lo, hi = min(lo, o["x"]), max(hi, o["x"])
        assert 0 <= o["x"] <= 100
    assert lo < 5 and hi > 95
    assert engine.time == pytest.approx(1.0, abs=1 / 60)  # ended by `life`
    # Restitution 0: stops dead on the bound.
    s = {"x": 0.0}
    c = engine.physics(s, {"x": {"velocity": 500, "max": 10}}, restitution=0)
    engine.tick(0.1)
    assert s["x"] == 10 and c.finished  # at rest on the bound
    # Per-property overrides of friction / restitution.
    p = {"a": 0.0, "b": 0.0}
    c = engine.physics(p, {"a": {"velocity": 100, "friction": 3}, "b": 100}, friction=0, life=1)
    engine.tick(0.5)
    assert c.velocity("a") == pytest.approx(100 * math.exp(-1.5))
    assert c.velocity("b") == pytest.approx(100)


def test_life_and_callbacks(engine):
    o = {"x": 0.0}
    updates = []
    done = []
    c = engine.physics(
        o, {"x": 10}, life=0.1, on_update=lambda: updates.append(o["x"]), on_complete=lambda: done.append(1)
    )
    engine.tick(0.06)
    assert not c.finished and done == []
    engine.tick(0.06)
    assert c.finished and done == [1]
    assert o["x"] == pytest.approx(1.2)
    assert len(updates) == 2
    with pytest.raises(ValueError):
        engine.physics(o, {"x": 1}, life=0)
    with pytest.raises(ValueError):
        engine.physics(o, {"x": 1}, life=-1)


def test_stop_and_takeover(engine):
    o = {"x": 0.0}
    done = []
    c = engine.physics(o, {"x": 100}, on_complete=lambda: done.append(1))
    engine.tick(0.1)
    c.stop()
    engine.tick(0.1)
    assert o["x"] == pytest.approx(10) and c.finished and done == []
    gc.collect()
    assert engine.node_count == 0
    # A tween takes the property over.
    p = engine.physics(o, {"x": 100})
    engine.animate(o, {"x": -50}, duration=0.2)
    engine.tick(0.3)
    assert o["x"] == -50 and p.finished


def test_handoff_between_simulations_keeps_momentum(engine):
    o = {"x": 0.0}
    p = engine.physics(o, {"x": 500})
    engine.tick(0.1)
    assert o["x"] == pytest.approx(50)
    # physics → spring: fresh spring with the physics velocity (not retargeted in place).
    s = engine.spring(o, {"x": 0}, stiffness=170, damping=26)
    assert p.finished and s.velocity("x") == pytest.approx(500)
    engine.tick(1 / 60)
    assert o["x"] > 50  # momentum carries on
    engine.run_until_idle()
    assert o["x"] == 0 and s.finished
    # Relative spring values after physics build on the current value.
    engine.physics(o, {"x": 100})
    engine.tick(0.1)
    s = engine.spring(o, {"x": "+=10"})
    assert s.target_value("x") == pytest.approx(20)
    engine.run_until_idle()
    # spring → physics inherits velocity when none is given.
    s = engine.spring(o, {"x": 100})
    engine.tick(0.1)
    v = s.velocity("x")
    p = engine.physics(o, {"x": {"acceleration": -10}}, life=1)
    assert s.finished and p.velocity("x") == pytest.approx(v)
    # physics → inertia, physics → physics.
    i = engine.inertia(o, {"x": {}})
    assert p.finished and i.velocity("x") == pytest.approx(v - 0.0, rel=1e-6)
    p2 = engine.physics(o, {"x": {}}, life=1)
    assert i.finished and p2.velocity("x") == pytest.approx(v)
    p2.stop()


def test_set_acceleration(engine):
    o = {"x": 0.0, "y": 0.0}
    c = engine.physics(o, {"x": 10, "y": 1}, life=5)
    assert c.set_acceleration({"x": 100}) is c
    engine.tick(1)
    assert o["x"] == pytest.approx(60) and c.acceleration("x") == 100
    c.set_acceleration({"x": 0, "y": -20})
    engine.tick(1)
    assert o["x"] == pytest.approx(170) and o["y"] == pytest.approx(2 - 10)
    with pytest.raises(KeyError):
        c.set_acceleration({"nope": 1})
    with pytest.raises(TypeError):
        c.target_value("x")


def test_multiple_targets_callables_and_units(engine):
    balls = [Ball(), Ball(), Ball()]
    c = engine.physics(balls, {"x": lambda t, i, n: (i + 1) * 10}, life=1)
    engine.tick(0.5)
    assert c.velocity("x", balls[2]) == pytest.approx(30)
    assert c.velocity("x") == pytest.approx(10)  # first target by default
    engine.tick(0.5)
    assert [b.x for b in balls] == pytest.approx([10, 20, 30])
    assert c.targets == balls and c.finished
    u = {"left": "10px", "missing_ok": None}
    engine.physics(u, {"left": {"velocity": 100}}, friction=1, life=0.5)
    engine.tick(0.5)
    assert isinstance(u["left"], str) and u["left"].endswith("px")
    assert float(u["left"][:-2]) == pytest.approx(10 + 100 * (1 - math.exp(-0.5)), abs=1e-3)
    m = {}
    engine.physics(m, {"x": 10}, life=0.5)
    engine.tick(0.5)
    assert m["x"] == pytest.approx(5)  # missing properties start at 0


def test_retarget_relaunches(engine):
    o = {"x": 0.0}
    c = engine.physics(o, {"x": 10}, friction=1, life=1)
    engine.tick(0.5)
    c2 = c.retarget({"x": -100})
    assert isinstance(c2, PhysicsAnimation) and c.finished
    assert c2.velocity("x") == -100
    engine.run_until_idle()
    assert c2.finished


def test_physics_errors(engine):
    with pytest.raises(ValueError):
        engine.physics({"x": 0}, {})
    with pytest.raises(TypeError):
        engine.physics({"x": 0}, {"x": {"speed": 1}})
    with pytest.raises(TypeError):
        engine.physics({"c": "red"}, {"c": 1})
    with pytest.raises(ValueError):
        engine.physics({"x": 0}, {"x": {"velocity": 1, "min": 10, "max": 0}})
    with pytest.raises(ValueError):
        engine.physics({"x": 0}, {"x": 1}, friction=-1)
    with pytest.raises(ValueError):
        engine.physics({"x": 0}, {"x": 1}, restitution=math.nan)
    assert engine.node_count == 0


def test_rest_speed(engine):
    o = {"x": 0.0}
    c = engine.physics(o, {"x": 100}, friction=2, rest_speed=5)
    t = engine.run_until_idle()
    assert c.finished and t == pytest.approx(math.log(100 / 5) / 2, abs=0.05)


def test_module_level_function():
    anikit.set_default_engine(None)
    try:
        o = {"x": 0.0}
        c = anikit.physics(o, {"x": 10}, life=0.5)
        anikit.run_until_idle()
        assert o["x"] == pytest.approx(5, abs=0.2) and c.finished  # life ends on a frame boundary
    finally:
        anikit.set_default_engine(None)


def test_low_level_create_physics():
    core = ak.Engine()
    core.set_lag_smoothing(0, 0)
    h = core.create_physics([0.0, 0.0], [10.0, 0.0], [0.0, 100.0], life=1000.0)
    core.tick(500.0)
    assert core.values(h) == pytest.approx([5.0, 12.5])
    assert core.sim_velocity(h) == pytest.approx([10.0, 50.0])
    core.sim_set_acceleration(h, [0.0, 0.0])
    core.tick(100.0)
    assert core.values(h) == pytest.approx([6.0, 17.5])
    core.tick(500.0)
    assert core.state(h) == ak.STATE_FINISHED
    # Bounds, friction, restitution.
    b = core.create_physics([0.0], [0.0], [980.0], 0.5, None, [300.0], 0.6)
    for _ in range(2000):
        _, _, ev = core.step(16.0)
        if (b, ak.EVENT_FINISH) in zip(ev[::2], ev[1::2]):
            break
    assert core.values(b) == [300.0]
    # No acceleration given = zero acceleration.
    z = core.create_physics([1.0], [2.0])
    core.tick(1000.0)
    assert core.values(z) == pytest.approx([3.0])
    with pytest.raises(ValueError):
        core.create_physics([0.0], [1.0, 2.0])
    with pytest.raises(ValueError):
        core.create_physics([0.0], [1.0], [1.0, 2.0])
    with pytest.raises(ValueError):
        core.create_physics([0.0], [1.0], min=[1.0, 2.0])
    with pytest.raises(ValueError):
        core.create_physics([0.0], [1.0], life=0.0)
    with pytest.raises(ValueError):
        core.sim_set_acceleration(h, [1.0])  # wrong channel count
    s = core.create_spring([0.0], [1.0])
    with pytest.raises(ValueError):
        core.sim_set_acceleration(s, [1.0])  # not a physics simulation


def test_callable_returning_options(engine):
    confetti = [{"y": 0.0} for _ in range(3)]
    c = engine.physics(confetti, {"y": lambda t, i, n: {"velocity": -400 - 40 * i, "acceleration": 900}}, life=3)
    engine.tick(0.1)
    assert [o["y"] for o in confetti] == pytest.approx([-35.5, -39.5, -43.5])
    assert c.velocity("y", confetti[2]) == pytest.approx(-390)
    assert c.acceleration("y", confetti[1]) == 900
