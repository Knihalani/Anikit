"""The high-level engine: owns the native engine, maps handles to Python
targets, writes values for exactly the nodes the core reports as updated and
fires callbacks."""

from __future__ import annotations

import inspect
import math
import time as _time
import weakref
from typing import TYPE_CHECKING, Any, Callable, Dict, Iterator, List, Mapping, Optional, Sequence, Set, Tuple

from . import _anikit
from ._easing import Easing, EasingLike, _parse
from ._values import MISSING, Kind, MotionKind, plan_track, read, setter_for

if TYPE_CHECKING:  # pragma: no cover
    from ._animation import Animation, Timeline
    from ._physics import PhysicsAnimation, SpringAnimation

__all__ = ["Engine", "default_engine", "set_default_engine"]

_START = _anikit.EVENT_START
_REPEAT = _anikit.EVENT_REPEAT
_COMPLETE = _anikit.EVENT_COMPLETE
_REVERSE_COMPLETE = _anikit.EVENT_REVERSE_COMPLETE
_CALLBACK = _anikit.EVENT_CALLBACK
_FINISH = _anikit.EVENT_FINISH
_LINEAR = _anikit.LINEAR

Callback = Callable[..., Any]


def _wants_arg(fn: Callable[..., Any]) -> bool:
    """Whether a callback accepts the controller as a positional argument."""
    try:
        sig = inspect.signature(fn)
    except (TypeError, ValueError):
        return False
    for p in sig.parameters.values():
        if p.kind in (p.POSITIONAL_ONLY, p.POSITIONAL_OR_KEYWORD, p.VAR_POSITIONAL):
            return True
    return False


class Hooks:
    """User callbacks of one tween / timeline / spring group.

    Callbacks are called with the controller (``Animation``, ``Timeline`` or
    ``SpringAnimation``) if they accept a positional argument, else with none.
    Holds only a weak reference to the controller so callbacks never keep a
    discarded animation alive.
    """

    __slots__ = (
        "handle",
        "on_start",
        "on_update",
        "on_complete",
        "on_repeat",
        "on_reverse_complete",
        "parent",
        "ctrl",
        "completed",
        "finish",
        "_args",
    )

    def __init__(
        self,
        on_start: Optional[Callback] = None,
        on_update: Optional[Callback] = None,
        on_complete: Optional[Callback] = None,
        on_repeat: Optional[Callback] = None,
        on_reverse_complete: Optional[Callback] = None,
        parent: Optional["Hooks"] = None,
    ) -> None:
        self.handle: Optional[int] = None
        self.on_start = on_start
        self.on_update = on_update
        self.on_complete = on_complete
        self.on_repeat = on_repeat
        self.on_reverse_complete = on_reverse_complete
        self.parent = parent
        self.ctrl: Optional["weakref.ReferenceType[Any]"] = None
        self.completed = False
        #: Internal hook run when the root finishes playing forwards (before user callbacks).
        self.finish: Optional[Callable[[], None]] = None
        self._args: Dict[int, bool] = {}
        for fn in (on_start, on_update, on_complete, on_repeat, on_reverse_complete):
            if fn is not None:
                if not callable(fn):
                    raise TypeError(f"callbacks must be callable, got {type(fn).__name__}")
                self._args[id(fn)] = _wants_arg(fn)

    def fire(self, fn: Optional[Callback]) -> None:
        if fn is None:
            return
        if self._args.get(id(fn), False):
            fn(self.ctrl() if self.ctrl is not None else None)
        else:
            fn()


def _depth(hk: Optional[Hooks]) -> int:
    d = 0
    while hk is not None and hk.parent is not None:
        d += 1
        hk = hk.parent
    return d


class Entry:
    """One property binding: decodes a slice of a node's output and writes it."""

    __slots__ = ("keys", "decode", "off", "track", "enabled", "multi", "group", "handle", "sim", "kind")

    def __init__(self, keys: Tuple[str, ...], kind: Kind, off: int, track: int, group: object) -> None:
        self.keys = keys
        self.kind = kind
        self.decode = kind.decode
        self.off = off
        self.track = track
        self.enabled = True
        self.multi = isinstance(kind, MotionKind)
        #: Identity of the owning animation tree; tracks of the same tree never overwrite each other.
        self.group = group
        self.handle = -1
        self.sim: Any = None


class Binder:
    """Writes a node's output values into one target."""

    __slots__ = ("target", "entries", "set")

    def __init__(self, target: Any, entries: List[Entry]) -> None:
        self.target = target
        self.entries = entries
        self.set = setter_for(target)

    def apply(self, vals: Sequence[float]) -> None:
        t = self.target
        s = self.set
        for e in self.entries:
            if e.enabled:
                v = e.decode(vals, e.off)
                if e.multi:
                    for k, x in zip(e.keys, v):
                        s(t, k, x)
                else:
                    s(t, e.keys[0], v)


class Node:
    __slots__ = ("binder", "hooks", "callback", "sim")

    def __init__(
        self,
        binder: Optional[Binder] = None,
        hooks: Optional[Hooks] = None,
        callback: Optional[Tuple[Callback, Tuple[Any, ...]]] = None,
        sim: Any = None,
    ) -> None:
        self.binder = binder
        self.hooks = hooks
        self.callback = callback
        self.sim = sim


class Tree:
    """Handles owned by one controller (released together)."""

    __slots__ = ("roots", "nodes", "__weakref__")

    def __init__(self) -> None:
        self.roots: List[int] = []
        self.nodes: List[int] = []


def release_tree(engine_ref: "weakref.ReferenceType[Engine]", tree: Tree) -> None:
    engine = engine_ref()
    if engine is not None:
        engine._free_tree(tree)


class Engine:
    """An animation world. Create animations with :meth:`animate`,
    :meth:`timeline`, :meth:`spring`, :meth:`inertia` and :meth:`physics`, then advance it with
    :meth:`tick` (or iterate :meth:`render`).

    All times are in **seconds**. The native engine is available as
    :attr:`core` (milliseconds, integer handles) for low-level use.

    ``lag_smoothing=(threshold, adjusted)`` (seconds) treats frame gaps longer
    than ``threshold`` as ``adjusted`` — useful for real-time loops. It is off
    by default so scripted ``tick()`` calls advance exactly.
    """

    def __init__(self, *, lag_smoothing: Optional[Tuple[float, float]] = None) -> None:
        self.core = _anikit.Engine()
        if lag_smoothing is None:
            self.core.set_lag_smoothing(0.0, 0.0)
        else:
            self.core.set_lag_smoothing(lag_smoothing[0] * 1000.0, lag_smoothing[1] * 1000.0)
        self._nodes: Dict[int, Node] = {}
        self._owners: Dict[Tuple[int, str], Entry] = {}
        self._sims: Dict[Tuple[int, str], Any] = {}
        self._alive: Set[Any] = set()
        self._easings: Dict[Any, int] = {}
        self._fn_easings: "weakref.WeakKeyDictionary[Any, int]" = weakref.WeakKeyDictionary()
        self._trees: "weakref.WeakSet[Tree]" = weakref.WeakSet()

    # ------------------------------------------------------------------
    # Easings
    # ------------------------------------------------------------------

    def easing_id(self, e: EasingLike) -> int:
        """Register an easing with the native engine and return its id."""
        if e is None:
            return _LINEAR
        if isinstance(e, str):
            try:
                return self.core.easing(e)
            except ValueError:
                _parse(e)  # raises with a "did you mean" suggestion
                raise
        if isinstance(e, Easing):
            native = e.native
        elif isinstance(e, _anikit.NativeEasing):
            native = e
        elif callable(e):
            try:
                cached = self._fn_easings.get(e)
            except TypeError:
                cached = None
            if cached is not None:
                return cached
            eid = self.core.add_easing(Easing(e).native)
            try:
                self._fn_easings[e] = eid
            except TypeError:
                pass
            return eid
        else:
            raise TypeError(f"expected an easing string, Easing or callable, got {type(e).__name__}")
        known = self._easings.get(native)
        if known is not None:
            return known
        eid = self._easings[native] = self.core.add_easing(native)
        return eid

    # ------------------------------------------------------------------
    # Registry
    # ------------------------------------------------------------------

    def _register(self, h: int, tree: Optional[Tree], node: Node) -> None:
        self._nodes[h] = node
        if tree is not None:
            tree.nodes.append(h)
            self._trees.add(tree)
        if node.binder is not None:
            for e in node.binder.entries:
                e.handle = h

    def _forget(self, h: int) -> None:
        node = self._nodes.pop(h, None)
        if node is None or node.binder is None:
            return
        tid = id(node.binder.target)
        for e in node.binder.entries:
            for k in e.keys:
                if self._owners.get((tid, k)) is e:
                    del self._owners[(tid, k)]

    def _free_tree(self, tree: Tree) -> None:
        for h in tree.roots:
            try:
                self.core.release(h)
            except (KeyError, RuntimeError):
                pass
        for h in tree.nodes:
            self._forget(h)
        tree.roots.clear()
        tree.nodes.clear()

    def _claim(self, binder: Binder) -> None:
        """Make ``binder``'s properties take over from other animations
        (``overwrite="auto"``): tracks of *other* trees animating the same
        target property are disabled."""
        tid = id(binder.target)
        owners = self._owners
        for e in binder.entries:
            if not e.enabled:
                continue
            for k in e.keys:
                prev = owners.get((tid, k))
                if prev is e:
                    continue
                if prev is not None and prev.enabled and prev.group is not e.group:
                    self._disable(prev)
                owners[(tid, k)] = e

    def _disable(self, e: Entry) -> None:
        e.enabled = False
        if e.sim is not None:
            self._release_sim(e.sim, completed=False)
        else:
            try:
                self.core.set_track_enabled(e.handle, e.track, False)
            except (KeyError, TypeError, ValueError):
                pass

    def _release_sim(self, sim: Any, completed: bool) -> List[Tuple[Hooks, Optional[Callback]]]:
        """Free a simulation node; returns callbacks to fire."""
        try:
            self.core.release(sim.handle)
        except KeyError:
            pass
        self._forget(sim.handle)
        key = (id(sim.target), sim.key)
        if self._sims.get(key) is sim:
            del self._sims[key]
        ctrl: Optional["SpringAnimation"] = sim.ctrl
        if ctrl is None:
            return []
        return ctrl._sim_finished(sim, completed)

    # ------------------------------------------------------------------
    # The frame
    # ------------------------------------------------------------------

    def tick(self, dt: float) -> None:
        """Advance every animation by ``dt`` seconds, write updated values to
        their targets and fire callbacks."""
        if not dt >= 0.0:
            raise ValueError(f"dt must be a non-negative number of seconds, got {dt!r}")
        hs, vs, ev = self.core.step(dt * 1000.0)
        if hs or ev:
            self._dispatch(hs, vs, ev)

    def flush(self, *, events: bool = True) -> None:
        """Apply values (and fire events) produced outside :meth:`tick`, e.g. by seeking."""
        hs, vs, ev = self.core.drain()
        if hs or ev:
            self._dispatch(hs, vs, ev if events else [])

    def _dispatch(self, hs: List[int], vs: List[List[float]], ev: List[int]) -> None:
        nodes = self._nodes
        # 1. Property takeover happens before values are written, so the
        #    newest animation wins deterministically from its first frame.
        if ev:
            for i in range(0, len(ev), 2):
                if ev[i + 1] == _START:
                    node = nodes.get(ev[i])
                    if node is not None and node.binder is not None:
                        self._claim(node.binder)
        # 2. Write values of exactly the nodes that changed.
        updates: Dict[int, Hooks] = {}
        for h, vals in zip(hs, vs):
            node = nodes.get(h)
            if node is None:
                continue
            b = node.binder
            if b is not None:
                b.apply(vals)
            hk = node.hooks
            while hk is not None:
                if hk.on_update is not None:
                    updates[id(hk)] = hk
                hk = hk.parent
        if not ev and not updates:
            return
        # 3. Bookkeeping for events (never raises), collecting user callbacks.
        pre: List[Tuple[Optional[Hooks], Any]] = []
        post: List[Tuple[Optional[Hooks], Any]] = []
        keep: List[Any] = []  # keeps finished controllers alive until their callbacks ran
        core = self.core
        for i in range(0, len(ev), 2):
            h = ev[i]
            k = ev[i + 1]
            node = nodes.get(h)
            if node is None:
                continue
            if k == _CALLBACK:
                if node.callback is not None:
                    pre.append((None, node.callback))
                continue
            if node.sim is not None:
                if k == _FINISH:
                    post.extend(self._release_sim(node.sim, completed=True))
                continue
            hk = node.hooks
            if hk is None or hk.handle != h:
                continue
            if k == _START:
                hk.completed = False
                if hk.on_start is not None:
                    pre.append((hk, hk.on_start))
            elif k == _REPEAT:
                if hk.on_repeat is not None:
                    pre.append((hk, hk.on_repeat))
            elif k == _COMPLETE:
                if not hk.completed:
                    hk.completed = True
                    if hk.on_complete is not None:
                        post.append((hk, hk.on_complete))
            elif k == _REVERSE_COMPLETE:
                hk.completed = False
                if hk.on_reverse_complete is not None:
                    post.append((hk, hk.on_reverse_complete))
            elif k == _FINISH:
                try:
                    forward = core.rate(h) > 0
                except KeyError:
                    forward = True
                if forward and hk.finish is not None:
                    hk.finish()
                if forward and not hk.completed:
                    # Roots whose end is not an "after" phase (repeat delays) still complete.
                    hk.completed = True
                    if hk.on_complete is not None:
                        post.append((hk, hk.on_complete))
                ctrl = hk.ctrl() if hk.ctrl is not None else None
                if ctrl is not None:
                    keep.append(ctrl)
                    self._alive.discard(ctrl)
        # 4. User callbacks: start/repeat/timeline calls, then updates, then completions.
        for hk, fn in pre:
            if hk is None:
                f, args = fn
                f(*args)
            else:
                hk.fire(fn)
        for hk in updates.values():
            hk.fire(hk.on_update)
        if len(post) > 1:
            # Children complete before their timelines (the core reports parents first).
            post.sort(key=lambda item: -_depth(item[0]))
        for hk, fn in post:
            if hk is not None:
                hk.fire(fn)
        del keep

    # ------------------------------------------------------------------
    # Driving
    # ------------------------------------------------------------------

    def render(
        self, fps: float = 60.0, duration: Optional[float] = None, *, max_duration: float = 600.0
    ) -> Iterator[Tuple[int, float]]:
        """Generator for producing frame sequences (video, GIFs, plots).

        Yields ``(frame_index, time)`` after advancing the engine to ``time``
        (seconds since rendering started; frame 0 is ``t = 0`` with every
        playing animation rendered at its start). With ``duration`` it yields
        ``round(duration * fps) + 1`` frames covering ``[0, duration]``;
        otherwise it stops after the first frame on which the engine is idle
        (or after ``max_duration``).
        """
        if not fps > 0:
            raise ValueError("fps must be > 0")
        frames = None if duration is None else int(round(duration * fps)) + 1
        limit = int(math.ceil(max_duration * fps)) + 1
        prev = 0.0
        i = 0
        while True:
            t = i / fps
            self.tick(t - prev)
            prev = t
            yield i, t
            i += 1
            if frames is not None:
                if i >= frames:
                    return
            elif self.core.active_count == 0 or i >= limit:
                return

    def run_until_idle(self, fps: float = 60.0, *, max_time: float = 600.0, realtime: bool = False) -> float:
        """Tick at ``fps`` until nothing is playing; returns the simulated seconds.

        With ``realtime=True`` frames are paced with :func:`time.sleep` (no
        busy waiting). Infinite animations stop at ``max_time``.
        """
        if not fps > 0:
            raise ValueError("fps must be > 0")
        dt = 1.0 / fps
        elapsed = 0.0
        next_frame = _time.perf_counter()
        while self.core.active_count > 0 and elapsed < max_time:
            if realtime:
                next_frame += dt
                delay = next_frame - _time.perf_counter()
                if delay > 0:
                    _time.sleep(delay)
            self.tick(dt)
            elapsed += dt
        return elapsed

    # ------------------------------------------------------------------
    # Introspection
    # ------------------------------------------------------------------

    @property
    def time(self) -> float:
        """World time advanced so far (seconds, scaled by :attr:`speed`)."""
        return self.core.now / 1000.0

    @property
    def speed(self) -> float:
        """Global time scale (1 = normal)."""
        return self.core.global_rate

    @speed.setter
    def speed(self, value: float) -> None:
        self.core.global_rate = float(value)

    @property
    def node_count(self) -> int:
        """Live native nodes (animations, groups, callbacks, simulations)."""
        return self.core.node_count

    @property
    def active_count(self) -> int:
        """Playing root animations and running simulations."""
        return self.core.active_count

    @property
    def is_idle(self) -> bool:
        return self.core.active_count == 0

    def clear(self) -> None:
        """Kill everything (targets keep their current values)."""
        self.core.clear()
        for tree in list(self._trees):
            # Handles are gone; make sure no finalizer ever touches a reused slot.
            tree.roots.clear()
            tree.nodes.clear()
        for sim in list(self._sims.values()):
            if sim.ctrl is not None:
                sim.ctrl._sims.clear()
        self._nodes.clear()
        self._owners.clear()
        self._sims.clear()
        self._alive.clear()

    def __repr__(self) -> str:
        return f"<anikit.Engine nodes={self.node_count} active={self.active_count} t={self.time:.3f}s>"

    # ------------------------------------------------------------------
    # Factories (documented on the module-level functions)
    # ------------------------------------------------------------------

    def animate(self, targets: Any, props: Mapping[str, Any], **options: Any) -> "Animation":
        """See :func:`anikit.animate`."""
        from ._animation import animate

        return animate(targets, props, engine=self, **options)

    def timeline(self, **options: Any) -> "Timeline":
        """See :func:`anikit.timeline`."""
        from ._animation import Timeline

        return Timeline(engine=self, **options)

    def spring(self, targets: Any, props: Mapping[str, Any], **options: Any) -> "SpringAnimation":
        """See :func:`anikit.spring`."""
        from ._physics import spring

        return spring(targets, props, engine=self, **options)

    def inertia(self, targets: Any, props: Mapping[str, Any], **options: Any) -> "SpringAnimation":
        """See :func:`anikit.inertia`."""
        from ._physics import inertia

        return inertia(targets, props, engine=self, **options)

    def physics(self, targets: Any, props: Mapping[str, Any], **options: Any) -> "PhysicsAnimation":
        """See :func:`anikit.physics`."""
        from ._physics import physics

        return physics(targets, props, engine=self, **options)

    def set(self, targets: Any, props: Mapping[str, Any], *, color_space: str = "oklab") -> None:
        """Write property values immediately (callables, relative values,
        colours, keyframes → their final value all work)."""
        tl = as_targets(targets)
        cs = _anikit.color_space_id(color_space)
        n = len(tl)
        for i, t in enumerate(tl):
            put = setter_for(t)
            for key, raw in props.items():
                plan = plan_track(key, raw, t, i, n, read(t, key), cs)
                v = plan.final_value()
                if isinstance(plan.kind, MotionKind):
                    for k, x in zip(plan.kind.keys, v):
                        put(t, k, x)
                else:
                    put(t, key, v)


def as_targets(targets: Any) -> List[Any]:
    """Normalise ``targets`` to a list (a dict or object is a single target)."""
    if isinstance(targets, (list, tuple)):
        tl = list(targets)
    else:
        tl = [targets]
    if not tl:
        raise ValueError("no targets to animate")
    for t in tl:
        if t is None or isinstance(t, (str, bytes, int, float)):
            raise TypeError(f"cannot animate {type(t).__name__} target {t!r}: use a dict or an object")
    return tl


_default: Optional[Engine] = None


def default_engine() -> Engine:
    """The engine used by module-level functions (created on first use)."""
    global _default
    if _default is None:
        _default = Engine()
    return _default


def set_default_engine(engine: Optional[Engine]) -> None:
    """Replace the module-level engine (``None`` creates a fresh one on next use)."""
    global _default
    _default = engine


def resolve_engine(engine: Optional[Engine]) -> Engine:
    return engine if engine is not None else default_engine()


__all__ += ["as_targets", "MISSING"]
