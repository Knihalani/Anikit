import math

import pytest

import anikit
from anikit import Noise


def test_deterministic_and_seeded():
    a, b, c = Noise(42), Noise(42), Noise(7)
    xs = [i * 0.137 for i in range(200)]
    va = [a.noise2(x, x * 0.7) for x in xs]
    assert va == [b.noise2(x, x * 0.7) for x in xs]
    assert va != [c.noise2(x, x * 0.7) for x in xs]
    assert a.seed == 42 and repr(a) == "Noise(seed=42)"
    assert Noise().seed == 0
    assert Noise(-1).seed == 2**32 - 1  # seeds wrap modulo 2**32
    assert Noise(2**32 + 5).noise2(0.3, 0.4) == Noise(5).noise2(0.3, 0.4)


def test_range_and_smoothness():
    n = Noise(1)
    lo = hi = 0.0
    for i in range(3000):
        x = i * 0.031
        v2 = n.noise2(x, -x * 0.5)
        v3 = n.noise3(x, x * 0.3, -x)
        assert -1.0 <= v2 <= 1.0 and -1.0 <= v3 <= 1.0
        # Continuity: a tiny step gives a tiny change.
        assert abs(n.noise2(x + 1e-4, -x * 0.5) - v2) < 0.01
        assert abs(n.noise3(x, x * 0.3, -x + 1e-4) - v3) < 0.01
        lo, hi = min(lo, v2), max(hi, v2)
    assert lo < -0.5 and hi > 0.5
    # Simplex noise is zero at lattice points.
    assert n.noise2(0, 0) == 0.0
    assert n.noise3(0, 0, 0) == 0.0


def test_fbm():
    n = Noise(3)
    for i in range(500):
        x = i * 0.05
        assert -1.0 <= n.fbm2(x, 0.3) <= 1.0
        assert -1.0 <= n.fbm3(x, 0.3, 0.9, octaves=6) <= 1.0
    # One octave is plain noise.
    assert n.fbm2(0.37, 0.81, octaves=1) == pytest.approx(n.noise2(0.37, 0.81))
    assert n.fbm3(0.37, 0.81, 0.2, octaves=1) == pytest.approx(n.noise3(0.37, 0.81, 0.2))
    # More octaves add detail (higher-frequency changes).
    rough = sum(abs(n.fbm2(i * 0.01 + 0.01, 0.5, octaves=8) - n.fbm2(i * 0.01, 0.5, octaves=8)) for i in range(500))
    smooth = sum(abs(n.fbm2(i * 0.01 + 0.01, 0.5, octaves=1) - n.fbm2(i * 0.01, 0.5, octaves=1)) for i in range(500))
    assert rough > smooth
    with pytest.raises(ValueError):
        n.fbm2(0, 0, octaves=0)
    with pytest.raises(ValueError):
        n.fbm3(0, 0, 0, gain=0)
    with pytest.raises(ValueError):
        n.fbm2(0, 0, lacunarity=math.inf)


def test_vectorised_helpers_match_scalar_calls():
    n = Noise(9)
    xs = [i * 0.1 for i in range(50)]
    ys = [i * -0.07 for i in range(50)]
    zs = [i * 0.02 for i in range(50)]
    assert n.noise2_many(xs, ys) == [n.noise2(x, y) for x, y in zip(xs, ys)]
    assert n.noise3_many(xs, ys, zs) == [n.noise3(x, y, z) for x, y, z in zip(xs, ys, zs)]
    assert n.fbm2_many(xs, ys, octaves=3) == [n.fbm2(x, y, octaves=3) for x, y in zip(xs, ys)]
    assert n.fbm3_many(xs, ys, zs, 5, 2.5, 0.4) == [n.fbm3(x, y, z, 5, 2.5, 0.4) for x, y, z in zip(xs, ys, zs)]
    # Scalars broadcast; tuples, ranges and generators are accepted.
    assert n.noise2_many(xs, 0.5) == [n.noise2(x, 0.5) for x in xs]
    assert n.noise3_many(0.1, (0.2, 0.3), 1.0) == [n.noise3(0.1, 0.2, 1.0), n.noise3(0.1, 0.3, 1.0)]
    assert n.noise2_many(range(3), (x / 2 for x in range(3))) == [n.noise2(i, i / 2) for i in range(3)]
    assert n.noise2_many(0.3, 0.4) == [n.noise2(0.3, 0.4)]
    assert n.noise2_many([], []) == []
    with pytest.raises(ValueError):
        n.noise2_many([1.0, 2.0], [1.0])


def test_grid():
    n = Noise(5)
    g = n.grid(4, 3, x0=1.0, y0=2.0, step=0.25)
    assert len(g) == 3 and all(len(row) == 4 for row in g)
    assert g[2][3] == n.noise2(1.0 + 3 * 0.25, 2.0 + 2 * 0.25)
    g3 = n.grid(2, 2, z=0.7, octaves=3)
    assert g3[1][1] == pytest.approx(n.fbm3(0.1, 0.1, 0.7, octaves=3))
    assert n.grid(2, 2, octaves=2)[0][1] == pytest.approx(n.fbm2(0.1, 0.0, octaves=2))
    assert n.grid(2, 1, z=0.3)[0][1] == pytest.approx(n.noise3(0.1, 0.0, 0.3))
    assert n.grid(0, 0) == []
    with pytest.raises(ValueError):
        n.grid(-1, 2)


def test_noise_drives_animation(engine):
    # Typical use: wandering motion from noise sampled along time.
    n = Noise(11)
    o = {"x": 0.0}
    anim = engine.animate(o, {"x": 100}, duration=1, ease=lambda t: t + 0.05 * n.noise2(t * 4, 0.0))
    engine.tick(0.5)
    assert o["x"] != 50 and 40 < o["x"] < 60
    engine.run_until_idle()
    assert anim.finished


def test_native_and_types():
    n = Noise(1)
    assert isinstance(n.native, anikit._anikit.NativeNoise)
    with pytest.raises(TypeError):
        Noise(1.5)  # type: ignore[arg-type]
    with pytest.raises(TypeError):
        n.noise2_many(["a"], 0.0)  # type: ignore[list-item]
