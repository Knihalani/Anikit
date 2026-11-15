"""Easing curves: parsing, evaluation, sampling and native CSS export."""

from __future__ import annotations

import math
from functools import lru_cache
from typing import Callable, List, Optional, Sequence, Tuple, Union

from ._anikit import NativeEasing

__all__ = ["Easing", "EasingLike", "ease"]

#: Anything accepted where an easing is expected: a string in any supported
#: syntax, an :class:`Easing`, a plain ``f(t) -> float`` callable, or ``None``
#: (linear).
EasingLike = Union[str, "Easing", NativeEasing, Callable[[float], float], None]

_TABLE_SAMPLES = 512


class Easing:
    """An easing curve mapping progress ``t ∈ [0, 1]`` to eased progress.

    Accepts every syntax the engine understands::

        Easing("outCubic")                     # Penner names (also easeOutCubic, cubic.out)
        Easing("power2.inOut")                 # GSAP names
        Easing("back.out(1.7)")                # with parameters
        Easing("cubic-bezier(.2, .8, .2, 1)")  # CSS
        Easing("steps(4, jump-start)")
        Easing("linear(0, 0.25 75%, 1)")
        Easing("spring(200, 20)")              # stiffness, damping[, mass, velocity]
        Easing("spring(bounce: 0.3, duration: 0.6)")
        Easing("wiggle(5)"), Easing("anticipate"), Easing("slowmo(0.7, 0.7)")
        Easing("custom(M0,0 C0.5,0 0.5,1 1,1)")  # CustomEase drawn as an SVG path
        Easing("reverse(inQuad)")              # 1 - f(1 - t): an "in" becomes an "out"
        Easing("mirror(outBack)")              # there and back within one iteration
        Easing("blend(inQuad, outBounce, 0.3)")  # weighted mix of two easings
        Easing("rough(20, 1, 7, outQuad)")     # seeded jitter: points, strength, seed, template
        Easing("expoScale(1, 10)")             # perceptually even zoom from scale 1 to 10
        Easing("spring.wobbly")                # presets: gentle wobbly stiff slow molasses snappy bouncy
        Easing(lambda t: t * t)                # any Python callable (sampled)

    Instances are callable: ``Easing("outQuad")(0.5) == 0.75``.
    """

    __slots__ = ("_native", "_spec")

    def __init__(self, spec: EasingLike = "linear", *, samples: int = _TABLE_SAMPLES) -> None:
        if spec is None:
            spec = "linear"
        if isinstance(spec, Easing):
            self._native: NativeEasing = spec._native
            self._spec: Optional[str] = spec._spec
        elif isinstance(spec, str):
            self._native = _parse(spec)
            self._spec = spec
        elif isinstance(spec, NativeEasing):
            self._native = spec
            self._spec = None
        elif callable(spec):
            self._native = _table_from_callable(spec, samples)
            self._spec = None
        else:
            raise TypeError(f"expected an easing string, Easing or callable, got {type(spec).__name__}")

    # -- constructors -----------------------------------------------------

    @classmethod
    def bezier(cls, x1: float, y1: float, x2: float, y2: float) -> "Easing":
        """CSS ``cubic-bezier(x1, y1, x2, y2)``."""
        return cls(NativeEasing.bezier(x1, y1, x2, y2))

    @classmethod
    def spring(
        cls,
        stiffness: float = 100.0,
        damping: float = 10.0,
        mass: float = 1.0,
        velocity: float = 0.0,
        *,
        bounce: Optional[float] = None,
        duration: Optional[float] = None,
    ) -> "Easing":
        """A spring easing; its natural length is :attr:`duration` (seconds).

        Pass ``bounce``/``duration`` for the perceptual parameterisation.
        """
        if bounce is not None or duration is not None:
            from ._anikit import spring_from_duration_bounce

            stiffness, damping, mass = spring_from_duration_bounce(
                0.5 if duration is None else duration, 0.25 if bounce is None else bounce
            )
        return cls(NativeEasing.spring(stiffness, damping, mass, velocity))

    @classmethod
    def points(cls, stops: Sequence[Tuple[float, float]]) -> "Easing":
        """Piecewise-linear easing through ``(x, y)`` stops (CSS ``linear()``)."""
        return cls(NativeEasing.points([(float(x), float(y)) for x, y in stops]))

    @classmethod
    def steps(cls, n: int, position: str = "jump-end") -> "Easing":
        """CSS ``steps(n, position)``."""
        return cls(f"steps({int(n)}, {position})")

    @classmethod
    def from_path(cls, d: str) -> "Easing":
        """A "CustomEase" drawn as SVG path data from ``(0, 0)`` to ``(1, 1)``, y up.

        Paths in other coordinate ranges (e.g. copied from a graph editor in
        SVG's y-down space) are normalised by their start and end points.

        >>> e = Easing.from_path("M0,0 C0.5,0 0.5,1 1,1")
        >>> e(0.25) < 0.25 < 0.75 < e(0.75)
        True
        """
        if not isinstance(d, str):
            raise TypeError(f"expected SVG path data (str), got {type(d).__name__}")
        e = cls(NativeEasing.from_path(d))
        e._spec = f"custom({d})"
        return e

    @classmethod
    def rough(cls, points: int = 20, strength: float = 1.0, seed: int = 1, template: EasingLike = None) -> "Easing":
        """Jittery, hand-drawn looking easing: ``points`` random offsets of up to
        ``0.1 * strength`` around ``template`` (linear by default). Deterministic
        for a given ``seed``; still starts at 0 and ends at 1.
        """
        if points < 1:
            raise ValueError("points must be >= 1")
        if seed < 0:
            raise ValueError("seed must be >= 0")
        tpl = None if template is None else Easing(template).native
        return cls(NativeEasing.rough(int(points), float(strength), int(seed), tpl))

    @classmethod
    def blend(cls, a: EasingLike, b: EasingLike, weight: float = 0.5) -> "Easing":
        """Weighted mix ``a(t) * (1 - weight) + b(t) * weight``."""
        return cls(NativeEasing.blend(Easing(a).native, Easing(b).native, float(weight)))

    @classmethod
    def expo_scale(cls, start: float, end: float) -> "Easing":
        """Perceptually even scaling from ``start`` to ``end`` (both > 0): when
        tweening a zoom factor, each step multiplies the scale by the same ratio."""
        if not (start > 0 and end > 0):
            raise ValueError("expo_scale needs positive start and end scales")
        return cls(f"expoScale({float(start)!r}, {float(end)!r})")

    # -- modifiers --------------------------------------------------------

    def reversed(self) -> "Easing":
        """Time-reversed copy ``1 - f(1 - t)`` (``inQuad`` → ``outQuad``).

        >>> Easing("inQuad").reversed()(0.5)
        0.75
        """
        return Easing(self._native.reversed())

    def mirrored(self) -> "Easing":
        """Copy that goes there and back within one iteration: ``f(2t)`` then ``f(2 - 2t)``.

        >>> Easing("linear").mirrored()(0.5)
        1.0
        """
        return Easing(self._native.mirrored())

    # -- evaluation -------------------------------------------------------

    def __call__(self, t: float) -> float:
        return self._native(t)

    def sample(self, n: int) -> List[float]:
        """``n`` uniform samples over ``[0, 1]`` (both ends included)."""
        return self._native.sample(n)

    def css(self, tolerance: float = 0.002) -> str:
        """The most compact native CSS easing: ``cubic-bezier()``/``steps()``
        when exact, otherwise a ``linear(...)`` approximation within ``tolerance``."""
        return self._native.css(tolerance)

    @property
    def duration(self) -> Optional[float]:
        """Natural duration in seconds for spring easings, else ``None``."""
        return self._native.duration

    @property
    def kind(self) -> str:
        """``linear``, ``curve``, ``bezier``, ``steps``, ``spring``, ``points``,
        ``table``, ``wiggle``, ``slowmo``, ``anticipate``, ``reverse``, ``mirror`` or ``blend``."""
        return self._native.kind

    @property
    def native(self) -> NativeEasing:
        """The underlying engine object."""
        return self._native

    @property
    def spec(self) -> Optional[str]:
        """The source string, if constructed from one."""
        return self._spec

    def __eq__(self, other: object) -> bool:
        if isinstance(other, Easing):
            return self._native == other._native
        return NotImplemented

    def __hash__(self) -> int:
        return hash(self._native)

    def __repr__(self) -> str:
        if self._spec is not None:
            return f"Easing({self._spec!r})"
        return f"Easing(<{self.kind}>)"


_CURVES = ("Quad", "Cubic", "Quart", "Quint", "Sine", "Expo", "Circ", "Back", "Elastic", "Bounce")
#: Easing names, for "did you mean" suggestions.
EASING_NAMES = (
    ["linear", "ease", "ease-in", "ease-out", "ease-in-out", "step-start", "step-end", "spring", "anticipate", "wiggle", "shake", "slowmo"]
    + [m + c for c in _CURVES for m in ("in", "out", "inOut", "outIn")]
    + [f"power{p}.{m}" for p in range(1, 5) for m in ("in", "out", "inOut")]
    + ["gentle", "wobbly", "stiff", "slow", "molasses", "snappy", "bouncy"]
)


@lru_cache(maxsize=256)
def _parse(spec: str) -> NativeEasing:
    try:
        return NativeEasing(spec)
    except ValueError:
        import difflib

        head = spec.split("(")[0].strip()
        near = difflib.get_close_matches(head, EASING_NAMES, n=1, cutoff=0.7)
        hint = f' Did you mean "{near[0]}"?' if near else ""
        raise ValueError(
            f"unknown easing: {spec!r}.{hint} Use a name such as 'outCubic', 'power2.inOut' or 'ease-in-out', "
            "a function form such as 'cubic-bezier(.2,.8,.2,1)' or 'spring(bounce: 0.3)', or a callable."
        ) from None


def _table_from_callable(fn: Callable[[float], float], samples: int) -> NativeEasing:
    n = max(2, int(samples))
    values = []
    for i in range(n):
        v = float(fn(i / (n - 1)))
        if not math.isfinite(v):
            raise ValueError(f"easing function returned a non-finite value at t={i / (n - 1)}")
        values.append(v)
    return NativeEasing.table(values)


def ease(easing: EasingLike, t: float) -> float:
    """Evaluate ``easing`` (any :data:`EasingLike`) at progress ``t``.

    >>> ease("outQuad", 0.5)
    0.75
    """
    if isinstance(easing, str):
        return _parse(easing)(t)
    if isinstance(easing, Easing):
        return easing(t)
    if easing is None:
        return float(t)
    if isinstance(easing, NativeEasing):
        return easing(t)
    if callable(easing):
        return float(easing(t))
    raise TypeError(f"expected an easing string, Easing or callable, got {type(easing).__name__}")
