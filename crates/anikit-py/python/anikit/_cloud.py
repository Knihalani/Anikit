"""Point clouds that flow between shapes, and additive "glow" rendering.

:class:`PointCloud` keeps its particles natively: each one spring-follows a
blend between two target shapes with its own timing (so transitions ripple
through the cloud) plus a turbulence field. Morphs are organic and
interruptible: drive :attr:`PointCloud.mix` yourself (from a scroll position,
an audio level, ...), tween it with :meth:`PointCloud.morph`, or blast the
particles with :meth:`PointCloud.explode` and let them find their way home.

:class:`Splat` renders projected points as additive light into an RGBA pixel
buffer (sub-pixel splats, soft bokeh discs for near points, depth fog, a lens,
motion trails, tone mapping over a background colour). Together with the
projector from :mod:`anikit.transform` this goes cloud → project → splat::

    cloud = anikit.PointCloud(20_000, shapes=[sphere, cube], noise=1)
    glow = anikit.Splat(640, 360, background="#05060a", trail=0.6)
    cam = anikit.transform.Camera(position=(0, 0, 900), fov=40)
    cloud.morph(1, duration=2.0)
    for _ in range(120):
        cloud.step(1 / 60)
        frame = glow.render(cloud, camera=cam, size=1.4)   # RGBA bytes

Bulk data crosses the boundary as flat ``array('f')`` buffers (no numpy
needed). Frames are row-major RGBA ``bytes`` of ``width * height * 4``; with
Pillow installed, ``Image.frombytes("RGBA", glow.size, frame)`` turns one into
an image you can save as PNG or collect into a GIF.
"""

from __future__ import annotations

import math
from array import array
from typing import Any, Callable, List, Optional, Sequence, Tuple, Union

from . import color as _color
from ._anikit import NativeCloud, NativeSplat
from ._easing import Easing, EasingLike
from .transform import Camera, MatrixLike, Projection, Projector, _flat_points

__all__ = ["PointCloud", "ShapeInput", "Splat"]

#: A shape: ``[(x, y, z), ...]`` (``(x, y)`` means z = 0), a flat
#: ``[x, y, z, ...]`` sequence or ``array('f')``, raw native ``f32`` bytes, or
#: a function ``(i, count) -> (x, y, z)``.
ShapeInput = Union[
    bytes, "array[float]", Sequence[float], Sequence[Sequence[float]], Callable[[int, int], Sequence[float]]
]

#: The longest single native step (the core clamps larger steps for stability).
_MAX_DT = 0.05


def _shape_data(s: Any, count: int) -> Union[bytes, List[float]]:
    if callable(s):
        flat = array("f", bytes(4 * 3 * count))
        for i in range(count):
            p = s(i, count)
            if len(p) == 3:
                flat[3 * i], flat[3 * i + 1], flat[3 * i + 2] = p[0], p[1], p[2]
            elif len(p) == 2:
                flat[3 * i], flat[3 * i + 1] = p[0], p[1]
            else:
                raise ValueError(f"shape functions must return (x, y, z), got {p!r}")
        return flat.tobytes()
    data, _ = _flat_points(s, 3)
    return data


def _floats(raw: bytes) -> "array[float]":
    a = array("f")
    a.frombytes(raw)
    return a


class PointCloud:
    """``count`` particles flowing between target ``shapes``.

    Particles follow ``blend(shape[from_], shape[to], mix)``; each one starts
    its part of the transition at its own time (``spread``), springs toward
    its target (``stiffness``, ``damping``) and is stirred by turbulence
    (``noise`` at rest, plus ``swirl`` mid-transition, at spatial
    ``frequency``). All parameters can be changed live. The cloud starts
    snapped onto ``shapes[0]``.

    >>> line = [(i, 0, 0) for i in range(4)]
    >>> cloud = PointCloud(4, shapes=[line, lambda i, n: (0, i * 10, 0)], noise=0, swirl=0)
    >>> cloud.positions[:6].tolist()
    [0.0, 0.0, 0.0, 1.0, 0.0, 0.0]
    >>> _ = cloud.morph(1, duration=0.5).run(3)
    >>> max(abs(a - b) for a, b in zip(cloud.positions, cloud.get_shape(1))) < 0.01
    True
    """

    def __init__(
        self,
        count: int,
        *,
        shapes: Sequence[ShapeInput] = (),
        stiffness: float = 60.0,
        damping: float = 9.0,
        spread: float = 0.6,
        noise: float = 2.0,
        swirl: float = 40.0,
        frequency: float = 0.01,
        seed: int = 1,
    ) -> None:
        count = int(count)
        if count < 0:
            raise ValueError(f"count must be >= 0, got {count}")
        self._native = NativeCloud(count, 0, int(seed) & 0xFFFFFFFF)
        self.count = count
        self.set(stiffness=stiffness, damping=damping, spread=spread, noise=noise, swirl=swirl, frequency=frequency)
        #: Blend state: particles follow ``blend(shape[from_], shape[to], mix)``.
        self.from_ = 0
        self.to = 0
        self.mix = 0.0
        #: Simulated seconds.
        self.time = 0.0
        self._tween: Optional[Tuple[float, float, Easing]] = None
        for k, s in enumerate(shapes):
            self.shape(k, s)
        if shapes:
            self._native.snap(0)

    @property
    def native(self) -> NativeCloud:
        """The underlying native cloud (flat ``f32`` bytes)."""
        return self._native

    def __len__(self) -> int:
        return self.count

    # ---- parameters (live) -----------------------------------------------------

    def set(
        self,
        *,
        stiffness: Optional[float] = None,
        damping: Optional[float] = None,
        spread: Optional[float] = None,
        noise: Optional[float] = None,
        swirl: Optional[float] = None,
        frequency: Optional[float] = None,
    ) -> "PointCloud":
        """Update any of the tunables at once."""
        n = self._native
        if stiffness is not None:
            n.stiffness = float(stiffness)
        if damping is not None:
            n.damping = float(damping)
        if spread is not None:
            n.spread = float(spread)
        if noise is not None:
            n.noise = float(noise)
        if swirl is not None:
            n.swirl = float(swirl)
        if frequency is not None:
            n.frequency = float(frequency)
        return self

    @property
    def stiffness(self) -> float:
        """Spring stiffness toward the target (1/s², default 60)."""
        return self._native.stiffness

    @stiffness.setter
    def stiffness(self, v: float) -> None:
        self._native.stiffness = float(v)

    @property
    def damping(self) -> float:
        """Velocity damping (1/s, default 9)."""
        return self._native.damping

    @damping.setter
    def damping(self, v: float) -> None:
        self._native.damping = float(v)

    @property
    def spread(self) -> float:
        """0 = every particle moves at once; higher = a rippling transition (default 0.6)."""
        return self._native.spread

    @spread.setter
    def spread(self, v: float) -> None:
        self._native.spread = float(v)

    @property
    def noise(self) -> float:
        """Resting turbulence amplitude in world units (default 2)."""
        return self._native.noise

    @noise.setter
    def noise(self, v: float) -> None:
        self._native.noise = float(v)

    @property
    def swirl(self) -> float:
        """Extra turbulence while a particle is mid-transition (default 40)."""
        return self._native.swirl

    @swirl.setter
    def swirl(self, v: float) -> None:
        self._native.swirl = float(v)

    @property
    def frequency(self) -> float:
        """Spatial frequency of the turbulence (default 0.01)."""
        return self._native.frequency

    @frequency.setter
    def frequency(self, v: float) -> None:
        self._native.frequency = float(v)

    # ---- shapes ------------------------------------------------------------------

    @property
    def shape_count(self) -> int:
        return self._native.shape_count()

    def shape(self, k: int, s: ShapeInput) -> "PointCloud":
        """Define (or replace) shape ``k`` — see :data:`ShapeInput`. It needs
        exactly ``count`` points; ``k`` may be at most :attr:`shape_count`
        (shapes are added in order)."""
        self._native.set_shape(int(k), _shape_data(s, self.count))
        return self

    def get_shape(self, k: int) -> "array[float]":
        """Shape ``k`` as a flat ``array('f')`` ``[x, y, z] * count``."""
        return _floats(self._native.shape(int(k)))

    def _check(self, k: int) -> int:
        k = int(k)
        if not 0 <= k < self.shape_count:
            raise IndexError(f"shape index {k} out of range ({self.shape_count} shapes)")
        return k

    def snap(self, k: int) -> "PointCloud":
        """Put every particle on shape ``k`` at rest (cancels a running morph)."""
        k = self._check(k)
        self._tween = None
        self.from_ = self.to = k
        self.mix = 0.0
        self._native.snap(k)
        return self

    def morph(
        self,
        k: int,
        *,
        duration: Optional[float] = None,
        ease: EasingLike = "linear",
        from_: Optional[int] = None,
    ) -> "PointCloud":
        """Flow to shape ``k``: the blend becomes ``from_ → k`` with ``mix = 0``
        (``from_`` defaults to whichever shape the cloud is closer to). With a
        ``duration`` (seconds) :meth:`step` advances ``mix`` to 1 along
        ``ease``; without one, drive :attr:`mix` yourself (e.g. from scroll)."""
        k = self._check(k)
        src = (self.to if self.mix >= 0.5 else self.from_) if from_ is None else self._check(from_)
        self.from_, self.to, self.mix = src, k, 0.0
        self._tween = None
        if duration is not None:
            d = float(duration)
            if math.isnan(d) or d < 0:
                raise ValueError(f"duration must be >= 0, got {duration!r}")
            if d == 0:
                self.mix = 1.0
            else:
                self._tween = (d, 0.0, Easing(ease))
        return self

    @property
    def morphing(self) -> bool:
        """``True`` while a :meth:`morph` with a duration is in progress."""
        return self._tween is not None

    # ---- simulation ----------------------------------------------------------------

    def step(
        self, dt: float, from_: Optional[int] = None, to: Optional[int] = None, mix: Optional[float] = None
    ) -> "PointCloud":
        """Advance ``dt`` seconds (split into equal steps of at most 1/20 s; the
        GIL is released while simulating). ``from_``, ``to`` and ``mix`` update
        the blend state first (an explicit ``mix`` cancels a running morph)."""
        dt = float(dt)
        if math.isnan(dt) or dt < 0:
            raise ValueError(f"dt must be >= 0, got {dt!r}")
        if from_ is not None:
            self.from_ = self._check(from_)
        if to is not None:
            self.to = self._check(to)
        if mix is not None:
            self._tween = None
            self.mix = float(mix)
        if dt == 0:
            return self
        n = max(1, math.ceil(dt / _MAX_DT - 1e-9))
        h = dt / n
        nat = self._native
        if self._tween is None:
            nat.step(h, self.from_, self.to, min(1.0, max(0.0, self.mix)), n)
        else:
            for _ in range(n):
                if self._tween is not None:
                    d, t, e = self._tween
                    t += h
                    if t >= d:
                        self._tween = None
                        self.mix = 1.0
                    else:
                        self._tween = (d, t, e)
                        self.mix = e(t / d)
                nat.step(h, self.from_, self.to, min(1.0, max(0.0, self.mix)), 1)
        self.time += dt
        return self

    def run(self, seconds: float, fps: float = 60.0) -> "PointCloud":
        """Simulate ``seconds`` at a fixed ``fps``."""
        if not fps > 0:
            raise ValueError("fps must be > 0")
        frames = round(float(seconds) * fps)
        if self._tween is None and frames > 0:
            dt = 1.0 / fps
            n = max(1, math.ceil(dt / _MAX_DT - 1e-9))
            self._native.step(dt / n, self.from_, self.to, min(1.0, max(0.0, self.mix)), frames * n)
            self.time += frames * dt
        else:
            for _ in range(frames):
                self.step(1.0 / fps)
        return self

    def explode(self, x: float = 0.0, y: float = 0.0, z: float = 0.0, strength: float = 400.0) -> "PointCloud":
        """Radial velocity kick away from ``(x, y, z)``, scaled by 1/distance
        (negative strength pulls in); the springs bring the particles home."""
        self._native.explode(float(x), float(y), float(z), float(strength))
        return self

    @property
    def positions(self) -> "array[float]":
        """Flat ``array('f')`` ``[x, y, z] * count`` (a copy; feed it to
        :func:`anikit.transform.project_points` or :meth:`Splat.render`)."""
        return _floats(self._native.positions())

    @property
    def speed(self) -> float:
        """Mean particle speed (world units per second)."""
        return self._native.mean_speed

    def __repr__(self) -> str:
        return (
            f"PointCloud({self.count} points, shapes={self.shape_count}, "
            f"blend={self.from_}->{self.to}@{self.mix:.2f}, time={self.time:.3f})"
        )


def _rgb_int(c: Union[int, _color.ColorLike]) -> int:
    if isinstance(c, int):
        if not 0 <= c <= 0xFFFFFF:
            raise ValueError(f"background must be 0xRRGGBB, got {c:#x}")
        return c
    r, g, b, _ = _color.parse(c)
    ch = [min(255, max(0, round(v))) for v in (r, g, b)]
    return (ch[0] << 16) | (ch[1] << 8) | ch[2]


def _rgb_data(c: Any, count: Optional[int]) -> Union[bytes, List[float]]:
    if callable(c):
        if count is None:
            raise TypeError("colors(fn) needs count")
        out: List[float] = []
        for i in range(int(count)):
            out.extend(_rgb(c(i)))
        return out
    if isinstance(c, (bytes, bytearray)):
        return bytes(c)
    if isinstance(c, array):
        return c.tobytes() if c.typecode == "f" else [float(v) for v in c]
    items = list(c)
    if items and isinstance(items[0], (int, float)):
        return [float(v) for v in items]
    flat: List[float] = []
    for v in items:
        flat.extend(_rgb(v))
    return flat


def _rgb(v: Any) -> Tuple[float, float, float]:
    """``(r, g, b)`` in 0…1 from a tuple or a CSS colour string."""
    if isinstance(v, str):
        r, g, b, _ = _color.parse(v)
        return (r / 255.0, g / 255.0, b / 255.0)
    if len(v) < 3:
        raise ValueError(f"colours are (r, g, b) in 0..1, got {v!r}")
    return (float(v[0]), float(v[1]), float(v[2]))


def _lens(lens: Optional[Sequence[float]]) -> Tuple[float, float, float, float]:
    if lens is None:
        return (0.0, 0.0, 0.0, 0.0)
    v = [float(x) for x in lens]
    if not 2 <= len(v) <= 4:
        raise ValueError(f"lens is (x, y[, radius[, strength]]), got {lens!r}")
    v += [140.0, 40.0][len(v) - 2 :]
    return (v[0], v[1], v[2], v[3])


class Splat:
    """Additive point renderer into a ``width × height`` RGBA buffer.

    Points are drawn as light (order-independent, no depth sort needed),
    then :meth:`resolve` tone-maps the light over ``background`` and returns
    the frame as row-major RGBA ``bytes``. ``trail`` is the fraction of light
    kept between frames (0 = fresh frames, 0.85 = long motion trails).
    :meth:`render` does project → draw → resolve in one call.

    >>> glow = Splat(8, 8, background="#102030")
    >>> frame = glow.resolve()
    >>> len(frame), frame[:4] == bytes([0x10, 0x20, 0x30, 255])
    (256, True)
    >>> frame = glow.draw([3.5, 3.5, 1.0, 0.5], intensity=5.0).resolve()
    >>> glow.pixel(3, 3)[0] > 0x10
    True
    """

    def __init__(
        self,
        width: int,
        height: int,
        *,
        background: Union[int, _color.ColorLike] = 0x000000,
        exposure: float = 1.0,
        trail: float = 0.0,
    ) -> None:
        self._native = NativeSplat(int(width), int(height))
        self.background = background
        #: Brightness multiplier applied when tone mapping.
        self.exposure = float(exposure)
        #: Fraction of light kept after each :meth:`resolve` (0 … 1).
        self.trail = float(trail)
        self._projector: Optional[Projector] = None

    @property
    def native(self) -> NativeSplat:
        return self._native

    @property
    def width(self) -> int:
        return self._native.width

    @property
    def height(self) -> int:
        return self._native.height

    @property
    def size(self) -> Tuple[int, int]:
        """``(width, height)`` in pixels."""
        return (self._native.width, self._native.height)

    def resize(self, width: int, height: int) -> "Splat":
        """Resize the buffer (clears the light)."""
        self._native.resize(int(width), int(height))
        return self

    @property
    def background(self) -> int:
        """Background colour as ``0xRRGGBB`` (set it from an int, a CSS
        colour string or an ``(r, g, b)`` tuple in 0…255)."""
        return self._bg

    @background.setter
    def background(self, c: Union[int, _color.ColorLike]) -> None:
        self._bg = _rgb_int(c)

    def colors(self, c: Any, count: Optional[int] = None) -> "Splat":
        """Per-point colours (linear RGB in 0…1): a flat ``[r, g, b, ...]``
        sequence / ``array('f')``, a sequence of ``(r, g, b)`` tuples or CSS
        colour strings, or ``fn(i) -> (r, g, b)`` with ``count``. ``None``
        resets to white. Colours apply when they cover every drawn point."""
        self._native.set_colors(b"" if c is None else _rgb_data(c, count))
        return self

    def draw(
        self,
        points: Any,
        *,
        size: float = 1.0,
        intensity: float = 0.25,
        fog: Optional[Sequence[float]] = None,
        lens: Optional[Sequence[float]] = None,
        max_radius: float = 6.0,
        scale: float = 1.0,
    ) -> "Splat":
        """Add projected points as light.

        ``points`` is a :class:`~anikit.transform.Projection`, or flat
        ``[screen_x, screen_y, depth, scale] * n`` (sequence, ``array('f')``
        or native ``f32`` bytes). ``size`` is the point radius in world units
        (times the perspective ``scale``; up to ~1.6 px points are exact
        sub-pixel splats, larger ones soft discs up to ``max_radius`` px),
        ``intensity`` the light per point, ``fog = (near, far)`` fades points
        by depth, ``lens = (x, y[, radius=140[, strength=40]])`` pushes points
        away from a screen point, and ``scale`` maps projected units to buffer
        pixels (e.g. 0.5 when projecting at twice the buffer size).
        """
        if isinstance(points, Projection):
            data: Union[bytes, List[float]] = points.points.tobytes()
        elif isinstance(points, (bytes, bytearray)):
            data = bytes(points)
        elif isinstance(points, array):
            data = points.tobytes() if points.typecode == "f" else [float(v) for v in points]
        else:
            data = [float(v) for v in points]
        if fog is None:
            near, far = 0.0, None
        else:
            if len(fog) != 2:
                raise ValueError(f"fog is (near, far), got {fog!r}")
            near, far = float(fog[0]), float(fog[1])
        self._native.draw(data, float(size), float(intensity), near, far, float(scale), _lens(lens), float(max_radius))
        return self

    def fade(self, k: float) -> "Splat":
        """Multiply the accumulated light by ``k`` (0 clears)."""
        self._native.fade(float(k))
        return self

    def clear(self) -> "Splat":
        return self.fade(0.0)

    def resolve(
        self,
        *,
        exposure: Optional[float] = None,
        background: Optional[Union[int, _color.ColorLike]] = None,
        keep: Optional[float] = None,
    ) -> bytes:
        """Tone-map the light into a frame (row-major RGBA ``bytes``), then
        keep ``keep`` (default :attr:`trail`) of the light for the next frame."""
        return self._native.resolve(
            self.exposure if exposure is None else float(exposure),
            self._bg if background is None else _rgb_int(background),
            min(1.0, max(0.0, self.trail if keep is None else float(keep))),
        )

    def render(
        self,
        source: Any,
        *,
        camera: Optional[Camera] = None,
        model: Optional[MatrixLike] = None,
        view_projection: Optional[MatrixLike] = None,
        size: float = 1.0,
        intensity: float = 0.25,
        fog: Optional[Sequence[float]] = None,
        lens: Optional[Sequence[float]] = None,
        max_radius: float = 6.0,
        exposure: Optional[float] = None,
    ) -> bytes:
        """Project ``source`` (a :class:`PointCloud`, 3D points as accepted by
        :func:`~anikit.transform.project_points`, or an existing
        :class:`~anikit.transform.Projection`) through ``camera`` (default
        ``Camera()``) or ``view_projection`` into this buffer, draw it and
        :meth:`resolve` the frame. Returns RGBA ``bytes``."""
        if isinstance(source, Projection):
            proj = source
        else:
            pts = source.positions if isinstance(source, PointCloud) else source
            if self._projector is None:
                self._projector = Projector()
            proj = self._projector.project(
                pts,
                camera=camera,
                size=(float(self.width), float(self.height)),
                model=model,
                view_projection=view_projection,
                sort=False,
            )
        self.draw(proj, size=size, intensity=intensity, fog=fog, lens=lens, max_radius=max_radius)
        return self.resolve(exposure=exposure)

    @property
    def rgba(self) -> bytes:
        """The last resolved frame (row-major RGBA ``bytes``)."""
        return self._native.rgba()

    def pixel(self, x: int, y: int) -> Tuple[int, int, int, int]:
        """``(r, g, b, a)`` of the last resolved frame at ``(x, y)``."""
        return self._native.pixel(int(x), int(y))

    def light(self, x: int, y: int) -> Tuple[float, float, float]:
        """Linear light accumulated at ``(x, y)`` (not yet tone-mapped)."""
        return self._native.light(int(x), int(y))

    @property
    def total_light(self) -> Tuple[float, float, float]:
        """Sum of the accumulated light ``(r, g, b)`` over the buffer."""
        return self._native.total_light()

    def __repr__(self) -> str:
        return f"Splat({self.width}x{self.height}, background=#{self._bg:06x}, trail={self.trail:g})"
