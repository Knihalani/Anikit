/**
 * `batch()` — instanced animation for canvas / WebGL / data-driven visuals.
 *
 * One core node animates `count` instances (each with its own values and
 * start delay, sharing timing and easing) into a single contiguous
 * `Float32Array`. JavaScript does no per-instance work at all: hand
 * `batch.data` straight to `gl.bufferSubData`, or read it in your draw loop.
 *
 * ```ts
 * const b = batch(100_000, {
 *   x: (i) => [Math.random() * w, Math.random() * w],
 *   y: (i) => [h + 10, Math.random() * h],
 *   size: [0, 4],
 * }, { duration: 2, ease: 'outCubic', delay: stagger(0.00002), onUpdate: () => draw(b.data) });
 * ```
 */
import { core, whenReady, F64, f64Buf, W } from './wasm';
import { register, unregister, wake, easingId, EasingInput, LINEAR, NodeRecord, FL_FILL_BACKWARDS, FL_FILL_FORWARDS } from './core';
import { Playable, Callbacks, Linked } from './playable';
import { checkKeys, checkSeconds } from './debug';
import type { OptionKeys } from './debug';

/** Keyframes for one channel: shared list, or a function of the instance index. */
export type BatchValue = number | number[] | ((index: number, count: number) => number | number[]);

export interface BatchOptions extends Callbacks<Batch> {
  /** Seconds per instance (default 0.5). */
  duration?: number;
  /** Per-instance start delay in seconds: number, `stagger()` or `(i, count) => s`. */
  delay?: number | ((target: unknown, index: number, count: number) => number);
  ease?: EasingInput | EasingInput[];
  repeat?: number;
  yoyo?: boolean;
  times?: number[];
  autoplay?: boolean;
  speed?: number;
}

const BATCH_KEYS: OptionKeys<BatchOptions> = {
  duration: 0, delay: 0, ease: 0, repeat: 0, yoyo: 0, times: 0, autoplay: 0, speed: 0,
  onStart: 0, onUpdate: 0, onComplete: 0, onRepeat: 0, onReverseComplete: 0,
};

export class Batch extends Playable {
  /** Channel index of each property within an instance's stride. */
  readonly channels: Record<string, number> = {};
  /** Values per instance. */
  stride = 0;
  readonly count: number;
  private _view: Float32Array = new Float32Array(0);
  private _buf: ArrayBuffer | null = null;

  constructor(count: number, props: Record<string, BatchValue>, o: BatchOptions = {}) {
    super(o);
    checkKeys('batch()', o, BATCH_KEYS);
    checkSeconds('batch()', 'duration', o.duration);
    this.count = count;
    const keys = Object.keys(props);
    keys.forEach((k, i) => (this.channels[k] = i));
    this.stride = keys.length;
    whenReady(() => this._build(count, keys, props, o));
  }

  private _build(n: number, keys: string[], props: Record<string, BatchValue>, o: BatchOptions): void {
    const ch = keys.length;
    // Resolve keyframes per channel per instance; nkf = longest list.
    const per: (number[] | null)[] = keys.map((k) => {
      const v = props[k];
      return typeof v === 'function' ? null : Array.isArray(v) ? v : [v, v];
    });
    let nkf = 2;
    for (const p of per) if (p) nkf = Math.max(nkf, p.length);
    const fns = keys.map((k) => props[k]);
    if (fns.some((f) => typeof f === 'function')) {
      const probe = (fns.find((f) => typeof f === 'function') as (i: number, n: number) => number | number[])(0, n);
      if (Array.isArray(probe)) nkf = Math.max(nkf, probe.length);
    }
    const easeList = Array.isArray(o.ease) && typeof o.ease[0] !== 'number' ? (o.ease as EasingInput[]) : null;
    const eases = easeList
      ? Array.from({ length: nkf - 1 }, (_, k) => easingId(easeList[k] ?? easeList[easeList.length - 1]))
      : [easingId(o.ease as EasingInput)];
    const offs = o.times && o.times.length === nkf ? o.times : null;
    const delayFn = o.delay;
    const hasDelays = typeof delayFn === 'function' || (typeof delayFn === 'number' && delayFn !== 0);
    // Resolve everything user-provided BEFORE touching the scratch buffer:
    // user functions (e.g. stagger) may call into the core and reuse it.
    const delays = hasDelays
      ? Array.from({ length: n }, (_, i) => (typeof delayFn === 'function' ? delayFn(null, i, n) : (delayFn as number)) * 1000)
      : [];
    const values = new Float64Array(n * nkf * ch);
    let vi = 0;
    for (let i = 0; i < n; i++) {
      const rows: number[][] = keys.map((_k, c) => {
        const p = per[c];
        if (p) return p;
        const v = (fns[c] as (i: number, n: number) => number | number[])(i, n);
        return Array.isArray(v) ? v : [v, v];
      });
      for (let kf = 0; kf < nkf; kf++) {
        for (let c = 0; c < ch; c++) {
          const r = rows[c];
          values[vi++] = r[Math.min(kf, r.length - 1)];
        }
      }
    }
    const len = 10 + (offs ? nkf : 0) + 1 + eases.length + 1 + (hasDelays ? n : 0) + n * nkf * ch;
    let at = f64Buf(len);
    const repeat = o.repeat === undefined ? 0 : o.repeat < 0 ? Infinity : o.repeat;
    const f = F64;
    f[at++] = (o.duration ?? 0.5) * 1000;
    f[at++] = repeat + 1;
    f[at++] = o.yoyo ? 2 : 0;
    f[at++] = 0;
    f[at++] = -1;
    f[at++] = FL_FILL_BACKWARDS | FL_FILL_FORWARDS;
    f[at++] = n;
    f[at++] = ch;
    f[at++] = nkf;
    f[at++] = offs ? 1 : 0;
    if (offs) for (const x of offs) f[at++] = x;
    f[at++] = eases[0] === LINEAR && eases.length === 1 ? 0 : eases.length;
    if (!(eases[0] === LINEAR && eases.length === 1)) for (const e of eases) f[at++] = e === LINEAR ? -1 : e;
    f[at++] = hasDelays ? 1 : 0;
    for (let i = 0; i < delays.length; i++) f[at++] = delays[i];
    // Values: instance-major, keyframe-major, channel-minor.
    f.set(values, at);
    const w = core();
    this._h = w.ak_batch_create(len);
    if (!this._h) throw new Error('anikit: invalid batch');
    const rec: NodeRecord & Linked = {
      h: this._h, at: 0, ctl: this,
      render: () => rec.ctl?._touch(),
      event: (k: number) => rec.ctl?._event(k),
    };
    // `at` is unused for batches (output is f32); keep render for onUpdate.
    register(rec);
    this._links.push(rec);
    if (o.speed !== undefined) w.ak_set_rate(this._h, o.speed);
    if (o.autoplay !== false) {
      w.ak_play(this._h);
      wake();
    }
  }

  /** Live `Float32Array` of `count * stride` values (instance-major). */
  get data(): Float32Array {
    if (!W || !this._h) return this._view;
    const b = W.memory.buffer as ArrayBuffer;
    if (b !== this._buf || this._view.length === 0) {
      this._buf = b;
      this._view = new Float32Array(b, W.ak_batch_ptr(this._h), this.count * this.stride);
    }
    return this._view;
  }

  /** Current value of `prop` for instance `i`. */
  get(i: number, prop: string): number {
    return this.data[i * this.stride + this.channels[prop]];
  }

  protected _kill(): void {
    for (const l of this._links) unregister(l as unknown as NodeRecord);
    if (this._h) core().ak_release(this._h);
    this._h = 0;
  }
  protected _cleanup(): () => void {
    const links = this._links.slice();
    const h = this._h;
    return () => {
      for (const l of links) unregister(l as unknown as NodeRecord);
      if (h) core().ak_release(h);
    };
  }
}

/** Animate `count` instances into one contiguous Float32Array. */
export function batch(count: number, props: Record<string, BatchValue>, options?: BatchOptions): Batch {
  return new Batch(count, props, options);
}
