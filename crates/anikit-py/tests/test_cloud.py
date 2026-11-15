import math
from array import array

import anikit
import pytest
from anikit import PointCloud, Splat
from anikit.transform import Camera, Projection, project_points

N = 300


def grid(n=N, z=0.0, step=3.0):
    return [((i % 20) * step, (i // 20) * step, z) for i in range(n)]


def ring(i, n, r=50.0):
    a = 2 * math.pi * i / n
    return (r * math.cos(a), r * math.sin(a), 10.0)


def calm(n=N, **kw):
    """A cloud without turbulence: grid (shape 0) and ring (shape 1)."""
    kw.setdefault("noise", 0)
    kw.setdefault("swirl", 0)
    return PointCloud(n, shapes=[grid(n), ring], **kw)


def max_err(cloud, k):
    return max(abs(a - b) for a, b in zip(cloud.positions, cloud.get_shape(k)))


# ---------------------------------------------------------------------------
# PointCloud: construction, parameters, shapes
# ---------------------------------------------------------------------------


def test_defaults_and_live_params():
    c = PointCloud(10)
    assert len(c) == c.count == 10
    assert c.shape_count == 0
    assert (c.stiffness, c.damping, c.spread, c.noise, c.swirl) == (60, 9, pytest.approx(0.6), 2, 40)
    assert c.frequency == pytest.approx(0.01)
    c.set(stiffness=80, damping=5).noise = 0.5
    c.spread = -1  # clamped
    assert (c.stiffness, c.damping, c.noise, c.spread) == (80, 5, 0.5, 0)
    with pytest.raises(ValueError):
        c.stiffness = float("nan")
    with pytest.raises(ValueError):
        c.swirl = float("inf")
    with pytest.raises(ValueError):
        PointCloud(-1)
    assert "PointCloud(10 points" in repr(c)
    assert "NativeCloud(points=10" in repr(c.native)


def test_starts_snapped_on_first_shape_at_rest():
    c = calm()
    assert c.shape_count == 2
    assert max_err(c, 0) == 0
    assert c.speed == 0


@pytest.mark.parametrize(
    "shape",
    [
        [(1.0, 2.0, 3.0), (4.0, 5.0, 6.0)],
        [1.0, 2.0, 3.0, 4.0, 5.0, 6.0],
        array("f", [1, 2, 3, 4, 5, 6]),
        array("d", [1, 2, 3, 4, 5, 6]),
        array("f", [1, 2, 3, 4, 5, 6]).tobytes(),
        lambda i, n: (1 + 3 * i, 2 + 3 * i, 3 + 3 * i),
        (p for p in [(1, 2, 3), (4, 5, 6)]),
    ],
)
def test_shape_inputs(shape):
    c = PointCloud(2, shapes=[shape])
    assert c.get_shape(0).tolist() == [1, 2, 3, 4, 5, 6]
    assert c.positions.tolist() == [1, 2, 3, 4, 5, 6]


def test_shape_2d_points_and_replacement():
    c = PointCloud(2, shapes=[[(1, 2), (3, 4)]])
    assert c.get_shape(0).tolist() == [1, 2, 0, 3, 4, 0]
    c.shape(0, lambda i, n: (i, i))
    assert c.get_shape(0).tolist() == [0, 0, 0, 1, 1, 0]
    c.shape(1, [0] * 6)  # appending the next one is fine
    assert c.shape_count == 2


def test_shape_validation():
    c = PointCloud(2, shapes=[[0] * 6])
    with pytest.raises(ValueError, match="needs 6 values"):
        c.shape(0, [1, 2, 3])
    with pytest.raises(ValueError, match="finite"):
        c.shape(0, [0, 0, 0, 0, float("nan"), 0])
    with pytest.raises(IndexError, match="in order"):
        c.shape(5, [0] * 6)
    with pytest.raises(ValueError):
        c.shape(0, lambda i, n: (1,))
    with pytest.raises(ValueError):
        c.shape(0, b"\x00" * 7)
    with pytest.raises(IndexError):
        c.get_shape(3)
    with pytest.raises(IndexError):
        c.snap(1)
    with pytest.raises(IndexError):
        c.morph(2)
    with pytest.raises(IndexError):
        c.step(0.1, to=4)
    with pytest.raises(ValueError):
        c.step(-1)
    with pytest.raises(ValueError):
        c.step(float("nan"))


def test_empty_and_shapeless_clouds_are_harmless():
    e = PointCloud(0, shapes=[[]])
    e.step(0.1).explode(0, 0, 0, 100)
    assert len(e.positions) == 0 and e.speed == 0
    c = PointCloud(5)
    c.step(0.5).run(1)
    assert c.positions.tolist() == [0.0] * 15


# ---------------------------------------------------------------------------
# PointCloud: simulation
# ---------------------------------------------------------------------------


def test_settles_onto_target_shape():
    c = calm()
    c.step(1 / 60, 0, 1, 1.0)
    assert c.speed > 0
    for _ in range(900):
        c.step(1 / 60)
    assert (c.from_, c.to, c.mix) == (0, 1, 1.0)
    assert max_err(c, 1) < 0.05
    assert c.speed < 0.01
    assert c.time == pytest.approx(901 / 60)


def test_resting_turbulence_keeps_particles_near_shape():
    c = PointCloud(N, shapes=[grid()], noise=2.0)
    c.run(5)
    assert 0 < max_err(c, 0) < 2.0 * 1.5
    assert c.speed > 0


def test_morph_with_duration_drives_mix():
    c = calm()
    c.morph(1, duration=1.0)
    assert c.morphing and (c.from_, c.to, c.mix) == (0, 1, 0.0)
    c.run(0.5)
    assert c.mix == pytest.approx(0.5, abs=0.02)
    c.run(0.6)
    assert not c.morphing and c.mix == 1.0
    c.run(10)
    assert max_err(c, 1) < 0.05
    # Next morph starts from the shape the cloud is closest to.
    c.morph(0, duration=0.5, ease="inOutCubic")
    assert (c.from_, c.to) == (1, 0)
    c.run(10)
    assert max_err(c, 0) < 0.05


def test_morph_with_custom_ease_and_zero_duration():
    c = calm()
    c.morph(1, duration=1.0, ease=lambda t: t * t)
    c.run(0.5)
    assert c.mix == pytest.approx(0.25, abs=0.02)
    c.morph(0, duration=0)
    assert c.mix == 1.0 and not c.morphing
    with pytest.raises(ValueError):
        c.morph(0, duration=-1)


def test_explicit_mix_cancels_morph_and_scrubs():
    c = calm(spread=0)
    c.morph(1, duration=5)
    c.step(1 / 60, mix=0.5)
    assert not c.morphing
    c.run(10)
    # spread 0: every particle sits at the eased midpoint (inOutCubic(0.5) = 0.5).
    a, b = c.get_shape(0), c.get_shape(1)
    mid = [(x + y) / 2 for x, y in zip(a, b)]
    assert max(abs(p - q) for p, q in zip(c.positions, mid)) < 0.05


def test_large_dt_is_split_not_clamped():
    a, b = calm(), calm()
    a.step(1.0, 0, 1, 1.0)
    b.step(0, 0, 1, 1.0)
    b.run(1.0, fps=20)
    assert a.positions == b.positions
    assert a.time == b.time == pytest.approx(1.0)


def test_deterministic_per_seed():
    def run(seed):
        c = calm(seed=seed)
        c.morph(1, duration=1).run(0.5)
        return c.positions

    assert run(3) == run(3)
    assert run(3) != run(4)


def test_explode_raises_speed_then_returns_home():
    c = calm()
    c.explode(0, 0, -10, 500)
    v0 = c.speed
    assert v0 > 10
    pos = c.positions
    c.run(15)
    assert c.positions != pos
    assert max_err(c, 0) < 0.05 and c.speed < 0.01
    # Negative strength pulls in (opposite velocity direction).
    p = calm()
    q = calm()
    p.explode(30, 30, 0, 200).step(1 / 60)
    q.explode(30, 30, 0, -200).step(1 / 60)
    dp = [x - y for x, y in zip(p.positions, p.get_shape(0))]
    dq = [x - y for x, y in zip(q.positions, q.get_shape(0))]
    assert sum(x * y for x, y in zip(dp, dq)) < 0
    with pytest.raises(ValueError):
        c.explode(float("nan"))


def test_snap_cancels_morph():
    c = calm()
    c.morph(1, duration=1).run(0.3)
    c.snap(1)
    assert (c.from_, c.to, c.mix, c.morphing) == (1, 1, 0.0, False)
    assert max_err(c, 1) == 0 and c.speed == 0


# ---------------------------------------------------------------------------
# Splat
# ---------------------------------------------------------------------------

BG = bytes([0x10, 0x20, 0x30, 255])


def test_background_frame():
    s = Splat(6, 4, background="#102030")
    assert s.size == (6, 4) and (s.width, s.height) == (6, 4)
    frame = s.resolve()
    assert isinstance(frame, bytes) and len(frame) == 6 * 4 * 4
    assert frame == BG * 24
    assert s.rgba == frame
    assert s.pixel(5, 3) == (0x10, 0x20, 0x30, 255)
    assert Splat(1, 1, background=0x102030).resolve() == BG
    assert Splat(1, 1, background=(16, 32, 48)).resolve() == BG
    assert Splat(1, 1).resolve(background="red") == bytes([255, 0, 0, 255])
    with pytest.raises(ValueError):
        Splat(1, 1, background=0x1000000)
    with pytest.raises(ValueError):
        Splat(1, 1, background="not a colour")
    assert "Splat(6x4" in repr(s)


def test_bilinear_splat_conserves_energy():
    s = Splat(16, 16)
    s.draw([8.25, 8.75, 10.0, 0.5, 3.6, 2.1, 10.0, 0.5], intensity=1.0)
    r, g, b = s.total_light
    assert r == pytest.approx(2.0, abs=1e-5) and g == pytest.approx(2.0, abs=1e-5) and b == pytest.approx(2.0, abs=1e-5)
    # Sub-pixel placement: the light's centroid is the point (pixel centres at +0.5).
    # (the second point only lights columns 3-4, the first columns 7-8)
    cx = sum((x + 0.5) * s.light(x, y)[0] for x in range(7) for y in range(16))
    assert cx == pytest.approx(3.6, abs=1e-4)


def test_disc_points_spread_light():
    s = Splat(32, 32)
    s.draw([16, 16, 1, 4.0], size=1.0, intensity=1.0)
    lit = sum(1 for x in range(32) for y in range(32) if s.light(x, y)[0] > 0)
    assert lit > 9
    assert s.light(15, 15)[0] > s.light(18, 15)[0] > 0


def test_offscreen_culled_and_nan_points_add_no_light():
    s = Splat(8, 8)
    s.draw(
        [
            -5,
            3,
            1,
            0.5,
            3,
            3,
            1,
            0.0,
            100,
            100,
            1,
            9,
            float("nan"),
            3,
            1,
            0.5,
            3,
            3,
            1,
            float("nan"),
            3,
            3,
            1,
            math.inf,
        ],
    )
    assert s.total_light == (0, 0, 0)
    # A large disc straddling the edge is clipped, not dropped.
    s.draw([7.0, 4.0, 1.0, 9.0])
    assert s.total_light[0] > 0
    assert all(b == 255 for b in s.resolve()[3::4])


def test_tone_mapping_saturates_and_respects_exposure():
    s = Splat(8, 8)
    s.draw([3.5, 3.5, 1, 0.5], intensity=1e4)
    s.resolve()
    assert s.pixel(3, 3) == (255, 255, 255, 255)
    dim, bright = Splat(8, 8), Splat(8, 8, exposure=4.0)
    for sp in (dim, bright):
        sp.draw([3.5, 3.5, 1, 0.5], intensity=0.2)
        sp.resolve()
    assert 0 < dim.pixel(3, 3)[0] < bright.pixel(3, 3)[0] < 255
    # Every channel stays within [background, 255].
    s = Splat(8, 8, background="#808080")
    s.draw([2.5, 2.5, 1, 0.5], intensity=1e6)
    s.draw([5.5, 5.5, 1, 0.5], intensity=1e-3)
    frame = s.resolve()
    assert min(frame) >= 0x80 and max(frame) == 255


def test_trails_and_fade():
    s = Splat(8, 8)
    s.draw([3.5, 3.5, 1, 0.5], intensity=1.0)
    s.resolve()  # trail 0: frame drawn, light cleared
    assert s.total_light == (0, 0, 0) and s.pixel(3, 3)[0] > 0
    s.trail = 0.5
    s.draw([3.5, 3.5, 1, 0.5], intensity=1.0)
    s.resolve()
    assert s.total_light[0] == pytest.approx(0.5, abs=1e-3)
    s.resolve(keep=1.0)
    assert s.total_light[0] == pytest.approx(0.5, abs=1e-3)
    s.fade(0.5)
    assert s.total_light[0] == pytest.approx(0.25, abs=1e-3)
    s.clear()
    assert s.total_light == (0, 0, 0)


def test_colors():
    s = Splat(8, 8)
    pts = [2.5, 2.5, 1, 0.5, 5.5, 5.5, 1, 0.5]
    s.colors([(1, 0, 0), "#00f"]).draw(pts, intensity=1)
    r, g, b = s.total_light
    assert (r, g, b) == (pytest.approx(1), 0, pytest.approx(1))
    s.clear().colors(lambda i: (0, 1, 0), count=2).draw(pts, intensity=1)
    assert s.total_light == (0, pytest.approx(2), 0)
    s.clear().colors(array("f", [0, 0, 1, 0, 0, 1])).draw(pts, intensity=1)
    assert s.total_light == (0, 0, pytest.approx(2))
    # Colours that don't cover every point are ignored (white).
    s.clear().colors([1, 0, 0]).draw(pts, intensity=1)
    assert s.total_light == (pytest.approx(2), pytest.approx(2), pytest.approx(2))
    s.clear().colors(None).draw(pts, intensity=1)
    assert s.native.color_count() == 0
    with pytest.raises(TypeError):
        s.colors(lambda i: (1, 1, 1))
    with pytest.raises(ValueError):
        s.colors([1, 0])


def test_fog_and_lens():
    s = Splat(16, 16)
    s.draw([4.5, 4.5, 50, 0.5, 10.5, 10.5, 150, 0.5], intensity=1, fog=(0, 100))
    assert s.light(4, 4)[0] > 0
    assert all(s.light(x, y)[0] == 0 for x in (10, 11) for y in (10, 11))
    with pytest.raises(ValueError):
        s.draw([], fog=(1,))

    def centroid(sp):
        m = t = 0.0
        for x in range(64):
            for y in range(8):
                v = sp.light(x, y)[0]
                m += x * v
                t += v
        return m / t

    a, b = Splat(64, 8), Splat(64, 8)
    a.draw([30, 4, 1, 0.5])
    b.draw([30, 4, 1, 0.5], lens=(26, 4, 20, 10))
    assert centroid(b) > centroid(a) + 3
    with pytest.raises(ValueError):
        b.draw([], lens=(1,))


def test_draw_validation_and_resize():
    s = Splat(4, 4)
    with pytest.raises(ValueError):
        s.draw([1, 2, 3])
    with pytest.raises(ValueError):
        s.draw([1, 1, 1, 1], scale=0)
    with pytest.raises(IndexError):
        s.light(4, 0)
    with pytest.raises(IndexError):
        s.pixel(0, 4)
    s.draw([1.5, 1.5, 1, 0.5]).resize(10, 3)
    assert s.size == (10, 3) and s.total_light == (0, 0, 0)
    assert len(s.resolve()) == 120
    assert Splat(0, 0).size == (1, 1)


# ---------------------------------------------------------------------------
# Cloud -> project -> splat
# ---------------------------------------------------------------------------


def lit_pixels(frame, bg=b"\x00\x00\x00"):
    return sum(1 for k in range(0, len(frame), 4) if frame[k : k + 3] != bg)


def test_render_cloud_end_to_end():
    c = PointCloud(2000, shapes=[ring, lambda i, n: (i - n / 2, 0, 0)], noise=0)
    cam = Camera(position=(0, 0, 400), fov=40)
    s = Splat(120, 80)
    frame = s.render(c, camera=cam, size=1.5, intensity=0.5)
    assert len(frame) == 120 * 80 * 4
    ring_lit = lit_pixels(frame)
    assert ring_lit > 50
    # The ring is centred: the middle of the frame stays dark.
    assert s.pixel(60, 40)[:3] == (0, 0, 0)
    c.morph(1, duration=0.5).run(5)
    frame = s.render(c, camera=cam, size=1.5, intensity=0.5)
    # Now a horizontal line through the centre.
    assert s.pixel(60, 40)[0] > 0
    assert s.pixel(60, 10)[:3] == (0, 0, 0)


def test_manual_projection_pipeline_matches_render():
    c = calm()
    cam = Camera(position=(0, 0, 300))
    proj = project_points(c.positions, camera=cam, size=(64, 48), sort=False)
    assert isinstance(proj, Projection)
    a, b, d = Splat(64, 48), Splat(64, 48), Splat(64, 48)
    a.draw(proj)
    fa = a.resolve()
    fb = b.render(c, camera=cam)
    fc = d.render(proj)
    assert fa == fb == fc
    assert lit_pixels(fa) > 0
    # Raw points (no cloud) work too.
    assert Splat(64, 48).render(grid(), camera=cam) == fa


def test_trail_frames_accumulate_motion():
    c = calm()
    c.morph(1, duration=0.5)
    cam = Camera(position=(0, 0, 300))
    plain, trail = Splat(64, 48), Splat(64, 48, trail=0.9)
    for _ in range(20):
        c.step(1 / 30)
        f1 = plain.render(c, camera=cam)
        f2 = trail.render(c, camera=cam)
    assert lit_pixels(f2) > lit_pixels(f1)


def test_exported_from_package():
    assert anikit.PointCloud is PointCloud and anikit.Splat is Splat
    assert {"PointCloud", "Splat", "ShapeInput"} <= set(anikit.__all__)
