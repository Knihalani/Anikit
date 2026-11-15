import math
import random
from array import array

import pytest

import anikit
from anikit import _anikit
from anikit.transform import (
    Camera,
    Matrix,
    Projection,
    Projector,
    Quaternion,
    mix_transforms,
    parse,
    project_points,
    to_css,
    try_parse,
)


def close(a, b, tol=1e-6):
    return all(abs(x - y) <= tol for x, y in zip(a, b))


# ---------------------------------------------------------------------------
# Parsing and CSS
# ---------------------------------------------------------------------------


def test_parse_basic_functions():
    m = Matrix.from_css("translate(10px, 20px) rotate(90deg)")
    assert close(m.transform_point((1, 0, 0)), (10, 21, 0))
    assert close(Matrix.from_css("rotateY(90deg)").transform_point((1, 0, 0)), (0, 0, -1))
    assert Matrix.from_css("none").is_identity and Matrix.from_css("").is_identity
    assert Matrix.from_css("matrix(1, 0, 0, 1, 5, 6)").translation == (5, 6, 0)
    assert Matrix.from_css("rotate3d(0, 0, 1, 0.25turn)").isclose(Matrix.rotate_z(90))
    assert Matrix.from_css("rotate(1.5707963267948966rad)").isclose(Matrix.rotate_z(90))
    assert Matrix.from_css("rotate(100grad)").isclose(Matrix.rotate_z(90))
    assert Matrix.from_css("translate3d(1px, 2px, 3px)") == Matrix.translate(1, 2, 3)
    assert Matrix.from_css("scale(2)") == Matrix.scale(2)
    assert Matrix.from_css("scale3d(2, 3, 4)") == Matrix.scale(2, 3, 4)
    assert Matrix.from_css("scaleX(2) scaleY(3) scaleZ(4)").isclose(Matrix.scale(2, 3, 4))
    assert Matrix.from_css("skew(30deg)").isclose(Matrix.skew(30))
    assert Matrix.from_css("skewX(10deg) skewY(20deg)").isclose(Matrix.skew(10) @ Matrix.skew(0, 20))
    assert Matrix.from_css("perspective(500px)") == Matrix.perspective(500)
    assert Matrix.from_css("perspective(500px)")[11] == pytest.approx(-1 / 500)
    assert Matrix.from_css("ROTATEX(30DEG)").isclose(Matrix.rotate_x(30))  # case-insensitive names
    assert Matrix.from_css("translateX(50%)").translation == (50, 0, 0)  # % treated as px


def test_parse_errors():
    for bad in ("rotate(", "wobble(3deg)", "translate()", "matrix(1, 2, 3)", "matrix3d(1, 2)", "red"):
        assert try_parse(bad) is None, bad
        with pytest.raises(ValueError):
            Matrix.from_css(bad)
    with pytest.raises(TypeError):
        Matrix.from_css(5)  # type: ignore[arg-type]
    assert try_parse(5) is None  # type: ignore[arg-type]
    assert parse("rotate(5deg)") == Matrix.from_css("rotate(5deg)")


def test_css_output_roundtrip():
    css = "translate3d(10px, -20px, 30px) rotateX(30deg) rotateY(-45deg) rotateZ(60deg) scale3d(2, 0.5, 1.5)"
    m = Matrix.from_css(css)
    out = m.to_css()
    assert out.startswith("matrix3d(") and out.count(",") == 15
    assert Matrix.from_css(out).isclose(m, 1e-5)
    assert to_css(css) == out and str(m) == out
    assert Matrix.translate(5, 6).to_css(prefer_2d=True) == "matrix(1, 0, 0, 1, 5, 6)"
    assert Matrix.rotate_x(10).to_css(prefer_2d=True).startswith("matrix3d(")
    assert Matrix.translate(5, 6).is_2d and not Matrix.rotate_y(1).is_2d
    # Perspective survives with enough precision.
    p = Matrix.perspective(1200)
    assert Matrix.from_css(p.to_css()).isclose(p, 1e-7)
    assert "Matrix('matrix3d(" in repr(p)
    assert Matrix.translate(1e-7, 1e-12, 123.4567891).to_css() == (
        "matrix3d(1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0.0000001, 0, 123.456789, 1)"
    )


def test_matrix_construction_and_access():
    assert Matrix() == Matrix.identity() and Matrix().is_identity
    m = Matrix(range(16))
    assert m.values == tuple(float(i) for i in range(16)) and len(m) == 16 and list(m)[3] == 3
    assert m.at(1, 2) == 9  # column-major: m[col * 4 + row]
    assert Matrix([2, 0, 0, 2, 7, 8]) == Matrix.translate(7, 8) @ Matrix.scale(2)  # 2D matrix(a..f)
    with pytest.raises(ValueError):
        Matrix([1, 2, 3])
    assert m.transposed().at(2, 1) == 9
    assert hash(Matrix.translate(1)) == hash(Matrix.translate(1))
    assert Matrix.scale(2, 3, 4).determinant == pytest.approx(24)


def test_multiply_invert_and_points():
    t = Matrix.translate(10, 0, 0)
    r = Matrix.rotate_z(90)
    # CSS order: "translate rotate" = T @ R (rotate first, then translate).
    assert (t @ r).isclose(Matrix.from_css("translateX(10px) rotate(90deg)"))
    assert t.multiply(r) == t @ r
    assert r.then(t) == t @ r
    assert (t @ "rotate(90deg)").isclose(t @ r)
    m = Matrix.from_css("translate(5px, 6px) rotate(30deg) scale(2, 3) skewX(10deg)")
    assert (m @ m.invert()).isclose(Matrix(), 1e-9)
    p = m.transform_point((3, 4))
    assert close(m.invert().transform_point(p), (3, 4, 0), 1e-9)
    with pytest.raises(ValueError):
        Matrix.scale(0).invert()
    # Perspective divide.
    q = Matrix.perspective(100).transform_point((10, 0, 50))
    assert close(q, (20, 0, 100))


def test_decompose_recompose_roundtrip():
    cases = [
        "translate3d(10px, -20px, 30px) rotateX(30deg) rotateY(-45deg) rotateZ(60deg) scale3d(2, 0.5, 1.5)",
        "rotateY(170deg)",
        "perspective(500px) rotateX(20deg)",
        "skewX(20deg) scale(1.2)",
        "matrix(0.5, 0.3, -0.2, 1.1, 4, 5)",
        "scale(-1, 1)",
    ]
    for css in cases:
        m = Matrix.from_css(css)
        d = m.decompose()
        assert set(d) == {"translate", "scale", "skew", "perspective", "quaternion", "rotate"}
        assert Matrix.recompose(d).isclose(m, 1e-6), css
    d = Matrix.from_css("translate3d(1px, 2px, 3px) rotateX(90deg) scale(2, 3)").decompose()
    assert d["translate"] == (1, 2, 3)
    assert close(d["scale"], (2, 3, 1))
    assert close(d["rotate"], (1, 0, 0, 90))
    assert close(Matrix.from_css("perspective(400px)").decompose()["perspective"], (0, 0, -1 / 400, 1))
    assert Matrix.recompose({"translate": (5, 0, 0)}) == Matrix.translate(5)
    with pytest.raises(ValueError):
        Matrix.scale(0).decompose()
    # Low-level decomposed mixing.
    a = _anikit.mat4_decompose(list(Matrix().values))
    b = _anikit.mat4_decompose(list(Matrix.rotate_z(90).values))
    mid = _anikit.mat4_recompose(_anikit.mat4_mix_decomposed(a, b, 0.5))
    assert Matrix(mid).isclose(Matrix.rotate_z(45))
    with pytest.raises(ValueError):
        _anikit.mat4_recompose([1, 2])


def test_mixing_matches_browsers():
    mid = mix_transforms("rotateY(0deg)", "rotateY(90deg)", 0.5)
    assert Matrix.from_css(mid).isclose(Matrix.rotate_y(45), 1e-6)
    m = Matrix().mix("translateX(100px) scale(2)", 0.5)
    assert m.translation[0] == pytest.approx(50) and m[0] == pytest.approx(1.5)
    # Translation and rotation interpolate independently (no swinging arcs).
    m = Matrix.from_css("rotate(0deg)").mix("translateX(100px) rotate(180deg)", 0.25)
    assert m.translation[0] == pytest.approx(25)
    assert m.decompose()["rotate"][3] == pytest.approx(45)
    # Ends are exact; t can overshoot.
    a, b = Matrix.from_css("rotateX(10deg)"), Matrix.from_css("translateZ(40px) rotateX(70deg)")
    assert a.mix(b, 0).isclose(a, 1e-7) and a.mix(b, 1).isclose(b, 1e-7)
    assert a.mix(b, 1.5).translation[2] == pytest.approx(60)
    # Native string helper.
    assert Matrix(_anikit.transform_mix("scale(1)", "scale(3)", 0.5)).isclose(Matrix.scale(2))
    assert _anikit.transform_mix("scale(1)", "bogus", 0.5) is None
    # Singular endpoints switch discretely.
    assert Matrix.scale(0).mix(Matrix(), 0.4) == Matrix.scale(0)
    assert Matrix.scale(0).mix(Matrix(), 0.6) == Matrix()


# ---------------------------------------------------------------------------
# Quaternions
# ---------------------------------------------------------------------------


def test_quaternions():
    q = Quaternion.from_euler(0, 0, 30)
    assert q.to_matrix().isclose(Matrix.rotate_z(30))
    assert (
        Quaternion.from_euler(30, 40, 50)
        .to_matrix()
        .isclose(Matrix.from_css("rotateX(30deg) rotateY(40deg) rotateZ(50deg)"))
    )
    axis = Quaternion.from_axis_angle((0, 2, 0), 90)
    assert close(axis, (0, math.sin(math.pi / 4), 0, math.cos(math.pi / 4)))
    assert close(axis.rotate((1, 0, 0)), (0, 0, -1))
    assert close(axis.axis_angle(), (0, 1, 0, 90))
    assert Quaternion.identity().axis_angle() == (0, 0, 1, 0)
    # Composition and slerp.
    qa, qb = Quaternion.from_axis_angle((0, 0, 1), 30), Quaternion.from_axis_angle((0, 0, 1), 60)
    assert close(qa @ qb, Quaternion.from_axis_angle((0, 0, 1), 90))
    assert close(qa.multiply(qa.conjugate()), Quaternion.identity())
    half = Quaternion.identity().slerp(Quaternion.from_axis_angle((1, 0, 0), 120), 0.5)
    assert close(half.axis_angle(), (1, 0, 0, 60))
    # Shortest arc: -q is the same rotation.
    neg = Quaternion(*(-v for v in qb))
    assert close(qa.slerp(neg, 1.0).to_matrix(), qb.to_matrix())
    assert Quaternion(0, 0, 0, 0).normalized() == Quaternion.identity()
    assert Quaternion(0, 0, 0, 2).normalized() == Quaternion.identity()
    assert Matrix.from_css(qb.to_css()).isclose(qb.to_matrix(), 1e-6)
    assert qb.to_css() == "rotate3d(0, 0, 1, 60deg)"


# ---------------------------------------------------------------------------
# Cameras and projection
# ---------------------------------------------------------------------------


def test_camera_matrices():
    cam = Camera((0, 0, 10), fov=60, near=0.1, far=100)
    assert cam.view.isclose(Matrix.look_at((0, 0, 10), (0, 0, 0)))
    # The view moves the eye to the origin, looking down -z.
    assert close(cam.view.transform_point((0, 0, 0)), (0, 0, -10))
    vp = cam.view_projection(1.0)
    assert vp.isclose(cam.projection(1.0) @ cam.view)
    with pytest.raises(ValueError):
        Matrix.projection(60, 1, 0, 10)
    cam.orbit(90)
    assert close(cam.position, (10, 0, 0), 1e-9)
    cam.orbit(0, 90, distance=5)
    assert close(cam.position, (0, 5, 0), 1e-9)
    assert "Camera(" in repr(cam)
    with pytest.raises(ValueError):
        Camera((0, 0))


def test_projection_and_depth_order():
    cam = Camera((0, 0, 10), fov=60, near=0.1, far=100)
    pts = [(0, 0, 0), (0, 0, 5), (0, 0, -5), (0, 0, 20)]  # last one is behind the camera
    pr = project_points(pts, camera=cam, size=(200, 200))
    assert isinstance(pr, Projection) and len(pr) == 4
    assert close(pr[0][:2], (100, 100), 1e-4)
    assert list(pr.order) == [3, 2, 0, 1]  # far → near, culled first
    assert not pr.visible(3) and pr[3][3] == 0 and pr.visible(0)
    assert pr[1][3] > pr[0][3] > pr[2][3]  # nearer → bigger
    assert pr[1][2] < pr[0][2] < pr[2][2]  # depth = distance along the view axis
    assert [i for i, *_ in pr.sorted()] == [2, 0, 1]
    assert [i for i, *_ in pr.sorted(visible_only=False)] == [3, 2, 0, 1]
    assert pr[-1] == pr[3] and len(pr.screen()) == 4
    with pytest.raises(IndexError):
        pr[4]
    # y is down on screen; x to the right.
    pr = project_points([(1, 1, 0)], camera=cam, size=(200, 200))
    assert pr[0][0] > 100 and pr[0][1] < 100
    # Unsorted keeps the input order.
    assert list(project_points(pts, camera=cam, size=(200, 200), sort=False).order) == [0, 1, 2, 3]


def test_projection_inputs_and_model():
    cam = Camera((0, 0, 10), fov=60, near=0.1, far=100)
    pts = [(1, 2, 3), (-1, 0, 2), (0.5, -2, -4)]
    ref = project_points(pts, camera=cam, size=(320, 240))
    flat = [v for p in pts for v in p]
    for src in (flat, array("f", flat), array("d", flat), array("f", flat).tobytes()):
        assert close(project_points(src, camera=cam, size=(320, 240)).points, ref.points, 1e-4)
    # Strided records with channels in a different order: [z, pad, x, y].
    rec = [v for x, y, z in pts for v in (z, 99, x, y)]
    strided = project_points(rec, camera=cam, size=(320, 240), stride=4, offsets=(2, 3, 0))
    assert close(strided.points, ref.points, 1e-4)
    # 2D points get z = 0.
    assert close(project_points([(1, 2)], camera=cam).points, project_points([(1, 2, 0)], camera=cam).points)
    # A model matrix (CSS string) is applied before the camera.
    moved = project_points([(0, 0, 0)], camera=cam, size=(320, 240), model="translate3d(1px, 2px, 3px)")
    assert close(moved.points, project_points([(1, 2, 3)], camera=cam, size=(320, 240)).points, 1e-5)
    # Explicit view-projection.
    vp = cam.view_projection(320 / 240)
    assert close(project_points(pts, view_projection=vp, size=(320, 240)).points, ref.points)
    with pytest.raises(ValueError):
        project_points(pts, size=(0, 10))
    with pytest.raises(ValueError):
        project_points(flat, stride=3, offsets=(0, 1, 3))
    with pytest.raises(ValueError):
        project_points([(1, 2, 3, 4)])
    assert len(project_points([], camera=cam)) == 0


def test_projector_large_cloud_sorted():
    rnd = random.Random(3)
    n = 20000
    flat = array("f", (rnd.uniform(-50, 50) for _ in range(n * 3)))
    proj = Projector()
    cam = Camera((0, 0, 300))
    pr = proj.project(flat, camera=cam, size=(640, 480))
    assert len(pr) == n and len(pr.order) == n and sorted(pr.order) == list(range(n))
    depth = pr.points[2::4]
    ordered = [depth[i] for i in pr.order]
    assert all(a >= b for a, b in zip(ordered, ordered[1:]))
    # Reuse the projector (buffers shrink/grow correctly).
    small = proj.project([(0, 0, 0), (0, 0, 10)], camera=cam, size=(640, 480))
    assert len(small) == 2 and list(small.order) == [0, 1]


def test_transform_module_exported():
    assert "transform" in anikit.__all__ and anikit.transform.Matrix is Matrix
