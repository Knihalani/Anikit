"""Physics-driven motion: retargetable springs, inertia (decay with bounds) and
ballistic simulation (velocity, gravity, drag, bouncing bounds)."""

from __future__ import annotations

import weakref
from typing import Any, Dict, List, Mapping, Optional, Sequence, Tuple, Union

from . import _anikit
from ._engine import Binder, Callback, Engine, Entry, Hooks, Node, as_targets, resolve_engine
from ._values import (
    _CURRENT,
    MISSING,
    DiscreteKind,
    Kind,
    NumberKind,
    UnitKind,
    frames_of,
    read,
    resolve_callable,
    resolve_values,
)

__all__ = ["SpringAnimation", "PhysicsAnimation", "spring", "inertia", "physics"]


class _Sim:
    """One simulation node animating one ``target[key]``."""

    __slots__ = ("handle", "target", "key", "kind", "entry", "ctrl", "channels", "ballistic")

    def __init__(self, handle: int, target: Any, key: str, kind: Kind, entry: Entry, ctrl: "SpringAnimation") -> None:
        self.handle = handle
        self.target = target
        self.key = key
        self.kind = kind
        self.entry = entry
        self.ctrl: Optional[SpringAnimation] = ctrl
        self.channels = kind.channels
        #: A :func:`physics` simulation (never retargeted in place by :func:`spring`).
        self.ballistic = False


class SpringAnimation:
    """Controller for physics animations created by :func:`spring` / :func:`inertia`.

    Calling :func:`spring` again for the same target property *retargets* the
    running simulation (position and velocity are preserved); that property
    then belongs to the newer controller.
    """

    def __init__(self, engine: Engine, hooks: Hooks, params: Dict[str, Any], targets: List[Any]) -> None:
        self._engine = engine
        self._hooks = hooks
        self._params = params
        self._targets = targets
        self._sims: List[_Sim] = []
        self._killed = False
        self._started = False
        hooks.ctrl = weakref.ref(self)

    def _sim_finished(self, sim: _Sim, completed: bool) -> List[Tuple[Hooks, Optional[Callback]]]:
        if sim.ctrl is not self:
            return []
        sim.ctrl = None
        try:
            self._sims.remove(sim)
        except ValueError:
            pass
        if not self._sims and completed and not self._killed and not self._hooks.completed:
            self._hooks.completed = True
            return [(self._hooks, self._hooks.on_complete)]
        return []

    @property
    def finished(self) -> bool:
        """True once every simulation of this controller came to rest (or was taken over)."""
        return not self._sims

    @property
    def is_active(self) -> bool:
        return bool(self._sims)

    @property
    def targets(self) -> List[Any]:
        return list(self._targets)

    def _find(self, key: str, target: Any) -> _Sim:
        for s in self._sims:
            if s.key == key and (target is None or s.target is target):
                return s
        raise KeyError(f"no running simulation for {key!r}")

    def velocity(self, key: str, target: Any = None) -> Union[float, List[float]]:
        """Current velocity (units/second) of ``key`` (first target unless given)."""
        s = self._find(key, target)
        v = self._engine.core.sim_velocity(s.handle)
        return v[0] if len(v) == 1 else v

    def target_value(self, key: str, target: Any = None) -> Any:
        """Where ``key`` is heading (decoded like the property value)."""
        s = self._find(key, target)
        return s.kind.decode(self._engine.core.sim_target(s.handle), 0)

    def retarget(self, props: Mapping[str, Any]) -> "SpringAnimation":
        """Send this controller's targets towards new values (velocity preserved)."""
        p = dict(self._params)
        return spring(self._targets, props, engine=self._engine, **p)

    def stop(self) -> None:
        """Stop all simulations where they are (no completion callback)."""
        self._killed = True
        for s in list(self._sims):
            self._engine._release_sim(s, completed=False)
        self._sims.clear()

    kill = stop

    def __repr__(self) -> str:
        return f"<SpringAnimation sims={len(self._sims)} {'running' if self._sims else 'finished'}>"


def _numeric_kind(kind: Kind) -> bool:
    return not isinstance(kind, DiscreteKind)


def _spring_params(
    stiffness: float, damping: float, mass: float, bounce: Optional[float], duration: Optional[float]
) -> Tuple[float, float, float]:
    if bounce is not None or duration is not None:
        return _anikit.spring_from_duration_bounce(
            0.5 if duration is None else float(duration), 0.25 if bounce is None else float(bounce)
        )
    return float(stiffness), float(damping), float(mass)


def _vec(v: Union[float, Sequence[float], None], ch: int) -> Optional[List[float]]:
    if v is None:
        return None
    if isinstance(v, (int, float)):
        return [float(v)] * ch
    vals = [float(x) for x in v]
    if len(vals) != ch:
        raise ValueError(f"velocity needs {ch} components, got {len(vals)}")
    return vals


def _install(
    engine: Engine, ctrl: SpringAnimation, target: Any, key: str, kind: Kind, handle: int, hooks: Hooks
) -> _Sim:
    entry = Entry((key,), kind, 0, 0, None)
    sim = _Sim(handle, target, key, kind, entry, ctrl)
    entry.group = sim
    entry.sim = sim
    binder = Binder(target, [entry])
    engine._register(handle, None, Node(binder=binder, hooks=hooks, sim=sim))
    engine._sims[(id(target), key)] = sim
    engine._claim(binder)
    ctrl._sims.append(sim)
    return sim


def _adopt(engine: Engine, sim: _Sim, ctrl: SpringAnimation, hooks: Hooks) -> None:
    old = sim.ctrl
    if old is not None and old is not ctrl:
        try:
            old._sims.remove(sim)
        except ValueError:
            pass
    sim.ctrl = ctrl
    node = engine._nodes.get(sim.handle)
    if node is not None:
        node.hooks = hooks
    if sim not in ctrl._sims:
        ctrl._sims.append(sim)


def spring(
    targets: Any,
    props: Mapping[str, Any],
    stiffness: float = 100.0,
    damping: float = 10.0,
    mass: float = 1.0,
    velocity: Union[float, Sequence[float], None] = None,
    bounce: Optional[float] = None,
    duration: Optional[float] = None,
    *,
    rest_delta: Optional[float] = None,
    rest_speed: Optional[float] = None,
    on_update: Optional[Callback] = None,
    on_complete: Optional[Callback] = None,
    engine: Optional[Engine] = None,
) -> SpringAnimation:
    """Move properties towards targets with a damped spring (exact closed-form physics).

    Values may be numbers, unit strings, colours, strings with numbers,
    relative values (``"+=50"``, relative to the current *target* when
    retargeting) or ``[from, to]``. ``bounce``/``duration`` (seconds) give a
    perceptual alternative to stiffness/damping/mass. ``velocity`` (units/s)
    sets the initial velocity; when omitted a running simulation keeps its
    velocity, which makes repeated calls (e.g. following a pointer) smooth.

    Settles when within ``rest_delta`` of the target moving slower than
    ``rest_speed`` (defaults 0.005 / 0.05). ``props`` maps property names to
    target values. ``on_update``/``on_complete`` are called every frame while moving / once at rest (with the controller if they take an argument).
    """
    engine = resolve_engine(engine)
    k, c, m = _spring_params(stiffness, damping, mass, bounce, duration)
    params = dict(
        stiffness=stiffness,
        damping=damping,
        mass=mass,
        bounce=bounce,
        duration=duration,
        rest_delta=rest_delta,
        rest_speed=rest_speed,
        on_update=on_update,
        on_complete=on_complete,
    )
    tl = as_targets(targets)
    if not isinstance(props, Mapping) or not props:
        raise ValueError("props must be a non-empty mapping of property -> value")
    hooks = Hooks(on_update=on_update, on_complete=on_complete)
    ctrl = SpringAnimation(engine, hooks, params, tl)
    core = engine.core
    n = len(tl)
    for i, t in enumerate(tl):
        for key, raw in props.items():
            existing = engine._sims.get((id(t), key))
            if existing is not None and not core.is_alive(existing.handle):
                existing = None
            frames = frames_of(resolve_callable(raw, t, i, n), t, i, n)
            if len(frames) > 2:
                raise ValueError("springs take a single target value or [from, to]")
            if existing is not None and not existing.ballistic:
                # Relative values build on where a running spring is heading.
                cur = existing.kind.decode(core.sim_target(existing.handle), 0)
            else:
                cur = read(t, key)
            res = resolve_values([f.value for f in frames], cur, 0, numeric_colors=True)
            kind = res.kind
            if not _numeric_kind(kind):
                raise TypeError(f"cannot spring non-numeric value for {key!r}: {frames[-1].value!r}")
            explicit_from = frames[0].value is not _CURRENT
            start = res.rows[0]
            goal = res.rows[-1]
            ch = len(goal)
            vel = _vec(velocity, ch)
            if existing is not None and existing.ballistic:
                # Replace a ballistic simulation with a fresh spring, keeping its momentum.
                if vel is None and existing.channels == ch:
                    vel = core.sim_velocity(existing.handle)
                engine._release_sim(existing, completed=False)
                existing = None
            if existing is not None and (existing.channels != ch or type(existing.kind) is not type(kind)):
                engine._release_sim(existing, completed=False)
                existing = None
            if existing is not None:
                h = existing.handle
                existing.kind = kind
                existing.entry.kind = kind
                existing.entry.decode = kind.decode
                existing.entry.enabled = True
                core.sim_set_params(h, k, c, m)
                if explicit_from or vel is not None:
                    core.sim_set_position(h, start if explicit_from else core.values(h), vel)
                core.sim_set_target(h, goal)
                _adopt(engine, existing, ctrl, hooks)
                binder = engine._nodes[h].binder
                if binder is not None:
                    engine._claim(binder)
                sim = existing
            else:
                if cur is MISSING and not explicit_from:
                    start = [0.0] * ch
                h = core.create_spring(start, goal, vel, k, c, m)
                sim = _install(engine, ctrl, t, key, kind, h, hooks)
            if rest_delta is not None or rest_speed is not None:
                core.sim_set_rest(
                    sim.handle, 0.005 if rest_delta is None else rest_delta, 0.05 if rest_speed is None else rest_speed
                )
    return ctrl


def inertia(
    targets: Any,
    props: Mapping[str, Any],
    power: float = 0.8,
    time_constant: float = 0.35,
    *,
    bounce_stiffness: float = 500.0,
    bounce_damping: float = 40.0,
    rest_delta: Optional[float] = None,
    rest_speed: Optional[float] = None,
    on_update: Optional[Callback] = None,
    on_complete: Optional[Callback] = None,
    engine: Optional[Engine] = None,
) -> SpringAnimation:
    """Glide with momentum and slow down exponentially (e.g. after a fling).

    ``props`` maps each property to its initial velocity (units/s) or a dict::

        inertia(card, {"x": {"velocity": 800, "min": 0, "max": 500}})

    Keys: ``velocity`` (default: the running simulation's velocity, else 0),
    ``min``/``max`` bounds (crossing one hands over to a bounce spring with
    ``bounce_stiffness``/``bounce_damping``) and ``end`` — the resting value: a
    number, a list of snap points (nearest to the natural end wins) or
    ``fn(natural_end) -> value``. The natural end is
    ``value + power * velocity * time_constant``.

    ``targets`` is a dict, an object or a list of them. ``rest_delta`` /
    ``rest_speed`` decide when it has settled. ``on_update``/``on_complete`` are called every frame while moving / once at rest (with the controller if they take an argument).
    """
    engine = resolve_engine(engine)
    tl = as_targets(targets)
    if not isinstance(props, Mapping) or not props:
        raise ValueError("props must be a non-empty mapping of property -> velocity or options")
    hooks = Hooks(on_update=on_update, on_complete=on_complete)
    ctrl = SpringAnimation(engine, hooks, {}, tl)
    core = engine.core
    tc = float(time_constant)
    if not tc > 0:
        raise ValueError("time_constant must be > 0")
    n = len(tl)
    for i, t in enumerate(tl):
        for key, spec in props.items():
            spec = resolve_callable(spec, t, i, n)
            if isinstance(spec, Mapping):
                opts = dict(spec)
            else:
                opts = {"velocity": spec}
            unknown = set(opts) - {"velocity", "min", "max", "end"}
            if unknown:
                raise TypeError(f"unexpected inertia option(s) for {key!r}: {sorted(unknown)}")
            cur = read(t, key)
            if cur is MISSING:
                cur = 0.0
            res = resolve_values([cur, cur], cur, 0)
            if not isinstance(res.kind, (NumberKind, UnitKind)):
                raise TypeError(f"inertia needs a numeric property, {key!r} is {cur!r}")
            pos = res.rows[0][0]
            existing = engine._sims.get((id(t), key))
            v = opts.get("velocity")
            if v is None:
                v = 0.0
                if existing is not None and core.is_alive(existing.handle):
                    v = core.sim_velocity(existing.handle)[0]
            v = float(v)
            if existing is not None:
                engine._release_sim(existing, completed=False)
            lo = opts.get("min")
            hi = opts.get("max")
            natural = pos + power * v * tc
            end = opts.get("end")
            target: Optional[float] = None
            if end is not None:
                if callable(end):
                    target = float(end(natural))
                elif isinstance(end, (list, tuple)):
                    if not end:
                        raise ValueError("snap point list is empty")
                    target = float(min(end, key=lambda s: abs(float(s) - natural)))
                else:
                    target = float(end)
                if lo is not None:
                    target = max(target, float(lo))
                if hi is not None:
                    target = min(target, float(hi))
            h = core.create_decay(
                [pos],
                [v],
                float(power),
                tc * 1000.0,
                None if target is None else [target],
                None if lo is None else [float(lo)],
                None if hi is None else [float(hi)],
                float(bounce_stiffness),
                float(bounce_damping),
                1.0,
            )
            sim = _install(engine, ctrl, t, key, res.kind, h, hooks)
            if rest_delta is not None or rest_speed is not None:
                core.sim_set_rest(
                    sim.handle, 0.005 if rest_delta is None else rest_delta, 0.05 if rest_speed is None else rest_speed
                )
    return ctrl


class PhysicsAnimation(SpringAnimation):
    """Controller for ballistic simulations created by :func:`physics`.

    Like :class:`SpringAnimation` (``velocity()``, ``stop()``, ``finished``,
    ``on_complete``), plus live control of the acceleration. A property that a
    later :func:`spring`, :func:`inertia`, :func:`physics` or tween takes over
    leaves this controller (its velocity carries over to springs/inertia/physics).
    """

    def acceleration(self, key: str, target: Any = None) -> float:
        """Current acceleration (units/s²) of ``key`` (first target unless given)."""
        s = self._find(key, target)
        # For physics simulations the core keeps the acceleration where springs keep their target.
        return self._engine.core.sim_target(s.handle)[0]

    def set_acceleration(self, props: Mapping[str, float], target: Any = None) -> "PhysicsAnimation":
        """Change the acceleration of running simulations, e.g. turn gravity
        off or add wind: ``ctrl.set_acceleration({"x": 40, "y": 0})``.

        Applies to every target (or only ``target``). Simulations that already
        came to rest are finished and cannot be changed; start a new
        :func:`physics` instead.
        """
        core = self._engine.core
        found = False
        for key, value in props.items():
            a = float(value)
            for s in self._sims:
                if s.key == key and (target is None or s.target is target):
                    core.sim_set_acceleration(s.handle, [a])
                    found = True
        if not found and props:
            raise KeyError(f"no running simulation for {sorted(props)!r}")
        return self

    def target_value(self, key: str, target: Any = None) -> Any:
        """Physics simulations have no target value."""
        raise TypeError("physics simulations have no target; use velocity() or acceleration()")

    def retarget(self, props: Mapping[str, Any]) -> "PhysicsAnimation":
        """Launch this controller's targets again with new ``props`` (same options)."""
        return physics(self._targets, props, engine=self._engine, **self._params)

    def __repr__(self) -> str:
        return f"<PhysicsAnimation sims={len(self._sims)} {'running' if self._sims else 'finished'}>"


_PHYSICS_KEYS = frozenset({"velocity", "acceleration", "min", "max", "friction", "restitution"})


def physics(
    targets: Any,
    props: Mapping[str, Any],
    friction: float = 0.0,
    restitution: float = 0.5,
    life: Optional[float] = None,
    *,
    rest_speed: Optional[float] = None,
    on_update: Optional[Callback] = None,
    on_complete: Optional[Callback] = None,
    engine: Optional[Engine] = None,
) -> PhysicsAnimation:
    """Ballistic motion: launch properties with a velocity under constant
    acceleration (gravity, wind), drag and bouncing bounds — confetti,
    projectiles, falling and bouncing objects.

    ``props`` maps each property to its initial velocity (units/s) or a dict::

        physics(ball, {
            "x": {"velocity": 300, "min": 0, "max": 800},
            "y": {"velocity": -600, "acceleration": 1500, "max": 400},   # gravity, floor
        }, friction=0.2, restitution=0.6)

    Keys: ``velocity`` (default: the velocity of a simulation already driving
    the property, else 0), ``acceleration`` (units/s², default 0), ``min`` /
    ``max`` bounds (hitting one reflects the motion, scaled by
    ``restitution``), and per-property ``friction`` / ``restitution``
    overrides. ``friction`` (1/s) applies exponential drag: velocity decays by
    ``e^(-friction·t)`` and terminal velocity is ``acceleration / friction``.
    ``restitution`` is 0 (stop dead) … 1 (perfectly elastic).
    ``targets`` is a dict, an object or a list of them. ``on_update``/``on_complete`` are called every frame while moving / once at rest (with the controller if they take an argument).

    Integration is exact (frame-rate independent). A simulation finishes when
    it comes to rest (slower than ``rest_speed``, default 0.05 units/s, with no
    acceleration or resting on a bound it is pushed against) or after ``life``
    seconds. Motion with acceleration and no bound in its direction, or
    perfectly elastic bounces without friction, never rests: pass ``life``.

    Numeric properties and unit strings (``"10px"``) are supported. Returns a
    :class:`PhysicsAnimation`.
    """
    engine = resolve_engine(engine)
    tl = as_targets(targets)
    if not isinstance(props, Mapping) or not props:
        raise ValueError("props must be a non-empty mapping of property -> velocity or options")
    if life is not None and not float(life) > 0:
        raise ValueError("life must be > 0 seconds (or None for unlimited)")
    params: Dict[str, Any] = dict(
        friction=friction,
        restitution=restitution,
        life=life,
        rest_speed=rest_speed,
        on_update=on_update,
        on_complete=on_complete,
    )
    hooks = Hooks(on_update=on_update, on_complete=on_complete)
    ctrl = PhysicsAnimation(engine, hooks, params, tl)
    core = engine.core
    life_ms = None if life is None else float(life) * 1000.0
    n = len(tl)
    for i, t in enumerate(tl):
        for key, spec in props.items():
            spec = resolve_callable(spec, t, i, n)
            opts = dict(spec) if isinstance(spec, Mapping) else {"velocity": spec}
            unknown = set(opts) - _PHYSICS_KEYS
            if unknown:
                raise TypeError(f"unexpected physics option(s) for {key!r}: {sorted(unknown)}")
            cur = read(t, key)
            if cur is MISSING:
                cur = 0.0
            res = resolve_values([cur, cur], cur, 0)
            if not isinstance(res.kind, (NumberKind, UnitKind)):
                raise TypeError(f"physics needs a numeric property, {key!r} is {cur!r}")
            pos = res.rows[0][0]
            existing = engine._sims.get((id(t), key))
            v = opts.get("velocity")
            if v is None:
                v = 0.0
                if existing is not None and core.is_alive(existing.handle):
                    v = core.sim_velocity(existing.handle)[0]
            acc = float(opts.get("acceleration") or 0.0)
            lo = opts.get("min")
            hi = opts.get("max")
            if lo is not None and hi is not None and float(lo) > float(hi):
                raise ValueError(f"min > max for {key!r}")
            fr = float(opts.get("friction", friction))
            rs = float(opts.get("restitution", restitution))
            h = core.create_physics(
                [pos],
                [float(v)],
                [acc],
                fr,
                None if lo is None else [float(lo)],
                None if hi is None else [float(hi)],
                rs,
                life_ms,
            )
            if existing is not None:
                engine._release_sim(existing, completed=False)
            sim = _install(engine, ctrl, t, key, res.kind, h, hooks)
            sim.ballistic = True
            if rest_speed is not None:
                core.sim_set_rest(sim.handle, 0.005, float(rest_speed))
    return ctrl
