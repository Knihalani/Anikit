"""Film grade and video frame utilities for numpy RGBA frames (needs ``numpy``).

The same post pass as the TypeScript composition's ``post`` option — bloom,
anamorphic streaks, god rays, zoom blur, chromatic aberration, a colour
grade with split toning, vignette and film grain — applied in place to an
``(height, width, 4)`` uint8 array::

    from anikit import video

    grade = video.PostFx(bloom=0.8, bloom_threshold=0.55, bloom_radius=40,
                         streaks=0.5, vignette=0.45, grain=4,
                         contrast=1.08, shadows=(0, 0.015, 0.04),
                         zoom_blur=lambda t: 0.25 if 6 < t < 7 else 0)
    for i in range(frames):
        frame = r3.render(cam, items)                  # anikit.raster.Renderer3D
        grade(frame, t=i / 30, seed=i)                 # in place
        yuv = video.rgba_to_i420(frame)                # for an encoder pipe

Frames returned by :class:`anikit.raster.Renderer3D` are graded with no
copy; any other writable C-contiguous uint8 frame is copied in and back.
:class:`MotionBlur` averages sub-frames (a film shutter).
"""

from __future__ import annotations

from typing import Any, Callable, Dict, Optional, Tuple, Union

import numpy as np

from ._anikit import NativeFrameMixer, NativePostFx
from ._anikit import rgba_to_i420 as _rgba_to_i420

__all__ = ["PostFx", "post", "rgba_to_i420", "MotionBlur", "POST_OPTIONS"]

#: Every option :class:`PostFx` and :func:`post` understand (core defaults: all effects off).
POST_OPTIONS = (
    "bloom",  # glow around bright pixels: strength (0-2)
    "bloom_threshold",  # luminance threshold (0-1, default 0.7)
    "bloom_radius",  # blur radius in px (default 24)
    "aberration",  # radial chromatic aberration, px at the corners
    "vignette",  # darkened corners (0-1)
    "grain",  # film grain amplitude in 8-bit levels (seeded per frame)
    "streaks",  # anamorphic horizontal flares from highlights (0-2)
    "streak_length",  # px (default 160)
    "streak_tint",  # (r, g, b) (default cool blue)
    "zoom_blur",  # radial speed lines towards zoom_center: 0.1 fast, 0.3 warp
    "zoom_center",  # (x, y) 0-1 of the frame (default centre)
    "rays",  # god rays from rays_center (0-2)
    "rays_center",  # (x, y) 0-1 of the frame (default (0.5, 0.4))
    "rays_length",  # 0-1 of the way (default 0.6)
    "exposure",  # 1 = none
    "contrast",  # 1 = none
    "saturation",  # 1 = none
    "shadows",  # (r, g, b) added to shadows (split toning)
    "highlights",  # (r, g, b) multiplied into highlights
)

_VECTORS = {"streak_tint": 3, "zoom_center": 2, "rays_center": 2, "shadows": 3, "highlights": 3}

PostValue = Union[float, Tuple[float, ...], Callable[[float], Any]]


def _check_names(options: Dict[str, Any]) -> None:
    for k in options:
        if k not in POST_OPTIONS:
            import difflib

            near = difflib.get_close_matches(k, POST_OPTIONS, 1)
            hint = f"; did you mean {near[0]!r}?" if near else ""
            raise TypeError(f"unknown post option {k!r}{hint}")


def _resolve(options: Dict[str, Any], t: Optional[float]) -> Dict[str, Any]:
    out: Dict[str, Any] = {}
    for k, v in options.items():
        if v is None:
            continue
        if callable(v):
            if t is None:
                raise TypeError(f"post option {k!r} is a function of time: pass t=")
            v = v(t)
        n = _VECTORS.get(k)
        if n is None:
            out[k] = float(v)
        else:
            vals = tuple(float(c) for c in v)
            if len(vals) != n:
                raise ValueError(f"post option {k!r} needs {n} values, got {v!r}")
            out[k] = vals
    return out


def _frame_bytearray(frame: np.ndarray) -> Optional[bytearray]:
    """The bytearray that ``frame`` views exactly (renderer output), if any."""
    base: Any = frame
    while isinstance(base, np.ndarray):
        if base.base is None:
            return None
        base = base.base
    if isinstance(base, memoryview):  # np.frombuffer keeps a memoryview of the bytearray
        base = base.obj
    if not isinstance(base, bytearray) or len(base) != frame.nbytes or not frame.flags.c_contiguous:
        return None
    start = np.frombuffer(base, np.uint8).__array_interface__["data"][0]
    return base if frame.__array_interface__["data"][0] == start else None


def _check_frame(frame: Any) -> np.ndarray:
    if not isinstance(frame, np.ndarray):
        raise TypeError(f"frames are (height, width, 4) uint8 numpy arrays, got {type(frame).__name__}")
    if frame.dtype != np.uint8 or frame.ndim != 3 or frame.shape[2] != 4:
        raise ValueError(f"frames are (height, width, 4) uint8 arrays, got {frame.dtype} {frame.shape}")
    return frame


class PostFx:
    """A reusable film grade: options (see :data:`POST_OPTIONS`) plus scratch
    buffers kept between frames.

    Every option may be a number (or tuple) or a function of time ``t`` in
    seconds, evaluated by :meth:`apply` — so the grade can follow the edit.
    """

    __slots__ = ("options", "_native")

    def __init__(self, **options: PostValue) -> None:
        _check_names(options)
        self.options: Dict[str, Any] = dict(options)
        self._native = NativePostFx()

    def apply(self, frame: np.ndarray, *, t: Optional[float] = None, seed: int = 0, **overrides: PostValue) -> np.ndarray:
        """Grade ``frame`` (``(height, width, 4)`` uint8) in place and return
        it. ``seed`` re-seeds the grain (use the frame index so parallel
        renders match); ``overrides`` replace options for this call."""
        _check_frame(frame)
        _check_names(overrides)
        opts = _resolve({**self.options, **overrides}, t)
        h, w = frame.shape[:2]
        buf = _frame_bytearray(frame)
        if buf is not None:
            if not frame.flags.writeable:
                raise ValueError("frame is read-only")
            self._native.apply(buf, w, h, seed=int(seed) & 0xFFFFFFFF, **opts)
            return frame
        if not frame.flags.writeable:
            raise ValueError("frame is read-only")
        tmp = bytearray(np.ascontiguousarray(frame).data)
        self._native.apply(tmp, w, h, seed=int(seed) & 0xFFFFFFFF, **opts)
        frame[...] = np.frombuffer(tmp, np.uint8).reshape(frame.shape)
        return frame

    __call__ = apply

    def __repr__(self) -> str:
        inner = ", ".join(f"{k}={v!r}" for k, v in self.options.items())
        return f"PostFx({inner})"


_DEFAULT: Optional[PostFx] = None


def post(frame: np.ndarray, *, t: Optional[float] = None, seed: int = 0, **options: PostValue) -> np.ndarray:
    """Grade ``frame`` in place with ``options`` (see :data:`POST_OPTIONS`)
    and return it. Keep a :class:`PostFx` instead when grading many frames
    from several threads."""
    global _DEFAULT
    if _DEFAULT is None:
        _DEFAULT = PostFx()
    return _DEFAULT.apply(frame, t=t, seed=seed, **options)


def rgba_to_i420(frame: np.ndarray, order: str = "rgba") -> np.ndarray:
    """Convert an RGBA (or ``order="bgra"``) frame to planar I420 — Y
    (``w*h``), then U and V (``w/2 * h/2`` each), BT.709 limited range — as a
    flat uint8 array. Width and height must be even."""
    _check_frame(frame)
    h, w = frame.shape[:2]
    buf = _frame_bytearray(frame)
    src: Union[bytes, bytearray] = buf if buf is not None else np.ascontiguousarray(frame).tobytes()
    return np.frombuffer(_rgba_to_i420(src, w, h, order), np.uint8)


class MotionBlur:
    """Average RGBA sub-frames (motion blur across a shutter): :meth:`add` up
    to 257 frames, then :meth:`resolve` the mean (which resets)."""

    __slots__ = ("_native",)

    def __init__(self, width: int, height: int) -> None:
        self._native = NativeFrameMixer(int(width), int(height))

    @property
    def size(self) -> Tuple[int, int]:
        return (self._native.width, self._native.height)

    @property
    def count(self) -> int:
        """Frames added since the last resolve/reset."""
        return self._native.count

    def add(self, frame: np.ndarray) -> int:
        """Accumulate one ``(height, width, 4)`` uint8 frame; returns the count."""
        _check_frame(frame)
        if (frame.shape[1], frame.shape[0]) != self.size:
            raise ValueError(f"frame is {frame.shape[1]}x{frame.shape[0]}, expected {self.size[0]}x{self.size[1]}")
        buf = _frame_bytearray(frame)
        return self._native.add(buf if buf is not None else np.ascontiguousarray(frame).tobytes())

    def resolve(self) -> np.ndarray:
        """The averaged frame as a new writable array; resets the accumulator."""
        w, h = self.size
        return np.frombuffer(self._native.resolve(), np.uint8).reshape(h, w, 4)

    def reset(self) -> None:
        self._native.reset()

    def __repr__(self) -> str:
        return f"MotionBlur({self.size[0]}x{self.size[1]}, count={self.count})"
