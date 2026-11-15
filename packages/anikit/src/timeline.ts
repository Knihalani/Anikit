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
import { core, whenReady } from './wasm';
import { register, unregister, easingId, NodeRecord, EasingInput, EV_CALLBACK, FL_FILL_BACKWARDS, FL_FILL_FORWARDS, LINEAR, wake } from './core';
import { Animation, AnimateOptions, Props, Keyframe, Chain, PropValue } from './animate';
import { Playable, Callbacks, Linked } from './playable';
import { Targets } from './props';

/**
 * Where to place a child (seconds):
 * - `number` absolute time
 * - `'<'` / `'>'` start / end of the previous child, optionally with an offset (`'<0.2'`, `'>-0.1'`)
 * - `'+=0.5'` / `'-=0.5'` relative to the end of the timeline
 * - `'label'`, `'label+=0.3'`, `'label-=0.3'`
 */
export type Position = number | string;

export interface TimelineOptions extends Callbacks<Timeline> {
  delay?: number;
  /** Extra iterations (`Infinity` / -1 loops forever). */
  repeat?: number;
  /** Pause after each iteration (seconds). */
  repeatDelay?: number;
  yoyo?: boolean;
  speed?: number;
  /** Easing applied to the timeline's overall progress. */
  ease?: EasingInput;
  /** Start automatically (default true). */
  autoplay?: boolean;
  /** Default options for every child animation. */
  defaults?: AnimateOptions;
}

type Child = { ctl: Playable | null; start: number; end: number };

export class Timeline extends Playable {
  readonly labels = new Map<string, number>();
  private _chain: Chain = new WeakMap();
  private _children: Child[] = [];
  private _cleanups: Array<() => void> = [];
  private _last: Child | null = null;
  private _endTime = 0;
  private _spacer = 0;
  private _opts: TimelineOptions;
  /** Set when nested inside another timeline (suppresses autoplay). */
  _nested = false;

  constructor(options: TimelineOptions = {}) {
    super(options);
    this._opts = options;
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
    const c: Child = { ctl: child, start: startSec, end: startSec + dur };
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
   * `add(targets, keyframes, options?, position?)`, `add(animation, position?)`,
   * `add(() => {...}, position?)`.
   */
  add(targets: Targets | Playable | (() => void), keyframes?: Props | Keyframe[] | Position, options?: AnimateOptions, position?: Position): this {
    if (typeof targets === 'function') return this.call(targets as () => void, keyframes as Position);
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
  to(targets: Targets, keyframes: Props | Keyframe[], options?: AnimateOptions, position?: Position): this {
    return this.add(targets, keyframes, options, position);
  }

  /** Animate from the given values to the current/chained ones. */
  from(targets: Targets, values: Props, options: AnimateOptions = {}, position?: Position): this {
    const kf: Props = {};
    for (const k of Object.keys(values)) kf[k] = [values[k], null] as PropValue;
    return this.add(targets, kf, { immediateRender: true, ...options }, position);
  }

  fromTo(targets: Targets, fromValues: Props, toValues: Props, options: AnimateOptions = {}, position?: Position): this {
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
      const c: Child = { ctl: null, start, end: start };
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

  protected _kill(): void {
    for (const c of this._cleanups) c();
    this._cleanups = [];
    for (const l of this._links) unregister(l as unknown as NodeRecord);
    if (this._h) core().ak_release(this._h);
    this._h = 0;
  }

  protected _cleanup(): () => void {
    const cleanups = this._cleanups;
    const links = this._links.slice();
    const h = this._h;
    return () => {
      for (const c of cleanups) c();
      for (const l of links) unregister(l as unknown as NodeRecord);
      if (h) core().ak_release(h);
    };
  }
}

/** Create a timeline. */
export function timeline(options?: TimelineOptions): Timeline {
  return new Timeline(options);
}
