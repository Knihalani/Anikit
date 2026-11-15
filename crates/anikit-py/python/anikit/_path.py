"""SVG path geometry, motion paths and shape morphing."""

from __future__ import annotations

import math
from typing import Iterable, List, Optional, Sequence, Tuple, Union, overload

from ._anikit import NativeMorph, NativeMorphSeq, NativePath
from ._easing import EasingLike, ease

__all__ = [
    "Path",
    "PathLike",
    "Morph",
    "MorphSequence",
    "morph",
    "morph_many",
    "MotionPath",
    "motion_path",
    "ClosestPoint",
]

PathLike = Union[str, "Path", NativePath]
Point = Tuple[float, float, float]
#: ``(progress, x, y, distance)`` — see :meth:`Path.closest_point`.
ClosestPoint = Tuple[float, float, float, float]


def _flat_points(points: Iterable[Union[float, Sequence[float]]]) -> List[float]:
    flat: List[float] = []
    for p in points:
        if isinstance(p, (int, float)):
            flat.append(float(p))
        else:
            x, y = p
            flat.extend((float(x), float(y)))
    return flat


class Path:
    """An SVG path, normalised to absolute cubic Bézier segments.

    >>> p = Path("M0 0 L100 0")
    >>> p.length
    100.0
    >>> p.point_at(0.5)
    (50.0, 0.0, 0.0)
    """

    __slots__ = ("_native",)

    def __init__(self, d: PathLike) -> None:
        if isinstance(d, Path):
            self._native: NativePath = d._native
        elif isinstance(d, NativePath):
            self._native = d
        elif isinstance(d, str):
            self._native = NativePath(d)
        else:
            raise TypeError(f"expected SVG path data (str) or Path, got {type(d).__name__}")

    # -- shapes ---------------------------------------------------------------

    @classmethod
    def circle(cls, cx: float, cy: float, r: float) -> "Path":
        return cls(NativePath.ellipse(cx, cy, r, r))

    @classmethod
    def ellipse(cls, cx: float, cy: float, rx: float, ry: float) -> "Path":
        return cls(NativePath.ellipse(cx, cy, rx, ry))

    @classmethod
    def rect(
        cls, x: float, y: float, width: float, height: float, rx: float = 0.0, ry: Optional[float] = None
    ) -> "Path":
        """Rectangle, optionally with rounded corners."""
        return cls(NativePath.rect(x, y, width, height, rx, ry))

    @classmethod
    def polygon(cls, points: Iterable[Union[float, Sequence[float]]], closed: bool = True) -> "Path":
        """Polygon (or polyline with ``closed=False``) from ``[(x, y), ...]`` or a flat ``[x0, y0, ...]``."""
        return cls(NativePath.polygon(_flat_points(points), closed))

    @classmethod
    def through(
        cls,
        points: Iterable[Union[float, Sequence[float]]],
        curviness: float = 1.0,
        closed: bool = False,
    ) -> "Path":
        """A smooth curve passing through every point (Catmull–Rom spline).

        ``points`` is ``[(x, y), ...]`` (or flat ``[x0, y0, ...]``) with at
        least two points; ``curviness`` 0 gives straight lines, 1 a natural
        curve, larger values exaggerate it. ``closed=True`` joins the last
        point smoothly back to the first.

        >>> p = Path.through([(0, 0), (50, 50), (100, 0)])
        >>> p.segment_count
        2
        """
        return cls(NativePath.through(_flat_points(points), float(curviness), bool(closed)))

    @classmethod
    def star(
        cls,
        cx: float,
        cy: float,
        points: int = 5,
        outer: float = 50.0,
        inner: float = 25.0,
        rotation: float = 0.0,
    ) -> "Path":
        """Star with ``points`` tips (``inner == outer`` gives a regular polygon)."""
        return cls(NativePath.star(cx, cy, int(points), outer, inner, rotation))

    # -- geometry -------------------------------------------------------------

    @property
    def length(self) -> float:
        """Total arc length."""
        return self._native.length

    @property
    def bounds(self) -> Tuple[float, float, float, float]:
        """Exact bounding box ``(min_x, min_y, max_x, max_y)``."""
        return self._native.bounds

    @property
    def segment_count(self) -> int:
        return self._native.segment_count

    @property
    def subpath_count(self) -> int:
        return self._native.subpath_count

    def point_at(self, t: float) -> Point:
        """``(x, y, angle_deg)`` at fraction ``t`` of the length (tangent angle in degrees)."""
        return self._native.point_at(t)

    def points(self, n: int) -> List[Point]:
        """``n`` points evenly spaced by arc length, both ends included."""
        if n < 1:
            raise ValueError("n must be >= 1")
        if n == 1:
            return [self._native.point_at(0.0)]
        return self._native.points_at([i / (n - 1) for i in range(n)])

    def point_at_length(self, distance: float) -> Point:
        """``(x, y, angle_deg)`` at ``distance`` units along the path (clamped to its ends).

        >>> Path("M0 0 L100 0").point_at_length(25)
        (25.0, 0.0, 0.0)
        """
        return self._native.point_at_length(distance)

    def closest_point(self, x: float, y: float) -> ClosestPoint:
        """The point of the path nearest to ``(x, y)``:
        ``(progress, px, py, distance)``, where ``progress`` is the fraction of
        the length (usable with :meth:`point_at`, :meth:`trim` or as an
        animation's progress — e.g. for dragging along a path).

        >>> Path("M0 0 L100 0").closest_point(30, 5)
        (0.3, 30.0, 0.0, 5.0)
        """
        return self._native.closest(x, y)

    def sample_points(self, n: int) -> List[Tuple[float, float]]:
        """``n`` ``(x, y)`` points evenly spaced by distance, both ends included
        (like :meth:`points` without the angle)."""
        if n < 1:
            raise ValueError("n must be >= 1")
        return self._native.sample_points(n)

    def flatten(self, tolerance: float = 0.25) -> List[List[Tuple[float, float]]]:
        """Polylines approximating the path, one ``[(x, y), ...]`` list per
        subpath, never deviating more than ``tolerance`` from the curve."""
        return self._native.flatten(tolerance)

    @property
    def area(self) -> float:
        """Signed enclosed area (positive when drawn clockwise on screen, i.e.
        in SVG's y-down space; use ``abs()`` for the size). Each subpath is
        treated as closed."""
        return self._native.area

    def trim(self, start: float = 0.0, end: float = 1.0) -> "Path":
        """The part between fractions ``start`` and ``end`` of the length
        (clamped to ``[0, 1]``, swapped if reversed). Animate ``end`` from 0 to
        1 for a "draw-on" effect.

        >>> round(Path("M0 0 L100 0").trim(0.25, 0.75).length, 6)
        50.0
        """
        return Path(self._native.trim(start, end))

    def simplify(self, tolerance: float = 1.0, smooth: bool = False) -> "Path":
        """Fewer points: flattens, drops points within ``tolerance``
        (Ramer–Douglas–Peucker) and rebuilds the path from straight lines, or
        from a smooth spline through the kept points with ``smooth=True``."""
        return Path(self._native.simplify(tolerance, smooth))

    def to_svg(self, decimals: int = 3) -> str:
        """Normalised absolute path data (``M``/``C``/``Z`` only)."""
        return self._native.to_svg(decimals)

    @property
    def d(self) -> str:
        return self._native.to_svg(3)

    def normalize(self) -> "Path":
        """A copy re-parsed from its normalised form (all-cubic, absolute)."""
        return Path(self._native.to_svg(9))

    def transform(self, a: float, b: float, c: float, d: float, e: float, f: float) -> "Path":
        """A copy transformed by the SVG matrix ``(a, b, c, d, e, f)``."""
        return Path(self._native.transformed([a, b, c, d, e, f]))

    def translate(self, dx: float, dy: float = 0.0) -> "Path":
        return self.transform(1, 0, 0, 1, dx, dy)

    def scale(self, sx: float, sy: Optional[float] = None) -> "Path":
        return self.transform(sx, 0, 0, sx if sy is None else sy, 0, 0)

    def rotate(self, degrees: float, cx: float = 0.0, cy: float = 0.0) -> "Path":
        r = math.radians(degrees)
        cos, sin = math.cos(r), math.sin(r)
        return self.transform(cos, sin, -sin, cos, cx - cx * cos + cy * sin, cy - cx * sin - cy * cos)

    def reversed(self) -> "Path":
        """A copy drawn in the opposite direction."""
        return Path(self._native.reversed())

    @property
    def native(self) -> NativePath:
        return self._native

    def __str__(self) -> str:
        return self._native.to_svg(3)

    def __eq__(self, other: object) -> bool:
        if isinstance(other, Path):
            return self._native == other._native
        return NotImplemented

    __hash__ = None  # type: ignore[assignment]

    def __repr__(self) -> str:
        d = self._native.to_svg(3)
        return f"Path({d if len(d) <= 60 else d[:57] + '...'!r})"


def _as_path(p: PathLike) -> Path:
    return p if isinstance(p, Path) else Path(p)


class Morph:
    """Two shapes normalised to the same structure so they can be interpolated.

    >>> m = morph(Path.rect(0, 0, 100, 100), Path.circle(50, 50, 50))
    >>> isinstance(m.at(0.5), str)
    True
    """

    __slots__ = ("_native", "start", "end")

    def __init__(self, a: PathLike, b: PathLike) -> None:
        self.start = _as_path(a)
        self.end = _as_path(b)
        self._native = NativeMorph(self.start.native, self.end.native)

    def at(self, t: float, decimals: int = 3) -> str:
        """Path data at progress ``t`` (0 → ``a``, 1 → ``b``; extrapolates outside)."""
        return self._native.at(t, decimals)

    def frames(self, n: int, ease: EasingLike = None, decimals: int = 3) -> List[str]:
        """``n`` path strings from ``a`` to ``b`` inclusive, optionally eased."""
        if n < 1:
            raise ValueError("n must be >= 1")
        if n == 1:
            return [self.at(1.0, decimals)]
        return [self.at(_ease(ease, i / (n - 1)), decimals) for i in range(n)]

    def write(self, values: Sequence[float], decimals: int = 3) -> str:
        """Format interpolated channel values (``len == channels``) as path data."""
        return self._native.write(list(values), decimals)

    @property
    def from_values(self) -> List[float]:
        return self._native.from_values

    @property
    def to_values(self) -> List[float]:
        return self._native.to_values

    @property
    def template(self) -> List[Tuple[int, bool]]:
        """Per subpath ``(segment_count, closed)``."""
        return self._native.template

    @property
    def channels(self) -> int:
        return self._native.channels

    @property
    def native(self) -> NativeMorph:
        return self._native

    def __repr__(self) -> str:
        return f"Morph(subpaths={len(self.template)}, channels={self.channels})"


class MorphSequence:
    """Any number of shapes normalised to one structure, morphing from one to
    the next. Created by :func:`morph` (three or more shapes) or :func:`morph_many`.

    >>> seq = morph_many([Path.rect(0, 0, 10, 10), Path.circle(5, 5, 5), Path.star(5, 5, 5, 5, 2)])
    >>> len(seq), len(seq.frames(5))
    (3, 5)
    """

    __slots__ = ("_native", "shapes")

    def __init__(self, shapes: Iterable[PathLike]) -> None:
        self.shapes: List[Path] = [_as_path(s) for s in shapes]
        if len(self.shapes) < 2:
            raise ValueError("a morph needs at least 2 shapes")
        self._native = NativeMorphSeq([s.native for s in self.shapes])

    def at(self, t: float, decimals: int = 3) -> str:
        """Path data at sequence progress ``t`` (0 → first shape, 1 → last;
        shapes are evenly spaced and interpolated linearly in between)."""
        return self._native.at(t, decimals)

    def frames(self, n: int, ease: EasingLike = None, decimals: int = 3) -> List[str]:
        """``n`` path strings from the first shape to the last inclusive,
        optionally eased over the whole sequence."""
        if n < 1:
            raise ValueError("n must be >= 1")
        if n == 1:
            return [self.at(1.0, decimals)]
        return [self.at(_ease(ease, i / (n - 1)), decimals) for i in range(n)]

    def write(self, values: Sequence[float], decimals: int = 3) -> str:
        """Format interpolated channel values (``len == channels``) as path data."""
        return self._native.write(list(values), decimals)

    @property
    def keyframes(self) -> List[List[float]]:
        """Channel values of each shape (all of length :attr:`channels`)."""
        return self._native.frames

    @property
    def template(self) -> List[Tuple[int, bool]]:
        """Per subpath ``(segment_count, closed)``."""
        return self._native.template

    @property
    def channels(self) -> int:
        return self._native.channels

    @property
    def native(self) -> NativeMorphSeq:
        return self._native

    def __len__(self) -> int:
        return len(self.shapes)

    def __repr__(self) -> str:
        return f"MorphSequence(shapes={len(self.shapes)}, subpaths={len(self.template)}, channels={self.channels})"


def _ease(e: EasingLike, t: float) -> float:
    return t if e is None else ease(e, t)


@overload
def morph(a: PathLike, b: PathLike, /) -> Morph: ...


@overload
def morph(a: PathLike, b: PathLike, c: PathLike, /, *more: PathLike) -> MorphSequence: ...


def morph(a: PathLike, b: PathLike, /, *more: PathLike) -> Union[Morph, MorphSequence]:
    """Prepare a morph between shapes (SVG path strings or :class:`Path`).

    Two shapes give a :class:`Morph`; three or more a :class:`MorphSequence`
    that passes through every shape in order. Both have ``at(t)`` and
    ``frames(n)`` and can be used directly as an animation property value.
    """
    if more:
        return MorphSequence((a, b) + more)
    return Morph(a, b)


def morph_many(shapes: Iterable[PathLike]) -> MorphSequence:
    """A :class:`MorphSequence` through ``shapes`` (at least two) in order."""
    return MorphSequence(shapes)


class MotionPath:
    """Animate a target along a path, writing ``x``/``y`` (and optionally the
    tangent angle) to the given keys. See :func:`motion_path`."""

    __slots__ = ("path", "x", "y", "rotate", "start", "end")

    def __init__(
        self,
        path: PathLike,
        x: str = "x",
        y: str = "y",
        rotate: Optional[str] = None,
        start: float = 0.0,
        end: float = 1.0,
    ) -> None:
        self.path = _as_path(path)
        self.x = x
        self.y = y
        self.rotate = rotate
        self.start = float(start)
        self.end = float(end)

    @property
    def keys(self) -> Tuple[str, ...]:
        return (self.x, self.y) + ((self.rotate,) if self.rotate else ())

    def __repr__(self) -> str:
        return f"MotionPath({self.path!r}, x={self.x!r}, y={self.y!r}, rotate={self.rotate!r})"


def motion_path(
    path: PathLike,
    *,
    x: str = "x",
    y: str = "y",
    rotate: Optional[str] = None,
    start: float = 0.0,
    end: float = 1.0,
) -> MotionPath:
    """Use as a property value to move a target along ``path``::

        animate(sprite, {"motion": motion_path("M0 0 C50 -50 100 50 150 0", rotate="angle")})

    Writes the point's coordinates to the ``x``/``y`` keys and, if ``rotate`` is
    given, the tangent angle (degrees) to that key. ``start``/``end`` select a
    portion of the path (fractions of its length).
    """
    return MotionPath(path, x, y, rotate, start, end)
