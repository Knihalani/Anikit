"""Camera flight paths: fly a camera smoothly through a list of points,
looking ahead along the path and banking into turns.

A pure-Python port of the TypeScript ``flightPath`` (same numbers)::

    import anikit
    from anikit.transform import Camera

    path = anikit.flight_path([(0, 0, 2000), (300, 80, 0), (-200, -60, -3000), (0, 0, -6000)],
                              look_ahead=500, bank=1)
    s = path.timing([(0, 0), (2, 0.3), (5, 1)])     # seconds -> distance (smooth, monotone)
    cam = Camera(fov=50)
    for frame in range(150):
        path.apply(cam, s(frame / 30))              # position, look-at target and roll
"""

from __future__ import annotations

import math
from typing import Any, Callable, List, Mapping, NamedTuple, Optional, Sequence, Tuple

__all__ = ["FlightPath", "FlightPose", "flight_path"]

Vec3 = Tuple[float, float, float]


class FlightPose(NamedTuple):
    """A camera pose on a :class:`FlightPath`."""

    eye: Vec3
    target: Vec3
    #: Bank angle in degrees (apply as the camera's ``roll``).
    roll: float
    #: Unit direction of travel.
    forward: Vec3


def _v3(p: Any) -> Vec3:
    if isinstance(p, Mapping):
        return (float(p["x"]), float(p["y"]), float(p["z"]))
    if hasattr(p, "x") and hasattr(p, "y") and hasattr(p, "z"):
        return (float(p.x), float(p.y), float(p.z))
    if len(p) != 3:
        raise ValueError(f"flight path points are (x, y, z), got {p!r}")
    return (float(p[0]), float(p[1]), float(p[2]))


def _mirror(a: Vec3, b: Vec3) -> Vec3:
    return (2 * a[0] - b[0], 2 * a[1] - b[1], 2 * a[2] - b[2])


class FlightPath:
    """A smooth camera path through ``points`` (centripetal Catmull–Rom, so no
    cusps or overshoot loops) with an arc-length table.

    ``look_ahead`` is how far ahead along the path the camera looks (world
    units), ``bank`` the banking strength (0 level, 1 natural, 2 aggressive),
    ``max_bank`` the largest bank angle in degrees, and ``closed`` joins the
    last point to the first. Distances ``s`` are arc lengths in world units
    (``0 … length``).
    """

    __slots__ = ("length", "look_ahead", "bank", "max_bank", "closed", "_pts", "_cum", "_tab")

    _PER = 48

    def __init__(
        self,
        points: Sequence[Any],
        *,
        look_ahead: float = 400.0,
        bank: float = 1.0,
        max_bank: float = 55.0,
        closed: bool = False,
    ) -> None:
        pts = [_v3(p) for p in points]
        if len(pts) < 2:
            raise ValueError("flight_path needs at least 2 points")
        self.look_ahead = float(look_ahead)
        self.bank = float(bank)
        self.max_bank = float(max_bank)
        self.closed = bool(closed)
        self._pts = pts
        segs = len(pts) if self.closed else len(pts) - 1
        n = segs * self._PER + 1
        tab: List[Vec3] = []
        cum: List[float] = [0.0] * n
        for i in range(n):
            p = self._raw(i / self._PER)
            if i:
                q = tab[i - 1]
                cum[i] = cum[i - 1] + math.hypot(p[0] - q[0], p[1] - q[1], p[2] - q[2])
            tab.append(p)
        self._tab = tab
        self._cum = cum
        #: Total arc length in world units.
        self.length: float = cum[-1]

    def _raw(self, u: float) -> Vec3:
        """Centripetal Catmull–Rom at parameter ``u`` in ``[0, segments]``."""
        P, m, closed = self._pts, len(self._pts), self.closed
        segs = m if closed else m - 1
        i = min(segs - 1, max(0, math.floor(u)))
        t = u - i

        def get(k: int) -> Vec3:
            if closed:
                return P[k % m]
            if k < 0:
                return _mirror(P[0], P[1])
            if k >= m:
                return _mirror(P[m - 1], P[m - 2])
            return P[k]

        p0, p1, p2, p3 = get(i - 1), get(i), get(i + 1), get(i + 2)

        def d(a: Vec3, b: Vec3) -> float:
            return max(1e-6, math.sqrt(math.hypot(b[0] - a[0], b[1] - a[1], b[2] - a[2])))

        t0 = 0.0
        t1 = t0 + d(p0, p1)
        t2 = t1 + d(p1, p2)
        t3 = t2 + d(p2, p3)
        tt = t1 + (t2 - t1) * t
        out = [0.0, 0.0, 0.0]
        for k in range(3):
            a1 = ((t1 - tt) / (t1 - t0)) * p0[k] + ((tt - t0) / (t1 - t0)) * p1[k]
            a2 = ((t2 - tt) / (t2 - t1)) * p1[k] + ((tt - t1) / (t2 - t1)) * p2[k]
            a3 = ((t3 - tt) / (t3 - t2)) * p2[k] + ((tt - t2) / (t3 - t2)) * p3[k]
            b1 = ((t2 - tt) / (t2 - t0)) * a1 + ((tt - t0) / (t2 - t0)) * a2
            b2 = ((t3 - tt) / (t3 - t1)) * a2 + ((tt - t1) / (t3 - t1)) * a3
            out[k] = ((t2 - tt) / (t2 - t1)) * b1 + ((tt - t1) / (t2 - t1)) * b2
        return (out[0], out[1], out[2])

    @property
    def points(self) -> List[Vec3]:
        """The control points."""
        return list(self._pts)

    def point(self, s: float) -> Vec3:
        """Position at arc length ``s`` (clamped; wraps when closed)."""
        L = self.length
        s = float(s)
        s = math.fmod(math.fmod(s, L) + L, L) if self.closed else min(L, max(0.0, s))
        c = self._cum
        lo, hi = 0, len(c) - 1
        while hi - lo > 1:
            mid = (lo + hi) >> 1
            if c[mid] <= s:
                lo = mid
            else:
                hi = mid
        f = (s - c[lo]) / max(1e-9, c[hi] - c[lo])
        a, b = self._tab[lo], self._tab[hi]
        return (a[0] + (b[0] - a[0]) * f, a[1] + (b[1] - a[1]) * f, a[2] + (b[2] - a[2]) * f)

    def tangent(self, s: float) -> Vec3:
        """Unit tangent at arc length ``s``."""
        h = max(1.0, self.length * 1e-4)
        a, b = self.point(s - h), self.point(s + h)
        d = (b[0] - a[0], b[1] - a[1], b[2] - a[2])
        n = math.hypot(*d) or 1.0
        return (d[0] / n, d[1] / n, d[2] / n)

    def at(self, s: float, *, look_ahead: Optional[float] = None, bank: Optional[float] = None) -> FlightPose:
        """Camera pose at arc length ``s``: position, look-at target ahead, and bank."""
        s = float(s)
        ahead = self.look_ahead if look_ahead is None else float(look_ahead)
        eye = self.point(s)
        end = math.inf if self.closed else self.length
        target = self.point(min(end, s + ahead))
        fwd = self.tangent(s)
        if not self.closed and s + ahead > self.length:
            # Past the end: keep looking along the final direction.
            over = s + ahead - self.length
            target = (target[0] + fwd[0] * over, target[1] + fwd[1] * over, target[2] + fwd[2] * over)
        # Bank from the signed horizontal turn rate, averaged over the look-ahead window.
        turn = 0.0
        k, span = 4, max(50.0, ahead)
        for i in range(k):
            s0 = s - span * 0.25 + (span * i) / k
            a, b = self.tangent(s0), self.tangent(s0 + span / k)
            turn += (a[0] * b[2] - a[2] * b[0]) / (span / k)  # y-component of a x b per unit length
        turn /= k
        bank_k = (self.bank if bank is None else float(bank)) * 900
        mx = self.max_bank
        roll = max(-mx, min(mx, math.atan(turn * bank_k) * 180 / math.pi))
        return FlightPose(eye, target, roll, fwd)

    def apply(self, camera: Any, s: float, extra_roll: float = 0.0) -> FlightPose:
        """Put ``camera`` (an :class:`anikit.transform.Camera`) at arc length
        ``s``: sets its ``position``, ``target`` and ``roll`` (plus
        ``extra_roll`` degrees, e.g. a barrel roll). Returns the pose."""
        p = self.at(s)
        camera.position = p.eye
        camera.target = p.target
        camera.roll = p.roll + float(extra_roll)
        return p

    def timing(self, keys: Sequence[Sequence[float]]) -> Callable[[float], float]:
        """A smooth, monotone time -> distance function from keys
        ``(seconds, fraction of length)`` (monotone cubic: speed changes
        smoothly and never reverses). Hold a fraction to stop."""
        k = sorted(((float(a), float(b)) for a, b in keys), key=lambda p: p[0])
        n = len(k)
        if n == 0:
            raise ValueError("timing needs at least one (seconds, fraction) key")
        L = self.length
        if n == 1:
            v = k[0][1] * L
            return lambda t: v
        x = [p[0] for p in k]
        y = [p[1] * L for p in k]
        dx = [max(1e-9, x[i + 1] - x[i]) for i in range(n - 1)]
        sl = [(y[i + 1] - y[i]) / dx[i] for i in range(n - 1)]
        # Fritsch–Carlson tangents.
        m = [0.0] * n
        m[0] = sl[0]
        m[n - 1] = sl[n - 2]
        for i in range(1, n - 1):
            if sl[i - 1] * sl[i] <= 0:
                m[i] = 0.0
            else:
                m[i] = (3 * (dx[i - 1] + dx[i])) / (
                    (2 * dx[i] + dx[i - 1]) / sl[i - 1] + (dx[i] + 2 * dx[i - 1]) / sl[i]
                )
        for i in range(n - 1):
            if sl[i] == 0:
                m[i] = 0.0
                m[i + 1] = 0.0
                continue
            a, b = m[i] / sl[i], m[i + 1] / sl[i]
            h = math.hypot(a, b)
            if h > 3:
                m[i] = (3 * a * sl[i]) / h
                m[i + 1] = (3 * b * sl[i]) / h

        def at(t: float) -> float:
            if t <= x[0]:
                return y[0]
            if t >= x[n - 1]:
                return y[n - 1]
            i = 0
            while i < n - 2 and t > x[i + 1]:
                i += 1
            h = dx[i]
            u = (t - x[i]) / h
            u2 = u * u
            u3 = u2 * u
            return (
                (2 * u3 - 3 * u2 + 1) * y[i]
                + (u3 - 2 * u2 + u) * h * m[i]
                + (-2 * u3 + 3 * u2) * y[i + 1]
                + (u3 - u2) * h * m[i + 1]
            )

        return at

    def __repr__(self) -> str:
        return f"FlightPath({len(self._pts)} points, length={self.length:g}{', closed' if self.closed else ''})"


def flight_path(
    points: Sequence[Any],
    *,
    look_ahead: float = 400.0,
    bank: float = 1.0,
    max_bank: float = 55.0,
    closed: bool = False,
) -> FlightPath:
    """A smooth camera flight path through ``points`` (see :class:`FlightPath`)."""
    return FlightPath(points, look_ahead=look_ahead, bank=bank, max_bank=max_bank, closed=closed)
