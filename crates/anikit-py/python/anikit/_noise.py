"""Seeded simplex noise and fractal Brownian motion for organic, procedural motion."""

from __future__ import annotations

from typing import Iterable, List, Optional, Sequence, Union

from ._anikit import NativeNoise

__all__ = ["Noise", "Coords"]

#: A coordinate argument of the vectorised helpers: one number (broadcast to
#: every sample) or a sequence of numbers.
Coords = Union[float, Sequence[float], Iterable[float]]


def _coords(v: Coords) -> Union[float, List[float]]:
    if isinstance(v, (int, float)):
        return float(v)
    if isinstance(v, list):
        return v
    return list(v)


class Noise:
    """Seeded 2D/3D simplex noise; values are smooth and roughly within ``[-1, 1]``.

    Use it for wiggles, drifting particles and "alive" idle motion — e.g. feed
    time as one coordinate::

        n = Noise(seed=7)
        n.noise2(t * 0.8, 0.0)                 # one smooth wandering value
        n.fbm3(x * 0.01, y * 0.01, t, octaves=5)  # evolving, detailed 2D field

    The same seed always produces the same field. For bulk evaluation use the
    ``*_many`` helpers (one native call for a whole list) or :meth:`grid`.

    >>> n = Noise(42)
    >>> n.noise2(0.3, 0.7) == Noise(42).noise2(0.3, 0.7)
    True
    >>> len(n.noise2_many([0.1, 0.2, 0.3], 0.0))
    3
    """

    __slots__ = ("_native",)

    def __init__(self, seed: int = 0) -> None:
        if not isinstance(seed, int) or isinstance(seed, bool):
            raise TypeError(f"seed must be an int, got {type(seed).__name__}")
        # Any int works: seeds are taken modulo 2**32.
        self._native = NativeNoise(seed & 0xFFFF_FFFF)

    @property
    def seed(self) -> int:
        return self._native.seed

    @property
    def native(self) -> NativeNoise:
        """The underlying engine object (slightly faster for hot loops)."""
        return self._native

    # -- single samples ------------------------------------------------------

    def noise2(self, x: float, y: float) -> float:
        """2D simplex noise at ``(x, y)``."""
        return self._native.noise2(x, y)

    def noise3(self, x: float, y: float, z: float) -> float:
        """3D simplex noise (use ``z`` as time to evolve a 2D field)."""
        return self._native.noise3(x, y, z)

    def fbm2(self, x: float, y: float, octaves: int = 4, lacunarity: float = 2.0, gain: float = 0.5) -> float:
        """Fractal Brownian motion: ``octaves`` layers of 2D noise, each
        ``lacunarity`` times the frequency and ``gain`` times the amplitude of
        the previous one (normalised to the same range as :meth:`noise2`)."""
        return self._native.fbm2(x, y, octaves, lacunarity, gain)

    def fbm3(self, x: float, y: float, z: float, octaves: int = 4, lacunarity: float = 2.0, gain: float = 0.5) -> float:
        """Fractal Brownian motion over 3D noise (see :meth:`fbm2`)."""
        return self._native.fbm3(x, y, z, octaves, lacunarity, gain)

    # -- vectorised -----------------------------------------------------------

    def noise2_many(self, x: Coords, y: Coords) -> List[float]:
        """:meth:`noise2` for many points in one call. Each coordinate is a
        number (broadcast) or a sequence; sequences must have equal lengths.

        >>> n = Noise(1)
        >>> n.noise2_many([0.5, 1.5], 2.0) == [n.noise2(0.5, 2.0), n.noise2(1.5, 2.0)]
        True
        """
        return self._native.noise2_many(_coords(x), _coords(y))

    def noise3_many(self, x: Coords, y: Coords, z: Coords) -> List[float]:
        """:meth:`noise3` for many points in one call (see :meth:`noise2_many`)."""
        return self._native.noise3_many(_coords(x), _coords(y), _coords(z))

    def fbm2_many(
        self, x: Coords, y: Coords, octaves: int = 4, lacunarity: float = 2.0, gain: float = 0.5
    ) -> List[float]:
        """:meth:`fbm2` for many points in one call (see :meth:`noise2_many`)."""
        return self._native.fbm2_many(_coords(x), _coords(y), octaves, lacunarity, gain)

    def fbm3_many(
        self, x: Coords, y: Coords, z: Coords, octaves: int = 4, lacunarity: float = 2.0, gain: float = 0.5
    ) -> List[float]:
        """:meth:`fbm3` for many points in one call (see :meth:`noise2_many`)."""
        return self._native.fbm3_many(_coords(x), _coords(y), _coords(z), octaves, lacunarity, gain)

    def grid(
        self,
        width: int,
        height: int,
        *,
        x0: float = 0.0,
        y0: float = 0.0,
        step: float = 0.1,
        z: Optional[float] = None,
        octaves: int = 1,
        lacunarity: float = 2.0,
        gain: float = 0.5,
    ) -> List[List[float]]:
        """A ``height × width`` field (list of rows) sampled at
        ``(x0 + i * step, y0 + j * step)``; ``z`` samples a slice of 3D noise
        (animate it over time), ``octaves > 1`` uses fBm."""
        if width < 0 or height < 0:
            raise ValueError("width and height must be >= 0")
        flat = self._native.grid(width, height, x0, y0, step, z, octaves, lacunarity, gain)
        return [flat[j * width : (j + 1) * width] for j in range(height)]

    def __repr__(self) -> str:
        return f"Noise(seed={self.seed})"
