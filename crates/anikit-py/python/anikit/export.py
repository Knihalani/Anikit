"""Export animations to native web formats: CSS ``@keyframes`` and the Web
Animations API (``element.animate(keyframes, timing)``).

When every property shares the same keyframe offsets and per-segment easings,
keyframes are emitted exactly at those offsets with the easing exported as a
native CSS timing function per segment (springs/bounces become ``linear()``).
Otherwise (or when ``samples`` is given) values are densely sampled and
interpolated linearly.

Property names are converted to CSS (``background_color`` / ``backgroundColor``
→ ``background-color``). Transform shorthands are composed into one
``transform``: ``x``, ``y``, ``z`` (px), ``rotate``/``rotation``,
``rotate_x``/``rotate_y`` (deg), ``scale``, ``scale_x``, ``scale_y``,
``skew_x``, ``skew_y`` (deg); an animated ``transform`` (CSS transform lists,
written as ``matrix3d(...)``) is appended after them. Unitless numbers get ``px`` except for known
unitless properties (opacity, z-index, ...).
"""

from __future__ import annotations

import math
import re
from typing import TYPE_CHECKING, Any, Dict, List, Optional, Sequence, Tuple, Union

from ._easing import Easing
from ._values import fmt

if TYPE_CHECKING:  # pragma: no cover
    from ._animation import Animation, _AnimRecord

__all__ = ["css_keyframes", "waapi", "keyframe_frames", "css_property_name"]

_DEFAULT_SAMPLES = 50

_TRANSFORM_KEYS: Dict[str, Tuple[str, str]] = {
    # key -> (css function, default unit)
    "x": ("translateX", "px"),
    "y": ("translateY", "px"),
    "z": ("translateZ", "px"),
    "translate_x": ("translateX", "px"),
    "translate_y": ("translateY", "px"),
    "translate_z": ("translateZ", "px"),
    "rotate": ("rotate", "deg"),
    "rotation": ("rotate", "deg"),
    "rotate_z": ("rotateZ", "deg"),
    "rotate_x": ("rotateX", "deg"),
    "rotate_y": ("rotateY", "deg"),
    "skew_x": ("skewX", "deg"),
    "skew_y": ("skewY", "deg"),
    "scale": ("scale", ""),
    "scale_x": ("scaleX", ""),
    "scale_y": ("scaleY", ""),
}
_TRANSFORM_ORDER = [
    "translateX",
    "translateY",
    "translateZ",
    "rotate",
    "rotateZ",
    "rotateX",
    "rotateY",
    "skewX",
    "skewY",
    "scale",
    "scaleX",
    "scaleY",
]
_UNITLESS = {
    "opacity",
    "z-index",
    "font-weight",
    "line-height",
    "flex",
    "flex-grow",
    "flex-shrink",
    "order",
    "zoom",
    "fill-opacity",
    "stroke-opacity",
    "stroke-miterlimit",
    "scale",
    "orphans",
    "widows",
    "column-count",
    "animation-iteration-count",
}


def css_property_name(key: str) -> str:
    """``background_color`` / ``backgroundColor`` → ``background-color``."""
    s = re.sub(r"(?<=[a-z0-9])([A-Z])", r"-\1", key).replace("_", "-").lower()
    return s


def _camel(name: str) -> str:
    parts = name.split("-")
    return parts[0] + "".join(p[:1].upper() + p[1:] for p in parts[1:])


def _transform_key(key: str) -> Optional[Tuple[str, str]]:
    k = re.sub(r"(?<=[a-z0-9])([A-Z])", r"_\1", key).lower()
    return _TRANSFORM_KEYS.get(k)


def _num_css(v: Any, unit: str) -> str:
    if isinstance(v, (int, float)) and not isinstance(v, bool):
        return fmt(float(v)) + unit
    return str(v)


def _css_props(values: Sequence[Tuple[str, Any]]) -> Dict[str, Any]:
    """Turn ``(key, python value)`` pairs into ordered CSS declarations."""
    out: Dict[str, Any] = {}
    transforms: Dict[str, str] = {}
    for key, v in values:
        tk = _transform_key(key)
        if tk is not None:
            transforms[tk[0]] = _num_css(v, tk[1])
            continue
        if isinstance(v, tuple) and len(v) == 3 and all(isinstance(x, float) for x in v):
            # Motion along a path written as one (x, y, angle) value.
            transforms["translateX"] = fmt(v[0]) + "px"
            transforms["translateY"] = fmt(v[1]) + "px"
            transforms["rotate"] = fmt(v[2]) + "deg"
            continue
        name = css_property_name(key)
        if isinstance(v, bool) or v is None:
            out[name] = "" if v is None else str(v).lower()
        elif isinstance(v, (int, float)):
            out[name] = float(v) if name in _UNITLESS else fmt(float(v)) + "px"
        else:
            out[name] = str(v)
    if transforms:
        composed = " ".join(f"{fn}({transforms[fn]})" for fn in _TRANSFORM_ORDER if fn in transforms)
        # An explicit `transform` (e.g. a matrix3d() track) applies after the shorthands.
        base = out.get("transform")
        out["transform"] = composed if not base or base == "none" else f"{composed} {base}"
    return out


def _record(anim: "Animation") -> "_AnimRecord":
    from ._animation import Animation

    if not isinstance(anim, Animation):
        raise TypeError("only Animation objects (not timelines or springs) can be exported")
    anim._root()
    return anim._tween.records[0]


def keyframe_frames(
    anim: "Animation", samples: Optional[int] = None
) -> Tuple[List[float], List[Dict[str, Any]], Optional[List[str]]]:
    """Compute ``(offsets, css_declarations_per_offset, easing_per_segment)``.

    ``easing_per_segment`` is ``None`` for densely sampled (linear) output.
    """
    rec = _record(anim)
    core = anim.engine.core
    plans = rec.plans
    native = samples is None
    if native:
        n0 = len(plans[0].rows)
        offs0 = plans[0].offsets or [i / (n0 - 1) for i in range(n0)]
        for plan, ids in zip(plans, rec.ease_ids):
            n = len(plan.rows)
            offs = plan.offsets or [i / (n - 1) for i in range(n)]
            if len(offs) != len(offs0) or any(abs(a - b) > 1e-12 for a, b in zip(offs, offs0)):
                native = False
                break
            if ids != rec.ease_ids[0]:
                native = False
                break
    if native:
        offsets = list(offs0)
        easings: Optional[List[str]] = [_easing_css(e) for e in rec.ease_specs[0]]
    else:
        count = max(2, int(samples if samples is not None else _DEFAULT_SAMPLES))
        pts = {i / count for i in range(count + 1)}
        for plan in plans:
            n = len(plan.rows)
            pts.update(plan.offsets or [i / (n - 1) for i in range(n)])
        offsets = sorted(pts)
        easings = None
    rows = core.sample_many(rec.handle, offsets)
    decls = []
    for vals in rows:
        pairs: List[Tuple[str, Any]] = []
        for e in rec.binder.entries:
            v = e.decode(vals, e.off)
            if e.multi:
                pairs.extend(zip(e.keys, v))
            else:
                pairs.append((e.keys[0], v))
        decls.append(_css_props(pairs))
    return offsets, decls, easings


def _easing_css(spec: Any) -> str:
    if spec is None or spec == "linear":
        return "linear"
    return Easing(spec).css()


def _timing(anim: "Animation") -> Dict[str, Any]:
    rec = _record(anim)
    core = anim.engine.core
    duration, delay, iterations, direction, end_delay, _ = core.timing(rec.handle)
    speed = abs(core.rate(rec.handle)) * (abs(core.rate(anim.handle)) if anim.handle != rec.handle else 1.0)
    start_delay = 0.0
    if anim.handle != rec.handle:
        start_delay = core.start(rec.handle)
    return {
        "duration": duration / speed,
        "delay": (delay + start_delay) / speed,
        "end_delay": end_delay / speed,
        "iterations": iterations,
        "direction": ("normal", "reverse", "alternate", "alternate-reverse")[direction],
    }


def _pct(o: float) -> str:
    return fmt(o * 100.0) + "%"


def css_keyframes(anim: "Animation", name: str, selector: Optional[str] = None, samples: Optional[int] = None) -> str:
    """CSS ``@keyframes name { ... }`` for the animation's (first) target, plus
    ``selector { animation: ... }`` when ``selector`` is given."""
    if not re.match(r"^-?[A-Za-z_][\w-]*$", name):
        raise ValueError(f"invalid CSS animation name: {name!r}")
    offsets, decls, easings = keyframe_frames(anim, samples)
    lines = [f"@keyframes {name} {{"]
    last = len(offsets) - 1
    for i, (o, d) in enumerate(zip(offsets, decls)):
        body = [f"{k}: {fmt(v) if isinstance(v, float) else v};" for k, v in d.items()]
        if easings is not None and i < last:
            body.append(f"animation-timing-function: {easings[i]};")
        lines.append(f"  {_pct(o)} {{ {' '.join(body)} }}")
    lines.append("}")
    if selector:
        t = _timing(anim)
        iters = "infinite" if math.isinf(t["iterations"]) else fmt(t["iterations"])
        timing_fn = "linear"
        lines.append(f"{selector} {{")
        lines.append(
            f"  animation: {name} {fmt(t['duration'] / 1000.0)}s {timing_fn} {fmt(t['delay'] / 1000.0)}s "
            f"{iters} {t['direction']} both;"
        )
        lines.append("}")
    return "\n".join(lines) + "\n"


def waapi(anim: "Animation", samples: Optional[int] = None) -> Tuple[List[Dict[str, Any]], Dict[str, Any]]:
    """``(keyframes, timing)`` for ``element.animate(keyframes, timing)``.

    Keyframes carry ``offset``, camelCase properties and (for exact
    keyframes) a per-segment ``easing``; timing values are milliseconds and
    ``iterations`` may be ``math.inf`` (serialise as ``Infinity``).
    """
    offsets, decls, easings = keyframe_frames(anim, samples)
    frames: List[Dict[str, Any]] = []
    for i, (o, d) in enumerate(zip(offsets, decls)):
        kf: Dict[str, Any] = {"offset": o}
        for k, v in d.items():
            kf[_camel(k)] = v
        if easings is not None and i < len(offsets) - 1:
            kf["easing"] = easings[i]
        frames.append(kf)
    t = _timing(anim)
    timing: Dict[str, Union[float, str]] = {
        "duration": t["duration"],
        "delay": t["delay"],
        "endDelay": t["end_delay"],
        "iterations": t["iterations"],
        "direction": t["direction"],
        "fill": "both",
        "easing": "linear",
    }
    return frames, timing
