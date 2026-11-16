/**
 * Timelines: sequence and overlap animations with GSAP-style position
 * parameters, labels, callbacks and nesting. Each timeline is a single group
 * node in the core; children are rendered in order (reverse order when
 * playing backwards) so overlapping writes resolve correctly in both
 * directions.
 *
 * ```ts
 * const tl = timeline({ defaults: { ease: 'outCubic', duration: 0.4 } })
 *   .add('.title', { y: [30, 0], opacity: [0, 1] })
 *   .add('.item', { x: [-20, 0], opacity: [0, 1] }, { delay: stagger(0.05) }, '-=0.2')
 *   .label('spin')
 *   .add('.logo', { rotate: 360 }, {}, 'spin')
 *   .call(() => console.log('done'));
 * ```
 */
import { core, whenReady, W } from './wasm';
import { register, unregister, easingId, NodeRecord, EasingInput, EV_CALLBACK, FL_FILL_BACKWARDS, FL_FILL_FORWARDS, LINEAR, wake, onFlush } from './core';
import { Animation, AnimateOptions, Props, Keyframe, Chain, PropValue } from './animate';
import type { Targets as _T } from './props';
import { Playable, Callbacks, Linked } from './playable';
import { Targets } from './props';
import { warn, warningsOn, checkKeys, checkSeconds } from './debug';
import type { OptionKeys } from './debug';

/**
 * Where to place a child (seconds):
 * - `number` absolute time
 * - `'<'` / `'>'` start / end of the previous child, optionally with an offset (`'<0.2'`, `'>-0.1'`)
 * - `'+=0.5'` / `'-=0.5'` relative to the end of the timeline
 * - `'label'`, `'label+=0.3'`, `'label-=0.3'`
 */
export type Position = number | string;

export interface TimelineOptions extends Callbacks<Timeline> {
  /** Seconds before the timeline starts. Default 0. */
  delay?: number;
  /** Extra iterations (`Infinity` / -1 loops forever). */
  repeat?: number;
  /** Pause after each iteration (seconds). */
  repeatDelay?: number;
  /** Play every other iteration backwards (with `repeat`). Default false. */
  yoyo?: boolean;
  /** Playback rate. Default 1; negative plays backwards. */
  speed?: number;
  /** Easing applied to the timeline's overall progress. */
  ease?: EasingInput;
  /** Start automatically (default true). */
  autoplay?: boolean;
  /** Default options for every child added with `add()` / `to()` / `from()`, e.g. `{ duration: 0.4, ease: 'outCubic' }`. */
  defaults?: AnimateOptions;
}

const TIMELINE_KEYS: OptionKeys<TimelineOptions> = {
  delay: 0, repeat: 0, repeatDelay: 0, yoyo: 0, speed: 0, ease: 0, autoplay: 0, defaults: 0,
  onStart: 0, onUpdate: 0, onComplete: 0, onRepeat: 0, onReverseComplete: 0,
};

type Child = { ctl: Playable | null; start: number; end: number; h: number };

/**
 * Timelines with an `onUpdate` callback. A group node has no values of its
 * own, so nothing marks it updated: once per flush (after the frame's DOM
 * writes), each one whose time moved is queued for `onUpdate`. Only attached
 * (playing or recently seeked) timelines are in the set.
 */
// (An array with an index loop: flush must not allocate.)
const updating: Timeline[] = [];
function watch(tl: Timeline, on: boolean): void {
  const i = updating.indexOf(tl);
  if (on && i < 0) updating.push(tl);
  else if (!on && i >= 0) updating.splice(i, 1);
}
onFlush(() => {
  const n = updating.length;
  if (n === 0) return;
  const w = W;
  if (!w) return;
  for (let i = 0; i < n; i++) {
    const tl = updating[i];
    if (!tl._h) continue;
    const t = w.ak_time(tl._h);
    if (t !== tl._lastTime) {
      tl._lastTime = t;
      tl._touch();
    }
  }
});

export class Timeline extends Playable {
  readonly labels = new Map<string, number>();
  private _chain: Chain = new WeakMap();
  private _children: Child[] = [];
  private _cleanups: Array<() => void> = [];
  private _last: Child | null = null;
  private _endTime = 0;
  private _spacer = 0;
  /** Time (ms) at the last `onUpdate` check. */
  _lastTime = NaN;
  private _opts: TimelineOptions;
  /** Set when nested inside another timeline (suppresses autoplay). */
  _nested = false;

  constructor(options: TimelineOptions = {}) {
    super(options);
    this._opts = options;
    if (warningsOn()) {
      checkKeys('timeline()', options, TIMELINE_KEYS);
      checkSeconds('timeline()', 'delay', options.delay);
      checkSeconds('timeline()', 'repeatDelay', options.repeatDelay);
      if (options.defaults && 'at' in options.defaults) warn('timeline(): "at" is a sequence() step option, not a default; it is ignored.');
    }
    whenReady(() => {
      const o = options;
      const repeat = o.repeat === undefined ? 0 : o.repeat < 0 ? Infinity : o.repeat;
      const ease = o.ease === undefined ? -1 : easingId(o.ease);
      const w = core();
      this._h = w.ak_group_create(
        (o.delay ?? 0) * 1000, 0, repeat + 1, o.yoyo ? 2 : 0, 0, ease === LINEAR ? -1 : ease,
        FL_FILL_BACKWARDS | FL_FILL_FORWARDS, 1,
      );
      const rec: NodeRecord & Linked = { h: this._h, at: 0, ctl: this, event: (k: number) => rec.ctl?._event(k) };
      register(rec);
      this._links.push(rec);
      if (o.onUpdate) watch(this, true);
      if (o.speed !== undefined) w.ak_set_rate(this._h, o.speed);
      if (o.repeatDelay) {
        // An invisible marker keeps the group's (auto) duration past the content.
        this._spacer = w.ak_callback_create();
      }
    });
    if (options.autoplay !== false) {
      // Autoplay after the synchronous `.add()` chain has been queued.
      queueMicrotask(() => {
        if (!this._nested) this.play();
      });
    }
  }

  /** Total duration of the content in seconds (without repeats). */
  get contentDuration(): number {
    return this._endTime;
  }

  private _pos(pos: Position | undefined): number {
    const end = this._endTime;
    if (pos === undefined) return end;
    if (typeof pos === 'number') return Math.max(0, pos);
    const s = pos.trim();
    const last = this._last;
    let m = /^([<>])\s*([+-]?=?\s*[\d.]+)?$/.exec(s);
    if (m) {
      const base = m[1] === '<' ? (last ? last.start : 0) : last ? last.end : end;
      return Math.max(0, base + (m[2] ? parseFloat(m[2].replace('=', '').replace(/\s/g, '')) : 0));
    }
    m = /^([+-])=\s*([\d.]+)$/.exec(s);
    if (m) return Math.max(0, end + (m[1] === '-' ? -1 : 1) * parseFloat(m[2]));
    m = /^(.*?)(?:([+-])=\s*([\d.]+))?$/.exec(s);
    if (m) {
      const name = m[1].trim();
      if (!this.labels.has(name)) this.labels.set(name, end);
      const base = this.labels.get(name)!;
      return Math.max(0, base + (m[2] ? (m[2] === '-' ? -1 : 1) * parseFloat(m[3]) : 0));
    }
    return end;
  }

  private _place(child: Playable, startSec: number): void {
    const w = core();
    w.ak_group_add(this._h, child._h, startSec * 1000);
    const dur = w.ak_duration(child._h) / 1000;
    const c: Child = { ctl: child, start: startSec, end: startSec + dur, h: child._h };
    this._children.push(c);
    this._last = c;
    if (c.end > this._endTime) this._endTime = c.end;
    this._updateSpacer();
  }

  private _updateSpacer(): void {
    if (!this._spacer) return;
    const w = core();
    const at = (this._endTime + (this._opts.repeatDelay ?? 0)) * 1000;
    if (w.ak_group_add(this._h, this._spacer, at) !== 0) w.ak_set_start(this._spacer, at);
  }

  /**
   * Add an animation (or a nested timeline / existing animation / function).
   * `add(targets, keyframes, options?, position?)` (or `add(targets, keyframes, position)`),
   * `add(animation, position?)`,
   * `add(() => {...}, position?)`.
   */
  add(targets: Targets | Playable | (() => void), keyframes?: Props | Keyframe[] | Position, options?: AnimateOptions | Position, position?: Position): this {
    if (typeof targets === 'function') return this.call(targets as () => void, keyframes as Position);
    // GSAP habit: `tl.add(el, { x: 100 }, '-=0.2')`; a position in the options slot.
    if (typeof options === 'string' || typeof options === 'number') {
      if (position !== undefined) warn(`timeline.add(): got two positions (${JSON.stringify(options)} and ${JSON.stringify(position)}); the third argument should be the options object.`);
      position = options;
      options = undefined;
    }
    if (targets instanceof Playable) {
      const child = targets;
      const pos = keyframes as Position | undefined;
      if (child instanceof Timeline) child._nested = true;
      whenReady(() => {
        if (!this._h || !child._h) return;
        this._place(child, this._pos(pos));
        this._cleanups.push(() => child.kill());
      });
      return this;
    }
    const opts = { ...this._opts.defaults, ...options };
    const anim = new Animation(targets as Targets, keyframes as Props, opts, { child: true, chain: this._chain });
    whenReady(() => {
      if (!this._h) return;
      if (!anim._h) return; // nothing to animate
      this._place(anim, this._pos(position));
      this._cleanups.push(() => anim.kill());
    });
    return this;
  }

  /** Alias of {@link add}. */
  to(targets: Targets, keyframes: Props | Keyframe[], options?: AnimateOptions | Position, position?: Position): this {
    return this.add(targets, keyframes, options, position);
  }

  /** Animate from the given values to the current/chained ones. */
  from(targets: Targets, values: Props, options: AnimateOptions | Position = {}, position?: Position): this {
    if (typeof options !== 'object') [position, options] = [options, {}];
    const kf: Props = {};
    for (const k of Object.keys(values)) kf[k] = [values[k], null] as PropValue;
    return this.add(targets, kf, { immediateRender: true, ...options }, position);
  }

  fromTo(targets: Targets, fromValues: Props, toValues: Props, options: AnimateOptions | Position = {}, position?: Position): this {
    if (typeof options !== 'object') [position, options] = [options, {}];
    const kf: Props = {};
    for (const k of Object.keys(toValues)) kf[k] = k in fromValues ? ([fromValues[k], toValues[k]] as PropValue) : toValues[k];
    return this.add(targets, kf, { immediateRender: true, ...options }, position);
  }

  /** Set values instantly at a position. */
  set(targets: Targets, values: Props, position?: Position): this {
    return this.add(targets, values, { duration: 0, ease: 'linear' }, position);
  }

  /** Call `fn` when playback crosses `position` (in either direction). */
  call(fn: () => void, position?: Position): this {
    whenReady(() => {
      if (!this._h) return;
      const w = core();
      const h = w.ak_callback_create();
      const start = this._pos(position);
      w.ak_group_add(this._h, h, start * 1000);
      const rec: NodeRecord = { h, at: 0, event: (k) => k === EV_CALLBACK && fn() };
      register(rec);
      const c: Child = { ctl: null, start, end: start, h };
      this._children.push(c);
      this._last = c;
      if (start > this._endTime) {
        this._endTime = start;
        this._updateSpacer();
      }
      this._cleanups.push(() => {
        unregister(rec);
        core().ak_release(h);
      });
    });
    return this;
  }

  /** Add a label at a position (default: the current end). */
  label(name: string, position?: Position): this {
    whenReady(() => this.labels.set(name, this._pos(position)));
    return this;
  }

  /** Seek to seconds or a label. */
  override seek(time: number | string): this {
    if (typeof time === 'string') {
      whenReady(() => super.seek(this.labels.get(time) ?? 0));
      return this;
    }
    return super.seek(time);
  }

  /** Play from a time/label. */
  playFrom(time: number | string): this {
    this.seek(time);
    this.play();
    wake();
    return this;
  }

  /** Revert every child (restoring original values), newest first. */
  override revert(): void {
    whenReady(() => {
      const kids = this._children.slice().reverse();
      for (const c of kids) c.ctl?.revert();
      this.kill();
    });
  }

  /** Child animations / timelines in order of insertion. */
  getChildren(): Playable[] {
    return this._children.map((c) => c.ctl).filter((c): c is Playable => c !== null);
  }

  /** Remove (and kill) a child animation or timeline. */
  remove(child: Playable): this {
    whenReady(() => {
      const i = this._children.findIndex((c) => c.ctl === child);
      if (i < 0) return;
      this._children.splice(i, 1);
      child.kill();
      this._endTime = this._children.reduce((m, c) => Math.max(m, c.end), 0);
      this._last = this._children[this._children.length - 1] ?? null;
    });
    return this;
  }

  /** Kill every child (labels are kept). */
  clear(): this {
    whenReady(() => {
      for (const c of this._children.splice(0)) {
        if (c.ctl) c.ctl.kill();
        else core().ak_release(c.h);
      }
      this._endTime = 0;
      this._last = null;
    });
    return this;
  }

  /** The most recent label at or before the playhead. */
  get currentLabel(): string | undefined {
    return this._labelAround(this.time, -1, true);
  }
  /** Label after `time` (default: the playhead). */
  nextLabel(time = this.time): string | undefined {
    return this._labelAround(time, 1, false);
  }
  /** Label before `time` (default: the playhead). */
  previousLabel(time = this.time): string | undefined {
    return this._labelAround(time, -1, false);
  }
  private _labelAround(time: number, dir: number, inclusive: boolean): string | undefined {
    let best: [string, number] | undefined;
    for (const [name, t] of this.labels) {
      const ok = dir > 0 ? t > time + 1e-9 : inclusive ? t <= time + 1e-9 : t < time - 1e-9;
      if (!ok) continue;
      if (!best || (dir > 0 ? t < best[1] : t > best[1])) best = [name, t];
    }
    return best?.[0];
  }

  /**
   * Animate the playhead to a time/label (pausing the timeline), e.g. for
   * step-through UIs: `tl.tweenTo('step2', { duration: 0.6, ease: 'inOutCubic' })`.
   */
  tweenTo(position: number | string, options: AnimateOptions = {}): Animation {
    return this.tweenFromTo(null, position, options);
  }
  /** Animate the playhead between two times/labels. */
  tweenFromTo(from: number | string | null, to: number | string, options: AnimateOptions = {}): Animation {
    this.pause();
    const sec = (p: number | string) => (typeof p === 'number' ? p : this.labels.get(p) ?? 0);
    const proxy = { t: from === null ? this.time : sec(from) };
    const target = sec(to);
    const dur = options.duration ?? Math.max(0.001, Math.abs(target - proxy.t) / Math.abs(this.speed || 1));
    const user = options.onUpdate;
    return new Animation(proxy, { t: [proxy.t, target] }, {
      ease: 'linear', ...options, duration: dur, overwrite: false,
      onUpdate: (a) => {
        this.seek(proxy.t);
        user?.(a);
      },
    });
  }

  override _attach(): void {
    super._attach();
    if (this._cb.onUpdate && this._h) watch(this, true);
  }
  protected override _detach(): void {
    super._detach();
    watch(this, false);
  }

  protected _kill(): void {
    watch(this, false);
    // Release the whole subtree first, in one pass. The children's own kills
    // below then find their nodes gone (stale handles are harmless) instead
    // of each detaching from this group and recomputing its duration over the
    // remaining children: O(n²) for n children.
    if (this._h) core().ak_release(this._h);
    this._h = 0;
    for (const c of this._cleanups) c();
    this._cleanups = [];
    for (const l of this._links) unregister(l as unknown as NodeRecord);
  }

  protected _cleanup(): () => void {
    const cleanups = this._cleanups;
    const links = this._links.slice();
    const h = this._h;
    return () => {
      if (h) core().ak_release(h);
      for (const c of cleanups) c();
      for (const l of links) unregister(l as unknown as NodeRecord);
    };
  }
}

/** Create a timeline. */
export function timeline(options?: TimelineOptions): Timeline {
  return new Timeline(options);
}

/** One step of a {@link sequence}: `[targets, keyframes, options & { at? }]`, a label, or a function. */
export type SequenceStep =
  | [_T, Props | Keyframe[], (AnimateOptions & { at?: Position })?]
  | [Playable, { at?: Position }?]
  | [() => void, { at?: Position }?]
  | string;

/**
 * Declarative timeline from an array (Motion-style):
 *
 * ```ts
 * sequence([
 *   ['.a', { x: 100 }],
 *   ['.b', { opacity: [0, 1] }, { at: '<' }],
 *   'middle',
 *   ['.c', { scale: 2 }, { at: 'middle+=0.2', duration: 1 }],
 *   [() => console.log('done')],
 * ], { defaults: { ease: 'outCubic' } });
 * ```
 */
export function sequence(steps: SequenceStep[], options?: TimelineOptions): Timeline {
  const tl = new Timeline(options);
  for (const step of steps) {
    if (typeof step === 'string') {
      tl.label(step);
      continue;
    }
    const [a, b, c] = step as any[];
    if (typeof a === 'function') tl.call(a, b?.at);
    else if (a instanceof Playable) tl.add(a, b?.at);
    else {
      const { at, ...opts } = (c ?? {}) as AnimateOptions & { at?: Position };
      tl.add(a, b, opts, at);
    }
  }
  return tl;
}

/** Call `fn` after `seconds` (engine time: respects global speed and pausing). */
export function delayedCall(seconds: number, fn: () => void): Timeline {
  return new Timeline().call(fn, seconds);
}
