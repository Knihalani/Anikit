"""Solid 3D: lit, z-buffered, instanced meshes rendered on the CPU into numpy
frames (needs ``numpy``).

The same renderer as the TypeScript ``renderer3d``: triangles are clipped at
the near plane, shaded per pixel (Blinn–Phong key and fill lights, a fresnel
rim, chrome reflections of a procedural studio environment, thin-film
iridescence, glowing UV grid lines and travelling light pulses, depth fog)
and tone-mapped so hot colours roll off towards white::

    import anikit
    from anikit import raster
    from anikit.transform import Camera

    r3 = raster.Renderer3D(640, 360)
    knot = raster.torus_knot(p=2, q=3, radius=300, tube=58, segments=600, sides=28)
    floor = raster.plane(40)
    cam = Camera((0, 250, 1400), (0, 0, 0), fov=45)
    frame = r3.render(cam, [
        raster.Item(knot, material=raster.Material(metal=0.9, specular=1.4, shininess=120,
                                                   lines=1.2, line_color=(0.3, 0.8, 1.6), grid=(0, 6))),
        raster.Item(floor, raster.instances(1, y=-420, scale=6000), mirror=True),
    ], background="#04050a", mirror=-420, sky=(0.05, 0.07, 0.16), strips=(2.2, 2.3, 2.6))
    frame.shape                                   # (360, 640, 4) uint8, writable
    anikit.video.post(frame, bloom=0.8, vignette=0.4)   # film grade, in place

Colours for 3D are CSS strings or ``(r, g, b)`` floats in 0…1 (values above
1 are hot and glow). Instance transforms live in an ``(n, 15)`` float32
numpy array (a column-major 3×4 affine, then rgb) that you can update in bulk.

Also here: :func:`shatter` (break a mesh into flying shards) and
:func:`flight_path` (smooth camera flights with banking).
"""

from __future__ import annotations

import dataclasses
import math
import time
from dataclasses import dataclass
from typing import Any, Callable, List, Mapping, Optional, Sequence, Tuple, Union

import numpy as np

from ._anikit import NativeMesh, NativeRaster, NativeTexture
from ._cloud import _rgb
from ._flight import FlightPath, FlightPose, flight_path
from .transform import Camera, MatrixLike, as_matrix

__all__ = [
    "Renderer3D",
    "Mesh",
    "cube",
    "sphere",
    "torus",
    "torus_knot",
    "plane",
    "cylinder",
    "heightfield",
    "custom",
    "Texture",
    "texture",
    "Instances",
    "instances",
    "Material",
    "Lighting",
    "Item",
    "Points",
    "Mirror",
    "Shards",
    "shatter",
    "FlightPath",
    "FlightPose",
    "flight_path",
    "STRIDE",
]

#: Floats per instance: a column-major 3×4 affine ``[r00 r10 r20 r01 r11 r21 r02 r12 r22 tx ty tz]`` then rgb.
STRIDE = 15

ColorLike = Union[str, Sequence[float]]
Vec3 = Tuple[float, float, float]


def _f32(a: Any, name: str) -> bytes:
    arr = np.ascontiguousarray(a, dtype=np.float32)
    if not np.isfinite(arr).all():
        raise ValueError(f"{name} must be finite")
    return arr.tobytes()


def _u32(a: Any, name: str) -> bytes:
    arr = np.asarray(a)
    if arr.size and arr.dtype.kind not in "iu":
        if not np.array_equal(arr, np.floor(arr)):
            raise ValueError(f"{name} must be integers")
    if arr.size and arr.min() < 0:
        raise ValueError(f"{name} must be non-negative")
    return np.ascontiguousarray(arr, dtype=np.uint32).tobytes()


# ---------------------------------------------------------------------------
# Meshes
# ---------------------------------------------------------------------------


class Mesh:
    """An immutable triangle mesh (positions, normals, UVs, triangles).

    Build one with :func:`cube`, :func:`sphere`, :func:`torus`,
    :func:`torus_knot`, :func:`plane`, :func:`cylinder`, :func:`heightfield`
    or :func:`custom`. ``aspect`` is a free depth/width hint for meshes built
    from images (multiply an instance's ``sz`` by it).
    """

    __slots__ = ("_native", "aspect")

    def __init__(self, native: NativeMesh) -> None:
        if not isinstance(native, NativeMesh):
            raise TypeError("Mesh wraps a NativeMesh; use cube(), sphere(), custom(), ...")
        self._native = native
        self.aspect = 1.0

    @property
    def native(self) -> NativeMesh:
        return self._native

    @property
    def vertex_count(self) -> int:
        return self._native.vertex_count

    @property
    def triangle_count(self) -> int:
        return self._native.triangle_count

    @property
    def positions(self) -> np.ndarray:
        """``(vertex_count, 3)`` float32 (read-only copy)."""
        return np.frombuffer(self._native.positions(), np.float32).reshape(-1, 3)

    @property
    def normals(self) -> np.ndarray:
        """``(vertex_count, 3)`` float32 (read-only copy)."""
        return np.frombuffer(self._native.normals(), np.float32).reshape(-1, 3)

    @property
    def uvs(self) -> np.ndarray:
        """``(vertex_count, 2)`` float32, or ``(0, 2)`` when the mesh has none."""
        return np.frombuffer(self._native.uvs(), np.float32).reshape(-1, 2)

    @property
    def indices(self) -> np.ndarray:
        """``(triangle_count, 3)`` uint32 vertex indices."""
        return np.frombuffer(self._native.indices(), np.uint32).reshape(-1, 3)

    def __repr__(self) -> str:
        return f"Mesh(vertices={self.vertex_count}, triangles={self.triangle_count})"


def cube() -> Mesh:
    """Unit cube centred on the origin (−0.5…0.5), flat-shaded; each face has UVs 0…1."""
    return Mesh(NativeMesh.cube())


def sphere(rings: int = 16, segments: int = 24) -> Mesh:
    """Unit sphere (radius 1)."""
    return Mesh(NativeMesh.sphere(int(rings), int(segments)))


def torus(radius: float = 300.0, tube: float = 60.0, segments: int = 96, sides: int = 24) -> Mesh:
    """A torus in the XY plane."""
    return Mesh(NativeMesh.torus(float(radius), float(tube), int(segments), int(sides)))


def torus_knot(
    p: float = 2.0,
    q: float = 5.0,
    radius: float = 300.0,
    spread: float = 120.0,
    tube: float = 40.0,
    segments: int = 360,
    sides: int = 16,
) -> Mesh:
    """A (p, q) torus knot tube; ``u`` runs along the tube and ``v`` around it."""
    return Mesh(NativeMesh.torus_knot(float(p), float(q), float(radius), float(spread), float(tube), int(segments), int(sides)))


def plane(cells: Union[int, Tuple[int, int]] = 1) -> Mesh:
    """A flat grid in the XZ plane (−0.5…0.5; scale it with the instance)
    facing +Y with ``cells`` (or ``(x, z)``) subdivisions; UVs span 0…1.
    Floors and horizons: pair with a material's ``lines`` + ``grid``."""
    if isinstance(cells, (tuple, list)):
        nx, nz = cells
    else:
        nx = nz = cells
    return Mesh(NativeMesh.plane(int(nx), int(nz)))


def cylinder(segments: int = 32, caps: bool = True) -> Mesh:
    """Capped cylinder, radius 1, height 1 (Y −0.5…0.5); ``u`` runs around, ``v`` along."""
    return Mesh(NativeMesh.cylinder(int(segments), bool(caps)))


def heightfield(
    width: int,
    depth: int,
    heights: Union[Any, Callable[[np.ndarray, np.ndarray], Any]],
    *,
    skip: Optional[float] = None,
) -> Mesh:
    """A heightfield in the XZ plane (−0.5…0.5) from a ``depth × width`` grid
    of heights (row = z), with smooth normals.

    ``heights`` is an array (``(depth, width)`` or flat) or a function
    ``f(x, z)`` called once with grids of ``x`` and ``z`` in 0…1 (numpy
    arrays, so write it vectorised: ``lambda x, z: np.sin(x * 6) * 0.05``).
    Cells whose four corners are all ``<= skip`` are cut out.
    """
    w, d = int(width), int(depth)
    if w < 2 or d < 2:
        raise ValueError(f"heightfield needs at least 2x2 samples, got {w}x{d}")
    if callable(heights):
        z, x = np.mgrid[0:d, 0:w].astype(np.float64)
        hs = np.broadcast_to(np.asarray(heights(x / (w - 1), z / (d - 1)), dtype=np.float64), (d, w))
    else:
        hs = np.asarray(heights, dtype=np.float64)
        if hs.size != w * d:
            raise ValueError(f"heightfield needs {w * d} heights ({w} x {d}), got {hs.size}")
    return Mesh(NativeMesh.heightfield(w, d, _f32(hs.reshape(-1), "heights"), -math.inf if skip is None else float(skip)))


def custom(positions: Any, normals: Any, indices: Any, uvs: Any = None) -> Mesh:
    """Your own geometry: xyz ``positions`` and ``normals`` per vertex
    (``(n, 3)`` or flat), three ``indices`` per triangle, optional uv pairs."""
    return Mesh(
        NativeMesh.custom(
            _f32(positions, "positions"),
            _f32(normals, "normals"),
            _u32(indices, "indices"),
            None if uvs is None else _f32(uvs, "uvs"),
        )
    )


# ---------------------------------------------------------------------------
# Textures
# ---------------------------------------------------------------------------


class Texture:
    """An image for :attr:`Material.texture` (mipmapped, sampled bilinearly
    with wrapping; row 0 is the top, v = 0)."""

    __slots__ = ("_native",)

    def __init__(self, image: Any) -> None:
        img = np.asarray(image)
        if img.ndim != 3 or img.shape[2] not in (3, 4):
            raise ValueError(f"texture images are (height, width, 3 or 4) arrays, got shape {img.shape}")
        if img.dtype != np.uint8:
            if img.dtype.kind == "f":
                img = np.clip(np.rint(img * 255.0), 0, 255)
            img = img.astype(np.uint8)
        if img.shape[2] == 3:
            img = np.concatenate([img, np.full(img.shape[:2] + (1,), 255, np.uint8)], axis=2)
        h, w = img.shape[:2]
        self._native = NativeTexture(w, h, np.ascontiguousarray(img).tobytes())

    @property
    def native(self) -> NativeTexture:
        return self._native

    @property
    def width(self) -> int:
        return self._native.width

    @property
    def height(self) -> int:
        return self._native.height

    def __repr__(self) -> str:
        return f"Texture({self.width}x{self.height})"


def texture(image: Any) -> Texture:
    """Upload an ``(h, w, 4)`` (or ``(h, w, 3)``) image for materials: uint8,
    or floats in 0…1. Row 0 is the top."""
    return Texture(image)


# ---------------------------------------------------------------------------
# Instances
# ---------------------------------------------------------------------------


def _is_single_color(c: Any) -> bool:
    if isinstance(c, str):
        return True
    if isinstance(c, np.ndarray):
        return c.ndim == 1
    try:
        return len(c) == 3 and all(isinstance(v, (int, float, np.number)) for v in c)
    except TypeError:
        return False


def _colors(c: Any) -> np.ndarray:
    """``(3,)`` for one colour, ``(k, 3)`` for many (0…1 floats)."""
    if c is None:
        return np.ones(3)
    if _is_single_color(c):
        return np.asarray(_rgb(c), dtype=np.float64)
    if isinstance(c, np.ndarray) and c.dtype.kind in "iuf":
        if c.ndim != 2 or c.shape[1] != 3:
            raise ValueError(f"colour arrays are (n, 3) rgb in 0..1, got shape {c.shape}")
        return c.astype(np.float64)
    return np.asarray([_rgb(v) for v in c], dtype=np.float64)


class Instances:
    """Per-instance transforms and colours for one draw: an ``(n, 15)`` float32
    array (:attr:`data`), each row a column-major 3×4 affine then rgb.

    Write rows with :meth:`set` (scalars or numpy arrays, vectorised), or edit
    :attr:`data`, :attr:`positions` and :attr:`colors` in place::

        cubes = Instances(5000)
        cubes.set(x=xs, y=ys, z=zs, ry=angles, scale=12, color="#ff3b14")
        cubes.positions[:, 1] += 5            # move all of them up
    """

    __slots__ = ("_buf", "_data")

    def __init__(self, count: int) -> None:
        n = int(count)
        if n < 0:
            raise ValueError("count must be >= 0")
        # Backed by a bytearray so a render hands it to the native side without an extra copy.
        self._buf = bytearray(n * STRIDE * 4)
        self._data = np.frombuffer(self._buf, np.float32).reshape(n, STRIDE)
        if n:
            self.set()

    @property
    def data(self) -> np.ndarray:
        """The ``(n, 15)`` float32 instance array (writable view)."""
        return self._data

    @property
    def count(self) -> int:
        return self._data.shape[0]

    def __len__(self) -> int:
        return self._data.shape[0]

    @property
    def positions(self) -> np.ndarray:
        """``(n, 3)`` translation columns (writable view)."""
        return self._data[:, 9:12]

    @property
    def colors(self) -> np.ndarray:
        """``(n, 3)`` rgb (writable view)."""
        return self._data[:, 12:15]

    def set(
        self,
        index: Any = slice(None),
        *,
        x: Any = 0.0,
        y: Any = 0.0,
        z: Any = 0.0,
        rx: Any = 0.0,
        ry: Any = 0.0,
        rz: Any = 0.0,
        scale: Any = 1.0,
        sx: Any = 1.0,
        sy: Any = 1.0,
        sz: Any = 1.0,
        color: Any = None,
    ) -> "Instances":
        """Set instance(s) ``index`` (an int, slice, index array or mask;
        default all). Every field is a scalar or an array with one value per
        selected instance. Euler rotations are degrees, applied X, then Y,
        then Z; ``scale`` multiplies ``sx``/``sy``/``sz``. Fields left out take
        their defaults (origin, no rotation, scale 1, white), as in
        TypeScript's ``Instances.set``."""
        rows = np.arange(self.count)[index]
        r = math.pi / 180.0
        f = [np.asarray(v, dtype=np.float64) for v in (x, y, z, rx, ry, rz, scale, sx, sy, sz)]
        x_, y_, z_, ax, ay, az, s, kx, ky, kz = f
        cx, snx = np.cos(ax * r), np.sin(ax * r)
        cy, sny = np.cos(ay * r), np.sin(ay * r)
        cz, snz = np.cos(az * r), np.sin(az * r)
        kx, ky, kz = kx * s, ky * s, kz * s
        # R = Rz · Ry · Rx (columns), then scale per axis.
        cols = (
            cz * cy * kx,
            snz * cy * kx,
            -sny * kx,
            (cz * sny * snx - snz * cx) * ky,
            (snz * sny * snx + cz * cx) * ky,
            cy * snx * ky,
            (cz * sny * cx + snz * snx) * kz,
            (snz * sny * cx - cz * snx) * kz,
            cy * cx * kz,
            x_,
            y_,
            z_,
        )
        shape = np.shape(rows)
        block = np.stack([np.broadcast_to(c, shape) for c in cols], axis=-1)
        self._data[rows, :12] = block
        self._data[rows, 12:] = np.broadcast_to(_colors(color), shape + (3,))
        return self

    def __repr__(self) -> str:
        return f"Instances({self.count})"


def instances(count: int, init: Optional[Callable[[int], Mapping[str, Any]]] = None, **fields: Any) -> Instances:
    """Create an instance buffer. Either pass vectorised ``fields`` (as for
    :meth:`Instances.set`: ``instances(900, x=xs, z=zs, sy=heights,
    color=(0.04, 0.045, 0.06))``) or an ``init(i)`` function returning a dict
    of fields per instance (slower, like TypeScript's ``instances(n, init)``)."""
    inst = Instances(count)
    if init is not None:
        if fields:
            raise TypeError("pass either init or fields, not both")
        for i in range(inst.count):
            inst.set(i, **init(i))
    elif fields:
        inst.set(**fields)
    return inst


# ---------------------------------------------------------------------------
# Materials, lighting, draw items
# ---------------------------------------------------------------------------


def _v3(v: Any, name: str) -> Vec3:
    t = tuple(float(c) for c in v)
    if len(t) != 3:
        raise ValueError(f"{name} must have 3 components, got {v!r}")
    return (t[0], t[1], t[2])


def _v2(v: Any, name: str) -> Tuple[float, float]:
    t = tuple(float(c) for c in v)
    if len(t) != 2:
        raise ValueError(f"{name} must have 2 components, got {v!r}")
    return (t[0], t[1])


@dataclass
class Material:
    """Surface options for a draw (all optional; defaults match the core).

    * ``ambient``, ``diffuse``, ``specular``, ``shininess``, ``rim``:
      Blinn–Phong key + fill light and a fresnel rim.
    * ``emissive``: base colour × emissive added unlit (glows under bloom).
    * ``metal`` (0–1): mirror reflection of the environment (``sky``,
      ``horizon``, ``ground`` and studio ``strips`` of the :class:`Lighting`).
    * ``iridescence`` (0–1) and ``hue``: thin-film rainbow tint on reflections.
    * ``lines``, ``line_color``, ``grid`` (cells along u, v; 0 = none that
      way), ``line_width`` (half-width in cells): antialiased glowing lines in
      UV space. Cube faces are 0–1 (outlines with ``grid=(1, 1)``); tubes and
      knots run u along, v around.
    * ``pulse``, ``pulse_count``, ``pulse_phase``, ``pulse_dir``: pulses of
      ``line_color`` travelling along u (animate ``pulse_phase``);
      ``pulse_dir`` offsets each instance's phase by ``dot(position, pulse_dir)``.
    * ``texture`` (:class:`Texture`), ``texture_mode`` (``"albedo"`` tints
      the lit colour, ``"emissive"`` adds it unlit × ``texture_gain``),
      ``texture_repeat`` (UV tiling).
    """

    ambient: float = 0.22
    diffuse: float = 0.85
    specular: float = 0.5
    shininess: float = 40.0
    rim: float = 0.35
    emissive: float = 0.0
    metal: float = 0.0
    iridescence: float = 0.0
    hue: float = 0.0
    lines: float = 0.0
    line_color: ColorLike = (1.0, 1.0, 1.0)
    grid: Tuple[float, float] = (8.0, 8.0)
    line_width: float = 0.04
    pulse: float = 0.0
    pulse_count: float = 1.0
    pulse_phase: float = 0.0
    pulse_dir: Vec3 = (0.0, 0.0, 0.0)
    texture: Optional[Texture] = None
    texture_mode: str = "albedo"
    texture_repeat: Tuple[float, float] = (1.0, 1.0)
    texture_gain: float = 1.0

    def _pack(self) -> List[float]:
        if self.texture_mode not in ("albedo", "emissive"):
            raise ValueError(f"texture_mode must be 'albedo' or 'emissive', got {self.texture_mode!r}")
        if self.texture is not None and not isinstance(self.texture, Texture):
            raise TypeError("material.texture must be a Texture (see anikit.raster.texture)")
        mode = 0.0 if self.texture is None else (2.0 if self.texture_mode == "emissive" else 1.0)
        return [
            float(self.ambient),
            float(self.diffuse),
            float(self.specular),
            float(self.shininess),
            float(self.rim),
            float(self.emissive),
            float(self.metal),
            float(self.iridescence),
            float(self.hue),
            float(self.lines),
            *_rgb(self.line_color),
            *_v2(self.grid, "grid"),
            float(self.line_width),
            float(self.pulse),
            float(self.pulse_count),
            float(self.pulse_phase),
            *_v3(self.pulse_dir, "pulse_dir"),
            mode,
            *_v2(self.texture_repeat, "texture_repeat"),
            float(self.texture_gain),
        ]


@dataclass
class Lighting:
    """Lights, fog and the environment reflected by metal materials.

    ``key``/``fill`` are directions *towards* the lights; colours are CSS
    strings or 0…1 rgb. ``fog=(near, far)`` fades to ``fog_color`` (default:
    the render's background). ``sky``/``horizon``/``ground`` colour the
    reflected environment, ``strips`` adds sharp studio light strips.
    """

    key: Vec3 = (0.4, 0.8, 0.6)
    key_color: ColorLike = (1.0, 1.0, 1.0)
    fill: Vec3 = (-0.6, -0.2, 0.4)
    fill_color: ColorLike = (0.25, 0.28, 0.35)
    rim_color: ColorLike = (1.0, 1.0, 1.0)
    fog: Optional[Tuple[float, float]] = None
    fog_color: Optional[ColorLike] = None
    sky: ColorLike = (0.25, 0.3, 0.55)
    horizon: ColorLike = (1.0, 0.85, 0.7)
    ground: ColorLike = (0.05, 0.05, 0.07)
    strips: ColorLike = (0.0, 0.0, 0.0)

    def _pack(self, background: Vec3) -> List[float]:
        fog = (0.0, math.inf) if self.fog is None else _v2(self.fog, "fog")
        return [
            *_v3(self.key, "key"),
            *_rgb(self.key_color),
            *_v3(self.fill, "fill"),
            *_rgb(self.fill_color),
            *_rgb(self.rim_color),
            fog[0],
            fog[1] if fog[1] > 0 else math.inf,
            *(background if self.fog_color is None else _rgb(self.fog_color)),
            *_rgb(self.sky),
            *_rgb(self.horizon),
            *_rgb(self.ground),
            *_rgb(self.strips),
        ]


_BLENDS = ("opaque", "additive", "gloss")


@dataclass
class Item:
    """One draw: ``mesh`` once per instance.

    ``instances`` is an :class:`Instances` (or an ``(n, 15)`` array; omit for
    one untransformed white instance). ``material`` is a :class:`Material` or
    a dict of its fields. ``cull=False`` shows back faces (tumbling shards).
    ``blend``: ``"opaque"`` (default), ``"additive"`` (light: beams,
    holograms; depth-tested, not occluding; draw after opaque items) or
    ``"gloss"`` (opaque, blended over what is already drawn by ``gloss``).
    ``mirror=True`` marks the glossy surface for the render's ``mirror``
    option (the floor); ``reflect=False`` leaves the item out of the
    mirrored pass.
    """

    mesh: Mesh
    instances: Optional[Any] = None
    material: Optional[Union[Material, Mapping[str, Any]]] = None
    cull: bool = True
    blend: str = "opaque"
    mirror: bool = False
    reflect: bool = True
    gloss: float = 0.5


@dataclass
class Points:
    """Light points drawn into the 3D frame (stars, dust, sparks): they add
    light, hide behind solid geometry drawn before them, and streak along
    their screen motion since the previous render.

    ``points`` is ``(n, 3)`` / flat xyz, or anything with a ``positions``
    attribute (a :class:`anikit.PointCloud`). ``colors`` is ``(n, 3)`` rgb
    (may exceed 1; default white). ``streak``: 0 round, 1 shutter-accurate,
    3+ warp. ``fog=(near, far)`` fades them by camera distance.
    """

    points: Any
    colors: Any = None
    size: float = 2.0
    intensity: float = 1.0
    streak: float = 0.0
    fog: Optional[Tuple[float, float]] = None


@dataclass
class Mirror:
    """A mirror floor: the scene reflected about the plane ``y``, under items
    marked ``mirror=True`` drawn glossily with ``strength`` (0–1)."""

    y: float
    strength: float = 0.5


def _item(it: Any) -> Union[Item, Points]:
    if isinstance(it, (Item, Points)):
        return it
    if isinstance(it, Mesh):
        return Item(it)
    if isinstance(it, Mapping):
        return Points(**it) if "points" in it else Item(**it)
    raise TypeError(f"render items are Item, Points, Mesh or dicts, got {type(it).__name__}")


def _mirror(m: Any) -> Optional[Mirror]:
    if m is None or m is False:
        return None
    if isinstance(m, Mirror):
        return m
    if isinstance(m, (int, float)):
        return Mirror(float(m))
    if isinstance(m, Mapping):
        return Mirror(**m)
    y, *rest = m
    return Mirror(float(y), *(float(v) for v in rest))


def _material(m: Any) -> Material:
    if m is None:
        return _DEFAULT_MATERIAL
    if isinstance(m, Material):
        return m
    if isinstance(m, Mapping):
        return Material(**m)
    raise TypeError(f"material must be a Material or a dict, got {type(m).__name__}")


_DEFAULT_MATERIAL = Material()


def _instance_data(inst: Any) -> Any:
    if inst is None:
        return None
    if isinstance(inst, Instances):
        return inst._buf
    arr = np.asarray(inst, dtype=np.float32)
    if arr.ndim == 1:
        arr = arr.reshape(-1, STRIDE) if arr.size % STRIDE == 0 else arr
    if arr.ndim != 2 or arr.shape[1] != STRIDE:
        raise ValueError(f"instances are (n, {STRIDE}) float arrays, got shape {arr.shape}")
    return np.ascontiguousarray(arr).tobytes()


def _point_arrays(p: Points) -> Tuple[bytes, Optional[bytes]]:
    src = p.points
    cols = p.colors
    if hasattr(src, "positions") and not isinstance(src, np.ndarray):
        if cols is None and getattr(src, "colors", None) is not None and not callable(src.colors):
            cols = src.colors
        src = src.positions
    pts = np.asarray(src, dtype=np.float32).reshape(-1)
    n = pts.size // 3
    pts = pts[: n * 3]
    colors = None
    if cols is not None:
        c = np.asarray(cols, dtype=np.float32).reshape(-1)
        if c.size < n * 3:
            raise ValueError(f"colors need {n * 3} values (rgb per point), got {c.size}")
        colors = np.ascontiguousarray(c[: n * 3]).tobytes()
    return np.ascontiguousarray(pts).tobytes(), colors


def _bg(c: Any) -> Vec3:
    if isinstance(c, int) and not isinstance(c, bool):
        if not 0 <= c <= 0xFFFFFF:
            raise ValueError(f"background must be 0xRRGGBB, got {c:#x}")
        return ((c >> 16 & 255) / 255.0, (c >> 8 & 255) / 255.0, (c & 255) / 255.0)
    r, g, b = _rgb(c)
    return (r, g, b)


def _mirror_matrix(m: Sequence[float], y: float) -> List[float]:
    """View-projection of the scene reflected about the plane ``y`` (the
    second column negated, the translation column shifted)."""
    mr = list(m)
    for r in range(4):
        mr[4 + r] = -m[4 + r]
        mr[12 + r] = m[12 + r] + 2 * y * m[4 + r]
    return mr


# ---------------------------------------------------------------------------
# Renderer
# ---------------------------------------------------------------------------


class Renderer3D:
    """A solid 3D renderer for ``width × height`` frames.

    :meth:`render` returns each frame as a new writable ``(height, width, 4)``
    uint8 numpy array (one copy of the native colour buffer per call).
    ``triangles`` and ``last_ms`` report the last frame.
    """

    __slots__ = ("_native", "triangles", "last_ms", "_prev_vp")

    def __init__(self, width: int, height: int) -> None:
        self._native = NativeRaster(int(width), int(height))
        #: Triangles rasterised by the last render.
        self.triangles = 0
        #: Milliseconds the last render took.
        self.last_ms = 0.0
        self._prev_vp: Optional[List[float]] = None

    @property
    def native(self) -> NativeRaster:
        return self._native

    @property
    def width(self) -> int:
        return self._native.width

    @property
    def height(self) -> int:
        return self._native.height

    @property
    def size(self) -> Tuple[int, int]:
        return (self._native.width, self._native.height)

    def resize(self, width: int, height: int) -> "Renderer3D":
        self._native.resize(int(width), int(height))
        self._prev_vp = None
        return self

    def view(self, camera: Any) -> Tuple[List[float], Vec3]:
        """``(view_projection, eye)`` for ``camera`` at this renderer's aspect:
        a :class:`~anikit.transform.Camera` (its ``roll`` included), or a
        ``(matrix, eye)`` pair you computed yourself."""
        if isinstance(camera, Camera):
            vp = camera.view_projection(self.width / self.height)
            return list(vp.values), camera.position
        try:
            m, eye = camera
        except (TypeError, ValueError):
            raise TypeError("camera must be an anikit.transform.Camera or a (view_projection, eye) pair") from None
        return list(as_matrix(m).values), _v3(eye, "eye")

    def render(
        self,
        camera: Union[Camera, Tuple[MatrixLike, Sequence[float]]],
        items: Sequence[Any],
        *,
        background: Any = "#000000",
        transparent: bool = False,
        mirror: Any = None,
        lighting: Optional[Lighting] = None,
        **light: Any,
    ) -> np.ndarray:
        """Render ``items`` (:class:`Item`, :class:`Points`, bare
        :class:`Mesh` or dicts of their fields) through ``camera``.

        Lighting comes from ``lighting`` and/or keyword overrides
        (``key``, ``key_color``, ``fill``, ``fill_color``, ``rim_color``,
        ``fog``, ``fog_color``, ``sky``, ``horizon``, ``ground``, ``strips``;
        see :class:`Lighting`). ``transparent=True`` clears to alpha 0.

        ``mirror`` (a :class:`Mirror`, a ``y``, ``(y, strength)`` or a dict)
        makes a mirror floor exactly like the TypeScript renderer: the scene
        reflected about ``y`` (winding flipped), then the depth is cleared,
        then items with ``mirror=True`` are drawn glossily over the
        reflection, then everything else.

        Returns the frame as a writable ``(height, width, 4)`` uint8 array.
        """
        t0 = time.perf_counter()
        nat = self._native
        bg = _bg(background)
        hexrgb = (round(bg[0] * 255) << 16) | (round(bg[1] * 255) << 8) | round(bg[2] * 255)
        nat.clear(max(0, min(0xFFFFFF, hexrgb)), 0 if transparent else 255)
        vp, eye = self.view(camera)
        lights = lighting if lighting is not None else Lighting()
        if light:
            lights = dataclasses.replace(lights, **light)
        lpack = lights._pack(bg)
        its = [_item(it) for it in items]
        for it in its:
            if isinstance(it, Item) and it.blend not in _BLENDS:
                raise ValueError(f"blend must be one of {_BLENDS}, got {it.blend!r}")
        meshes = [it for it in its if isinstance(it, Item)]
        tris = 0

        def draw(it: Item, m: List[float], e: Vec3, *, flip: bool = False, blend: Optional[str] = None, gloss: float = 0.5) -> int:
            mat = _material(it.material)
            tex = mat.texture.native if mat.texture is not None else None
            return int(
                nat.draw(
                    it.mesh.native,
                    _instance_data(it.instances),
                    m,
                    e,
                    mat._pack(),
                    lpack,
                    cull=bool(it.cull),
                    blend=blend or it.blend,
                    flip_winding=flip,
                    gloss=gloss,
                    texture=tex,
                )
            )

        mir = _mirror(mirror)
        if mir is not None:
            # 1. The scene reflected about y (winding flips), 2. clear depth,
            # 3. the glossy surface blended over the reflection, 4. the scene itself.
            mr = _mirror_matrix(vp, mir.y)
            er = (eye[0], 2 * mir.y - eye[1], eye[2])
            for it in meshes:
                if not it.mirror and it.reflect and it.blend != "additive":
                    tris += draw(it, mr, er, flip=True, blend="opaque")
            nat.clear_depth()
            for it in meshes:
                if it.mirror:
                    tris += draw(it, vp, eye, blend="gloss", gloss=mir.strength)
        for it in its:
            if isinstance(it, Points):
                self._draw_points(it, vp, eye)
                continue
            if mir is not None and it.mirror:
                continue
            tris += draw(it, vp, eye, gloss=it.gloss)
        self._prev_vp = vp
        self.triangles = tris
        out = np.frombuffer(nat.rgba(), np.uint8).reshape(self.height, self.width, 4)
        self.last_ms = (time.perf_counter() - t0) * 1000.0
        return out

    def _draw_points(self, p: Points, vp: List[float], eye: Vec3) -> None:
        pts, cols = _point_arrays(p)
        if not pts:
            return
        fog = (0.0, math.inf) if p.fog is None else _v2(p.fog, "fog")
        self._native.points(
            pts,
            vp,
            eye,
            colors=cols,
            prev_view_projection=self._prev_vp,
            size=float(p.size),
            intensity=float(p.intensity),
            streak=float(p.streak),
            fog=fog,
        )

    def frame(self) -> np.ndarray:
        """The current frame as a new ``(height, width, 4)`` uint8 array."""
        return np.frombuffer(self._native.rgba(), np.uint8).reshape(self.height, self.width, 4)

    def pixel(self, x: int, y: int) -> Tuple[int, int, int, int]:
        """``(r, g, b, a)`` of the last frame at ``(x, y)``."""
        return self._native.pixel(int(x), int(y))

    def __repr__(self) -> str:
        return f"Renderer3D({self.width}x{self.height})"


# ---------------------------------------------------------------------------
# Shatter
# ---------------------------------------------------------------------------

_TRI: Optional[Mesh] = None


def _unit_triangle() -> Mesh:
    global _TRI
    if _TRI is None:
        _TRI = custom([0, 0, 0, 1, 0, 0, 0, 1, 0], [0, 0, 1, 0, 0, 1, 0, 0, 1], [0, 1, 2], [0, 0, 1, 0, 0, 1])
    return _TRI


class Shards:
    """A mesh broken into per-triangle shards (see :func:`shatter`).

    Every triangle becomes one instance of a shared unit triangle
    (:attr:`mesh`), mapped onto it by its instance transform; :meth:`update`
    poses them all ``t`` seconds after the hit (vectorised). Draw with
    ``Item(shards.mesh, shards.instances, cull=False)`` or :meth:`item`.
    Deterministic: the same inputs give the same shards everywhere (and the
    same numbers as the TypeScript ``shatter``).
    """

    __slots__ = ("mesh", "instances", "count", "force", "spread", "spin", "gravity", "drag", "ripple", "_base", "_motion")

    def __init__(
        self,
        source: Mesh,
        *,
        center: Optional[Sequence[float]] = None,
        force: float = 600.0,
        spread: float = 0.5,
        spin: float = 360.0,
        gravity: float = 0.0,
        drag: float = 1.2,
        transform: Optional[Mapping[str, Any]] = None,
        color: Any = None,
        ripple: float = 0.0,
    ) -> None:
        self.force, self.spread, self.spin = float(force), float(spread), float(spin)
        self.gravity, self.drag, self.ripple = float(gravity), float(drag), float(ripple)
        tf = dict(transform or {})
        pos = source.positions.astype(np.float64)
        idx = source.indices.astype(np.int64)
        nt = idx.shape[0]
        self.count = nt
        # The source placement as an instance (float32, like the TypeScript buffer).
        m = Instances(1).set(0, **{k: v for k, v in tf.items() if k != "color"}).data[0].astype(np.float64)
        rot = np.array([[m[0], m[3], m[6]], [m[1], m[4], m[7]], [m[2], m[5], m[8]]])
        world = pos @ rot.T + m[9:12]
        a, b, c = world[idx[:, 0]], world[idx[:, 1]], world[idx[:, 2]]
        e1, e2 = b - a, c - a
        n = np.cross(e1, e2)
        ln = np.linalg.norm(n, axis=1)
        ln[ln == 0] = 1.0
        n = n / ln[:, None]
        cents = (a + b + c) / 3.0
        base = np.concatenate([a, e1, e2, n, cents], axis=1).astype(np.float32)
        if center is None:
            ctr = cents.sum(axis=0) / max(1, nt) if nt else np.zeros(3)
        else:
            ctr = np.asarray(_v3(center, "center"))
        motion = np.zeros((nt, 9), np.float32)
        # Deterministic per-shard randomness (the TypeScript generator, bit for bit).
        seed = (0x9E3779B9 ^ nt) & 0xFFFFFFFF
        j = self.spread
        for t in range(nt):
            vals = []
            for _ in range(8):
                seed = ((((seed ^ (seed >> 15)) * 0x2C1B3C6D) & 0xFFFFFFFF) + 0x6D2B79F5) & 0xFFFFFFFF
                vals.append(seed / 4294967296.0)
            ce = cents[t]
            d = ce - ctr
            dist = math.hypot(d[0], d[1], d[2])
            dd = dist or 1.0
            d = [d[0] / dd + (vals[0] - 0.5) * j, d[1] / dd + (vals[1] - 0.5) * j, d[2] / dd + (vals[2] - 0.5) * j]
            dl = math.hypot(*d) or 1.0
            ax = [vals[3] - 0.5, vals[4] - 0.5, vals[5] - 0.5]
            al = math.hypot(*ax) or 1.0
            motion[t] = (
                d[0] / dl,
                d[1] / dl,
                d[2] / dl,
                self.force * (1 - j / 2 + vals[6] * j),
                ax[0] / al,
                ax[1] / al,
                ax[2] / al,
                self.spin * (0.4 + vals[7] * 1.2),
                (dist / 1000) * self.ripple,
            )
        # Stored as float32 like the TypeScript buffers; maths in float64.
        self._base = base.astype(np.float64)
        self._motion = motion.astype(np.float64)
        self.mesh = _unit_triangle()
        self.instances = Instances(nt)
        if callable(color):
            cols = np.asarray([_rgb(color(i, tuple(float(v) for v in cents[i]))) for i in range(nt)], np.float64).reshape(-1, 3)
        else:
            cols = _colors(color if color is not None else tf.get("color"))
        self.instances.colors[:] = np.broadcast_to(cols, (nt, 3))
        self.update(0.0)

    def update(self, t: float) -> "Shards":
        """Pose every shard ``t`` seconds after the hit (``t <= 0``: the intact mesh)."""
        if not self.count:
            return self
        B, M = self._base, self._motion
        tt = np.maximum(0.0, float(t) - M[:, 8])
        drag, g = self.drag, self.gravity
        # Distance travelled with linear drag: v·(1 − e^(−drag·t))/drag.
        s = M[:, 3] * (1 - np.exp(-drag * tt)) / drag if drag > 0 else M[:, 3] * tt
        off = M[:, 0:3] * s[:, None]
        off[:, 1] -= 0.5 * g * tt * tt
        # Rotation about each shard's centroid (Rodrigues).
        ang = M[:, 7] * tt * math.pi / 180
        ux, uy, uz = M[:, 4], M[:, 5], M[:, 6]
        c, sn = np.cos(ang), np.sin(ang)
        C = 1 - c
        R = np.empty((self.count, 3, 3))
        R[:, 0, 0] = c + ux * ux * C
        R[:, 0, 1] = ux * uy * C - uz * sn
        R[:, 0, 2] = ux * uz * C + uy * sn
        R[:, 1, 0] = uy * ux * C + uz * sn
        R[:, 1, 1] = c + uy * uy * C
        R[:, 1, 2] = uy * uz * C - ux * sn
        R[:, 2, 0] = uz * ux * C - uy * sn
        R[:, 2, 1] = uz * uy * C + ux * sn
        R[:, 2, 2] = c + uz * uz * C
        D = self.instances.data
        D[:, 0:3] = np.einsum("nij,nj->ni", R, B[:, 3:6])  # e1
        D[:, 3:6] = np.einsum("nij,nj->ni", R, B[:, 6:9])  # e2
        D[:, 6:9] = np.einsum("nij,nj->ni", R, B[:, 9:12])  # normal
        # Origin: R·(p0 − centroid) + centroid + offset.
        D[:, 9:12] = np.einsum("nij,nj->ni", R, B[:, 0:3] - B[:, 12:15]) + B[:, 12:15] + off
        return self

    def item(self, material: Any = None, **kw: Any) -> Item:
        """An :class:`Item` drawing the shards (``cull=False`` by default)."""
        kw.setdefault("cull", False)
        return Item(self.mesh, self.instances, material, **kw)

    def __repr__(self) -> str:
        return f"Shards({self.count})"


def shatter(
    mesh: Mesh,
    *,
    center: Optional[Sequence[float]] = None,
    force: float = 600.0,
    spread: float = 0.5,
    spin: float = 360.0,
    gravity: float = 0.0,
    drag: float = 1.2,
    transform: Optional[Mapping[str, Any]] = None,
    color: Any = None,
    ripple: float = 0.0,
) -> Shards:
    """Break ``mesh`` into flying shards (every triangle is one).

    ``transform`` places the source like an instance (``dict(rx=20,
    scale=1.2)``); ``center`` is where the blast comes from (default: the
    placed mesh's centre); ``force`` the outward speed (units/s) with random
    ``spread`` (0–1); ``spin`` the tumble (degrees/s); ``gravity`` a downward
    acceleration; ``drag`` air drag per second; ``ripple`` delays each shard
    by its distance from the centre (s per 1000 units); ``color`` one colour
    or ``f(i, centroid) -> rgb`` (default: ``transform["color"]`` or white).

    ::

        shards = raster.shatter(knot, transform=dict(rx=20, scale=1.2), force=900, spin=540)
        shards.update(t - 3.2)        # seconds since the hit (<= 0: intact)
        r3.render(cam, [shards.item(material=raster.Material(metal=0.9))])
    """
    return Shards(
        mesh,
        center=center,
        force=force,
        spread=spread,
        spin=spin,
        gravity=gravity,
        drag=drag,
        transform=transform,
        color=color,
        ripple=ripple,
    )
