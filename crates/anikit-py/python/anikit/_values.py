"""Turning user property values into engine tracks (and engine output back
into Python values).

A property value may be:

* a number, or a string with a unit (``"100px"``, ``"45deg"``, ``"50%"``)
* a relative value: ``"+=10"``, ``"-=10px"``, ``"*=2"``, ``"/=2"``
* a CSS colour (``"#f80"``, ``"red"``, ``"oklch(...)"``) → interpolated in a colour space
* any other string containing numbers (``"0px 0px 10px rgba(0,0,0,.5)"``) →
  every number interpolates through a template
* a list/tuple of keyframes ``[from, to]`` / ``[k0, k1, k2, ...]`` (``None``
  as first keyframe means "the current value"); keyframes may be dicts
  ``{"value": v, "ease": e, "offset": o}`` (``ease`` applies to the segment
  *arriving* at that keyframe)
* ``{"from": a, "to": b, "ease": e}``
* for the ``transform`` property: CSS transform lists (``"rotateY(45deg)
  translateZ(100px)"``) or :class:`~anikit.transform.Matrix` objects (any
  property) → a matrix track interpolated like browsers do (decompose →
  lerp + quaternion slerp → recompose); writes ``matrix3d(...)`` strings (or
  ``Matrix`` objects when the keyframes are ``Matrix`` objects)
* a :class:`~anikit.Path` (motion: writes ``(x, y, angle)``), a
  :class:`~anikit.MotionPath`, or a :class:`~anikit.Morph` /
  :class:`~anikit.MorphSequence` / list of two or more paths (shape morphing:
  writes path data strings; every shape is a keyframe)
* anything else (booleans, objects) → discrete (switches half-way)
* a callable ``fn(target, index, total)`` returning any of the above
"""

from __future__ import annotations

import math
import operator
import re
from typing import Any, Callable, List, Optional, Sequence, Tuple, Union

from . import _anikit
from ._easing import Easing, EasingLike
from ._path import Morph, MorphSequence, MotionPath, Path
from .transform import Matrix

TRACK_NUMBER = _anikit.TRACK_NUMBER
TRACK_COLOR = _anikit.TRACK_COLOR
TRACK_DISCRETE = _anikit.TRACK_DISCRETE
TRACK_PATH = _anikit.TRACK_PATH
TRACK_MATRIX = _anikit.TRACK_MATRIX


class _Missing:
    __slots__ = ()

    def __repr__(self) -> str:
        return "MISSING"


#: Sentinel: the target has no such key/attribute.
MISSING: Any = _Missing()

_NUM = r"[-+]?(?:\d+\.?\d*|\.\d+)(?:[eE][-+]?\d+)?"
_NUM_RE = re.compile(_NUM)
_UNIT_RE = re.compile(rf"^\s*(?:([-+*/])=)?\s*({_NUM})\s*([a-zA-Z%]*)\s*$")
_COLOR_TOKEN_RE = re.compile(
    r"#[0-9a-fA-F]{3,8}\b|\b(?:rgba?|hsla?|hwb|oklab|oklch|lab|lch|color)\([^()]*\)|\b[a-zA-Z]+\b"
)


def fmt(v: float) -> str:
    """Compact number formatting for CSS-like strings (max 4 decimals)."""
    if not math.isfinite(v):
        return "0"
    r = round(v, 4)
    if r == 0:
        return "0"
    if r == int(r) and abs(r) < 1e15:
        return str(int(r))
    return f"{r:.4f}".rstrip("0").rstrip(".")


# ---------------------------------------------------------------------------
# Target access
# ---------------------------------------------------------------------------


def getter_for(target: Any) -> Callable[[Any, str], Any]:
    if isinstance(target, dict):
        return lambda t, k: t.get(k, MISSING)
    return lambda t, k: getattr(t, k, MISSING)


def setter_for(target: Any) -> Callable[[Any, str, Any], None]:
    return operator.setitem if isinstance(target, dict) else setattr


def read(target: Any, key: str) -> Any:
    if isinstance(target, dict):
        return target.get(key, MISSING)
    return getattr(target, key, MISSING)


# ---------------------------------------------------------------------------
# Kinds: how channel values are produced and decoded
# ---------------------------------------------------------------------------


class Kind:
    """Describes one track's encoding. ``decode(vals, off)`` turns the engine's
    output slice back into the Python value written to the target."""

    track = TRACK_NUMBER
    channels = 1
    out_channels = 1

    def decode(self, vals: Sequence[float], off: int) -> Any:  # pragma: no cover - abstract
        raise NotImplementedError


class NumberKind(Kind):
    def decode(self, vals: Sequence[float], off: int) -> float:
        return vals[off]


class UnitKind(Kind):
    __slots__ = ("unit",)

    def __init__(self, unit: str) -> None:
        self.unit = unit

    def decode(self, vals: Sequence[float], off: int) -> str:
        return fmt(vals[off]) + self.unit


def css_color(r: float, g: float, b: float, a: float) -> str:
    """Same format as the engine's ``color_to_css``."""
    if a >= 1.0:
        return f"rgb({round(r)},{round(g)},{round(b)})"
    return f"rgba({round(r)},{round(g)},{round(b)},{fmt(round(a, 3))})"


class ColorKind(Kind):
    __slots__ = ("hex", "space")
    track = TRACK_COLOR
    channels = 4
    out_channels = 4

    def __init__(self, space: int, hex_out: bool) -> None:
        self.space = space
        self.hex = hex_out

    def decode(self, vals: Sequence[float], off: int) -> str:
        c = (vals[off], vals[off + 1], vals[off + 2], vals[off + 3])
        if self.hex:
            return _anikit.color_to_hex(c)
        return css_color(*c)


class RgbaKind(Kind):
    """Colour encoded as plain numeric channels (used by springs)."""

    __slots__ = ("hex",)
    channels = 4
    out_channels = 4

    def __init__(self, hex_out: bool) -> None:
        self.hex = hex_out

    def decode(self, vals: Sequence[float], off: int) -> str:
        r = min(255.0, max(0.0, vals[off]))
        g = min(255.0, max(0.0, vals[off + 1]))
        b = min(255.0, max(0.0, vals[off + 2]))
        a = min(1.0, max(0.0, vals[off + 3]))
        if self.hex:
            return _anikit.color_to_hex((r, g, b, a))
        return css_color(r, g, b, a)


class TemplateKind(Kind):
    __slots__ = ("parts", "channels", "out_channels")

    def __init__(self, parts: List[str]) -> None:
        self.parts = parts
        self.channels = self.out_channels = len(parts) - 1

    def decode(self, vals: Sequence[float], off: int) -> str:
        parts = self.parts
        out = [parts[0]]
        for i in range(len(parts) - 1):
            out.append(fmt(vals[off + i]))
            out.append(parts[i + 1])
        return "".join(out)


class DiscreteKind(Kind):
    __slots__ = ("choices",)
    track = TRACK_DISCRETE

    def __init__(self, choices: List[Any]) -> None:
        self.choices = choices

    def decode(self, vals: Sequence[float], off: int) -> Any:
        i = int(round(vals[off]))
        return self.choices[min(max(i, 0), len(self.choices) - 1)]


class PathKind(Kind):
    """Motion along a path, written as an ``(x, y, angle)`` tuple."""

    __slots__ = ("path",)
    track = TRACK_PATH
    out_channels = 3

    def __init__(self, path: Path) -> None:
        self.path = path

    def decode(self, vals: Sequence[float], off: int) -> Tuple[float, float, float]:
        return (vals[off], vals[off + 1], vals[off + 2])


class MotionKind(PathKind):
    """Motion along a path written to separate keys."""

    __slots__ = ("keys",)

    def __init__(self, path: Path, keys: Tuple[str, ...]) -> None:
        super().__init__(path)
        self.keys = keys


class MorphKind(Kind):
    """Shape morph: every channel is a number; decoded into path data."""

    __slots__ = ("morph", "channels", "out_channels")

    def __init__(self, m: Union[Morph, MorphSequence]) -> None:
        self.morph = m
        self.channels = self.out_channels = m.channels

    def decode(self, vals: Sequence[float], off: int) -> str:
        return self.morph.write(vals[off : off + self.channels])


class MatrixKind(Kind):
    """3D transform: 16 matrix channels written as ``matrix3d(...)`` (or a
    :class:`~anikit.transform.Matrix` when ``as_matrix``)."""

    __slots__ = ("as_matrix",)
    track = TRACK_MATRIX
    channels = 16
    out_channels = 16

    def __init__(self, as_matrix: bool) -> None:
        self.as_matrix = as_matrix

    def decode(self, vals: Sequence[float], off: int) -> Any:
        m = Matrix._raw(tuple(vals[off : off + 16]))
        return m if self.as_matrix else m.to_css()


# ---------------------------------------------------------------------------
# Keyframe resolution
# ---------------------------------------------------------------------------


class Frame:
    __slots__ = ("value", "ease", "offset")

    def __init__(self, value: Any, ease: EasingLike = None, offset: Optional[float] = None) -> None:
        self.value = value
        self.ease = ease
        self.offset = offset


_CURRENT: Any = object()  # keyframe placeholder: read the current value


def resolve_callable(v: Any, target: Any, index: int, total: int) -> Any:
    if callable(v) and not isinstance(v, (Easing, Path, Morph, MorphSequence, MotionPath, Matrix, type)):
        return v(target, index, total)
    return v


def _frame(item: Any, target: Any, index: int, total: int) -> Frame:
    if isinstance(item, dict):
        if "value" not in item:
            raise ValueError(f"keyframe dicts need a 'value' key, got {sorted(item)}")
        value = resolve_callable(item["value"], target, index, total)
        off = item.get("offset")
        return Frame(_CURRENT if value is None else value, item.get("ease"), None if off is None else float(off))
    item = resolve_callable(item, target, index, total)
    return Frame(_CURRENT if item is None else item)


def frames_of(raw: Any, target: Any, index: int, total: int) -> List[Frame]:
    """Normalise a property value into >= 2 keyframes (first may be ``_CURRENT``)."""
    if isinstance(raw, dict):
        if "to" in raw or "from" in raw:
            if "to" not in raw:
                raise ValueError("{'from': ...} needs a 'to' value")
            a = resolve_callable(raw.get("from"), target, index, total)
            b = resolve_callable(raw["to"], target, index, total)
            return [Frame(_CURRENT if a is None else a), Frame(b, raw.get("ease"))]
        return [Frame(_CURRENT), _frame(raw, target, index, total)]
    if isinstance(raw, (list, tuple)):
        if not raw:
            raise ValueError("empty keyframe list")
        frames = [_frame(x, target, index, total) for x in raw]
        if len(frames) == 1:
            frames.insert(0, Frame(_CURRENT))
        for f in frames[1:]:
            if f.value is _CURRENT:
                raise ValueError("only the first keyframe may be None (current value)")
        return frames
    return [Frame(_CURRENT), Frame(raw)]


def fill_offsets(frames: List[Frame]) -> List[float]:
    """CSS-style offsets: missing ones are spread evenly between known ones."""
    n = len(frames)
    if all(f.offset is None for f in frames):
        return []
    offs: List[Optional[float]] = [f.offset for f in frames]
    if offs[0] is None:
        offs[0] = 0.0
    if offs[-1] is None:
        offs[-1] = 1.0
    i = 1
    while i < n:
        if offs[i] is None:
            j = i
            while offs[j] is None:
                j += 1
            a, b = offs[i - 1], offs[j]
            assert a is not None and b is not None
            for k in range(i, j):
                offs[k] = a + (b - a) * (k - i + 1) / (j - i + 1)
            i = j
        i += 1
    for o in offs:
        assert o is not None
        if not 0.0 <= o <= 1.0:
            raise ValueError(f"keyframe offsets must be within [0, 1], got {o}")
    return [float(o) for o in offs]  # type: ignore[arg-type]


def _is_number(v: Any) -> bool:
    return isinstance(v, (int, float)) and not isinstance(v, bool)


def _unit_match(v: Any) -> Optional["re.Match[str]"]:
    return _UNIT_RE.match(v) if isinstance(v, str) else None


def _apply_rel(op: Optional[str], base: float, x: float) -> float:
    if op is None:
        return x
    if op == "+":
        return base + x
    if op == "-":
        return base - x
    if op == "*":
        return base * x
    if x == 0:
        raise ZeroDivisionError("relative '/=0'")
    return base / x


def _normalize_colors(s: str) -> str:
    def repl(m: "re.Match[str]") -> str:
        c = _anikit.color_try_parse(m.group(0))
        if c is None:
            return m.group(0)
        return f"rgba({fmt(c[0])},{fmt(c[1])},{fmt(c[2])},{fmt(c[3])})"

    return _COLOR_TOKEN_RE.sub(repl, s)


class Resolved:
    """Keyframe values resolved into one kind + numeric channel rows."""

    __slots__ = ("kind", "rows", "final")

    def __init__(self, kind: Kind, rows: List[List[float]], final: Any) -> None:
        self.kind = kind
        self.rows = rows
        #: Python value of the last keyframe (used to chain timeline tweens).
        self.final = final


def resolve_values(values: List[Any], current: Any, color_space: int, numeric_colors: bool = False) -> Resolved:
    """Resolve keyframe values (first may be ``_CURRENT``) into channel rows.

    ``current`` is the target's current value (or :data:`MISSING`). Relative
    values build on the previous keyframe. ``numeric_colors`` encodes colours as
    plain rgba channels instead of a colour track (springs).
    """
    vals = list(values)
    if vals and vals[0] is _CURRENT:
        vals[0] = current
    explicit = [v for v in vals if v is not MISSING]

    # 1) numbers / unit strings / relative values
    if explicit and all(_is_number(v) or _unit_match(v) is not None for v in explicit):
        unit = ""
        as_str = False
        for v in reversed(vals):
            m = _unit_match(v)
            if m is not None:
                # Absolute strings (or relative ones with a unit) make the output a
                # string; "+=5" on a number keeps it a number.
                if m.group(1) is None or m.group(3):
                    as_str = True
                if m.group(3) and not unit:
                    unit = m.group(3)
        rows: List[List[float]] = []
        prev = 0.0
        for v in vals:
            if v is MISSING:
                x = 0.0
            elif _is_number(v):
                x = float(v)
            else:
                m = _unit_match(v)
                assert m is not None
                x = _apply_rel(m.group(1), prev, float(m.group(2)))
            prev = x
            rows.append([x])
        kind: Kind = UnitKind(unit) if as_str else NumberKind()
        final = kind.decode(rows[-1], 0)
        return Resolved(kind, rows, final)

    # 2) colours
    if explicit and all(isinstance(v, str) for v in explicit):
        parsed = [None if v is MISSING else _anikit.color_try_parse(v) for v in vals]
        if all(p is not None or v is MISSING for p, v in zip(parsed, vals)):
            first = next(p for p in parsed if p is not None)
            rows = []
            for p in parsed:
                rows.append(list(p) if p is not None else [first[0], first[1], first[2], 0.0])
            last_str = next(v for v in reversed(vals) if v is not MISSING)
            hex_out = last_str.strip().startswith("#")
            ckind: Kind = RgbaKind(hex_out) if numeric_colors else ColorKind(color_space, hex_out)
            return Resolved(ckind, rows, ckind.decode(rows[-1], 0))

        # 3) templates: every number interpolates
        norm = [None if v is MISSING else _normalize_colors(v) for v in vals]
        splits = [None if s is None else (_NUM_RE.split(s), [float(x) for x in _NUM_RE.findall(s)]) for s in norm]
        counts = {len(sp[1]) for sp in splits if sp is not None}
        if len(counts) == 1 and counts.pop() > 0:
            ref = next(sp for sp in reversed(splits) if sp is not None)
            n = len(ref[1])
            rows = [list(sp[1]) if sp is not None else [0.0] * n for sp in splits]
            tkind = TemplateKind(ref[0])
            return Resolved(tkind, rows, tkind.decode(rows[-1], 0))

    # 4) anything else: discrete
    if vals[0] is MISSING:
        vals[0] = vals[1] if len(vals) > 1 else None
    dkind = DiscreteKind(vals)
    return Resolved(dkind, [[float(i)] for i in range(len(vals))], vals[-1])


# ---------------------------------------------------------------------------
# Track plans
# ---------------------------------------------------------------------------


class TrackPlan:
    """Everything needed to create one engine track for one property."""

    __slots__ = ("key", "kind", "rows", "offsets", "eases", "final")

    def __init__(
        self,
        key: str,
        kind: Kind,
        rows: List[List[float]],
        offsets: List[float],
        eases: List[EasingLike],
        final: Any,
    ) -> None:
        self.key = key
        self.kind = kind
        self.rows = rows
        self.offsets = offsets
        #: One easing per segment (``None`` → use the animation default).
        self.eases = eases
        self.final = final

    @property
    def keys(self) -> Tuple[str, ...]:
        if isinstance(self.kind, MotionKind):
            return self.kind.keys
        return (self.key,)

    def remap(self, a: float, b: float) -> None:
        """Squeeze the keyframes into ``[a, b]`` of the iteration and hold the
        first/last values outside it (used to implement repeat delays)."""
        n = len(self.rows)
        offs = self.offsets or [i / (n - 1) for i in range(n)]
        offsets = [a + o * (b - a) for o in offs]
        rows = list(self.rows)
        eases = list(self.eases)
        if a > 0.0:
            offsets.insert(0, 0.0)
            rows.insert(0, rows[0])
            eases.insert(0, "linear")
        if b < 1.0:
            offsets.append(1.0)
            rows.append(rows[-1])
            eases.append("linear")
        self.offsets, self.rows, self.eases = offsets, rows, eases

    def final_value(self) -> Any:
        """The value this track ends on, as written to the target."""
        k = self.kind
        if isinstance(k, MotionKind):
            x, y, a = k.path.point_at(self.rows[-1][0])
            return (x, y, a)[: len(k.keys)]
        if isinstance(k, PathKind):
            return k.path.point_at(self.rows[-1][0])
        return k.decode(self.rows[-1], 0)


def _matrix_of(v: Any) -> Optional[Matrix]:
    if isinstance(v, Matrix):
        return v
    if isinstance(v, str):
        vals = _anikit.transform_parse(v)
        return None if vals is None else Matrix._raw(vals)
    return None


def _plan_matrix(key: str, frames: List[Frame], current: Any) -> Optional[TrackPlan]:
    """A matrix track for ``transform`` strings / ``Matrix`` keyframes, else ``None``."""
    explicit = [f.value for f in frames if f.value is not _CURRENT]
    has_matrix = any(isinstance(v, Matrix) for v in explicit)
    if not has_matrix and (key != "transform" or not all(isinstance(v, str) for v in explicit)):
        return None
    parsed = [_matrix_of(v) for v in explicit]
    for v, pm in zip(explicit, parsed):
        if pm is None:
            if not has_matrix:
                return None  # not a transform list: keep the generic (template) interpolation
            raise ValueError(f"cannot animate {key!r} to {v!r}: not a CSS transform or Matrix")
    it = iter(parsed)
    mats: List[Matrix] = []
    for f in frames:
        if f.value is not _CURRENT:
            nxt = next(it)
            assert nxt is not None
            mats.append(nxt)
        elif current is MISSING or current is None or (isinstance(current, str) and not current.strip()):
            mats.append(Matrix())
        else:
            cm = _matrix_of(current)
            if cm is None:
                raise ValueError(f"cannot animate {key!r} from its current value {current!r}: not a CSS transform")
            mats.append(cm)
    kind = MatrixKind(isinstance(explicit[-1], Matrix))
    rows = [list(mt.values) for mt in mats]
    return TrackPlan(key, kind, rows, fill_offsets(frames), [f.ease for f in frames[1:]], kind.decode(rows[-1], 0))


def plan_track(
    key: str,
    raw: Any,
    target: Any,
    index: int,
    total: int,
    current: Any,
    color_space: int,
) -> TrackPlan:
    """Plan the track animating ``target[key]`` according to ``raw``."""
    raw = resolve_callable(raw, target, index, total)
    if isinstance(raw, MotionPath):
        kind: Kind = MotionKind(raw.path, raw.keys)
        return TrackPlan(key, kind, [[raw.start], [raw.end]], [], [None], None)
    if isinstance(raw, Path):
        return TrackPlan(key, PathKind(raw), [[0.0], [1.0]], [], [None], None)
    if isinstance(raw, Morph):
        mk = MorphKind(raw)
        return TrackPlan(key, mk, [raw.from_values, raw.to_values], [], [None], mk.decode(raw.to_values, 0))
    if isinstance(raw, MorphSequence):
        # One numeric track whose keyframes are the normalised shapes.
        mk = MorphKind(raw)
        rows = raw.keyframes
        return TrackPlan(key, mk, rows, [], [None] * (len(rows) - 1), mk.decode(rows[-1], 0))

    frames = frames_of(raw, target, index, total)
    mplan = _plan_matrix(key, frames, current)
    if mplan is not None:
        return mplan
    if any(isinstance(f.value, Path) for f in frames):
        shapes = [f.value for f in frames]
        if shapes[0] is _CURRENT:
            shapes[0] = current
        if shapes[0] is MISSING or not isinstance(shapes[0], (str, Path)):
            raise ValueError(f"cannot morph {key!r}: no start shape")
        for s in shapes[1:]:
            if not isinstance(s, (str, Path)):
                raise TypeError(f"cannot morph {key!r} to {s!r}: every keyframe must be a Path or path data")
        eases = [f.ease for f in frames[1:]]
        if len(shapes) == 2:
            m = Morph(shapes[0], shapes[1])
            mk = MorphKind(m)
            return TrackPlan(key, mk, [m.from_values, m.to_values], [], eases, mk.decode(m.to_values, 0))
        seq = MorphSequence(shapes)
        mk = MorphKind(seq)
        rows = seq.keyframes
        return TrackPlan(key, mk, rows, fill_offsets(frames), eases, mk.decode(rows[-1], 0))

    res = resolve_values([f.value for f in frames], current, color_space)
    offsets = fill_offsets(frames)
    eases = [f.ease for f in frames[1:]]
    return TrackPlan(key, res.kind, res.rows, offsets, eases, res.final)
