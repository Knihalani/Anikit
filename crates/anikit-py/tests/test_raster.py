"""Solid 3D rendering into numpy frames (anikit.raster)."""

import json
import math
import pathlib

import pytest

np = pytest.importorskip("numpy")

import anikit  # noqa: E402
from anikit import raster  # noqa: E402
from anikit.transform import Camera  # noqa: E402

DATA = pathlib.Path(__file__).parent / "data"
W, H = 96, 64


def cam(z=600.0, **kw):
    return Camera((0, 0, z), (0, 0, 0), fov=50, **kw)


def lit(frame, bg=(0, 0, 0)):
    """Mask of pixels that differ from the background colour."""
    return np.any(frame[..., :3] != np.array(bg, np.uint8), axis=-1)


# ---------------------------------------------------------------------------
# Meshes
# ---------------------------------------------------------------------------


def test_builtin_meshes():
    c = raster.cube()
    assert (c.vertex_count, c.triangle_count) == (24, 12)
    assert c.positions.shape == (24, 3) and c.normals.shape == (24, 3)
    assert c.uvs.shape == (24, 2) and c.indices.shape == (12, 3)
    assert c.indices.dtype == np.uint32 and c.indices.max() < 24
    assert np.allclose(np.abs(c.positions), 0.5)
    s = raster.sphere(4, 6)
    assert np.allclose(np.linalg.norm(s.positions, axis=1), 1.0, atol=1e-6)
    assert raster.sphere(1, 1).triangle_count == raster.sphere(3, 3).triangle_count  # clamped like the wasm shim
    assert raster.torus(100, 20, 8, 4).triangle_count == 8 * 4 * 2
    assert raster.torus_knot(segments=40, sides=6).triangle_count == 40 * 6 * 2
    p = raster.plane((2, 3))
    assert p.triangle_count == 2 * 3 * 2 and np.allclose(p.positions[:, 1], 0)
    assert raster.plane(4).triangle_count == 32
    cyl = raster.cylinder(8)
    assert cyl.triangle_count == 8 * 2 + 2 * 8 and raster.cylinder(8, caps=False).triangle_count == 16
    assert "triangles=12" in repr(c)
    with pytest.raises(ValueError):
        raster.sphere(100000, 100000)


def test_heightfield():
    hf = raster.heightfield(5, 4, lambda x, z: x * 0.5 + z)
    pos = hf.positions
    assert hf.vertex_count == 20 and hf.triangle_count == 4 * 3 * 2
    assert pos[0, 1] == pytest.approx(0) and pos[-1, 1] == pytest.approx(1.5)
    arr = np.zeros((4, 5))
    arr[1:3, 1:3] = 1.0
    cut = raster.heightfield(5, 4, arr, skip=0)
    assert 0 < cut.triangle_count < raster.heightfield(5, 4, arr).triangle_count
    with pytest.raises(ValueError):
        raster.heightfield(5, 4, [0.0] * 7)
    with pytest.raises(ValueError):
        raster.heightfield(1, 4, [])


def test_custom_mesh_validation():
    pos = [0, 0, 0, 1, 0, 0, 0, 1, 0]
    nrm = [0, 0, 1] * 3
    m = raster.custom(np.array(pos).reshape(3, 3), nrm, [0, 1, 2], uvs=[0, 0, 1, 0, 0, 1])
    assert m.triangle_count == 1 and m.uvs.shape == (3, 2)
    assert raster.custom(pos, nrm, [[0, 1, 2]]).uvs.shape == (0, 2)
    with pytest.raises(IndexError):
        raster.custom(pos, nrm, [0, 1, 3])
    with pytest.raises(ValueError):
        raster.custom(pos, nrm[:6], [0, 1, 2])
    with pytest.raises(ValueError):
        raster.custom(pos, nrm, [0, 1, -1])
    with pytest.raises(ValueError):
        raster.custom(pos, nrm, [0, 1])
    with pytest.raises(ValueError):
        raster.custom([0, 0, float("nan")] + pos[3:], nrm, [0, 1, 2])


# ---------------------------------------------------------------------------
# Instances
# ---------------------------------------------------------------------------


def ts_set(o):
    """TypeScript's Instances.set for one instance (reference)."""
    r = math.pi / 180
    cx, sx_ = math.cos(o.get("rx", 0) * r), math.sin(o.get("rx", 0) * r)
    cy, sy_ = math.cos(o.get("ry", 0) * r), math.sin(o.get("ry", 0) * r)
    cz, sz_ = math.cos(o.get("rz", 0) * r), math.sin(o.get("rz", 0) * r)
    s = o.get("scale", 1)
    kx, ky, kz = o.get("sx", 1) * s, o.get("sy", 1) * s, o.get("sz", 1) * s
    return [
        cz * cy * kx, sz_ * cy * kx, -sy_ * kx,
        (cz * sy_ * sx_ - sz_ * cx) * ky, (sz_ * sy_ * sx_ + cz * cx) * ky, cy * sx_ * ky,
        (cz * sy_ * cx + sz_ * sx_) * kz, (sz_ * sy_ * cx - cz * sx_) * kz, cy * cx * kz,
        o.get("x", 0), o.get("y", 0), o.get("z", 0),
    ]  # fmt: skip


def test_instances_set_matches_typescript_and_vectorises():
    inst = raster.Instances(3)
    assert inst.data.shape == (3, 15) and inst.data.dtype == np.float32
    assert np.allclose(inst.data[:, :12], [1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0])
    assert np.allclose(inst.colors, 1)
    o = dict(x=3, y=-4, z=5, rx=10, ry=20, rz=30, scale=2, sx=1.5, sy=0.5, sz=3)
    inst.set(1, **o, color="#ff8000")
    assert np.allclose(inst.data[1, :12], ts_set(o), atol=1e-5)
    assert np.allclose(inst.data[1, 12:], [1, 128 / 255, 0])
    # Vectorised: arrays per instance, scalars broadcast.
    xs = np.array([1.0, 2.0, 3.0])
    angles = np.array([0.0, 45.0, 90.0])
    inst.set(x=xs, ry=angles, scale=2, color=[(1, 0, 0), (0, 1, 0), (0, 0, 1)])
    for i in range(3):
        assert np.allclose(inst.data[i, :12], ts_set(dict(x=xs[i], ry=angles[i], scale=2)), atol=1e-5)
    assert np.allclose(inst.colors, np.eye(3))
    inst.set(np.array([True, False, True]), y=7, color=(0.5, 0.5, 0.5))
    assert list(inst.positions[:, 1]) == [7, 0, 7]
    inst.positions[:, 2] += 10  # writable views
    assert list(inst.data[:, 11]) == [10, 10, 10]


def test_instances_helper():
    a = raster.instances(4, lambda i: dict(x=i * 10, color=(i / 4, 0, 0)))
    assert list(a.positions[:, 0]) == [0, 10, 20, 30]
    b = raster.instances(4, x=np.arange(4) * 10.0, color=np.array([[i / 4, 0, 0] for i in range(4)]))
    assert np.array_equal(a.data, b.data)
    assert len(raster.instances(0)) == 0
    with pytest.raises(TypeError):
        raster.instances(2, lambda i: {}, x=1)


# ---------------------------------------------------------------------------
# Rendering
# ---------------------------------------------------------------------------


def test_render_returns_independent_writable_frames():
    r3 = raster.Renderer3D(W, H)
    f = r3.render(cam(), [], background="#102030")
    assert f.shape == (H, W, 4) and f.dtype == np.uint8 and f.flags.writeable
    assert np.all(f == np.array([16, 32, 48, 255], np.uint8))
    f[...] = 0
    g = r3.render(cam(), [], background=(1, 0, 0), transparent=True)
    assert np.all(g == np.array([255, 0, 0, 0], np.uint8)) and not np.any(f)
    assert r3.size == (W, H) and r3.pixel(0, 0) == (255, 0, 0, 0)
    assert np.array_equal(r3.frame(), g)
    # Hot background channels clamp per channel; 0xRRGGBB ints work too.
    assert r3.render(cam(), [], background=(1.2, 0.5, -1))[0, 0].tolist() == [255, 128, 0, 255]
    assert r3.render(cam(), [], background=0x00FF80)[0, 0].tolist() == [0, 255, 128, 255]
    assert anikit.raster is raster  # lazy submodule


def test_cube_renders_lit_and_coloured():
    r3 = raster.Renderer3D(W, H)
    inst = raster.instances(1, ry=30, rx=20, scale=200, color="#3080ff")
    f = r3.render(cam(), [raster.Item(raster.cube(), inst)])
    mask = lit(f)
    assert mask[H // 2, W // 2] and not mask[0, 0]
    assert r3.triangles > 0 and r3.last_ms > 0
    centre = f[H // 2, W // 2, :3].astype(int)
    assert centre[2] > centre[0]  # blue-ish
    # A bare mesh, a dict, and a (matrix, eye) camera give the same result.
    c = cam()
    vp = c.view_projection(W / H)
    g = r3.render((vp, c.position), [{"mesh": raster.cube(), "instances": inst}])
    assert np.array_equal(f, g)
    h = r3.render(c, [raster.cube()])
    assert lit(h).sum() < mask.sum()  # unit cube, not scaled 200


def test_materials_and_lighting_change_the_frame():
    r3 = raster.Renderer3D(W, H)
    knot = raster.torus_knot(radius=150, spread=60, tube=30, segments=120, sides=10)
    base = r3.render(cam(), [raster.Item(knot)])
    variants = [
        raster.Material(metal=1.0, iridescence=0.5),
        raster.Material(emissive=1.0),
        {"lines": 2.0, "line_color": (2, 0.5, 0.2), "grid": (0, 6)},
        raster.Material(pulse=3.0, pulse_count=2, pulse_phase=0.3, lines=0.5),
    ]
    for m in variants:
        assert not np.array_equal(base, r3.render(cam(), [raster.Item(knot, material=m)])), m
    fogged = r3.render(cam(), [raster.Item(knot)], fog=(100, 700), background="#204060")
    assert not np.array_equal(base, fogged)
    keyed = r3.render(cam(), [raster.Item(knot)], lighting=raster.Lighting(key=(-1, 0, 0), key_color="red"))
    assert not np.array_equal(base, keyed)
    strips = r3.render(cam(), [raster.Item(knot, material={"metal": 1})], strips=(3, 3, 3))
    assert not np.array_equal(r3.render(cam(), [raster.Item(knot, material={"metal": 1})]), strips)
    with pytest.raises(TypeError):
        r3.render(cam(), [knot], fogg=(1, 2))
    with pytest.raises(TypeError):
        r3.render(cam(), [raster.Item(knot, material={"metall": 1})])
    with pytest.raises(ValueError):
        r3.render(cam(), [raster.Item(knot, blend="screen")])


def test_textures():
    r3 = raster.Renderer3D(W, H)
    img = np.zeros((8, 8, 3), np.uint8)
    img[:, :, 1] = 255
    tex = raster.texture(img)
    assert (tex.width, tex.height) == (8, 8)
    quad = raster.instances(1, rx=90, scale=500)
    plain = r3.render(
        cam(), [raster.Item(raster.plane(), quad, material={"ambient": 0, "diffuse": 0, "specular": 0, "rim": 0})]
    )
    glow = r3.render(
        cam(),
        [
            raster.Item(
                raster.plane(),
                quad,
                material=raster.Material(ambient=0, diffuse=0, specular=0, rim=0, texture=tex, texture_mode="emissive"),
            )
        ],
    )
    assert glow[H // 2, W // 2, 1] > 200 > plain[H // 2, W // 2, 1]
    assert raster.texture(np.ones((2, 3, 4))).width == 3  # floats in 0..1
    with pytest.raises(ValueError):
        raster.texture(np.zeros((4, 4)))
    with pytest.raises(ValueError):
        r3.render(cam(), [raster.Item(raster.plane(), quad, material={"texture": tex, "texture_mode": "glow"})])


def test_additive_and_cull():
    r3 = raster.Renderer3D(W, H)
    wall = raster.Item(raster.cube(), raster.instances(1, z=-200, scale=(1), sx=600, sy=600, color=(0.2, 0.2, 0.2)))
    base = r3.render(cam(), [wall])
    beam = raster.Item(
        raster.cube(), raster.instances(1, scale=150, color=(1, 0.2, 0.1)), material={"emissive": 1}, blend="additive"
    )
    added = r3.render(cam(), [wall, beam])
    diff = added.astype(int) - base.astype(int)
    assert diff.min() >= 0 and diff.max() > 0
    # A plane seen from below is culled unless cull=False.
    below = Camera((0, -500, 0.01), (0, 0, 0))
    floor = raster.instances(1, scale=400)
    assert not lit(r3.render(below, [raster.Item(raster.plane(), floor)])).any()
    assert lit(r3.render(below, [raster.Item(raster.plane(), floor, cull=False)])).any()


def test_mirror_floor_reflects_the_scene():
    r3 = raster.Renderer3D(W, H)
    c = Camera((0, 150, 700), (0, -40, 0), fov=50)
    ball = raster.Item(
        raster.sphere(12, 16), raster.instances(1, y=60, scale=90, color="#ff4020"), material={"emissive": 0.6}
    )
    floor = raster.Item(raster.plane(), raster.instances(1, y=-100, scale=4000, color=(0.1, 0.1, 0.12)), mirror=True)
    plain = r3.render(c, [ball, floor])
    mirrored = r3.render(c, [ball, floor], mirror=-100)
    # Above the ball nothing changes; on the floor under it the red reflection appears.
    assert np.array_equal(plain[:5], mirrored[:5])
    changed = np.any(plain != mirrored, axis=-1)
    rows = np.nonzero(changed.any(axis=1))[0]
    assert rows.size and rows.min() > 5  # only the floor (below the horizon) changes
    red_gain = mirrored[..., 0].astype(int) - plain[..., 0].astype(int)
    assert red_gain.max() > 20
    y, x = np.unravel_index(np.argmax(red_gain), red_gain.shape)
    assert y > H // 2 and abs(x - W // 2) < W // 8  # the ball's reflection, under it
    # Strength scales the gloss; reflect=False removes the ball from the reflection.
    weak = r3.render(c, [ball, floor], mirror=raster.Mirror(-100, strength=0.1))
    assert (weak[..., 0].astype(int) - plain[..., 0]).max() < red_gain.max()
    hidden = r3.render(
        c, [raster.Item(ball.mesh, ball.instances, ball.material, reflect=False), floor], mirror={"y": -100}
    )
    assert (hidden[..., 0].astype(int) - plain[..., 0]).max() < red_gain.max()
    assert np.array_equal(mirrored, r3.render(c, [ball, floor], mirror=(-100, 0.5)))


def test_points_and_streaks():
    r3 = raster.Renderer3D(W, H)
    rng = np.random.default_rng(1)
    pts = rng.uniform(-300, 300, (400, 3)).astype(np.float32)
    f = r3.render(cam(), [raster.Points(pts, size=3, intensity=2)])
    assert lit(f).sum() > 50
    red = r3.render(cam(), [raster.Points(pts, colors=np.tile([1, 0, 0], (400, 1)), size=3, intensity=2)])
    assert red[..., 1].max() == 0 and red[..., 0].max() > 0
    fog = r3.render(cam(), [raster.Points(pts, size=3, intensity=2, fog=(100, 400))])
    assert lit(fog).sum() < lit(f).sum()
    # Streaks use the previous render's camera.
    r3.render(cam(600), [raster.Points(pts, size=3, intensity=2)])
    streaked = r3.render(cam(450, roll=8), [raster.Points(pts, size=3, intensity=2, streak=2)])
    round_ = r3.render(cam(450, roll=8), [raster.Points(pts, size=3, intensity=2, streak=0)])
    assert not np.array_equal(streaked, round_)
    # Points hide behind solid geometry drawn before them.
    wall = raster.Item(raster.cube(), raster.instances(1, z=320, sx=2000, sy=2000, color=(0, 0, 0)))
    wall_only = r3.render(cam(), [wall])
    hidden = r3.render(cam(), [wall, raster.Points(pts, size=3, intensity=2)])
    assert np.any(hidden != wall_only, axis=-1).sum() < lit(f).sum() // 2
    # A PointCloud works as a source.
    cloud = anikit.PointCloud(200, shapes=[lambda i, n: (i - n / 2, 0, 0)])
    assert lit(r3.render(cam(), [{"points": cloud, "size": 4, "intensity": 3}])).any()
    with pytest.raises(ValueError):
        r3.render(cam(), [raster.Points(pts, colors=[1, 0, 0])])


# ---------------------------------------------------------------------------
# Shatter
# ---------------------------------------------------------------------------


def test_shatter_matches_typescript():
    ref = json.loads((DATA / "shatter" / "shatter_ref.json").read_text())
    src = raster.torus(100, 30, 12, 6)
    a = raster.shatter(
        src, transform=dict(rx=20, ry=-35, scale=1.2, x=30, y=-10, z=5, color=(0.2, 0.4, 0.9)), force=900, spin=540
    )
    assert a.count == ref["a"]["count"] == src.triangle_count
    assert np.allclose(a.instances.data[0, 12:], ref["a"]["colors"])
    pick = np.arange(a.count) % 9 == 0
    for t, rows in ref["a"]["t"].items():
        got = a.update(float(t)).instances.data[pick].reshape(-1)
        assert np.allclose(got, rows, rtol=1e-5, atol=1e-3), t
    b = raster.shatter(src, center=(0, 200, 0), spread=0.9, gravity=600, drag=0, ripple=2, spin=90)
    for t, rows in ref["b"]["t"].items():
        got = b.update(float(t)).instances.data[pick].reshape(-1)
        assert np.allclose(got, rows, rtol=1e-5, atol=1e-3), t


def test_shatter_intact_at_zero_and_renders():
    src = raster.cube()
    sh = raster.shatter(src, transform=dict(x=10, ry=30, scale=50), color=lambda i, c: (i / 12, 0.5, 0.5))
    d = sh.instances.data
    # Each shard maps the unit triangle (0,0,0), (1,0,0), (0,1,0) onto a source triangle.
    world = src.positions @ np.array(raster.instances(1, x=10, ry=30, scale=50).data[0, :9]).reshape(3, 3) + [10, 0, 0]
    for k, tri in enumerate(src.indices):
        o, e1, e2 = d[k, 9:12], d[k, 0:3], d[k, 3:6]
        assert np.allclose([o, o + e1, o + e2], world[tri], atol=1e-3)
    assert np.allclose(d[:, 12], np.arange(12) / 12)
    before = d.copy()
    sh.update(-1)
    assert np.array_equal(before, sh.instances.data)
    sh.update(0.5)
    assert not np.allclose(before[:, 9:12], sh.instances.data[:, 9:12])
    r3 = raster.Renderer3D(W, H)
    assert lit(r3.render(cam(), [sh.item(material={"metal": 0.5})])).any()
    assert sh.item().cull is False and sh.mesh is raster.shatter(raster.sphere(3, 3)).mesh


def test_example_script_writes_pngs(tmp_path, monkeypatch):
    import runpy
    import sys

    script = pathlib.Path(__file__).parent.parent / "examples" / "knot_mirror.py"
    monkeypatch.setattr(sys, "argv", [str(script), str(tmp_path), "1"])
    runpy.run_path(str(script), run_name="__main__")
    png = (tmp_path / "knot_000.png").read_bytes()
    assert png.startswith(b"\x89PNG\r\n\x1a\n") and png.endswith(b"IEND\xaeB`\x82")
