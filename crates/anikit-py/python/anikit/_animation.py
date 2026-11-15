"""Tweens, keyframe animations and timelines over plain Python targets."""

from __future__ import annotations

import math
import re
import weakref
from typing import Any, Callable, Dict, List, Mapping, Optional, Tuple, Union

from . import _anikit
from ._easing import EasingLike
from ._engine import Binder, Callback, Engine, Entry, Hooks, Node, Tree, as_targets, release_tree, resolve_engine
from ._values import MISSING, ColorKind, MotionKind, PathKind, TrackPlan, plan_track, read

__all__ = ["Animation", "Timeline", "animate", "timeline", "Position"]

Position = Union[float, int, str, None]

_STATES = {
    _anikit.STATE_IDLE: "idle",
    _anikit.STATE_PLAYING: "playing",
    _anikit.STATE_PAUSED: "paused",
    _anikit.STATE_FINISHED: "finished",
}
_FLAGS = _anikit.FLAG_DEFAULT
_IMMEDIATE = _anikit.FLAG_IMMEDIATE


def _per_target(value: Any, target: Any, index: int, total: int) -> Any:
    """Options like ``delay``/``duration`` may be per-target lists or ``fn(target, index, total)``."""
    if isinstance(value, (list, tuple)):
        if not value:
            raise ValueError("empty per-target option list")
        return value[index % len(value)]
    if callable(value):
        return value(target, index, total)
    return value


def _seconds(name: str, v: Any, allow_negative: bool = False) -> float:
    try:
        x = float(v)
    except (TypeError, ValueError):
        raise TypeError(f"{name} must be a number of seconds, got {v!r}") from None
    if math.isnan(x) or (x < 0 and not allow_negative):
        raise ValueError(f"{name} must be >= 0, got {v!r}")
    if 100 <= x < math.inf:
        import warnings

        warnings.warn(
            f"anikit: {name} is {x:g} seconds. Times in anikit are seconds; did you mean {name}={x / 1000:g}?",
            stacklevel=4,
        )
    return x


class _Timing:
    """Engine timing (ms) for one iteration scheme, with repeat delays folded in.

    The core timing model has no gap between iterations, so a repeat delay
    is implemented by lengthening each iteration and holding keyframe values
    for the gap: after the motion for plain repeats (``lead = 0``,
    ``trail = gap``), half before and half after it for yoyo. The start is
    shifted by ``lead`` and the end trimmed by ``trail`` so the first motion
    begins immediately and the animation ends when the last motion does.
    """

    __slots__ = ("duration", "delay", "iterations", "direction", "end_delay", "remap", "lead", "trail")

    def __init__(self, duration: float, delay: float, repeat: float, yoyo: bool, repeat_delay: float) -> None:
        if repeat is None:
            repeat = 0
        repeat = float(repeat)
        iterations = math.inf if (repeat < 0 or math.isinf(repeat)) else repeat + 1.0
        d = duration * 1000.0
        self.delay = delay * 1000.0
        self.iterations = iterations
        self.direction = 2 if yoyo else 0
        self.end_delay = 0.0
        self.remap: Optional[Tuple[float, float]] = None
        self.lead = 0.0
        self.trail = 0.0
        rd = repeat_delay * 1000.0
        if rd > 0 and iterations > 1:
            total = d + rd
            self.lead = rd / 2.0 if yoyo else 0.0
            self.trail = rd - self.lead
            self.remap = (self.lead / total, (self.lead + d) / total)
            self.delay -= self.lead
            self.end_delay = -self.trail
            d = total
        self.duration = d


class _AnimRecord:
    """One native keyframe animation driving one target."""

    __slots__ = ("handle", "target", "plans", "binder", "ease_ids", "ease_specs", "timing")

    def __init__(
        self,
        handle: int,
        target: Any,
        plans: List[TrackPlan],
        binder: Binder,
        ease_ids: List[List[int]],
        ease_specs: List[List[EasingLike]],
        timing: _Timing,
    ) -> None:
        self.handle = handle
        self.target = target
        self.plans = plans
        self.binder = binder
        #: Per track, per segment: registered easing ids and the easings they came from.
        self.ease_ids = ease_ids
        self.ease_specs = ease_specs
        self.timing = timing


class _Tween:
    __slots__ = ("handle", "records")

    def __init__(self, handle: int, records: List[_AnimRecord]) -> None:
        self.handle = handle
        self.records = records


_OPTION_DEFAULTS: Dict[str, Any] = {
    "duration": 0.5,
    "delay": 0.0,
    "ease": "outQuad",
    "repeat": 0,
    "yoyo": False,
    "repeat_delay": 0.0,
    "color_space": "oklab",
    "immediate_render": False,
}


def _build_tween(
    engine: Engine,
    targets: Any,
    props: Mapping[str, Any],
    opts: Mapping[str, Any],
    tree: Tree,
    hooks: Hooks,
    planned: Optional[Dict[Tuple[int, str], Any]],
) -> _Tween:
    if not isinstance(props, Mapping) or not props:
        raise ValueError("props must be a non-empty mapping of property -> value")
    tl = as_targets(targets)
    n = len(tl)
    core = engine.core
    cs = _anikit.color_space_id(opts["color_space"])
    default_ease = opts["ease"]
    repeat = opts["repeat"]
    yoyo = bool(opts["yoyo"])
    flags = _FLAGS | (_IMMEDIATE if opts["immediate_render"] else 0)
    records: List[_AnimRecord] = []
    group_id = tree
    try:
        for i, t in enumerate(tl):
            dur = _seconds("duration", _per_target(opts["duration"], t, i, n))
            delay = _seconds("delay", _per_target(opts["delay"], t, i, n), allow_negative=True)
            rd = _seconds("repeat_delay", _per_target(opts["repeat_delay"], t, i, n))
            timing = _Timing(dur, delay, repeat, yoyo, rd)
            plans: List[TrackPlan] = []
            for key, raw in props.items():
                if not isinstance(key, str):
                    raise TypeError(f"property names must be strings, got {key!r}")
                cur = MISSING
                if planned is not None:
                    cur = planned.get((id(t), key), MISSING)
                if cur is MISSING:
                    cur = read(t, key)
                plan = plan_track(key, raw, t, i, n, cur, cs)
                if timing.remap is not None:
                    plan.remap(*timing.remap)
                plans.append(plan)
            tracks = []
            entries: List[Entry] = []
            ease_ids: List[List[int]] = []
            ease_specs: List[List[EasingLike]] = []
            off = 0
            for ti, plan in enumerate(plans):
                specs = [default_ease if e is None else e for e in plan.eases]
                ids = [engine.easing_id(e) for e in specs]
                ease_ids.append(ids)
                ease_specs.append(specs)
                k = plan.kind
                flat = [x for row in plan.rows for x in row]
                compact = ids[:1] if len(set(ids)) == 1 else ids
                tracks.append(
                    (
                        k.track,
                        k.channels,
                        flat,
                        plan.offsets,
                        compact,
                        k.space if isinstance(k, ColorKind) else 0,
                        k.path.native if isinstance(k, PathKind) else None,
                    )
                )
                entries.append(Entry(plan.keys, k, off, ti, group_id))
                off += k.out_channels
            h = core.create_anim(
                tracks,
                duration=timing.duration,
                delay=timing.delay,
                iterations=timing.iterations,
                direction=timing.direction,
                end_delay=timing.end_delay,
                flags=flags,
            )
            binder = Binder(t, entries)
            engine._register(h, tree, Node(binder=binder, hooks=hooks))
            records.append(_AnimRecord(h, t, plans, binder, ease_ids, ease_specs, timing))
            if planned is not None:
                for plan in plans:
                    if isinstance(plan.kind, MotionKind):
                        for k2, v in zip(plan.kind.keys, plan.final_value()):
                            planned[(id(t), k2)] = v
                    elif plan.final is not None:
                        planned[(id(t), plan.key)] = plan.final
        if n == 1:
            handle = records[0].handle
        else:
            handle = core.create_group(auto_duration=True)
            engine._register(handle, tree, Node(hooks=hooks))
            for r in records:
                core.group_add(handle, r.handle, 0.0)
    except BaseException:
        for r in records:
            try:
                core.release(r.handle)
            except KeyError:
                pass
            engine._forget(r.handle)
            if r.handle in tree.nodes:
                tree.nodes.remove(r.handle)
        raise
    hooks.handle = handle
    return _Tween(handle, records)


def _snap_to_end(core: Any, records: List[_AnimRecord]) -> Callable[[], None]:
    """With repeat delays the engine finishes inside the (held) last iteration,
    where float rounding can leave values a hair off; write the exact end."""

    def snap() -> None:
        for r in records:
            tm = r.timing
            if tm.remap is None or math.isinf(tm.iterations):
                continue
            last = int(math.ceil(tm.iterations)) - 1
            p = tm.remap[0] if (tm.direction == 2 and last % 2 == 1) else tm.remap[1]
            try:
                vals = core.sample(r.handle, p)
            except KeyError:
                continue
            r.binder.apply(vals)

    return snap


class _Controller:
    """Playback control shared by :class:`Animation` and :class:`Timeline`."""

    def __init__(self, engine: Engine, hooks: Hooks) -> None:
        self._engine = engine
        self._hooks = hooks
        self._tree = Tree()
        self._h = -1
        self._nested = False
        hooks.ctrl = weakref.ref(self)
        self._finalizer = weakref.finalize(self, release_tree, weakref.ref(engine), self._tree)

    # -- internals -------------------------------------------------------

    def _root(self) -> int:
        if not self._finalizer.alive:
            raise RuntimeError(f"{type(self).__name__} was killed")
        if self._nested:
            raise RuntimeError(f"this {type(self).__name__} is nested in a timeline; control the timeline instead")
        return self._h

    def _rewind_silently(self, h: int) -> None:
        """Move back to the start without firing events, so the next forward
        render reports a fresh START (the core only emits START when entering
        the active phase from before it)."""
        core = self._engine.core
        if core.time(h) > 0.0:
            core.seek(h, 0.0)
            self._engine.flush(events=False)

    # -- playback -----------------------------------------------------------

    def play(self) -> "_Controller":
        """Play forwards (from the start if finished)."""
        h = self._root()
        core = self._engine.core
        rate = core.rate(h)
        if rate < 0:
            core.set_rate(h, -rate)
        if core.state(h) == _anikit.STATE_FINISHED and core.time(h) >= core.duration(h):
            self._rewind_silently(h)
        core.play(h)
        self._engine._alive.add(self)
        return self

    def resume(self) -> "_Controller":
        """Continue playing in the current direction."""
        h = self._root()
        self._engine.core.play(h)
        self._engine._alive.add(self)
        return self

    def pause(self) -> "_Controller":
        h = self._root()
        self._engine.core.pause(h)
        self._engine._alive.discard(self)
        return self

    def reverse(self) -> "_Controller":
        """Flip the playback direction and play (from the end if at the start)."""
        h = self._root()
        core = self._engine.core
        core.reverse(h)
        core.play(h)
        self._engine._alive.add(self)
        return self

    def restart(self) -> "_Controller":
        """Play forwards from the beginning."""
        h = self._root()
        core = self._engine.core
        rate = core.rate(h)
        if rate < 0:
            core.set_rate(h, -rate)
        self._rewind_silently(h)
        core.restart(h)
        self._engine.flush()
        self._engine._alive.add(self)
        return self

    def seek(self, time: float) -> "_Controller":
        """Jump to ``time`` seconds (delay included) and render immediately."""
        h = self._root()
        self._engine.core.seek(h, float(time) * 1000.0)
        self._engine.flush()
        return self

    def complete(self) -> "_Controller":
        """Jump to the end (fires completion callbacks)."""
        h = self._root()
        end = self._engine.core.duration(h)
        if math.isinf(end):
            raise ValueError("cannot complete an infinitely repeating animation")
        return self.seek(end / 1000.0)

    def kill(self) -> None:
        """Stop and free all engine resources (targets keep their current values)."""
        self._engine._alive.discard(self)
        self._finalizer()

    # -- state ---------------------------------------------------------------

    @property
    def progress(self) -> float:
        """Overall progress ``0..1`` (settable)."""
        return self._engine.core.progress(self._root())

    @progress.setter
    def progress(self, p: float) -> None:
        h = self._root()
        self._engine.core.set_progress(h, float(p))
        self._engine.flush()

    @property
    def time(self) -> float:
        """Local time in seconds, delay included (settable)."""
        return self._engine.core.time(self._root()) / 1000.0

    @time.setter
    def time(self, t: float) -> None:
        self.seek(t)

    @property
    def duration(self) -> float:
        """Total duration in seconds (delay + all repeats); ``inf`` if infinite."""
        return self._engine.core.duration(self._root()) / 1000.0

    @property
    def state(self) -> str:
        """``"idle"``, ``"playing"``, ``"paused"`` or ``"finished"``."""
        return _STATES[self._engine.core.state(self._root())]

    @property
    def finished(self) -> bool:
        return self._engine.core.state(self._root()) == _anikit.STATE_FINISHED

    @property
    def is_active(self) -> bool:
        """Currently playing."""
        return self._engine.core.state(self._root()) == _anikit.STATE_PLAYING

    @property
    def alive(self) -> bool:
        """False once :meth:`kill` was called."""
        return self._finalizer.alive

    @property
    def speed(self) -> float:
        """Playback rate (2 = twice as fast); direction is kept separately."""
        return abs(self._engine.core.rate(self._root()))

    @speed.setter
    def speed(self, value: float) -> None:
        v = float(value)
        if not v > 0:
            raise ValueError("speed must be > 0")
        h = self._root()
        r = self._engine.core.rate(h)
        self._engine.core.set_rate(h, -v if r < 0 else v)

    @property
    def reversed(self) -> bool:
        return self._engine.core.rate(self._root()) < 0

    @property
    def engine(self) -> Engine:
        return self._engine

    @property
    def handle(self) -> int:
        """Native root handle (for low-level use)."""
        return self._h

    def __repr__(self) -> str:
        if not self.alive:
            return f"<{type(self).__name__} killed>"
        if self._nested:
            return f"<{type(self).__name__} nested>"
        return f"<{type(self).__name__} {self.state} t={self.time:.3f}/{self.duration:.3f}s>"


def _make_hooks(opts: Dict[str, Any], parent: Optional[Hooks] = None) -> Hooks:
    return Hooks(
        on_start=opts.pop("on_start", None),
        on_update=opts.pop("on_update", None),
        on_complete=opts.pop("on_complete", None),
        on_repeat=opts.pop("on_repeat", None),
        on_reverse_complete=opts.pop("on_reverse_complete", None),
        parent=parent,
    )


def _merge_options(base: Mapping[str, Any], opts: Dict[str, Any]) -> Dict[str, Any]:
    out = dict(_OPTION_DEFAULTS)
    out.update(base)
    for k, v in opts.items():
        if k not in _OPTION_DEFAULTS:
            import difflib

            near = difflib.get_close_matches(k, list(_OPTION_DEFAULTS), n=1)
            hint = f" Did you mean {near[0]!r}?" if near else f" Options: {', '.join(sorted(_OPTION_DEFAULTS))}."
            raise TypeError(f"unexpected option {k!r}.{hint}")
        out[k] = v
    return out


class Animation(_Controller):
    """Controller for an animation created by :func:`animate`.

    Methods return ``self`` so they can be chained:
    ``animate(obj, {"x": 100}, autoplay=False).seek(0.25).play()``.
    """

    def __init__(
        self,
        targets: Any,
        props: Mapping[str, Any],
        *,
        engine: Optional[Engine] = None,
        speed: float = 1.0,
        autoplay: bool = True,
        **options: Any,
    ) -> None:
        engine = resolve_engine(engine)
        hooks = _make_hooks(options)
        super().__init__(engine, hooks)
        opts = _merge_options({}, options)
        try:
            tween = _build_tween(engine, targets, props, opts, self._tree, hooks, None)
        except BaseException:
            self._finalizer()
            raise
        self._h = tween.handle
        self._tree.roots.append(tween.handle)
        self._tween = tween
        if any(r.timing.remap is not None for r in tween.records):
            hooks.finish = _snap_to_end(engine.core, tween.records)
        if speed != 1.0:
            self.speed = speed
        if autoplay:
            engine.core.play(self._h)
            engine._alive.add(self)

    @property
    def targets(self) -> List[Any]:
        return [r.target for r in self._tween.records]

    def to_css_keyframes(self, name: str, selector: Optional[str] = None, samples: Optional[int] = None) -> str:
        """Export as a CSS ``@keyframes`` block (plus a ``selector { animation: ... }``
        rule when ``selector`` is given). See :mod:`anikit.export`."""
        from .export import css_keyframes

        return css_keyframes(self, name, selector=selector, samples=samples)

    def to_waapi(self, samples: Optional[int] = None) -> Tuple[List[Dict[str, Any]], Dict[str, Any]]:
        """Export as Web Animations API ``(keyframes, timing)`` for ``element.animate()``."""
        from .export import waapi

        return waapi(self, samples=samples)


def animate(
    targets: Any,
    props: Mapping[str, Any],
    duration: Any = 0.5,
    delay: Any = 0.0,
    ease: EasingLike = "outQuad",
    repeat: float = 0,
    yoyo: bool = False,
    repeat_delay: Any = 0.0,
    speed: float = 1.0,
    autoplay: bool = True,
    color_space: str = "oklab",
    on_start: Optional[Callback] = None,
    on_update: Optional[Callback] = None,
    on_complete: Optional[Callback] = None,
    on_repeat: Optional[Callback] = None,
    *,
    on_reverse_complete: Optional[Callback] = None,
    immediate_render: bool = False,
    engine: Optional[Engine] = None,
) -> Animation:
    """Animate properties of plain Python targets.

    ``targets`` is a dict (keys are animated), any object (attributes are
    animated) or a list of those. ``props`` maps property names to values —
    numbers, unit strings, colours, strings with numbers, relative values,
    ``[from, to]`` / keyframe lists, paths or callables; see :mod:`anikit`.

    Times are in seconds. ``duration``/``delay``/``repeat_delay`` may be
    per-target lists (e.g. from :func:`stagger`) or ``fn(target, index, total)``.
    ``repeat=-1`` repeats forever; ``yoyo`` alternates direction.

    Callbacks receive the :class:`Animation` if they take an argument.
    Values are written by :func:`tick` (or :func:`render`).
    """
    return Animation(
        targets,
        props,
        engine=engine,
        speed=speed,
        autoplay=autoplay,
        duration=duration,
        delay=delay,
        ease=ease,
        repeat=repeat,
        yoyo=yoyo,
        repeat_delay=repeat_delay,
        color_space=color_space,
        immediate_render=immediate_render,
        on_start=on_start,
        on_update=on_update,
        on_complete=on_complete,
        on_repeat=on_repeat,
        on_reverse_complete=on_reverse_complete,
    )


_NUM = r"[-+]?(?:\d+\.?\d*|\.\d+)(?:[eE][-+]?\d+)?"
_REL_RE = re.compile(rf"^([-+])=\s*({_NUM})$")
_PREV_RE = re.compile(rf"^([<>])\s*(?:([-+]=?)?\s*({_NUM})\s*(%)?)?$")
_LABEL_RE = re.compile(rf"^([A-Za-z_][\w\-.]*)\s*(?:([-+])=\s*({_NUM}))?$")


class Timeline(_Controller):
    """Sequence and overlap animations. Create with :func:`timeline`.

    Positions (``position=``) are GSAP-style:

    * ``2`` — absolute seconds
    * default (``None``) — the end of the timeline (as in GSAP and the TypeScript API)
    * ``">"`` — end of the previous child, ``"<"`` — its start
    * ``">0.5"``, ``">-0.1"``, ``"<0.2"``, ``"<+=0.2"``, ``"<50%"`` — offsets from those
    * ``"+=0.5"`` / ``"-=0.2"`` — relative to the end of the timeline
    * ``"intro"``, ``"intro+=0.3"`` — labels (unknown labels are created at the end)

    Tweens of the same target property chain: when ``from`` is omitted, a
    tween starts from the previous tween's end value in this timeline.
    """

    def __init__(
        self,
        *,
        engine: Optional[Engine] = None,
        delay: float = 0.0,
        repeat: float = 0,
        yoyo: bool = False,
        repeat_delay: float = 0.0,
        speed: float = 1.0,
        autoplay: bool = True,
        defaults: Optional[Mapping[str, Any]] = None,
        on_start: Optional[Callback] = None,
        on_update: Optional[Callback] = None,
        on_complete: Optional[Callback] = None,
        on_repeat: Optional[Callback] = None,
        on_reverse_complete: Optional[Callback] = None,
    ) -> None:
        engine = resolve_engine(engine)
        hooks = Hooks(on_start, on_update, on_complete, on_repeat, on_reverse_complete)
        super().__init__(engine, hooks)
        self._defaults = dict(defaults or {})
        for k in self._defaults:
            if k not in _OPTION_DEFAULTS:
                raise TypeError(f"unexpected default option {k!r}")
        timing = _Timing(1.0, _seconds("delay", delay, True), repeat, yoyo, _seconds("repeat_delay", repeat_delay))
        # Repeat delays: children are shifted by `lead` and a spacer pads the end by `trail`.
        self._shift = timing.lead / 1000.0
        self._pad = timing.trail / 1000.0
        self._h = engine.core.create_group(
            delay=timing.delay,
            iterations=timing.iterations,
            direction=timing.direction,
            end_delay=timing.end_delay,
            auto_duration=True,
        )
        engine._register(self._h, self._tree, Node(hooks=hooks))
        hooks.handle = self._h
        self._tree.roots.append(self._h)
        self._spacer: Optional[int] = None
        if self._pad > 0:
            self._spacer = engine.core.create_callback()
            engine._register(self._spacer, self._tree, Node())
            engine.core.group_add(self._h, self._spacer, (self._shift + self._pad) * 1000.0)
        self._planned: Dict[Tuple[int, str], Any] = {}
        self._labels: Dict[str, float] = {}
        self._prev_start = 0.0
        self._prev_end = 0.0
        self._end = 0.0
        self._children: List[Any] = []
        if speed != 1.0:
            self.speed = speed
        if autoplay:
            engine.core.play(self._h)
            engine._alive.add(self)

    # -- positions -----------------------------------------------------------

    def _position(self, position: Position) -> float:
        if position is None:
            return self._end
        if isinstance(position, bool):
            raise TypeError("position must be a number or a string")
        if isinstance(position, (int, float)):
            return max(0.0, float(position))
        if not isinstance(position, str):
            hint = " (options are keyword arguments: tl.add(target, props, \"-=0.2\", duration=0.5))" if isinstance(position, Mapping) else ""
            raise TypeError(f"position must be a number or a string, got {type(position).__name__}{hint}")
        p = position.strip()
        try:
            return max(0.0, float(p))
        except ValueError:
            pass
        m = _PREV_RE.match(p)
        if m:
            base = self._prev_start if m.group(1) == "<" else self._prev_end
            if m.group(3) is None:
                return max(0.0, base)
            x = float(m.group(3))
            if m.group(2) and m.group(2).startswith("-"):
                x = -x
            if m.group(4):
                x = x / 100.0 * (self._prev_end - self._prev_start)
            return max(0.0, base + x)
        m = _REL_RE.match(p)
        if m:
            x = float(m.group(2))
            return max(0.0, self._end + (x if m.group(1) == "+" else -x))
        m = _LABEL_RE.match(p)
        if m:
            name = m.group(1)
            if name not in self._labels:
                self._labels[name] = self._end
            base = self._labels[name]
            if m.group(3) is None:
                return base
            x = float(m.group(3))
            return max(0.0, base + (x if m.group(2) == "+" else -x))
        raise ValueError(f"invalid timeline position: {position!r}")

    def _place(self, handle: int, start: float, length: float) -> None:
        self._engine.core.group_add(self._h, handle, (start + self._shift) * 1000.0)
        end = start + length
        self._prev_start, self._prev_end = start, end
        if end > self._end:
            self._end = end
            if self._spacer is not None:
                self._engine.core.set_start(self._spacer, (self._end + self._shift + self._pad) * 1000.0)

    # -- building ---------------------------------------------------------------

    def add(
        self,
        targets: Any,
        props: Optional[Mapping[str, Any]] = None,
        position: Position = None,
        **options: Any,
    ) -> "Timeline":
        """Add a tween (``targets, props``), a nested :class:`Animation` /
        :class:`Timeline`, or a callable (like :meth:`call`) at ``position``.

        Options are those of :func:`animate` (minus ``autoplay``); the
        timeline's ``defaults`` apply. Callbacks fire for this tween.
        """
        self._root_or_nested()
        if isinstance(targets, _Controller):
            if props is not None and position is None and not isinstance(props, Mapping):
                position, props = props, None  # tl.add(anim, 0.5)
            if props is not None or options:
                raise TypeError("nesting an animation takes only a position")
            return self._nest(targets, position)
        if callable(targets) and not isinstance(props, Mapping):
            return self.call(targets, props if position is None else position)
        if props is None:
            raise TypeError("add() needs props for a tween")
        speed = float(options.pop("speed", 1.0))
        hooks = _make_hooks(options, parent=self._hooks)
        hooks.ctrl = self._hooks.ctrl  # child callbacks receive the timeline
        opts = _merge_options(self._defaults, options)
        start = self._position(position)
        tween = _build_tween(self._engine, targets, props, opts, self._tree, hooks, self._planned)
        core = self._engine.core
        if speed != 1.0:
            if not speed > 0:
                raise ValueError("speed must be > 0")
            core.set_rate(tween.handle, speed)
        self._place(tween.handle, start, core.duration(tween.handle) / 1000.0 / speed)
        self._children.append(hooks)  # keep child callbacks alive with the timeline
        return self

    to = add

    def set(self, targets: Any, props: Mapping[str, Any], position: Position = None, **options: Any) -> "Timeline":
        """Zero-duration tween: set values when the playhead reaches ``position``
        (and restore them when playing back past it)."""
        options.setdefault("ease", "linear")
        options["duration"] = 0.0
        return self.add(targets, props, position, **options)

    def call(self, fn: Callable[..., Any], position: Position = None, *args: Any) -> "Timeline":
        """Call ``fn(*args)`` whenever the playhead crosses ``position`` (either direction)."""
        self._root_or_nested()
        if not callable(fn):
            raise TypeError("call() needs a callable")
        start = self._position(position)
        h = self._engine.core.create_callback()
        self._engine._register(h, self._tree, Node(callback=(fn, tuple(args))))
        self._place(h, start, 0.0)
        return self

    def label(self, name: str, position: Position = None) -> "Timeline":
        """Name a time position (default: the end of the timeline)."""
        if not isinstance(name, str) or not re.match(r"^[A-Za-z_][\w\-.]*$", name):
            raise ValueError(f"invalid label name {name!r}")
        self._labels[name] = self._end if position is None else self._position(position)
        return self

    @property
    def labels(self) -> Dict[str, float]:
        return dict(self._labels)

    def _nest(self, child: "_Controller", position: Position) -> "Timeline":
        if child is self:
            raise ValueError("cannot add a timeline to itself")
        h = child._root()
        start = self._position(position)
        core = self._engine.core
        if child._engine is not self._engine:
            raise ValueError("cannot nest an animation from another engine")
        rate = abs(core.rate(h))
        core.pause(h)
        self._place(h, start, core.duration(h) / 1000.0 / rate)
        # Ownership moves to this timeline.
        self._engine._alive.discard(child)
        child._finalizer.detach()
        child._nested = True
        self._tree.nodes.extend(child._tree.nodes)
        child._tree.roots.clear()
        child._hooks.parent = self._hooks
        for hnode in child._tree.nodes:
            node = self._engine._nodes.get(hnode)
            if node is not None and node.binder is not None:
                for e in node.binder.entries:
                    e.group = self._tree
        self._children.append(child)
        return self

    def _root_or_nested(self) -> None:
        if not self._finalizer.alive:
            raise RuntimeError("Timeline was killed")

    def __len__(self) -> int:
        return len(self._children)


def timeline(
    *,
    delay: float = 0.0,
    repeat: float = 0,
    yoyo: bool = False,
    repeat_delay: float = 0.0,
    speed: float = 1.0,
    autoplay: bool = True,
    defaults: Optional[Mapping[str, Any]] = None,
    on_start: Optional[Callback] = None,
    on_update: Optional[Callback] = None,
    on_complete: Optional[Callback] = None,
    on_repeat: Optional[Callback] = None,
    on_reverse_complete: Optional[Callback] = None,
    engine: Optional[Engine] = None,
) -> Timeline:
    """Create a :class:`Timeline`. ``defaults`` (e.g. ``{"duration": 0.3,
    "ease": "outCubic"}``) apply to every tween added to it."""
    return Timeline(
        engine=engine,
        delay=delay,
        repeat=repeat,
        yoyo=yoyo,
        repeat_delay=repeat_delay,
        speed=speed,
        autoplay=autoplay,
        defaults=defaults,
        on_start=on_start,
        on_update=on_update,
        on_complete=on_complete,
        on_repeat=on_repeat,
        on_reverse_complete=on_reverse_complete,
    )
