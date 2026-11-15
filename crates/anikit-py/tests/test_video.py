"""Film grade and frame utilities (anikit.video)."""

import pytest

np = pytest.importorskip("numpy")

from anikit import raster, video  # noqa: E402
from anikit.transform import Camera  # noqa: E402


def frame(w=64, h=48, value=40):
    f = np.full((h, w, 4), value, np.uint8)
    f[..., 3] = 255
    return f


def bright_spot(w=64, h=48):
    f = frame(w, h, 10)
    f[h // 2 - 2 : h // 2 + 2, w // 2 - 2 : w // 2 + 2, :3] = 255
    return f


def test_no_options_is_a_no_op():
    f = bright_spot()
    before = f.copy()
    assert video.post(f) is f
    assert np.array_equal(f, before)


def test_bloom_streaks_rays_spread_light():
    for opts in ({"bloom": 1.5, "bloom_threshold": 0.5, "bloom_radius": 8}, {"streaks": 1.5, "streak_length": 40}, {"rays": 1.5, "rays_center": (0.5, 0.5)}):
        f = bright_spot()
        before = f.copy()
        video.post(f, **opts)
        gain = f[..., :3].astype(int) - before[..., :3]
        assert gain.min() >= -1 and gain.max() > 5, opts
    # Streaks are horizontal: more light beside the spot than above it.
    f = bright_spot()
    video.post(f, streaks=2, streak_length=40)
    assert f[24, 32 + 12, :3].sum() > f[24 + 12, 32, :3].sum()


def test_grade_vignette_aberration_zoom():
    f = frame(value=100)
    video.post(f, vignette=0.8)
    assert f[0, 0, 0] < f[24, 32, 0]
    f = frame(value=100)
    video.post(f, exposure=1.5, contrast=1.0)
    assert f[10, 10, 0] > 100
    g = frame(value=100)
    g[..., 0] = 200
    video.post(g, saturation=0.0)
    assert abs(int(g[5, 5, 0]) - int(g[5, 5, 1])) < 4
    s = frame(value=100)
    video.post(s, shadows=(0, 0, 0.2))
    assert s[5, 5, 2] > s[5, 5, 0]
    stripes = frame(value=0)
    stripes[:, ::4, :3] = 255
    a = stripes.copy()
    video.post(a, aberration=4)
    assert not np.array_equal(a, stripes)
    z = stripes.copy()
    video.post(z, zoom_blur=0.3, zoom_center=(0.5, 0.5))
    assert z[0, 1, 0] > 0  # dark columns pick up light along the rays


def test_grain_is_seeded():
    a, b, c = frame(), frame(), frame()
    video.post(a, grain=8, seed=3)
    video.post(b, grain=8, seed=3)
    video.post(c, grain=8, seed=4)
    assert np.array_equal(a, b) and not np.array_equal(a, c)
    assert not np.array_equal(a, frame())


def test_postfx_options_overrides_and_time_functions():
    fx = video.PostFx(vignette=lambda t: 0.9 if t > 1 else 0.0)
    a = fx(frame(value=100), t=0.5)
    assert np.array_equal(a, frame(value=100))
    b = fx.apply(frame(value=100), t=2)
    assert b[0, 0, 0] < 100
    c = fx.apply(frame(value=100), t=2, vignette=0)
    assert np.array_equal(c, frame(value=100))
    with pytest.raises(TypeError, match="pass t="):
        fx.apply(frame())
    with pytest.raises(TypeError, match="did you mean 'bloom'"):
        video.PostFx(blom=1)
    with pytest.raises(ValueError):
        video.post(frame(), shadows=(1, 2))
    assert "vignette" in repr(fx)


def test_render_then_grade_in_place_without_copy():
    r3 = raster.Renderer3D(80, 60)
    cam = Camera((0, 0, 600))
    f = r3.render(cam, [raster.Item(raster.cube(), raster.instances(1, scale=200, ry=30, rx=20), material={"emissive": 1.5})])
    ptr = f.__array_interface__["data"][0]
    assert video._frame_bytearray(f) is not None
    before = f.copy()
    out = video.post(f, bloom=1.2, bloom_threshold=0.3, vignette=0.5)
    assert out is f and f.__array_interface__["data"][0] == ptr
    assert not np.array_equal(f, before)
    # Same result as grading a foreign copy (copy-in / copy-out path).
    g = before.copy()
    assert video._frame_bytearray(g) is None
    video.post(g, bloom=1.2, bloom_threshold=0.3, vignette=0.5)
    assert np.array_equal(f, g)


def test_frame_validation_and_views():
    with pytest.raises(ValueError):
        video.post(np.zeros((4, 4, 3), np.uint8))
    with pytest.raises(ValueError):
        video.post(np.zeros((4, 4, 4), np.float32))
    with pytest.raises(TypeError):
        video.post(b"\0" * 64)
    ro = frame()
    ro.flags.writeable = False
    with pytest.raises(ValueError, match="read-only"):
        video.post(ro, vignette=0.5)
    # A non-contiguous view is graded and written back through the view.
    big = frame(128, 48, 100)
    view = big[:, ::2]
    video.post(view, vignette=0.9)
    assert big[0, 0, 0] < 100 and big[0, 1, 0] == 100


def test_rgba_to_i420():
    white = frame(4, 2, 255)
    yuv = video.rgba_to_i420(white)
    assert yuv.shape == (4 * 2 + 2 * 2 * 1,)
    assert list(yuv[:8]) == [235] * 8 and list(yuv[8:]) == [128] * 4
    black = video.rgba_to_i420(frame(4, 2, 0))
    assert list(black[:8]) == [16] * 8
    red = frame(2, 2, 0)
    red[..., 0] = 255
    blue = frame(2, 2, 0)
    blue[..., 2] = 255
    assert np.array_equal(video.rgba_to_i420(red), video.rgba_to_i420(blue, order="bgra"))
    with pytest.raises(ValueError):
        video.rgba_to_i420(frame(3, 2))
    with pytest.raises(ValueError):
        video.rgba_to_i420(frame(2, 2), order="argb")


def test_motion_blur_averages():
    mb = video.MotionBlur(8, 4)
    assert mb.add(frame(8, 4, 0)) == 1
    assert mb.add(frame(8, 4, 100)) == 2
    out = mb.resolve()
    assert out.shape == (4, 8, 4) and out[0, 0, 0] == 50 and out.flags.writeable
    assert mb.count == 0
    with pytest.raises(ValueError):
        mb.add(frame(4, 4))
