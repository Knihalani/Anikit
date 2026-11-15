"""A fast 2D physics world: circles, static boxes, distance constraints
(ropes, cloth, soft bodies), attractors/repulsors and bounds.

The simulation runs natively (position-based dynamics with sub-stepping and a
grid broadphase); bodies are identified by integer indices and bulk data
crosses the boundary as flat ``array('f')`` buffers, so worlds with tens of
thousands of bodies need no per-body Python objects. :class:`Body` is a light
handle (world + index) for convenient one-off access.
"""

from __future__ import annotations

import math
from array import array
from typing import Any, Callable, Iterable, Iterator, List, NamedTuple, Optional, Sequence, Tuple, Union

from ._anikit import NativeWorld
from ._values import setter_for

__all__ = ["World", "Body", "Link", "Rope", "Cloth", "Attractor", "BodyRef", "Bounds"]

#: ``(min_x, min_y, max_x, max_y)``.
Bounds = Tuple[float, float, float, float]

#: The longest single native step (the core clamps larger steps for stability).
_MAX_DT = 0.05


class Link(NamedTuple):
    """A distance constraint between bodies ``a`` and ``b``."""

    a: int
    b: int
    length: float
    stiffness: float
    #: Breaks when stretched beyond ``length * tear`` (0 = never).
    tear: float
    #: ``False`` once torn.
    alive: bool


def _f32(data: Union["array[float]", Sequence[float]]) -> Union[bytes, List[float], Sequence[float]]:
    """Pass ``array('f')`` as raw bytes (one memcpy); anything else as a sequence."""
    if isinstance(data, array):
        if data.typecode == "f":
            return data.tobytes()
        return data.tolist()
    return data


def _floats(raw: bytes) -> "array[float]":
    a = array("f")
    a.frombytes(raw)
    return a


class Body:
    """Handle to one body of a :class:`World` (cheap: a world and an index).

    >>> w = World(gravity=(0, 0))
    >>> b = w.add_body(10, 20, radius=5)
    >>> b.position, b.radius, b.index
    ((10.0, 20.0), 5.0, 0)
    """

    __slots__ = ("world", "index")

    def __init__(self, world: "World", index: int) -> None:
        self.world = world
        self.index = index

    # ---- state ---------------------------------------------------------------

    @property
    def position(self) -> Tuple[float, float]:
        return self.world._native.position(self.index)

    @position.setter
    def position(self, p: Sequence[float]) -> None:
        self.world._native.set_position(self.index, float(p[0]), float(p[1]), True)

    @property
    def x(self) -> float:
        return self.position[0]

    @x.setter
    def x(self, v: float) -> None:
        self.position = (v, self.position[1])

    @property
    def y(self) -> float:
        return self.position[1]

    @y.setter
    def y(self, v: float) -> None:
        self.position = (self.position[0], v)

    @property
    def velocity(self) -> Tuple[float, float]:
        """Units per second."""
        return self.world._native.velocity(self.index)

    @velocity.setter
    def velocity(self, v: Sequence[float]) -> None:
        self.world._native.set_velocity(self.index, float(v[0]), float(v[1]))

    @property
    def angle(self) -> float:
        """Rolling angle in radians (accumulated from horizontal travel)."""
        return self.world._native.angle(self.index)

    @property
    def rotation(self) -> float:
        """Rolling angle in degrees."""
        return math.degrees(self.angle)

    @property
    def speed(self) -> float:
        vx, vy = self.velocity
        return math.hypot(vx, vy)

    @property
    def radius(self) -> float:
        return float(self.world._radii[self.index])

    @radius.setter
    def radius(self, r: float) -> None:
        self.world._native.set_radius(self.index, float(r))
        self.world._radii[self.index] = max(0.0, float(r))

    @property
    def mass(self) -> float:
        """Mass given at creation (``<= 0`` = static)."""
        return float(self.world._masses[self.index])

    @property
    def static(self) -> bool:
        """``True`` for immovable bodies (mass ``<= 0`` or pinned)."""
        return self.mass <= 0 or self.pinned

    @property
    def pinned(self) -> bool:
        return self.index in self.world._pinned

    @pinned.setter
    def pinned(self, on: bool) -> None:
        self.world._native.set_pinned(self.index, bool(on))
        if on:
            self.world._pinned.add(self.index)
        else:
            self.world._pinned.discard(self.index)

    @property
    def active(self) -> bool:
        """Inactive bodies are skipped by the simulation (their index stays valid)."""
        return self.index not in self.world._inactive

    @active.setter
    def active(self, on: bool) -> None:
        self.world._native.set_active(self.index, bool(on))
        if on:
            self.world._inactive.discard(self.index)
        else:
            self.world._inactive.add(self.index)

    # ---- actions -------------------------------------------------------------

    def move_to(self, x: float, y: float, *, keep_velocity: bool = False) -> "Body":
        """Teleport the body (stopping it unless ``keep_velocity``)."""
        self.world._native.set_position(self.index, float(x), float(y), keep_velocity)
        return self

    def push(self, vx: float, vy: float) -> "Body":
        """Add ``(vx, vy)`` to the velocity (an impulse per unit mass)."""
        ox, oy = self.velocity
        self.world._native.set_velocity(self.index, ox + float(vx), oy + float(vy))
        return self

    def pin(self) -> "Body":
        self.pinned = True
        return self

    def unpin(self) -> "Body":
        self.pinned = False
        return self

    def remove(self) -> None:
        """Deactivate the body (indices of other bodies are unaffected)."""
        self.active = False

    def bind(self, target: Any, **keys: Any) -> "Body":
        """Write this body's position/rotation into ``target`` after every step;
        see :meth:`World.bind`."""
        self.world.bind(self, target, **keys)
        return self

    def __eq__(self, other: object) -> bool:
        return isinstance(other, Body) and other.world is self.world and other.index == self.index

    def __hash__(self) -> int:
        return hash((id(self.world), self.index))

    def __repr__(self) -> str:
        x, y = self.position
        return f"Body({self.index}, x={x:.3f}, y={y:.3f}, r={self.radius:g})"


#: A body given as a :class:`Body` handle or its index.
BodyRef = Union[Body, int]


class Attractor:
    """A point that pulls (``strength > 0``) or pushes (``< 0``) bodies within
    ``radius``; the acceleration (units/s²) fades linearly from ``strength`` at
    the centre to 0 at the edge. Attributes are live: move it every frame to
    follow a pointer."""

    __slots__ = ("_world", "_x", "_y", "_strength", "_radius")

    def __init__(self, world: "World", x: float, y: float, strength: float, radius: float) -> None:
        self._world = world
        self._x, self._y = float(x), float(y)
        self._strength, self._radius = float(strength), float(radius)

    def _set(self, name: str, v: float) -> None:
        setattr(self, name, float(v))
        self._world._attractors_dirty = True

    @property
    def x(self) -> float:
        return self._x

    @x.setter
    def x(self, v: float) -> None:
        self._set("_x", v)

    @property
    def y(self) -> float:
        return self._y

    @y.setter
    def y(self, v: float) -> None:
        self._set("_y", v)

    @property
    def strength(self) -> float:
        return self._strength

    @strength.setter
    def strength(self, v: float) -> None:
        self._set("_strength", v)

    @property
    def radius(self) -> float:
        return self._radius

    @radius.setter
    def radius(self, v: float) -> None:
        self._set("_radius", v)

    def move_to(self, x: float, y: float) -> "Attractor":
        self._x, self._y = float(x), float(y)
        self._world._attractors_dirty = True
        return self

    def remove(self) -> None:
        self._world.remove_attractor(self)

    def __repr__(self) -> str:
        return f"Attractor(x={self._x:g}, y={self._y:g}, strength={self._strength:g}, radius={self._radius:g})"


class Rope:
    """Bodies chained by distance constraints (returned by :meth:`World.rope`)."""

    __slots__ = ("world", "bodies", "links")

    def __init__(self, world: "World", bodies: range, links: range) -> None:
        self.world = world
        #: Body indices from start to end.
        self.bodies = bodies
        #: Constraint indices (see :meth:`World.constraints`).
        self.links = links

    def __len__(self) -> int:
        return len(self.bodies)

    def __getitem__(self, i: int) -> Body:
        return Body(self.world, self.bodies[i])

    def __iter__(self) -> Iterator[Body]:
        return (Body(self.world, i) for i in self.bodies)

    @property
    def start(self) -> Body:
        return self[0]

    @property
    def end(self) -> Body:
        return self[-1]

    def points(self) -> List[Tuple[float, float]]:
        """Current ``(x, y)`` of every body (e.g. for :meth:`anikit.Path.through`)."""
        pos = self.world.positions()
        return [(pos[2 * i], pos[2 * i + 1]) for i in self.bodies]

    @property
    def length(self) -> float:
        """Current length along the bodies."""
        pts = self.points()
        return sum(math.dist(a, b) for a, b in zip(pts, pts[1:]))

    @property
    def broken(self) -> bool:
        """``True`` when any link has torn."""
        native = self.world._native.constraints()
        return any(not native[i][5] for i in self.links)

    def __repr__(self) -> str:
        return f"Rope(bodies={len(self.bodies)}, links={len(self.links)})"


class Cloth:
    """A grid of bodies joined by constraints (returned by :meth:`World.cloth`).
    Bodies are row-major: ``index(col, row) = first + row * cols + col``."""

    __slots__ = ("world", "cols", "rows", "bodies", "links")

    def __init__(self, world: "World", cols: int, rows: int, bodies: range, links: range) -> None:
        self.world = world
        self.cols = cols
        self.rows = rows
        self.bodies = bodies
        self.links = links

    def index(self, col: int, row: int) -> int:
        if not (0 <= col < self.cols and 0 <= row < self.rows):
            raise IndexError(f"cloth cell ({col}, {row}) out of range ({self.cols} x {self.rows})")
        return self.bodies[row * self.cols + col]

    def body(self, col: int, row: int) -> Body:
        return Body(self.world, self.index(col, row))

    def __len__(self) -> int:
        return len(self.bodies)

    def __iter__(self) -> Iterator[Body]:
        return (Body(self.world, i) for i in self.bodies)

    def __repr__(self) -> str:
        return f"Cloth({self.cols} x {self.rows}, links={len(self.links)})"


class _Binding:
    __slots__ = ("index", "target", "set", "x", "y", "rotation", "ox", "oy")

    def __init__(
        self, index: int, target: Any, x: Optional[str], y: Optional[str], rotation: Optional[str], ox: float, oy: float
    ) -> None:
        self.index = index
        self.target = target
        self.set = setter_for(target)
        self.x = x
        self.y = y
        self.rotation = rotation
        self.ox = ox
        self.oy = oy


class World:
    """A 2D physics world (units are arbitrary — typically pixels, y down).

    ``gravity`` is in units/s²; ``damping`` is linear drag per second;
    ``substeps``/``iterations`` trade speed for stiffness; ``restitution``
    (body–body) and ``wall_restitution`` (bounds and boxes) are bounciness
    0–1; ``friction`` is tangential friction 0–1; ``bounds`` is
    ``(min_x, min_y, max_x, max_y)`` or ``None``. Bodies with ``mass <= 0``
    are static.

    >>> w = World(bounds=(0, 0, 400, 300))
    >>> ball = w.add_body(200, 50, radius=10)
    >>> w.run(3.0)
    180
    >>> round(ball.y)
    290
    """

    __slots__ = (
        "_native",
        "_radii",
        "_masses",
        "_pinned",
        "_inactive",
        "_boxes",
        "_attractors",
        "_attractors_dirty",
        "_bindings",
        "time",
    )

    def __init__(
        self,
        *,
        gravity: Tuple[float, float] = (0.0, 980.0),
        damping: float = 0.05,
        substeps: int = 4,
        iterations: int = 2,
        restitution: float = 0.2,
        wall_restitution: float = 0.3,
        friction: float = 0.05,
        bounds: Optional[Sequence[float]] = None,
    ) -> None:
        self._native = NativeWorld(
            gravity=(float(gravity[0]), float(gravity[1])),
            damping=float(damping),
            substeps=int(substeps),
            iterations=int(iterations),
            restitution=float(restitution),
            wall_restitution=float(wall_restitution),
            friction=float(friction),
            bounds=self._bounds(bounds),
        )
        self._radii: "array[float]" = array("f")
        self._masses: "array[float]" = array("f")
        self._pinned: "set[int]" = set()
        self._inactive: "set[int]" = set()
        self._boxes: List[Bounds] = []
        self._attractors: List[Attractor] = []
        self._attractors_dirty = False
        self._bindings: List[_Binding] = []
        #: Simulated seconds.
        self.time = 0.0

    @staticmethod
    def _bounds(b: Optional[Sequence[float]]) -> Optional[Bounds]:
        if b is None:
            return None
        if len(b) != 4:
            raise ValueError(f"bounds must be (min_x, min_y, max_x, max_y), got {b!r}")
        return (float(b[0]), float(b[1]), float(b[2]), float(b[3]))

    @property
    def native(self) -> NativeWorld:
        """The underlying native world (indices, flat ``f32`` bytes)."""
        return self._native

    # ---- configuration (live) ---------------------------------------------------

    @property
    def gravity(self) -> Tuple[float, float]:
        return self._native.gravity

    @gravity.setter
    def gravity(self, g: Sequence[float]) -> None:
        self._native.gravity = (float(g[0]), float(g[1]))

    @property
    def damping(self) -> float:
        return self._native.damping

    @damping.setter
    def damping(self, v: float) -> None:
        self._native.damping = float(v)

    @property
    def substeps(self) -> int:
        return self._native.substeps

    @substeps.setter
    def substeps(self, v: int) -> None:
        self._native.substeps = int(v)

    @property
    def iterations(self) -> int:
        return self._native.iterations

    @iterations.setter
    def iterations(self, v: int) -> None:
        self._native.iterations = int(v)

    @property
    def restitution(self) -> float:
        return self._native.restitution

    @restitution.setter
    def restitution(self, v: float) -> None:
        self._native.restitution = float(v)

    @property
    def wall_restitution(self) -> float:
        return self._native.wall_restitution

    @wall_restitution.setter
    def wall_restitution(self, v: float) -> None:
        self._native.wall_restitution = float(v)

    @property
    def friction(self) -> float:
        return self._native.friction

    @friction.setter
    def friction(self, v: float) -> None:
        self._native.friction = float(v)

    @property
    def bounds(self) -> Optional[Bounds]:
        return self._native.bounds

    @bounds.setter
    def bounds(self, b: Optional[Sequence[float]]) -> None:
        self._native.bounds = self._bounds(b)

    # ---- bodies --------------------------------------------------------------------

    def __len__(self) -> int:
        return len(self._radii)

    def __getitem__(self, i: int) -> Body:
        return self.body(i)

    def __iter__(self) -> Iterator[Body]:
        return (Body(self, i) for i in range(len(self)))

    def body(self, i: int) -> Body:
        """Handle for body ``i`` (negative indices count from the end)."""
        n = len(self)
        j = i + n if i < 0 else i
        if not 0 <= j < n:
            raise IndexError(f"body index {i} out of range ({n} bodies)")
        return Body(self, j)

    def _index(self, b: BodyRef) -> int:
        if isinstance(b, Body):
            if b.world is not self:
                raise ValueError("body belongs to another world")
            return b.index
        i = int(b)
        if not 0 <= i < len(self):
            raise IndexError(f"body index {i} out of range ({len(self)} bodies)")
        return i

    def add_body(
        self,
        x: float,
        y: float,
        radius: float = 10.0,
        mass: float = 1.0,
        *,
        velocity: Optional[Sequence[float]] = None,
        pinned: bool = False,
    ) -> Body:
        """Add a circle and return its handle (``mass <= 0`` = static)."""
        i = self._native.add_body(float(x), float(y), float(radius), float(mass))
        self._radii.append(max(0.0, float(radius)))
        self._masses.append(float(mass))
        b = Body(self, i)
        if velocity is not None:
            b.velocity = velocity
        if pinned:
            b.pinned = True
        return b

    def add_bodies(
        self,
        bodies: Union[Iterable[Sequence[float]], "array[float]"],
        *,
        radius: float = 10.0,
        mass: float = 1.0,
    ) -> range:
        """Add many bodies in one native call; returns their index range.

        ``bodies`` is an iterable of ``(x, y)``, ``(x, y, radius)`` or
        ``(x, y, radius, mass)`` tuples (missing values default to ``radius`` /
        ``mass``), or an ``array('f')`` of flat ``[x, y, radius, mass] * n``.

        >>> w = World()
        >>> w.add_bodies([(0, 0), (30, 0, 5), (60, 0, 5, 2)])
        range(0, 3)
        """
        if isinstance(bodies, array):
            flat = bodies if bodies.typecode == "f" else array("f", bodies)
            if len(flat) % 4:
                raise ValueError("a flat body array needs [x, y, radius, mass] * n")
        else:
            flat = array("f")
            r, m = float(radius), float(mass)
            for b in bodies:
                k = len(b)
                if k == 2:
                    flat.extend((b[0], b[1], r, m))
                elif k == 3:
                    flat.extend((b[0], b[1], b[2], m))
                elif k == 4:
                    flat.extend(b)
                else:
                    raise ValueError(f"bodies are (x, y[, radius[, mass]]), got {b!r}")
        first = self._native.add_bodies(flat.tobytes())
        n = len(flat) // 4
        self._radii.extend(max(0.0, v) for v in flat[2::4])
        self._masses.extend(flat[3::4])
        return range(first, first + n)

    def positions(self) -> "array[float]":
        """Flat ``array('f')`` ``[x0, y0, x1, y1, ...]`` (one native copy)."""
        return _floats(self._native.positions())

    def output(self) -> "array[float]":
        """Flat ``array('f')`` ``[x, y, angle_radians] * n`` — the render buffer."""
        return _floats(self._native.output())

    def velocities(self) -> "array[float]":
        """Flat ``array('f')`` ``[vx, vy] * n``."""
        return _floats(self._native.velocities())

    def radii(self) -> "array[float]":
        """Flat ``array('f')`` of body radii (a copy)."""
        return array("f", self._radii)

    def set_positions(
        self, positions: Union[Sequence[float], "array[float]"], *, first: int = 0, keep_velocity: bool = False
    ) -> None:
        """Teleport bodies ``first, first + 1, ...`` to flat ``[x, y, ...]`` positions."""
        self._native.set_positions(_f32(positions), first, keep_velocity)

    def pick(self, x: float, y: float) -> Optional[Body]:
        """The topmost (most recently added) active body under a point."""
        i = self._native.pick(float(x), float(y))
        return None if i is None else Body(self, i)

    def impulse(self, x: float, y: float, radius: float, strength: float) -> None:
        """Blast: every dynamic body within ``radius`` gets a velocity change
        away from ``(x, y)`` of ``strength`` units/s at the centre fading to 0
        at the edge (negative strength sucks bodies in)."""
        self._native.impulse(float(x), float(y), float(radius), float(strength))

    @property
    def kinetic_energy(self) -> float:
        """Total kinetic energy ``Σ ½·m·v²`` of the dynamic bodies."""
        return self._native.kinetic_energy

    @property
    def pair_tests(self) -> int:
        """Broadphase pair tests of the last step (diagnostics)."""
        return self._native.pair_tests

    # ---- static geometry ---------------------------------------------------------------

    def add_box(self, x0: float, y0: float, x1: float, y1: float) -> int:
        """Add a static axis-aligned box (a platform/wall); returns its index."""
        i = self._native.add_box(float(x0), float(y0), float(x1), float(y1))
        self._boxes.append((min(x0, x1), min(y0, y1), max(x0, x1), max(y0, y1)))
        return i

    @property
    def boxes(self) -> List[Bounds]:
        return list(self._boxes)

    def clear_boxes(self) -> None:
        self._native.clear_boxes()
        self._boxes.clear()

    # ---- constraints ---------------------------------------------------------------------

    def connect(
        self, a: BodyRef, b: BodyRef, length: Optional[float] = None, *, stiffness: float = 1.0, tear: float = 0.0
    ) -> int:
        """Keep two bodies ``length`` apart (default: their current distance).

        ``stiffness`` (0–1) is the fraction of the error corrected per solver
        iteration; ``tear > 0`` breaks the link when stretched beyond
        ``length * tear``. Returns the constraint index.
        """
        ln = -1.0 if length is None else float(length)
        if ln < 0 and length is not None:
            raise ValueError("length must be >= 0")
        return self._native.add_constraint(self._index(a), self._index(b), ln, float(stiffness), float(tear))

    def constraints(self) -> List[Link]:
        """Every constraint (including torn ones, with ``alive=False``)."""
        return [Link(*c) for c in self._native.constraints()]

    def rope(
        self,
        points: Iterable[Sequence[float]],
        *,
        radius: Optional[float] = None,
        mass: float = 1.0,
        stiffness: float = 1.0,
        tear: float = 0.0,
        pin_start: bool = True,
        pin_end: bool = False,
    ) -> Rope:
        """A chain of bodies through ``points`` linked at their current distances.

        ``radius`` defaults to a quarter of the shortest segment (so neighbours
        never collide). The first (and optionally last) body is pinned.

        >>> w = World()
        >>> r = w.rope([(0, 0), (10, 0), (20, 0)])
        >>> len(r), len(r.links), r.start.pinned
        (3, 2, True)
        """
        pts = [(float(p[0]), float(p[1])) for p in points]
        if len(pts) < 2:
            raise ValueError("a rope needs at least 2 points")
        seg = min(math.dist(a, b) for a, b in zip(pts, pts[1:]))
        r = seg * 0.25 if radius is None else float(radius)
        bodies = self.add_bodies([(x, y, r, mass) for x, y in pts])
        first = self._native.add_constraints(
            [v for i in bodies[:-1] for v in (i, i + 1, -1.0, float(stiffness), float(tear))]
        )
        links = range(first, first + len(bodies) - 1)
        if pin_start:
            self.body(bodies[0]).pinned = True
        if pin_end:
            self.body(bodies[-1]).pinned = True
        return Rope(self, bodies, links)

    def cloth(
        self,
        cols: int,
        rows: int,
        spacing: float,
        origin: Sequence[float] = (0.0, 0.0),
        *,
        pin_top: bool = True,
        radius: Optional[float] = None,
        mass: float = 1.0,
        stiffness: float = 1.0,
        tear: float = 0.0,
        shear: bool = False,
    ) -> Cloth:
        """A ``cols × rows`` grid of bodies ``spacing`` apart with its top-left
        corner at ``origin``, joined to their right and lower neighbours (and
        diagonally with ``shear=True``). ``pin_top`` pins the first row.

        >>> w = World()
        >>> c = w.cloth(4, 3, 10, origin=(100, 0))
        >>> len(c), len(c.links), c.body(3, 2).position
        (12, 17, (130.0, 20.0))
        """
        cols, rows = int(cols), int(rows)
        if cols < 1 or rows < 1 or cols * rows < 2:
            raise ValueError("a cloth needs at least 2 bodies")
        sp = float(spacing)
        if not sp > 0:
            raise ValueError("spacing must be > 0")
        r = sp * 0.25 if radius is None else float(radius)
        ox, oy = float(origin[0]), float(origin[1])
        m = float(mass)
        bodies = self.add_bodies([(ox + c * sp, oy + rw * sp, r, m) for rw in range(rows) for c in range(cols)])
        f = bodies.start
        k, t = float(stiffness), float(tear)
        flat: List[float] = []
        for rw in range(rows):
            for c in range(cols):
                i = f + rw * cols + c
                if c + 1 < cols:
                    flat += (i, i + 1, sp, k, t)
                if rw + 1 < rows:
                    flat += (i, i + cols, sp, k, t)
                if shear and c + 1 < cols and rw + 1 < rows:
                    d = sp * math.sqrt(2.0)
                    flat += (i, i + cols + 1, d, k, t)
                    flat += (i + 1, i + cols, d, k, t)
        first = self._native.add_constraints(flat)
        links = range(first, first + len(flat) // 5)
        if pin_top:
            for c in range(cols):
                self.body(f + c).pinned = True
        return Cloth(self, cols, rows, bodies, links)

    # ---- attractors -------------------------------------------------------------------------

    def attract(self, x: float, y: float, strength: float = 2000.0, radius: float = 200.0) -> Attractor:
        """Pull bodies within ``radius`` towards ``(x, y)`` (``strength`` units/s² at the centre)."""
        a = Attractor(self, x, y, abs(float(strength)), radius)
        self._attractors.append(a)
        self._attractors_dirty = True
        return a

    def repel(self, x: float, y: float, strength: float = 2000.0, radius: float = 200.0) -> Attractor:
        """Push bodies within ``radius`` away from ``(x, y)``."""
        a = Attractor(self, x, y, -abs(float(strength)), radius)
        self._attractors.append(a)
        self._attractors_dirty = True
        return a

    @property
    def attractors(self) -> List[Attractor]:
        return list(self._attractors)

    def remove_attractor(self, a: Attractor) -> None:
        if a in self._attractors:
            self._attractors.remove(a)
            self._attractors_dirty = True

    def clear_attractors(self) -> None:
        self._attractors.clear()
        self._attractors_dirty = True

    # ---- bindings ---------------------------------------------------------------------------

    def bind(
        self,
        body: BodyRef,
        target: Any,
        *,
        x: Optional[str] = "x",
        y: Optional[str] = "y",
        rotation: Optional[str] = "rotation",
        offset: Sequence[float] = (0.0, 0.0),
    ) -> None:
        """After every step write the body's position (plus ``offset``) and
        rolling rotation (degrees) into ``target`` (a dict or object). Pass
        ``None`` for a key to skip it. The target is written immediately.

        >>> w = World(gravity=(0, 100))
        >>> sprite = {"x": 0, "y": 0}
        >>> w.bind(w.add_body(5, 5), sprite, rotation=None)
        >>> sprite
        {'x': 5.0, 'y': 5.0}
        """
        b = _Binding(self._index(body), target, x, y, rotation, float(offset[0]), float(offset[1]))
        self._bindings.append(b)
        self._write([b], self._native.output())

    def bind_many(self, bodies: Iterable[BodyRef], targets: Iterable[Any], **keys: Any) -> None:
        """:meth:`bind` pairwise (e.g. ``world.bind_many(world.add_bodies(pts), sprites)``)."""
        for b, t in zip(bodies, targets):
            self.bind(b, t, **keys)

    def unbind(self, what: Any = None) -> None:
        """Remove bindings of a target, a body, or (``None``) all of them."""
        if what is None:
            self._bindings.clear()
        elif isinstance(what, (Body, int)) and not isinstance(what, bool):
            i = self._index(what)
            self._bindings = [b for b in self._bindings if b.index != i]
        else:
            self._bindings = [b for b in self._bindings if b.target is not what]

    @staticmethod
    def _write(bindings: List[_Binding], raw: bytes) -> None:
        out = _floats(raw)
        deg = 180.0 / math.pi
        for b in bindings:
            k = b.index * 3
            t, s = b.target, b.set
            if b.x is not None:
                s(t, b.x, out[k] + b.ox)
            if b.y is not None:
                s(t, b.y, out[k + 1] + b.oy)
            if b.rotation is not None:
                s(t, b.rotation, out[k + 2] * deg)

    def sync(self) -> None:
        """Write every bound target now (done automatically after each step)."""
        if self._bindings:
            self._write(self._bindings, self._native.output())

    # ---- simulation ---------------------------------------------------------------------------

    def _flush(self) -> None:
        if self._attractors_dirty:
            self._native.set_attractors([(a.x, a.y, a.strength, a.radius) for a in self._attractors])
            self._attractors_dirty = False

    def step(self, dt: float) -> None:
        """Advance by ``dt`` seconds (split into equal steps of at most 1/20 s;
        the GIL is released while simulating) and update bound targets."""
        dt = float(dt)
        if math.isnan(dt) or dt < 0:
            raise ValueError(f"dt must be >= 0, got {dt!r}")
        if dt == 0:
            return
        n = max(1, math.ceil(dt / _MAX_DT - 1e-9))
        self._flush()
        self._native.step(dt / n, n)
        self.time += dt
        self.sync()

    def run(self, seconds: float, fps: float = 60.0, *, on_frame: Optional[Callable[[int, float], Any]] = None) -> int:
        """Simulate ``seconds`` at a fixed ``fps``; returns the number of frames.

        Without ``on_frame`` the whole run is one native call (targets are
        written once at the end); with ``on_frame(frame, time)`` targets are
        written and the callback called after every frame.
        """
        if not fps > 0:
            raise ValueError("fps must be > 0")
        frames = int(round(float(seconds) * fps))
        if frames <= 0:
            return 0
        dt = 1.0 / fps
        if on_frame is None:
            n = max(1, math.ceil(dt / _MAX_DT - 1e-9))
            self._flush()
            self._native.step(dt / n, frames * n)
            self.time += frames * dt
            self.sync()
        else:
            for i in range(frames):
                self.step(dt)
                on_frame(i + 1, self.time)
        return frames

    def render(self, fps: float = 60.0, duration: float = 1.0) -> Iterator[Tuple[int, float]]:
        """Generator of ``(frame, time)``: frame 0 is the current state, then
        one step of ``1 / fps`` per frame up to ``duration`` seconds (like
        :func:`anikit.render`)."""
        if not fps > 0:
            raise ValueError("fps must be > 0")
        frames = int(round(float(duration) * fps)) + 1
        self.sync()
        for i in range(frames):
            if i:
                self.step(1.0 / fps)
            yield i, i / fps

    def max_speed(self) -> float:
        """Largest velocity component magnitude over all bodies (units/s)."""
        v = self.velocities()
        return max(max(v), -min(v)) if v else 0.0

    def settle(self, *, max_time: float = 30.0, fps: float = 60.0, speed: float = 5.0, frames: int = 10) -> float:
        """Run until no body moves faster than ``speed`` units/s (per axis) for
        ``frames`` consecutive frames, or until ``max_time``; returns the
        simulated seconds.

        (Bodies resting on bounds keep a small residual velocity of about
        ``wall_restitution · gravity / (fps · substeps)``, so ``speed`` should
        stay above that.)
        """
        if not fps > 0:
            raise ValueError("fps must be > 0")
        dt = 1.0 / fps
        n = max(1, math.ceil(dt / _MAX_DT - 1e-9))
        calm = 0
        elapsed = 0.0
        self._flush()
        while elapsed < max_time:
            self._native.step(dt / n, n)
            elapsed += dt
            self.time += dt
            if self.max_speed() < speed:
                calm += 1
                if calm >= frames:
                    break
            else:
                calm = 0
        self.sync()
        return elapsed

    def __repr__(self) -> str:
        return (
            f"World(bodies={len(self)}, constraints={self._native.constraint_count()}, "
            f"attractors={len(self._attractors)}, time={self.time:.3f})"
        )
