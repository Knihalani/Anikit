"""A physically based, animated Schwarzschild black hole rendered to pixels.

Exact null geodesics, a Page–Thorne accretion disk coloured by the blackbody
at the Doppler- and gravitationally shifted temperature with g⁴ beaming, the
lensed arc over and under the shadow, the photon ring, a lensed star field,
orbiting hot spots, and an infalling observer who can cross the horizon. See
``docs/black-hole.md`` for the physics::

    bh = anikit.BlackHole(640, 400, mode="cinematic")
    frame = bh.render(distance=30, inclination=84, fov=34, time=1.0)  # RGBA bytes
    Image.frombytes("RGBA", bh.size, frame).save("black-hole.png")
"""

from __future__ import annotations

from typing import Any, Optional, Tuple

from ._anikit import NativeBlackHole

__all__ = ["BlackHole"]


class BlackHole:
    """Black-hole renderer producing row-major RGBA ``bytes`` frames.

    ``mode`` is ``"physical"`` (Doppler colour and g⁴ beaming, EHT/Luminet)
    or ``"cinematic"`` (Interstellar: no Doppler colour, softened beaming).
    """

    def __init__(self, width: int, height: int, mode: str = "physical", seed: int = 1) -> None:
        if mode not in ("physical", "cinematic"):
            raise ValueError(f"mode must be 'physical' or 'cinematic', got {mode!r}")
        self.mode = mode
        self._n = NativeBlackHole(int(width), int(height), 1 if mode == "cinematic" else 0, int(seed))

    @property
    def size(self) -> Tuple[int, int]:
        """``(width, height)`` in pixels."""
        return (self._n.width, self._n.height)

    def resize(self, width: int, height: int) -> None:
        """Change the output size."""
        self._n.resize(int(width), int(height))

    def render(
        self,
        distance: float = 30.0,
        inclination: float = 80.0,
        fov: float = 40.0,
        time: float = 0.0,
        exposure: float = 1.0,
        bloom: float = 1.0,
        grain: float = 1.0,
        hot_spots: float = 1.0,
        jet: float = 0.0,
        azimuth: float = 0.0,
        look: float = 0.0,
        roll: float = 0.0,
        fall: float = 0.0,
        temperature: Optional[float] = None,
        disk_outer: float = 22.0,
        stars: float = 1.0,
        speed: float = 1.0,
        nebula: float = 1.0,
    ) -> bytes:
        """Render one frame (same parameters as the TypeScript ``render()``,
        in snake_case; ``distance`` in black-hole masses)."""
        return self._n.render(
            distance, inclination, fov, time, exposure, bloom, grain, hot_spots, jet,
            azimuth, look, roll, fall, temperature, disk_outer, stars, speed, nebula,
        )

    @property
    def stats(self) -> dict[str, Any]:
        """Counters from the last frame."""
        samples, stars, table, geometry, shadow = self._n.stats()
        return {
            "samples": samples,
            "stars": stars,
            "table_built": table,
            "geometry_built": geometry,
            "shadow_radius": shadow,
        }
