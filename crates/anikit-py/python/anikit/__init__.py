"""Anikit — a high-performance animation engine with a Rust core.

Animate plain Python objects (dicts, objects, lists of them), drive the world
with :func:`tick` or :func:`render`, and export to CSS/WAAPI. Also: a fast 2D
physics :class:`World` (bodies, ropes, cloth, attractors), 3D transforms /
projection in :mod:`anikit.transform`, and particle clouds that morph between
shapes (:class:`PointCloud`) rendered as glowing light (:class:`Splat`).
With numpy installed: solid 3D rendering into numpy frames in
:mod:`anikit.raster` (meshes, instancing, materials, mirror floors,
:func:`~anikit.raster.shatter`) and the film grade in :mod:`anikit.video`;
camera flights with :func:`flight_path`::

    import anikit

    box = {"x": 0, "opacity": 0, "color": "#ff0000"}
    anim = anikit.animate(box, {"x": 100, "opacity": [0, 1], "color": "blue"},
                          duration=0.6, ease="outBack")
    for frame, t in anikit.render(fps=30):
        print(frame, box)

All public times are in **seconds**.
"""

from __future__ import annotations

from typing import Any, Iterator, Mapping, Optional, Tuple

from . import color, export, transform
from ._anikit import __version__
from ._animation import Animation, Position, Timeline, animate, timeline
from ._cloud import PointCloud, ShapeInput, Splat
from ._easing import Easing, EasingLike, ease
from ._engine import Engine, default_engine, resolve_engine, set_default_engine
from ._flight import FlightPath, FlightPose, flight_path
from ._noise import Noise
from ._path import ClosestPoint, Morph, MorphSequence, MotionPath, Path, PathLike, morph, morph_many, motion_path
from ._physics import PhysicsAnimation, SpringAnimation, inertia, physics, spring
from ._stagger import Stagger, stagger
from ._world import Attractor, Body, Cloth, Link, Rope, World

__all__ = [
    "__version__",
    # engine
    "Engine",
    "default_engine",
    "set_default_engine",
    "tick",
    "render",
    "run_until_idle",
    "set",
    "kill_all",
    # animation
    "animate",
    "Animation",
    "timeline",
    "Timeline",
    "Position",
    "spring",
    "inertia",
    "physics",
    "SpringAnimation",
    "PhysicsAnimation",
    # 2D physics world
    "World",
    "Body",
    "Rope",
    "Cloth",
    "Link",
    "Attractor",
    # point clouds
    "PointCloud",
    "ShapeInput",
    "Splat",
    # camera flights
    "FlightPath",
    "FlightPose",
    "flight_path",
    # easing
    "Easing",
    "EasingLike",
    "ease",
    # geometry
    "Path",
    "PathLike",
    "Morph",
    "MorphSequence",
    "morph",
    "morph_many",
    "ClosestPoint",
    "MotionPath",
    "motion_path",
    # utilities
    "stagger",
    "Stagger",
    "Noise",
    "color",
    "export",
    "transform",
]

# Submodules that need numpy load on first access (``anikit.raster``, ``anikit.video``).
_LAZY = ("raster", "video")


def __getattr__(name: str) -> Any:
    if name in _LAZY:
        import importlib

        return importlib.import_module(f".{name}", __name__)
    raise AttributeError(f"module 'anikit' has no attribute {name!r}")


def tick(dt: float, *, engine: Optional[Engine] = None) -> None:
    """Advance the (default) engine by ``dt`` seconds, writing values and firing callbacks."""
    resolve_engine(engine).tick(dt)


def render(
    fps: float = 60.0,
    duration: Optional[float] = None,
    *,
    max_duration: float = 600.0,
    engine: Optional[Engine] = None,
) -> Iterator[Tuple[int, float]]:
    """Iterate frames ``(frame_index, time)`` while advancing the engine; see :meth:`Engine.render`."""
    return resolve_engine(engine).render(fps, duration, max_duration=max_duration)


def run_until_idle(
    fps: float = 60.0, *, max_time: float = 600.0, realtime: bool = False, engine: Optional[Engine] = None
) -> float:
    """Tick until nothing is playing; returns the simulated seconds. See :meth:`Engine.run_until_idle`."""
    return resolve_engine(engine).run_until_idle(fps, max_time=max_time, realtime=realtime)


def set(  # noqa: A001 - mirrors the animation API's `set`
    targets: Any, props: Mapping[str, Any], *, color_space: str = "oklab", engine: Optional[Engine] = None
) -> None:
    """Write property values immediately (no animation)."""
    resolve_engine(engine).set(targets, props, color_space=color_space)


def kill_all(*, engine: Optional[Engine] = None) -> None:
    """Kill every animation and simulation of the (default) engine."""
    resolve_engine(engine).clear()
