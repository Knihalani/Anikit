"""CSS colour helpers backed by the engine's colour module.

Colours are given either as CSS strings (``"#f80"``, ``"rebeccapurple"``,
``"hsl(200 50% 40% / .5)"``, ``"oklch(70% 0.1 200)"``, ...) or as
``(r, g, b[, a])`` tuples with r/g/b in 0–255 and alpha in 0–1.

Interpolation happens in a perceptual space (OKLab by default) with
premultiplied alpha, so mixes never go muddy or flash through black.
"""

from __future__ import annotations

from typing import Optional, Sequence, Tuple, Union

from . import _anikit

__all__ = ["RGBA", "ColorLike", "SPACES", "parse", "try_parse", "mix", "mix_rgba", "to_css", "to_hex", "is_color"]

RGBA = Tuple[float, float, float, float]
ColorLike = Union[str, Sequence[float]]

#: Interpolation spaces understood by :func:`mix` and colour tracks.
SPACES = ("oklab", "oklch", "srgb", "linear-srgb", "hsl")


def _rgba(c: ColorLike) -> RGBA:
    if isinstance(c, str):
        return _anikit.color_parse(c)
    try:
        vals = [float(v) for v in c]
    except TypeError:
        raise TypeError(f"expected a CSS colour string or (r, g, b[, a]) tuple, got {type(c).__name__}") from None
    if len(vals) == 3:
        vals.append(1.0)
    if len(vals) != 4:
        raise ValueError(f"colour tuples need 3 or 4 components, got {len(vals)}")
    return (vals[0], vals[1], vals[2], vals[3])


def parse(c: ColorLike) -> RGBA:
    """Parse a colour into ``(r, g, b, a)``; raises :class:`ValueError` if invalid."""
    return _rgba(c)


def try_parse(c: str) -> Optional[RGBA]:
    """Like :func:`parse` but returns ``None`` for anything that is not a colour."""
    return _anikit.color_try_parse(c) if isinstance(c, str) else None


def is_color(value: object) -> bool:
    """``True`` if ``value`` is a string the engine parses as a CSS colour."""
    return isinstance(value, str) and _anikit.color_try_parse(value) is not None


def mix(a: ColorLike, b: ColorLike, t: float, space: str = "oklab") -> str:
    """Mix two colours: ``t=0`` gives ``a``, ``t=1`` gives ``b``. Returns a CSS colour string.

    ``space`` is one of :data:`SPACES`.
    """
    return _anikit.color_to_css(_anikit.color_mix(_rgba(a), _rgba(b), float(t), space))


def mix_rgba(a: ColorLike, b: ColorLike, t: float, space: str = "oklab") -> RGBA:
    """Like :func:`mix` but returns an ``(r, g, b, a)`` tuple."""
    return _anikit.color_mix(_rgba(a), _rgba(b), float(t), space)


def to_css(c: ColorLike) -> str:
    """``rgb(r,g,b)`` or ``rgba(r,g,b,a)``."""
    return _anikit.color_to_css(_rgba(c))


def to_hex(c: ColorLike) -> str:
    """``#rrggbb`` (or ``#rrggbbaa`` when translucent)."""
    return _anikit.color_to_hex(_rgba(c))
