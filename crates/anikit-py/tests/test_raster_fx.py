"""Renderer passes and options added after the first raster release: tubes,
line patterns, ground fog, skybox, shadows, depth of field, camera motion
blur, the light-last pass order, and the grade's edge antialiasing."""

import math

import pytest

np = pytest.importorskip("numpy")

import anikit  # noqa: E402
from anikit import raster, video  # noqa: E402
from anikit.transform import Camera  # noqa: E402

W, H = 96, 64


def cam(z=600.0, **kw):
    return Camera((0, 0, z), (0, 0, 0), fov=50, **kw)


def lum(frame):
    return frame[..., :3].astype(np.float64) @ np.array([0.2126, 0.7152, 0.0722])


# ---------------------------------------------------------------------------
# Tubes and flight path sampling
# ---------------------------------------------------------------------------


def test_tube_counts_and_uvs():
    pts = [(i * 20.0, math.sin(i * 0.4) * 30, 0.0) for i in range(12)]
    t = raster.tube(pts, radius=5, sides=8)
    assert t.triangle_count == (12 - 1) * 8 * 2
    assert t.vertex_count == 12 * (8 + 1)
    uv = t.uvs
    assert uv[0, 0] == 0.0 and uv[-1, 0] == pytest.approx(1.0)
    # Every ring is `radius` away from its path point.
    rings = t.positions.reshape(12, 9, 3)
    d = np.linalg.norm(rings - np.asarray(pts, np.float32)[:, None, :], axis=-1)
    assert np.allclose(d, 5, atol=1e-3)
    # Flat and (n, 3) inputs give the same mesh; default sides is 8.
    flat = raster.tube(np.asarray(pts).reshape(-1), radius=5)
    assert np.array_equal(flat.positions, t.positions)
    assert raster.tube(pts, sides=3).triangle_count == 11 * 3 * 2


def test_tube_taper_and_radii():
    pts = np.stack([np.linspace(0, 300, 7), np.zeros(7), np.zeros(7)], axis=1)
    t = raster.tube(pts, radius=10, sides=6, taper=(0, 1))
    rings = t.positions.reshape(7, 7, 3)
    r = np.linalg.norm(rings - pts[:, None, :].astype(np.float32), axis=-1).mean(axis=1)
    k = np.arange(7) / 6
    assert np.allclose(r, 10 * k * k * (3 - 2 * k), atol=1e-3)  # smoothstep, as in TypeScript
    # radii override taper.
    t2 = raster.tube(pts, radius=10, sides=6, radii=[2] * 7, taper=(0, 1))
    r2 = np.linalg.norm(t2.positions.reshape(7, 7, 3) - pts[:, None, :].astype(np.float32), axis=-1)
    assert np.allclose(r2, 20, atol=1e-3)
    with pytest.raises(ValueError, match="radii"):
        raster.tube(pts, radii=[1, 2])
    with pytest.raises(ValueError, match="at least 2"):
        raster.tube([(0, 0, 0)])
    with pytest.raises(ValueError, match="finite"):
        raster.tube([(0, 0, 0), (math.nan, 0, 0)])


def test_flight_path_points_feed_a_tube():
    path = anikit.flight_path([(0, 0, 0), (400, 100, -200), (800, 0, -600)])
    pts = path.points(50)
    assert len(pts) == 50
    assert pts[0] == pytest.approx(path.point(0))
    assert pts[-1] == pytest.approx(path.point(path.length))
    gaps = np.linalg.norm(np.diff(np.asarray(pts), axis=0), axis=1)
    assert gaps.max() / gaps.min() < 1.05  # evenly spaced by arc length
    part = path.points(3, from_=100, to=300)
    assert part == [pytest.approx(path.point(s)) for s in (100, 200, 300)]
    assert len(path.points(0)) == 2 and len(path.points(1)) == 2
    assert path.control_points[1] == (400.0, 100.0, -200.0)
    trail = raster.tube(pts, radius=8, sides=6, taper=(0, 1))
    assert trail.triangle_count == 49 * 6 * 2
    r3 = raster.Renderer3D(W, H)
    frame = r3.render(
        Camera((400, 300, 900), (400, 0, -300), fov=60),
        [raster.Item(trail, blend="additive", material=raster.Material(emissive=1, pulse=2, pulse_phase=0.3))],
    )
    assert frame[..., :3].any()


# ---------------------------------------------------------------------------
# Material patterns and ground fog
# ---------------------------------------------------------------------------


def test_line_patterns_render_and_differ():
    r3 = raster.Renderer3D(W, H)
    wall = raster.instances(1, rx=90, scale=900, color=(0.05, 0.05, 0.06))
    frames = {}
    for p in raster.PATTERNS:
        mat = raster.Material(lines=1.5, line_color=(0.2, 0.9, 1.4), grid=(10, 10), line_width=0.08, pattern=p)
        frames[p] = r3.render(cam(), [raster.Item(raster.plane(), wall, mat, cull=False)])
    plain = r3.render(cam(), [raster.Item(raster.plane(), wall, {"pattern": "hex"}, cull=False)])
    for p, f in frames.items():
        assert (lum(f) > lum(plain) + 20).sum() > 50, p  # the pattern lights up
    names = list(frames)
    for i, a in enumerate(names):
        for b in names[i + 1 :]:
            assert not np.array_equal(frames[a], frames[b]), (a, b)
    assert (
        frames["grid"].tobytes()
        == r3.render(
            cam(),
            [
                raster.Item(
                    raster.plane(),
                    wall,
                    dict(lines=1.5, line_color=(0.2, 0.9, 1.4), grid=(10, 10), line_width=0.08),
                    cull=False,
                )
            ],
        ).tobytes()
    )  # grid is the default
    with pytest.raises(ValueError, match="pattern"):
        r3.render(cam(), [raster.Item(raster.cube(), material={"pattern": "stripes"})])


def test_ground_fog_tints_low_geometry():
    r3 = raster.Renderer3D(W, H)
    c = Camera((0, 300, 900), (0, -100, 0), fov=60)
    floor = raster.Item(raster.plane(8), raster.instances(1, y=-200, scale=3000, color=(0.6, 0.6, 0.6)))
    tall = raster.Item(raster.cube(), raster.instances(1, y=100, scale=(1), sy=600, sx=150, sz=150))
    base = r3.render(c, [floor, tall], background="#000000")
    fog = dict(fog_color=(1.0, 0.0, 0.0), ground_fog=(-150, 0, 400))
    fogged = r3.render(c, [floor, tall], background="#000000", **fog)
    red_gain = fogged[..., 0].astype(int) - base[..., 0].astype(int)
    assert (red_gain > 30).sum() > W * H // 10  # the floor turns red
    assert np.array_equal(
        r3.render(c, [floor, tall], **{**fog, "ground_fog": {"bottom": -150, "top": 0, "range": 400}}), fogged
    )
    # range 0 = off.
    assert np.array_equal(r3.render(c, [floor, tall], fog_color=(1, 0, 0), ground_fog=(-150, 0, 0)), base)
    with pytest.raises(TypeError, match="ground_fog"):
        r3.render(c, [floor], ground_fog={"bottom": 0, "height": 3})
    with pytest.raises(ValueError, match="3 components"):
        r3.render(c, [floor], ground_fog=(0, 1))


# ---------------------------------------------------------------------------
# Skybox and pass order
# ---------------------------------------------------------------------------


def test_skybox_fills_only_the_background():
    r3 = raster.Renderer3D(W, H)
    cube = raster.Item(raster.cube(), raster.instances(1, rx=20, ry=30, scale=200, color="#ff3b14"))
    env = dict(sky=(0.1, 0.3, 0.9), horizon=(1.0, 0.6, 0.3), ground=(0.1, 0.05, 0.02))
    plain = r3.render(cam(), [cube], background="#000000", **env)
    sky = r3.render(cam(), [cube], background="#000000", skybox=True, **env)
    covered = plain[..., :3].any(axis=-1)
    assert covered.sum() > 200 and (~covered).sum() > 200
    assert np.array_equal(sky[covered], plain[covered])  # geometry untouched
    assert sky[~covered][..., :3].any(axis=-1).mean() > 0.95  # background painted
    # A sun towards the key light (in view: key points into the screen).
    key = dict(key=(0.0, 0.1, -1.0), **env)
    no_sun = r3.render(cam(), [cube], skybox=True, **key)
    sun = r3.render(cam(), [cube], skybox=3, **key)
    assert lum(sun).sum() > lum(no_sun).sum()
    assert np.array_equal(r3.render(cam(), [cube], skybox={"sun": 3}, **key), sun)
    with pytest.raises(TypeError, match="skybox"):
        r3.render(cam(), [cube], skybox={"size": 3})


def test_light_items_draw_last_over_sky_and_geometry():
    r3 = raster.Renderer3D(W, H)
    env = dict(sky=(0.1, 0.3, 0.9), horizon=(0.3, 0.3, 0.3), ground=(0.1, 0.1, 0.1))
    glow = raster.Item(
        raster.cube(), raster.instances(1, x=-150, scale=80, color=(0.4, 0.4, 0.4)), {"emissive": 1}, blend="additive"
    )
    sky_only = r3.render(cam(), [], skybox=True, **env)
    with_glow = r3.render(cam(), [glow], skybox=True, **env)
    # Additive light adds onto the sky rather than being painted over by it.
    assert (lum(with_glow) > lum(sky_only) + 20).sum() > 100
    # Points listed before an opaque wall but in front of it still show.
    wall = raster.Item(
        raster.plane(), raster.instances(1, rx=90, z=-100, scale=2000, color=(0.2, 0.2, 0.2)), cull=False
    )
    pts = raster.Points(np.array([[0, 0, 100.0]]), colors=[(3, 3, 3)], size=20)
    first = r3.render(cam(), [pts, wall])
    last = r3.render(cam(), [wall, pts])
    assert np.array_equal(first, last)
    assert lum(first)[H // 2, W // 2] > lum(r3.render(cam(), [wall]))[H // 2, W // 2] + 50


# ---------------------------------------------------------------------------
# Shadows
# ---------------------------------------------------------------------------


def _shadow_scene(cast=True):
    floor = raster.Item(raster.plane(4), raster.instances(1, y=-100, scale=1500, color=(0.8, 0.8, 0.8)))
    box = raster.Item(raster.cube(), raster.instances(1, y=150, scale=120, color=(0.9, 0.3, 0.2)), cast_shadow=cast)
    return [floor, box]


SHADOW_CAM = Camera((0, 700, 600), (0, -100, 0), fov=55)


def test_shadow_darkens_floor_under_an_object():
    r3 = raster.Renderer3D(W, H)
    light = dict(key=(0.0, 1.0, 0.0), fill_color=(0, 0, 0))
    casting = r3.render(SHADOW_CAM, _shadow_scene(True), shadows=raster.Shadows(600, strength=1.0), **light)
    not_casting = r3.render(SHADOW_CAM, _shadow_scene(False), shadows={"extent": 600, "strength": 1.0}, **light)
    no_shadows = r3.render(SHADOW_CAM, _shadow_scene(True), **light)
    darker = lum(casting) < lum(not_casting) - 20
    assert darker.sum() > 20  # a shadow on the floor
    assert not (lum(casting) > lum(not_casting) + 2).any()  # and nothing gets brighter
    ys, xs = np.nonzero(darker)
    # The shadow is under the box: around the image's horizontal centre.
    assert abs(xs.mean() - W / 2) < W * 0.1
    # The floor (cast_shadow=False on the box) matches no shadows at all.
    assert np.abs(lum(not_casting) - lum(no_shadows)).max() < 8
    # Strength 0 = no darkening.
    assert np.array_equal(
        r3.render(SHADOW_CAM, _shadow_scene(True), shadows=dict(extent=600, strength=0.0), **light),
        r3.render(SHADOW_CAM, _shadow_scene(False), shadows=dict(extent=600, strength=0.0), **light),
    )
    # A slanted key moves the shadow (exercises the general light basis).
    slanted = r3.render(SHADOW_CAM, _shadow_scene(True), shadows=600, key=(0.6, 0.8, 0.0), fill_color=(0, 0, 0))
    assert not np.array_equal(slanted, casting)


def test_light_camera_matches_typescript():
    # Reference values from the light-camera block of the TypeScript Renderer3D.render.
    ref = [
        [0.0013867504905630728, -0.0006867016034994731, -0.0003094922302950865, 0, 0, 0.0011158901056866437,
         -0.000618984460590173, 0, -0.0009245003270420487, -0.0010300524052492097, -0.0004642383454426297, 0,
         0.013867504905630693, 0.06008639030620392, 0.004642383454426691, 1],
        [-0.004, 0, 0, 0, 0, 0, -0.002, 0, 0, 0.004, 0, 0, 0, 0, 0, 1],
    ]  # fmt: skip
    got = [
        raster._light_view_projection((0.4, 0.8, 0.6), (10, -20, 30), 600),
        raster._light_view_projection((0, 1, 0), (0, 0, 0), 250),
    ]
    assert np.allclose(got, ref, rtol=0, atol=1e-15)


def test_shadow_options_and_validation():
    r3 = raster.Renderer3D(W, H)
    light = dict(key=(0.0, 1.0, 0.0))
    ref = r3.render(SHADOW_CAM, _shadow_scene(), shadows=600, **light)
    # Shadows reset with every render (clear ends the map).
    r3.render(SHADOW_CAM, _shadow_scene(), **light)
    assert np.array_equal(r3.render(SHADOW_CAM, _shadow_scene(), shadows=600, **light), ref)
    # A (matrix, eye) camera needs an explicit centre.
    vp, eye = r3.view(SHADOW_CAM)
    with pytest.raises(ValueError, match="center"):
        r3.render((vp, eye), _shadow_scene(), shadows=600, **light)
    explicit = r3.render((vp, eye), _shadow_scene(), shadows=dict(extent=600, center=(0, -100, 0)), **light)
    assert np.array_equal(explicit, ref)
    for bad in (0, -5, math.inf):
        with pytest.raises(ValueError, match="extent"):
            r3.render(SHADOW_CAM, _shadow_scene(), shadows=bad)
    with pytest.raises(TypeError):
        r3.render(SHADOW_CAM, _shadow_scene(), shadows="yes")
    # Additive and mirror items never cast.
    glow = raster.Item(raster.cube(), raster.instances(1, y=50, scale=120), blend="additive")
    floor = _shadow_scene()[0]
    assert np.array_equal(
        r3.render(SHADOW_CAM, [floor, glow], shadows=600, **light), r3.render(SHADOW_CAM, [floor, glow], **light)
    )


# ---------------------------------------------------------------------------
# Depth of field
# ---------------------------------------------------------------------------


def _grid_wall(z):
    return raster.Item(
        raster.plane(),
        raster.instances(1, rx=90, z=z, scale=4000, color=(0.05, 0.05, 0.05)),
        raster.Material(lines=2.0, line_color=(1, 1, 1), grid=(40, 40), line_width=0.1),
        cull=False,
    )


def _contrast(f):
    y = lum(f)
    return np.abs(np.diff(y, axis=1)).mean() + np.abs(np.diff(y, axis=0)).mean()


def test_dof_blurs_out_of_focus():
    r3 = raster.Renderer3D(W, H)
    c = cam(1000, far=10000)
    sharp = r3.render(c, [_grid_wall(-1000)])
    near_focus = r3.render(c, [_grid_wall(-1000)], dof={"focus": 200, "aperture": 30})
    in_focus = r3.render(c, [_grid_wall(-1000)], dof=dict(focus=2000, aperture=30))
    assert _contrast(near_focus) < 0.6 * _contrast(sharp)
    assert np.abs(lum(in_focus) - lum(sharp)).mean() < 0.25 * np.abs(lum(near_focus) - lum(sharp)).mean()
    # A number is the focus with the default aperture (16) and cap (3% of the width).
    assert np.array_equal(
        r3.render(c, [_grid_wall(-1000)], dof=200),
        r3.render(c, [_grid_wall(-1000)], dof=dict(focus=200, aperture=16, max=W * 0.03)),
    )
    with pytest.raises(TypeError, match="focus"):
        r3.render(c, [], dof={"aperture": 10})
    with pytest.raises(TypeError, match="dof"):
        r3.render(c, [], dof={"focus": 10, "fstop": 2})


# ---------------------------------------------------------------------------
# Camera motion blur
# ---------------------------------------------------------------------------


def test_motion_blur_only_when_the_camera_moved():
    scene = [_grid_wall(-200)]
    blur = raster.Renderer3D(W, H)
    plain = raster.Renderer3D(W, H)
    a = cam(800, far=10000)
    b = Camera((120, 0, 800), (120, 0, 0), fov=50, far=10000)
    # No previous camera yet: identical to an unblurred render.
    assert np.array_equal(blur.render(a, scene, motion_blur=True), plain.render(a, scene))
    # Same camera again: nothing moved.
    assert np.array_equal(blur.render(a, scene, motion_blur=True), plain.render(a, scene))
    # The camera moved: the frame smears.
    moved = blur.render(b, scene, motion_blur=True)
    ref = plain.render(b, scene)
    assert not np.array_equal(moved, ref)
    assert _contrast(moved) < _contrast(ref)
    # reset_motion() forgets the previous camera (a cut).
    blur.render(a, scene)
    assert blur.reset_motion() is blur
    assert np.array_equal(blur.render(b, scene, motion_blur=True), ref)
    # Options: a number is the shutter; shutter 0 = no blur.
    blur.render(a, scene)
    assert np.array_equal(blur.render(b, scene, motion_blur=0.0), ref)
    blur.render(a, scene)
    assert np.array_equal(blur.render(b, scene, motion_blur={"shutter": 0.5, "max": W * 0.06}), moved)
    with pytest.raises(TypeError, match="motion_blur"):
        blur.render(a, scene, motion_blur={"shutter": 0.5, "samples": 4})


# ---------------------------------------------------------------------------
# Grade: edge antialiasing
# ---------------------------------------------------------------------------


def test_post_aa_smooths_edges_only():
    assert "aa" in video.POST_OPTIONS
    r3 = raster.Renderer3D(W, H)
    frame = r3.render(
        cam(),
        [
            raster.Item(
                raster.cube(),
                raster.instances(1, rz=17, scale=220, color=(1, 1, 1)),
                {"ambient": 1, "diffuse": 0, "specular": 0, "rim": 0},
            )
        ],
    )
    before = frame.copy()
    video.post(frame, aa=1)
    changed = np.any(frame != before, axis=-1)
    assert changed.sum() > 10
    # Only near edges: interior and background pixels stay put.
    assert not changed[0, :].any() and not changed[H // 2, W // 2]
    flat = np.full((H, W, 4), 90, np.uint8)
    assert np.array_equal(video.post(flat.copy(), aa=1), flat)
    assert np.array_equal(video.post(before.copy(), aa=0), before)


def test_point_lights_add_coloured_light():
    r = raster.Renderer3D(48, 48)
    cam = Camera((0, 300, 1), (0, 0, 0), fov=50)
    floor = raster.Item(raster.plane(), raster.instances(1, y=-50, scale=3000, color=(1, 1, 1)), material=raster.Material(ambient=0, diffuse=1, specular=0, rim=0))
    dark = r.render(cam, [floor], key=(0, -1, 0), fill=(0, -1, 0)).copy()
    lit = r.render(cam, [floor], key=(0, -1, 0), fill=(0, -1, 0), lights=[{"position": (0, 50, 0), "color": (0, 1, 0), "radius": 300, "intensity": 2}])
    assert lit[..., 1].sum() > dark[..., 1].sum() + 1000
    assert lit[..., 0].sum() < lit[..., 1].sum() / 4  # green light (hot cores burn slightly towards white)
