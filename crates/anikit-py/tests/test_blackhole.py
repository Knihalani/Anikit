import math

import pytest

import anikit


def test_renders_shadow_disk_and_reuses_geometry():
    bh = anikit.BlackHole(160, 100)
    frame = bh.render(distance=30, inclination=80, fov=40, time=1.0)
    assert len(frame) == 160 * 100 * 4
    s = bh.stats
    assert s["geometry_built"]
    assert s["samples"][0] > 1000 and s["samples"][1] > 50
    exact = math.asin(math.sqrt(27) / 30 * math.sqrt(1 - 2 / 30))
    assert abs(s["shadow_radius"] - exact) < 2e-3
    c = (36 * 160 + 80) * 4
    assert frame[c] < 15
    bh.render(distance=30, inclination=80, fov=40, time=1.1)
    assert not bh.stats["geometry_built"]


def test_cinematic_inside_horizon_and_resize():
    bh = anikit.BlackHole(64, 48, mode="cinematic")
    frame = bh.render(distance=1.5, fall=1, look=180, fov=90)
    assert any(frame[i] > 30 for i in range(0, len(frame), 4))
    bh.resize(32, 24)
    assert bh.size == (32, 24)
    assert len(bh.render()) == 32 * 24 * 4
    with pytest.raises(ValueError):
        anikit.BlackHole(8, 8, mode="nope")
