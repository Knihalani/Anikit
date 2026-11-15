"""Stagger distributions (delays or any value) across many targets."""

from __future__ import annotations

from typing import Any, Dict, List, Optional, Sequence, Tuple, Union, overload

from . import _anikit
from ._easing import Easing, EasingLike

__all__ = ["stagger", "Stagger", "StaggerFrom"]

StaggerFrom = Union[str, int, float]


class Stagger:
    """A lazily-sized stagger: call it as ``fn(target, index, total)``.

    Returned by :func:`stagger` when ``count`` is omitted, so it can be passed
    directly as ``delay=`` (or as a property value) without knowing the number
    of targets up front.
    """

    __slots__ = ("_kw", "_cache")

    def __init__(self, **kw: Any) -> None:
        self._kw = kw
        self._cache: Dict[int, List[float]] = {}

    def values(self, count: int) -> List[float]:
        v = self._cache.get(count)
        if v is None:
            v = self._cache[count] = stagger(count, **self._kw)
        return v

    def __call__(self, target: Any, index: int, total: int) -> float:
        return self.values(total)[index]

    def __repr__(self) -> str:
        args = ", ".join(f"{k}={v!r}" for k, v in self._kw.items())
        return f"Stagger({args})"


@overload
def stagger(
    count: Union[int, Sequence[Any]],
    each: float = 0.1,
    *,
    total: Optional[float] = None,
    start: float = 0.0,
    from_: StaggerFrom = "first",
    grid: Optional[Tuple[int, int]] = None,
    axis: Optional[str] = None,
    ease: EasingLike = None,
    seed: int = 1,
) -> List[float]: ...


@overload
def stagger(
    count: None = None,
    each: float = 0.1,
    *,
    total: Optional[float] = None,
    start: float = 0.0,
    from_: StaggerFrom = "first",
    grid: Optional[Tuple[int, int]] = None,
    axis: Optional[str] = None,
    ease: EasingLike = None,
    seed: int = 1,
) -> Stagger: ...


def stagger(
    count: Union[int, Sequence[Any], None] = None,
    each: float = 0.1,
    *,
    total: Optional[float] = None,
    start: float = 0.0,
    from_: StaggerFrom = "first",
    grid: Optional[Tuple[int, int]] = None,
    axis: Optional[str] = None,
    ease: EasingLike = None,
    seed: int = 1,
) -> Union[List[float], Stagger]:
    """Distribute values over ``count`` items (typically delays in seconds).

    * ``each`` — step between neighbours; or ``total`` — spread over exactly this amount
    * ``start`` — added to every value
    * ``from_`` — ``"first"``, ``"last"``, ``"center"``, ``"edges"``, ``"random"`` or an index
    * ``grid=(columns, rows)`` — 2D distance; ``axis="x"|"y"`` restricts it to one axis
    * ``ease`` — eases the distribution; ``seed`` — for ``"random"``

    ``count`` may also be a sequence of targets (its length is used). Without
    ``count`` a :class:`Stagger` callable is returned, sized lazily.

    >>> stagger(4, 0.1)
    [0.0, 0.1, 0.2, 0.30000000000000004]
    >>> stagger(5, 1, from_="center")
    [2.0, 1.0, 0.0, 1.0, 2.0]
    """
    if count is None:
        return Stagger(each=each, total=total, start=start, from_=from_, grid=grid, axis=axis, ease=ease, seed=seed)
    n = count if isinstance(count, int) else len(count)
    if n < 0:
        raise ValueError("count must be >= 0")
    index = 0.0
    if isinstance(from_, bool):
        raise TypeError("from_ must be a string or an index")
    if isinstance(from_, (int, float)):
        origin, index = "index", float(from_)
    elif isinstance(from_, str):
        origin = from_.lower()
    else:
        raise TypeError("from_ must be a string or an index")
    native_ease = None if ease is None else Easing(ease).native
    g = None
    if grid is not None:
        cols, rows = grid
        g = (int(cols), int(rows))
    return _anikit.stagger(
        n,
        float(each),
        None if total is None else float(total),
        float(start),
        origin,
        index,
        int(seed) & 0xFFFFFFFF,
        g,
        axis,
        native_ease,
    )
