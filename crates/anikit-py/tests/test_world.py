import math
import threading
from array import array

import pytest

import anikit
from anikit import Body, Cloth, Link, Rope, World


def boxed(**kw):
    return World(bounds=(0, 0, 400, 400), **kw)


# ---------------------------------------------------------------------------
# Construction / configuration
# ---------------------------------------------------------------------------


def test_config_roundtrip_and_live_changes():
    w = World(
        gravity=(10, 20),
        damping=0.5,
        substeps=6,
        iterations=3,
        restitution=0.4,
        wall_restitution=0.6,
        friction=0.1,
        bounds=(100, 50, 0, 0),
    )
    assert w.gravity == (10, 20)
    assert w.damping == pytest.approx(0.5)
    assert (w.substeps, w.iterations) == (6, 3)
    assert w.restitution == pytest.approx(0.4) and w.wall_restitution == pytest.approx(0.6)
    assert w.friction == pytest.approx(0.1)
    assert w.bounds == (0, 0, 100, 50)  # normalised
    w.bounds = None
    assert w.bounds is None
    w.gravity = (0, 0)
    w.substeps = 0
    assert w.gravity == (0, 0) and w.substeps == 1  # clamped
    assert World().gravity == (0, 980) and World().bounds is None
    with pytest.raises(ValueError):
        World(bounds=(0, 0, 1))
    with pytest.raises(ValueError):
        World(gravity=(math.nan, 0))
    assert "World(bodies=0" in repr(w)


def test_bodies_and_handles():
    w = World(gravity=(0, 0))
    b = w.add_body(10, 20, radius=5, mass=2, velocity=(3, 4))
    assert isinstance(b, Body) and b.index == 0 and len(w) == 1
    assert b.position == (10, 20) and b.velocity == (3, 4) and b.speed == pytest.approx(5)
    assert (b.radius, b.mass, b.pinned, b.active, b.static) == (5, 2, False, True, False)
    b.x = 50
    assert b.position == (50, 20) and b.velocity == (3, 4)  # setters keep the velocity
    b.move_to(1, 2)
    assert b.position == (1, 2) and b.velocity == (0, 0)
    b.push(10, 0).push(0, 5)
    assert b.velocity == (10, 5)
    b.radius = 7
    assert b.radius == 7
    assert w[0] == b and w[-1] == b and hash(w.body(0)) == hash(b)
    assert list(w) == [b]
    with pytest.raises(IndexError):
        w.body(1)
    with pytest.raises(IndexError):
        w.native.position(5)
    s = w.add_body(0, 0, 3, mass=0)
    assert s.static and not s.pinned
    assert "Body(0" in repr(b)


def test_add_bodies_bulk_forms():
    w = World()
    r = w.add_bodies([(0, 0), (10, 0, 3), (20, 0, 4, 0)], radius=2, mass=5)
    assert r == range(0, 3)
    assert [w[i].radius for i in r] == [2, 3, 4]
    assert [w[i].mass for i in r] == [5, 5, 0]
    flat = array("f", [30, 0, 1, 1, 40, 0, 1, 1])
    assert w.add_bodies(flat) == range(3, 5)
    assert w.add_bodies(array("d", [50, 0, 1, 1])) == range(5, 6)
    assert len(w) == 6 and len(w.radii()) == 6
    with pytest.raises(ValueError):
        w.add_bodies([(1,)])
    with pytest.raises(ValueError):
        w.add_bodies(array("f", [1, 2, 3]))


def test_bulk_outputs_are_flat_arrays():
    w = World(gravity=(0, 0))
    w.add_bodies([(i * 10.0, i * 20.0) for i in range(100)])
    pos = w.positions()
    assert isinstance(pos, array) and pos.typecode == "f" and len(pos) == 200
    assert (pos[0], pos[1], pos[198], pos[199]) == (0, 0, 990, 1980)
    out = w.output()
    assert len(out) == 300 and out[3 * 7] == 70 and out[3 * 7 + 2] == 0
    w[3].velocity = (1, 2)
    vel = w.velocities()
    assert len(vel) == 200 and (vel[6], vel[7]) == (1, 2)
    # Moves show up immediately, even before a step.
    w.set_positions([5, 6, 7, 8], first=10)
    assert w[10].position == (5, 6) and w.positions()[22:24].tolist() == [7, 8]
    w.set_positions(array("f", [1, 1]), first=99)
    assert w[99].position == (1, 1)
    with pytest.raises(ValueError):
        w.set_positions([1, 2], first=100)


# ---------------------------------------------------------------------------
# Behaviour
# ---------------------------------------------------------------------------


def test_falls_and_rests_on_floor():
    w = boxed()
    b = w.add_body(200, 50, radius=10)
    w.run(10)
    assert b.y == pytest.approx(390, abs=0.5)
    assert abs(b.velocity[1]) < 5
    assert w.kinetic_energy < 1
    assert w.time == pytest.approx(10)


def test_free_fall_matches_gravity():
    w = World(gravity=(0, 100), damping=0)
    b = w.add_body(0, 0, 1)
    w.run(1.0, fps=120)
    # Semi-implicit Euler: within a few percent of ½·g·t².
    assert b.y == pytest.approx(50, rel=0.05)
    assert b.velocity[1] == pytest.approx(100, rel=1e-3)


def test_pile_does_not_overlap_and_stays_in_bounds():
    w = boxed()
    w.add_bodies([(20 + (i % 20) * 18, 20 + (i // 20) * 9, 4 + i % 3) for i in range(400)])
    w.run(10)
    pos = w.positions()
    radii = w.radii()
    worst = 0.0
    for i in range(len(w)):
        xi, yi = pos[2 * i], pos[2 * i + 1]
        assert -0.01 <= xi - radii[i] + 0.01 and xi + radii[i] <= 400.01
        assert yi + radii[i] <= 400.01
        for j in range(i + 1, len(w)):
            d = math.hypot(xi - pos[2 * j], yi - pos[2 * j + 1])
            worst = max(worst, radii[i] + radii[j] - d)
    assert worst < 2.0
    assert w.pair_tests > 0


def test_settle_stops_when_at_rest():
    w = boxed()
    w.add_bodies([(100 + i * 25, 100) for i in range(8)])
    t = w.settle(max_time=20)
    assert t < 5 and w.max_speed() < 5
    for b in w:
        assert b.y == pytest.approx(390, abs=0.5)
    assert World().max_speed() == 0


def test_wall_restitution_bounces():
    w = World(gravity=(0, 0), damping=0, wall_restitution=1, friction=0, bounds=(0, 0, 100, 100))
    b = w.add_body(50, 50, 5)
    b.velocity = (300, 0)
    w.run(0.2)
    assert b.velocity[0] < -250
    dead = World(gravity=(0, 0), damping=0, wall_restitution=0, friction=0, bounds=(0, 0, 100, 100))
    c = dead.add_body(50, 50, 5)
    c.velocity = (300, 0)
    dead.run(0.5)
    assert abs(c.velocity[0]) < 1 and c.x == pytest.approx(95, abs=0.01)


def test_boxes_block_bodies():
    w = boxed()
    assert w.add_box(250, 220, 150, 200) == 0
    assert w.boxes == [(150, 200, 250, 220)]
    b = w.add_body(200, 100, 10)
    w.run(5)
    assert b.y == pytest.approx(190, abs=1)
    w.clear_boxes()
    assert w.boxes == []
    w.run(5)
    assert b.y == pytest.approx(390, abs=1)


def test_static_pinned_and_inactive_bodies():
    w = boxed()
    s = w.add_body(200, 300, 20, mass=0)
    ball = w.add_body(200, 100, 10)
    w.run(5)
    assert s.position == (200, 300)
    assert ball.y == pytest.approx(270, abs=1)  # resting on the static body (or rolled off)
    p = w.add_body(50, 50, 5, pinned=True)
    w.run(1)
    assert p.position == (50, 50) and p.pinned
    p.unpin()
    w.run(2)
    assert p.y > 300
    gone = w.add_body(300, 50, 5)
    gone.remove()
    assert not gone.active
    w.run(1)
    assert gone.position == (300, 50)
    assert w.pick(300, 50) is None
    gone.active = True
    w.run(1)
    assert gone.y > 50


def test_pick():
    w = World(gravity=(0, 0))
    a = w.add_body(0, 0, 10)
    b = w.add_body(5, 0, 10)
    assert w.pick(5, 0) == b  # topmost = last added
    assert w.pick(-8, 0) == a
    assert w.pick(100, 100) is None


def test_impulse_pushes_away():
    w = World(gravity=(0, 0), damping=0)
    left = w.add_body(-10, 0, 1)
    right = w.add_body(10, 0, 1)
    far = w.add_body(500, 0, 1)
    w.impulse(0, 0, radius=100, strength=200)
    assert left.velocity[0] < 0 < right.velocity[0]
    assert right.velocity[0] == pytest.approx(180, rel=1e-4)  # 200 · (1 - 10/100)
    assert far.velocity == (0, 0)


def test_attractor_and_repulsor():
    w = World(gravity=(0, 0), damping=3)
    b = w.add_body(100, 0, 5)
    a = w.attract(0, 0, strength=5000, radius=500)
    w.step(1 / 60)
    assert b.velocity[0] < 0
    w.run(5)
    assert math.hypot(*b.position) < 5  # pulled in and settled at the centre
    a.remove()
    assert w.attractors == []

    w2 = World(gravity=(0, 0))
    c = w2.add_body(10, 0, 1)
    out = w2.add_body(1000, 0, 1)
    rep = w2.repel(0, 0, strength=5000, radius=200)
    assert rep.strength == -5000
    w2.run(0.5)
    assert c.x > 50 and out.velocity == (0, 0)

    # Attractors are live objects: moving one re-targets the pull.
    w3 = World(gravity=(0, 0))
    d = w3.add_body(0, 0, 1)
    at = w3.attract(100, 0, 3000, 1000)
    w3.run(0.2)
    assert d.velocity[0] > 0
    at.move_to(-1000, 0)
    at.x = -100
    at.strength = 10000
    at.radius = 1000
    d.velocity = (0, 0)
    w3.step(1 / 60)
    assert d.velocity[0] < 0
    w3.clear_attractors()
    d.velocity = (0, 0)
    w3.step(1 / 60)
    assert d.velocity == (0, 0)
    assert "Attractor(" in repr(at)


def test_rope_keeps_lengths_and_hangs():
    w = World(iterations=8, damping=2.0)
    rope = w.rope([(k * 10.0, 0.0) for k in range(20)], radius=2)
    assert isinstance(rope, Rope) and len(rope) == 20 and len(rope.links) == 19
    assert rope.start.pinned and not rope.end.pinned
    assert rope.length == pytest.approx(190)
    w.run(15)
    x, y = rope.end.position
    assert abs(x) < 10 and y > 150  # hangs straight down from the pin
    assert rope.start.position == (0, 0)
    for link in w.constraints():
        assert isinstance(link, Link) and link.alive and link.length == pytest.approx(10)
        a, b = w[link.a].position, w[link.b].position
        assert math.dist(a, b) == pytest.approx(10, abs=1.5)
    assert len(rope.points()) == 20 and not rope.broken
    assert [b.index for b in rope] == list(rope.bodies) and rope[3].index == 3


def test_rope_pinned_both_ends_sags_and_tears():
    w = World(iterations=8)
    rope = w.rope([(x, 0.0) for x in range(0, 101, 10)], pin_end=True)
    assert rope[0].radius == pytest.approx(2.5)  # default: a quarter of the segment
    w.run(3)
    mid = rope[5].position
    assert mid[1] > 0 and rope.end.position == (100, 0)

    w2 = World(iterations=8)
    weak = w2.rope([(x, 0.0) for x in range(0, 101, 10)], pin_end=True, tear=1.01)
    heavy = weak[5]
    w2.connect(heavy, w2.add_body(50, 10, 1, mass=1000))  # dead weight
    w2.run(2)
    assert weak.broken
    assert any(not link.alive for link in w2.constraints())

    with pytest.raises(ValueError):
        w2.rope([(0, 0)])


def test_connect_and_stiffness():
    w = World(gravity=(0, 0))
    a = w.add_body(0, 0, 1)
    b = w.add_body(30, 0, 1)
    i = w.connect(a, b, 10)
    assert i == 0 and w.constraints()[0] == Link(0, 1, 10, 1, 0, True)
    w.run(1)
    assert math.dist(a.position, b.position) == pytest.approx(10, abs=0.1)
    j = w.connect(0, 1, stiffness=5)  # current distance; stiffness clamped to 1
    assert w.constraints()[j].length == pytest.approx(10, abs=0.1)
    assert w.constraints()[j].stiffness == 1
    with pytest.raises(ValueError):
        w.connect(a, b, -5)
    with pytest.raises(IndexError):
        w.connect(a, 99)
    other = World()
    with pytest.raises(ValueError):
        w.connect(a, other.add_body(0, 0))


def test_cloth_grid_and_pins():
    w = World(iterations=6, damping=1)
    c = w.cloth(10, 8, 12, origin=(50, 20))
    assert isinstance(c, Cloth) and len(c) == 80
    assert len(c.links) == 9 * 8 + 10 * 7
    assert c.body(0, 0).position == (50, 20) and c.body(9, 7).position == (50 + 9 * 12, 20 + 7 * 12)
    assert all(c.body(col, 0).pinned for col in range(10)) and not c.body(0, 1).pinned
    w.run(5)
    for col in range(10):
        assert c.body(col, 0).position == (50 + col * 12, 20)
    # Hangs below the pinned row with links (nearly) at rest length.
    assert c.body(5, 7).y > 20 + 6 * 12
    for link in w.constraints():
        d = math.dist(w[link.a].position, w[link.b].position)
        assert d == pytest.approx(12, abs=2)
    with pytest.raises(IndexError):
        c.index(10, 0)

    sheared = World().cloth(3, 3, 10, shear=True, pin_top=False)
    assert len(sheared.links) == 12 + 8
    with pytest.raises(ValueError):
        World().cloth(1, 1, 10)
    with pytest.raises(ValueError):
        World().cloth(2, 2, 0)


def test_bindings_write_targets():
    class Sprite:
        x = 0.0
        y = 0.0
        rotation = 0.0

    w = World(bounds=(0, 0, 400, 300))
    s = Sprite()
    d = {}
    ball = w.add_body(100, 100, 10).bind(s)
    w.bind(w.add_body(200, 50, 10), d, x="left", y="top", rotation=None, offset=(-10, -10))
    assert (s.x, s.y) == (100, 100) and d == {"left": 190, "top": 40}
    w.step(1 / 60)
    assert s.y > 100 and d["top"] > 40
    ball.velocity = (200, 0)
    w.run(1)
    assert s.y == pytest.approx(290, abs=1) and s.x == pytest.approx(ball.x)
    assert s.rotation == pytest.approx(ball.rotation) and s.rotation != 0  # rolled
    w.unbind(s)
    before = s.x
    w.run(1)
    assert s.x == before
    w.unbind()
    sprites = [{} for _ in range(3)]
    w.bind_many(w.add_bodies([(10, 10), (40, 10), (70, 10)]), sprites, rotation=None)
    assert [sp["x"] for sp in sprites] == [10, 40, 70]
    w.unbind(w[-1])
    w.run(1)
    assert sprites[2]["y"] == 10 and sprites[0]["y"] > 10


def test_bind_many_matches_individual_bind_with_keys():
    pts = [(10, 20), (40, 50), (70, 80)]
    batch, individual = World(gravity=(0, 0)), World(gravity=(0, 0))
    batch_bodies = batch.add_bodies(pts)
    individual_bodies = individual.add_bodies(pts)
    for world in (batch, individual):
        for i, body in enumerate(world):
            body.velocity = (10 + i * 3, 0)
        world.step(0.25)

    batch_targets = [{} for _ in pts]
    individual_targets = [{} for _ in pts]
    keys = {"x": "left", "y": "top", "rotation": "angle", "offset": (2, -3)}
    batch.bind_many(batch_bodies, batch_targets, **keys)
    for body, target in zip(individual_bodies, individual_targets):
        individual.bind(body, target, **keys)

    assert batch_targets == individual_targets
    assert len(batch._bindings) == len(individual._bindings) == len(pts)


def test_bind_many_exports_world_output_once():
    class CountingNative:
        def __init__(self, native):
            self.native = native
            self.output_calls = 0

        def output(self):
            self.output_calls += 1
            return self.native.output()

        def __getattr__(self, name):
            return getattr(self.native, name)

    w = World(gravity=(0, 0))
    bodies = w.add_bodies([(float(i), 0) for i in range(50)])
    native = CountingNative(w._native)
    w._native = native

    w.bind_many(bodies, [{} for _ in bodies])

    assert native.output_calls == 1


def test_bind_many_stops_registering_after_target_setter_fails():
    class Target:
        pass

    class RaisingTarget:
        @property
        def x(self):
            raise AttributeError

        @x.setter
        def x(self, value):
            raise ValueError("x setter failed")

    w = World(gravity=(0, 0))
    bodies = w.add_bodies([(10, 20), (30, 40), (50, 60)])
    first, middle, third = Target(), RaisingTarget(), Target()

    with pytest.raises(ValueError, match="x setter failed"):
        w.bind_many(bodies, [first, middle, third])

    assert len(w._bindings) == 2
    assert not hasattr(third, "x") and not hasattr(third, "y")

    w.unbind(middle)
    w.sync()

    assert not hasattr(third, "x") and not hasattr(third, "y")


def test_bind_many_preserves_bindings_before_invalid_body():
    w = World(gravity=(0, 0))
    w.add_bodies([(10, 20), (30, 40), (50, 60)])
    targets = [{}, {}, {}]

    with pytest.raises(IndexError):
        w.bind_many([0, 1, 3], targets)

    assert len(w._bindings) == 2
    assert targets == [{"x": 10, "y": 20, "rotation": 0}, {"x": 30, "y": 40, "rotation": 0}, {}]


def test_run_on_frame_and_render():
    w = World()
    b = w.add_body(0, 0)
    seen = []
    assert w.run(0.5, fps=10, on_frame=lambda i, t: seen.append((i, round(t, 6)))) == 5
    assert seen == [(1, 0.1), (2, 0.2), (3, 0.3), (4, 0.4), (5, 0.5)]
    frames = list(w.render(fps=4, duration=1))
    assert [f for f, _ in frames] == [0, 1, 2, 3, 4] and frames[-1][1] == 1.0
    assert w.time == pytest.approx(1.5)
    assert w.run(0) == 0
    with pytest.raises(ValueError):
        w.run(1, fps=0)
    with pytest.raises(ValueError):
        w.step(-1)
    y = b.y
    w.step(0)
    assert b.y == y


def test_large_steps_are_subdivided():
    a, b = World(damping=0), World(damping=0)
    pa, pb = a.add_body(0, 0, 1), b.add_body(0, 0, 1)
    a.step(0.5)  # 10 native steps of 1/20 s
    for _ in range(10):
        b.step(0.05)
    assert pa.position == pytest.approx(pb.position)
    assert pa.velocity[1] == pytest.approx(490, rel=1e-3)


def test_deterministic():
    def sim():
        w = boxed()
        w.add_bodies([(50 + (i * 37) % 300, 20 + (i * 53) % 200, 5) for i in range(200)])
        w.run(2)
        return w.positions().tobytes()

    assert sim() == sim()


def test_big_world_steps_without_holding_the_gil():
    w = World(bounds=(0, 0, 3000, 3000))
    w.add_bodies([((i % 150) * 20 + 10, (i // 150) * 20 + 10, 6) for i in range(30000)])
    ticks = []
    done = threading.Event()

    def worker():
        w.run(1.0)
        done.set()

    t = threading.Thread(target=worker)
    t.start()
    while not done.is_set():
        ticks.append(1)  # the main thread keeps running while the world steps
        done.wait(0.001)
    t.join()
    assert len(ticks) > 1
    assert w.time == pytest.approx(1.0)


def test_exported_names():
    for name in ("World", "Body", "Rope", "Cloth", "Link", "Attractor"):
        assert name in anikit.__all__ and hasattr(anikit, name)
