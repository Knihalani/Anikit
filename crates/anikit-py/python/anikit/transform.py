"""3D transforms: 4×4 matrices, quaternions, CSS ``transform`` parsing and
browser-exact interpolation, cameras and bulk perspective projection.

Matrices are column-major in CSS ``matrix3d()`` order (``m[col * 4 + row]``),
lengths are pixels and angles degrees::

    from anikit.transform import Matrix, mix_transforms, project_points, Camera

    m = Matrix.from_css("translate(10px, 20px) rotateY(45deg)")
    m.to_css()                                   # 'matrix3d(...)'
    mix_transforms("rotateY(0deg)", "rotateY(90deg)", 0.5)   # = rotateY(45deg)

    proj = project_points(points, camera=Camera(position=(0, 0, 800)), size=(640, 480))
    for i in proj.order:                         # painter's order: far → near
        sx, sy, depth, scale = proj[i]

Interpolation follows CSS Transforms 2: decompose both matrices into
translate / scale / skew / perspective / rotation quaternion, interpolate
(the rotation by slerp along the shortest arc) and recompose.
"""

from __future__ import annotations

import math
from array import array
from typing import Any, Dict, Iterable, Iterator, List, NamedTuple, Optional, Sequence, Tuple, Union

from . import _anikit

__all__ = [
    "Matrix",
    "MatrixLike",
    "Quaternion",
    "Camera",
    "Projection",
    "Projector",
    "parse",
    "try_parse",
    "to_css",
    "mix_transforms",
    "project_points",
]

Vec3 = Tuple[float, float, float]

_IDENTITY = (1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0)


def _num(v: float) -> str:
    """Compact CSS number: 6 decimals, or 6 significant digits below 1 (so
    perspective terms like ``-1/1200`` keep their precision)."""
    if not math.isfinite(v) or abs(v) < 1e-9:
        return "0"
    if abs(v) >= 1:
        r = round(v, 6)
        if r == int(r) and abs(r) < 1e15:
            return str(int(r))
        return f"{r:.6f}".rstrip("0").rstrip(".")
    s = f"{v:.6g}"
    if "e" in s:
        s = f"{float(s):.15f}".rstrip("0").rstrip(".")
    return s


def _vec3(v: Sequence[float], name: str) -> Vec3:
    if len(v) != 3:
        raise ValueError(f"{name} must be (x, y, z), got {v!r}")
    return (float(v[0]), float(v[1]), float(v[2]))


class Matrix:
    """An immutable 4×4 matrix in CSS ``matrix3d`` (column-major) order.

    ``a @ b`` (or ``a.multiply(b)``) applies ``b`` first, then ``a`` — the same
    order as writing ``a b`` in a CSS transform list.

    >>> m = Matrix.from_css("translateX(10px) scale(2)")
    >>> m.transform_point((1, 1, 0))
    (12.0, 2.0, 0.0)
    >>> Matrix.from_css("rotate(90deg)").isclose(Matrix.rotate_z(90))
    True
    >>> Matrix.translate(5, 6).to_css()
    'matrix3d(1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 5, 6, 0, 1)'
    """

    __slots__ = ("_m",)

    def __init__(self, values: Optional[Iterable[float]] = None) -> None:
        if values is None:
            self._m: Tuple[float, ...] = _IDENTITY
            return
        m = tuple(float(v) for v in values)
        if len(m) == 6:  # 2D matrix(a, b, c, d, e, f)
            a, b, c, d, e, f = m
            m = (a, b, 0.0, 0.0, c, d, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, e, f, 0.0, 1.0)
        if len(m) != 16:
            raise ValueError(f"a matrix needs 16 values (or 6 for a 2D matrix), got {len(m)}")
        self._m = m

    # ---- constructors ----------------------------------------------------------

    @classmethod
    def identity(cls) -> "Matrix":
        return cls()

    @classmethod
    def _raw(cls, values: Sequence[float]) -> "Matrix":
        m = cls.__new__(cls)
        m._m = tuple(values)
        return m

    @classmethod
    def from_css(cls, css: str) -> "Matrix":
        """Parse a CSS ``transform`` list (``"none"`` → identity; ``%`` is treated as px).

        Raises :class:`ValueError` for anything that is not a transform list.
        """
        if not isinstance(css, str):
            raise TypeError(f"expected a CSS transform string, got {type(css).__name__}")
        v = _anikit.transform_parse(css)
        if v is None:
            raise ValueError(f"not a CSS transform: {css!r}")
        return cls._raw(v)

    @classmethod
    def translate(cls, x: float = 0.0, y: float = 0.0, z: float = 0.0) -> "Matrix":
        return cls._raw(_anikit.mat4_translate(float(x), float(y), float(z)))

    @classmethod
    def scale(cls, x: float, y: Optional[float] = None, z: float = 1.0) -> "Matrix":
        """``scale(x)`` scales x and y uniformly (like CSS ``scale()``)."""
        return cls._raw(_anikit.mat4_scale(float(x), float(x if y is None else y), float(z)))

    @classmethod
    def rotate(cls, axis: Sequence[float], degrees: float) -> "Matrix":
        """Rotation of ``degrees`` about an axis vector (CSS ``rotate3d``)."""
        return cls._raw(_anikit.mat4_rotate(_vec3(axis, "axis"), float(degrees)))

    @classmethod
    def rotate_x(cls, degrees: float) -> "Matrix":
        return cls.rotate((1.0, 0.0, 0.0), degrees)

    @classmethod
    def rotate_y(cls, degrees: float) -> "Matrix":
        return cls.rotate((0.0, 1.0, 0.0), degrees)

    @classmethod
    def rotate_z(cls, degrees: float) -> "Matrix":
        """Same as CSS ``rotate()``."""
        return cls.rotate((0.0, 0.0, 1.0), degrees)

    @classmethod
    def skew(cls, x_degrees: float, y_degrees: float = 0.0) -> "Matrix":
        return cls._raw(_anikit.mat4_skew(float(x_degrees), float(y_degrees)))

    @classmethod
    def perspective(cls, distance: float) -> "Matrix":
        """CSS ``perspective(distance)``."""
        return cls._raw(_anikit.mat4_perspective_css(float(distance)))

    @classmethod
    def look_at(cls, eye: Sequence[float], target: Sequence[float], up: Sequence[float] = (0.0, 1.0, 0.0)) -> "Matrix":
        """Right-handed view matrix of a camera at ``eye`` looking at ``target``."""
        return cls._raw(_anikit.mat4_look_at(_vec3(eye, "eye"), _vec3(target, "target"), _vec3(up, "up")))

    @classmethod
    def projection(cls, fov: float, aspect: float, near: float, far: float) -> "Matrix":
        """OpenGL-style perspective projection (vertical ``fov`` in degrees)."""
        if not (near > 0 and far > near):
            raise ValueError("projection needs 0 < near < far")
        return cls._raw(_anikit.mat4_perspective(float(fov), float(aspect), float(near), float(far)))

    @classmethod
    def recompose(cls, parts: Dict[str, Sequence[float]]) -> "Matrix":
        """Inverse of :meth:`decompose` (missing parts default to identity)."""
        d = (
            list(parts.get("translate", (0.0, 0.0, 0.0)))
            + list(parts.get("scale", (1.0, 1.0, 1.0)))
            + list(parts.get("skew", (0.0, 0.0, 0.0)))
            + list(parts.get("perspective", (0.0, 0.0, 0.0, 1.0)))
            + list(parts.get("quaternion", (0.0, 0.0, 0.0, 1.0)))
        )
        return cls._raw(_anikit.mat4_recompose([float(v) for v in d]))

    # ---- access ------------------------------------------------------------------

    @property
    def values(self) -> Tuple[float, ...]:
        """The 16 values in ``matrix3d`` (column-major) order."""
        return self._m

    def __len__(self) -> int:
        return 16

    def __iter__(self) -> Iterator[float]:
        return iter(self._m)

    def __getitem__(self, i: int) -> float:
        return self._m[i]

    def at(self, row: int, col: int) -> float:
        return self._m[col * 4 + row]

    @property
    def is_identity(self) -> bool:
        return self._m == _IDENTITY

    @property
    def is_2d(self) -> bool:
        """``True`` when expressible as a CSS 2D ``matrix(a, b, c, d, e, f)``."""
        m = self._m
        return (
            m[2] == 0 and m[3] == 0 and m[6] == 0 and m[7] == 0 and m[8] == 0 and m[9] == 0
            and m[10] == 1 and m[11] == 0 and m[14] == 0 and m[15] == 1
        )  # fmt: skip

    @property
    def determinant(self) -> float:
        return _anikit.mat4_determinant(list(self._m))

    @property
    def translation(self) -> Vec3:
        return (self._m[12], self._m[13], self._m[14])

    # ---- operations ----------------------------------------------------------------

    def multiply(self, other: "MatrixLike") -> "Matrix":
        """``self × other`` (``other`` is applied first)."""
        return Matrix._raw(_anikit.mat4_mul(list(self._m), list(as_matrix(other)._m)))

    def __matmul__(self, other: "MatrixLike") -> "Matrix":
        return self.multiply(other)

    def then(self, other: "MatrixLike") -> "Matrix":
        """Apply ``self`` first, then ``other`` (``other × self``)."""
        return as_matrix(other).multiply(self)

    def invert(self) -> "Matrix":
        """The inverse; raises :class:`ValueError` for singular matrices."""
        v = _anikit.mat4_invert(list(self._m))
        if v is None:
            raise ValueError("matrix is not invertible")
        return Matrix._raw(v)

    def transposed(self) -> "Matrix":
        m = self._m
        return Matrix._raw([m[r * 4 + c] for c in range(4) for r in range(4)])

    def transform_point(self, p: Sequence[float]) -> Vec3:
        """Transform ``(x, y[, z])`` with the perspective divide."""
        z = float(p[2]) if len(p) > 2 else 0.0
        x, y, zz, w = _anikit.mat4_transform_point(list(self._m), (float(p[0]), float(p[1]), z))
        if w != 1.0 and w != 0.0:
            return (x / w, y / w, zz / w)
        return (x, y, zz)

    def decompose(self) -> Dict[str, Tuple[float, ...]]:
        """Split into ``translate`` (x, y, z), ``scale`` (x, y, z), ``skew``
        (xy, xz, yz), ``perspective`` (4) and ``quaternion`` (x, y, z, w), plus
        ``rotate`` — the quaternion as ``(axis_x, axis_y, axis_z, degrees)``.
        Raises :class:`ValueError` for singular matrices.

        >>> d = Matrix.from_css("translate3d(1px, 2px, 3px) rotateZ(90deg) scale(2)").decompose()
        >>> d["translate"], [round(v, 6) for v in d["scale"]], [round(v, 3) for v in d["rotate"]]
        ((1.0, 2.0, 3.0), [2.0, 2.0, 1.0], [0.0, 0.0, 1.0, 90.0])
        """
        d = _anikit.mat4_decompose(list(self._m))
        if d is None:
            raise ValueError("cannot decompose a singular matrix")
        q = Quaternion(d[13], d[14], d[15], d[16])
        return {
            "translate": (d[0], d[1], d[2]),
            "scale": (d[3], d[4], d[5]),
            "skew": (d[6], d[7], d[8]),
            "perspective": (d[9], d[10], d[11], d[12]),
            "quaternion": tuple(q),
            "rotate": q.axis_angle(),
        }

    def mix(self, other: "MatrixLike", t: float) -> "Matrix":
        """Browser-style interpolation towards ``other`` (``t`` may overshoot)."""
        return Matrix._raw(_anikit.mat4_mix(list(self._m), list(as_matrix(other)._m), float(t)))

    def isclose(self, other: "MatrixLike", tol: float = 1e-9) -> bool:
        o = as_matrix(other)._m
        return all(abs(a - b) <= tol for a, b in zip(self._m, o))

    def to_css(self, *, prefer_2d: bool = False) -> str:
        """``'matrix3d(...)'`` (or ``'matrix(a, b, c, d, e, f)'`` for 2D
        matrices when ``prefer_2d``)."""
        m = self._m
        if prefer_2d and self.is_2d:
            return "matrix(" + ", ".join(_num(m[i]) for i in (0, 1, 4, 5, 12, 13)) + ")"
        return "matrix3d(" + ", ".join(_num(v) for v in m) + ")"

    def __str__(self) -> str:
        return self.to_css()

    def __repr__(self) -> str:
        return f"Matrix({self.to_css()!r})"

    def __eq__(self, other: object) -> bool:
        return isinstance(other, Matrix) and other._m == self._m

    def __hash__(self) -> int:
        return hash(self._m)


#: A matrix given as a :class:`Matrix`, a CSS transform string or 16 (or 6) numbers.
MatrixLike = Union[Matrix, str, Sequence[float]]


def as_matrix(m: MatrixLike) -> Matrix:
    if isinstance(m, Matrix):
        return m
    if isinstance(m, str):
        return Matrix.from_css(m)
    return Matrix(m)


def parse(css: str) -> Matrix:
    """:meth:`Matrix.from_css`."""
    return Matrix.from_css(css)


def try_parse(css: str) -> Optional[Matrix]:
    """Like :func:`parse` but returns ``None`` for invalid input."""
    v = _anikit.transform_parse(css) if isinstance(css, str) else None
    return None if v is None else Matrix._raw(v)


def to_css(m: MatrixLike) -> str:
    """Normalise any transform to ``'matrix3d(...)'``.

    >>> to_css("translateX(5px)")
    'matrix3d(1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 5, 0, 0, 1)'
    """
    return as_matrix(m).to_css()


def mix_transforms(a: MatrixLike, b: MatrixLike, t: float) -> str:
    """Interpolate two transforms like browsers do; returns ``'matrix3d(...)'``.

    >>> mix_transforms("rotateY(0deg)", "rotateY(90deg)", 0.5) == to_css("rotateY(45deg)")
    True
    """
    return as_matrix(a).mix(b, t).to_css()


class Quaternion(NamedTuple):
    """Unit quaternion ``(x, y, z, w)`` describing a 3D rotation.

    >>> q = Quaternion.from_euler(0, 0, 90)
    >>> q.to_matrix().isclose(Matrix.rotate_z(90))
    True
    >>> [round(v, 3) for v in Quaternion.identity().slerp(q, 0.5).axis_angle()]
    [0.0, 0.0, 1.0, 45.0]
    """

    x: float
    y: float
    z: float
    w: float

    @classmethod
    def identity(cls) -> "Quaternion":
        return cls(0.0, 0.0, 0.0, 1.0)

    @classmethod
    def from_euler(cls, x: float, y: float, z: float) -> "Quaternion":
        """From angles in degrees applied as CSS ``rotateX(x) rotateY(y) rotateZ(z)``."""
        return cls(*_anikit.quat_euler(float(x), float(y), float(z)))

    @classmethod
    def from_axis_angle(cls, axis: Sequence[float], degrees: float) -> "Quaternion":
        ax = _vec3(axis, "axis")
        return cls(*_anikit.quat_axis_angle(ax[0], ax[1], ax[2], float(degrees)))

    def multiply(self, other: "Quaternion") -> "Quaternion":
        """``self × other`` (rotate by ``other`` first)."""
        return Quaternion(*_anikit.quat_mul(self, other))

    def __matmul__(self, other: "Quaternion") -> "Quaternion":
        return self.multiply(other)

    def slerp(self, other: "Quaternion", t: float) -> "Quaternion":
        """Spherical interpolation along the shortest arc."""
        return Quaternion(*_anikit.quat_slerp(self, other, float(t)))

    def conjugate(self) -> "Quaternion":
        return Quaternion(-self.x, -self.y, -self.z, self.w)

    def normalized(self) -> "Quaternion":
        n = math.sqrt(self.x * self.x + self.y * self.y + self.z * self.z + self.w * self.w)
        if n == 0:
            return Quaternion.identity()
        return Quaternion(self.x / n, self.y / n, self.z / n, self.w / n)

    def axis_angle(self) -> Tuple[float, float, float, float]:
        """``(axis_x, axis_y, axis_z, degrees)``; identity → ``(0, 0, 1, 0)``."""
        q = self.normalized()
        w = max(-1.0, min(1.0, q.w))
        s = math.sqrt(max(0.0, 1.0 - w * w))
        if s < 1e-12:
            return (0.0, 0.0, 1.0, 0.0)
        return (q.x / s, q.y / s, q.z / s, math.degrees(2.0 * math.acos(w)))

    def to_matrix(self) -> Matrix:
        return Matrix._raw(_anikit.quat_to_matrix(self))

    def rotate(self, v: Sequence[float]) -> Vec3:
        """Rotate a vector."""
        return self.to_matrix().transform_point(v)

    def to_css(self) -> str:
        """``'rotate3d(x, y, z, <angle>deg)'``."""
        x, y, z, deg = self.axis_angle()
        return f"rotate3d({_num(x)}, {_num(y)}, {_num(z)}, {_num(deg)}deg)"


class Camera:
    """A perspective camera at ``position`` looking at ``target``.

    ``fov`` is the vertical field of view in degrees; ``near``/``far`` clip
    depth. Points behind the camera (or nearer than ``near``) are culled.
    """

    __slots__ = ("position", "target", "up", "fov", "near", "far")

    def __init__(
        self,
        position: Sequence[float] = (0.0, 0.0, 1000.0),
        target: Sequence[float] = (0.0, 0.0, 0.0),
        *,
        up: Sequence[float] = (0.0, 1.0, 0.0),
        fov: float = 50.0,
        near: float = 1.0,
        far: float = 100000.0,
    ) -> None:
        self.position = _vec3(position, "position")
        self.target = _vec3(target, "target")
        self.up = _vec3(up, "up")
        self.fov = float(fov)
        self.near = float(near)
        self.far = float(far)

    @property
    def view(self) -> Matrix:
        return Matrix.look_at(self.position, self.target, self.up)

    def projection(self, aspect: float) -> Matrix:
        return Matrix.projection(self.fov, aspect, self.near, self.far)

    def view_projection(self, aspect: float) -> Matrix:
        return self.projection(aspect) @ self.view

    def orbit(self, yaw: float, pitch: float = 0.0, distance: Optional[float] = None) -> "Camera":
        """Place the camera on a sphere around ``target`` (degrees; yaw about y,
        pitch up); ``distance`` defaults to the current distance."""
        tx, ty, tz = self.target
        px, py, pz = self.position
        d = math.dist((px, py, pz), (tx, ty, tz)) if distance is None else float(distance)
        yr, pr = math.radians(yaw), math.radians(pitch)
        self.position = (
            tx + d * math.cos(pr) * math.sin(yr),
            ty + d * math.sin(pr),
            tz + d * math.cos(pr) * math.cos(yr),
        )
        return self

    def __repr__(self) -> str:
        return f"Camera(position={self.position}, target={self.target}, fov={self.fov:g})"


class Projection:
    """Result of :func:`project_points`.

    ``points`` is a flat ``array('f')`` of ``[screen_x, screen_y, depth,
    scale] * n`` (``scale`` is the on-screen size of one world unit, 0 for
    culled points); ``order`` is an ``array('I')`` of point indices from far to
    near (painter's order, culled points first).
    """

    __slots__ = ("points", "order")

    def __init__(self, points: "array[float]", order: "array[int]") -> None:
        self.points = points
        self.order = order

    def __len__(self) -> int:
        return len(self.points) // 4

    def __getitem__(self, i: int) -> Tuple[float, float, float, float]:
        n = len(self)
        j = i + n if i < 0 else i
        if not 0 <= j < n:
            raise IndexError(f"point index {i} out of range ({n} points)")
        p = self.points
        k = 4 * j
        return (p[k], p[k + 1], p[k + 2], p[k + 3])

    def visible(self, i: int) -> bool:
        """``False`` for points behind the camera."""
        return self[i][3] > 0

    def screen(self) -> List[Tuple[float, float]]:
        """``(x, y)`` per point in input order."""
        p = self.points
        return [(p[k], p[k + 1]) for k in range(0, len(p), 4)]

    def sorted(self, *, visible_only: bool = True) -> Iterator[Tuple[int, float, float, float, float]]:
        """``(index, x, y, depth, scale)`` in painter's order (far → near)."""
        p = self.points
        for i in self.order:
            k = 4 * i
            if visible_only and p[k + 3] <= 0:
                continue
            yield (i, p[k], p[k + 1], p[k + 2], p[k + 3])

    def __repr__(self) -> str:
        return f"Projection({len(self)} points)"


def _flat_points(points: Any, stride: int) -> Tuple[Union[bytes, List[float]], int]:
    if isinstance(points, (bytes, bytearray)):
        return bytes(points), stride
    if isinstance(points, array):
        if points.typecode == "f":
            return points.tobytes(), stride
        return points.tolist(), stride
    pts = points if isinstance(points, (list, tuple)) else list(points)
    if pts and isinstance(pts[0], (int, float)):
        return [float(v) for v in pts], stride
    flat: List[float] = []
    for p in pts:
        if len(p) == 3:
            flat.extend(p)
        elif len(p) == 2:
            flat.extend((p[0], p[1], 0.0))
        else:
            raise ValueError(f"points must be (x, y, z) tuples, got {p!r}")
    return flat, 3


class Projector:
    """Reusable bulk projector (keeps its native buffers between calls —
    use one per animated point cloud)."""

    __slots__ = ("_native",)

    def __init__(self) -> None:
        self._native = _anikit.NativeProjector()

    def project(
        self,
        points: Any,
        *,
        camera: Optional[Camera] = None,
        size: Sequence[float] = (800.0, 600.0),
        model: Optional[MatrixLike] = None,
        view_projection: Optional[MatrixLike] = None,
        sort: bool = True,
        stride: int = 3,
        offsets: Sequence[int] = (0, 1, 2),
    ) -> Projection:
        """See :func:`project_points`."""
        w, h = float(size[0]), float(size[1])
        if not (w > 0 and h > 0):
            raise ValueError("size must be positive (width, height)")
        if view_projection is not None:
            vp = as_matrix(view_projection)
        else:
            vp = (camera if camera is not None else Camera()).view_projection(w / h)
        mm = Matrix() if model is None else as_matrix(model)
        src, stride = _flat_points(points, int(stride))
        if len(offsets) != 3:
            raise ValueError("offsets must be (x, y, z) channel offsets")
        out, order = self._native.project(
            src,
            stride,
            (int(offsets[0]), int(offsets[1]), int(offsets[2])),
            list(mm.values),
            list(vp.values),
            w,
            h,
            sort,
        )
        pts = array("f")
        pts.frombytes(out)
        idx = array("I")
        idx.frombytes(order)
        return Projection(pts, idx)


def project_points(
    points: Any,
    *,
    camera: Optional[Camera] = None,
    size: Sequence[float] = (800.0, 600.0),
    model: Optional[MatrixLike] = None,
    view_projection: Optional[MatrixLike] = None,
    sort: bool = True,
    stride: int = 3,
    offsets: Sequence[int] = (0, 1, 2),
) -> Projection:
    """Project 3D points to a ``size = (width, height)`` viewport (y down).

    ``points`` is a sequence of ``(x, y, z)`` (or ``(x, y)``) tuples, a flat
    sequence / ``array('f')`` read at ``stride`` with x/y/z channel
    ``offsets``, or raw native ``f32`` bytes. Points are transformed by
    ``model`` (any :data:`MatrixLike`, e.g. a CSS string) and then by the
    camera (default ``Camera()``) or an explicit ``view_projection``. With
    ``sort`` the result's ``order`` is depth-sorted far → near (radix sort,
    GIL released — fine for 10⁵+ points).

    >>> pr = project_points([(0, 0, 0), (0, 0, 500), (0, 0, -500)], size=(200, 200))
    >>> [round(v, 3) for v in pr[0][:2]], list(pr.order)
    ([100.0, 100.0], [2, 0, 1])
    """
    return Projector().project(
        points,
        camera=camera,
        size=size,
        model=model,
        view_projection=view_projection,
        sort=sort,
        stride=stride,
        offsets=offsets,
    )
