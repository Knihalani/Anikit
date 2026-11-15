/**
 * Shared controller behaviour for everything with a timeline in the core
 * (animations and timelines): playback control, promises, callbacks, and
 * memory management.
 *
 * Memory model: while playing, the engine's records hold the controller
 * strongly (callbacks must fire even for fire-and-forget animations). When a
 * root finishes, records drop to a weak reference; if user code no longer
 * references the controller, a FinalizationRegistry frees the engine nodes.
 * Animations whose properties are all taken over by newer animations are
 * freed immediately.
 */
import { core, whenReady, W } from './wasm';
import { flush, wake, EV_START, EV_REPEAT, EV_COMPLETE, EV_REVERSE_COMPLETE, EV_FINISH } from './core';

export type PlayState = 'idle' | 'running' | 'paused' | 'finished';
const STATES: PlayState[] = ['idle', 'running', 'paused', 'finished'];

export interface Callbacks<T> {
  onStart?: (self: T) => void;
  onUpdate?: (self: T) => void;
  onComplete?: (self: T) => void;
  onRepeat?: (self: T) => void;
  onReverseComplete?: (self: T) => void;
}

/** A record that can point back at its controller. */
export interface Linked {
  ctl: Playable | null;
}

const hasWeak = typeof WeakRef !== 'undefined' && typeof FinalizationRegistry !== 'undefined';
const registry = hasWeak ? new FinalizationRegistry<() => void>((cleanup) => cleanup()) : null;

export abstract class Playable implements PromiseLike<void> {
  /** Root handle in the core (0 until built / after kill). */
  _h = 0;
  _cb: Callbacks<any>;
  _links: Linked[] = [];
  _dirty = false;
  _killed = false;
  private _registered = false;
  private _done!: Promise<void>;
  private _resolve!: () => void;
  private _settled = false;

  constructor(cb: Callbacks<any>) {
    this._cb = cb;
    this._fresh();
  }

  private _fresh(): void {
    this._settled = false;
    this._done = new Promise<void>((r) => (this._resolve = r));
  }

  /** Resolves when the animation finishes playing (re-armed on replay). */
  get finished(): Promise<void> {
    return this._done;
  }
  then<A = void, B = never>(
    ok?: ((v: void) => A | PromiseLike<A>) | null,
    err?: ((e: unknown) => B | PromiseLike<B>) | null,
  ): Promise<A | B> {
    return this._done.then(ok, err);
  }

  /** Run `fn` against the core once built and alive. */
  protected _op(fn: (h: number) => void): this {
    whenReady(() => {
      if (this._h && !this._killed) fn(this._h);
    });
    return this;
  }

  /** Re-attach strongly (called whenever playback may produce callbacks). */
  _attach(): void {
    for (const l of this._links) l.ctl = this;
    if (this._settled) this._fresh();
  }

  /** Drop to a weak link so an unreferenced, finished controller can be collected. */
  protected _detach(): void {
    if (!hasWeak) return;
    for (const l of this._links) l.ctl = null;
    if (!this._registered) {
      this._registered = true;
      registry!.register(this, this._cleanup(), this);
    }
  }

  /** Cleanup closure for GC (must not capture `this`). */
  protected abstract _cleanup(): () => void;

  play(): this {
    return this._op((h) => {
      this._attach();
      core().ak_play(h);
      wake();
    });
  }
  pause(): this {
    return this._op((h) => core().ak_pause(h));
  }
  /** Flip the playback direction (and resume if it was not paused). */
  reverse(): this {
    return this._op((h) => {
      this._attach();
      core().ak_reverse(h);
      wake();
    });
  }
  restart(): this {
    return this._op((h) => {
      this._attach();
      core().ak_restart(h);
      flush();
      wake();
    });
  }
  /** Jump to `time` seconds and render immediately. */
  seek(time: number): this {
    return this._op((h) => {
      this._attach();
      core().ak_seek(h, time * 1000);
      flush();
    });
  }
  /** Jump to the end. */
  complete(): this {
    return this._op((h) => {
      this._attach();
      core().ak_seek(h, core().ak_duration(h));
      flush();
    });
  }
  /** Return to the start and stop. */
  cancel(): this {
    return this._op((h) => {
      core().ak_pause(h);
      core().ak_seek(h, 0);
      flush();
    });
  }

  get progress(): number {
    return W && this._h ? W.ak_progress(this._h) || 0 : 0;
  }
  set progress(p: number) {
    this._op((h) => {
      this._attach();
      core().ak_set_progress(h, p);
      flush();
    });
  }
  /** Current time in seconds. */
  get time(): number {
    return W && this._h ? (W.ak_time(this._h) || 0) / 1000 : 0;
  }
  set time(t: number) {
    this.seek(t);
  }
  /** Total duration in seconds (delay + repeats included). */
  get duration(): number {
    return W && this._h ? (W.ak_duration(this._h) || 0) / 1000 : 0;
  }
  /** Playback rate; negative plays backwards. */
  get speed(): number {
    return W && this._h ? W.ak_rate(this._h) : 1;
  }
  set speed(r: number) {
    this._op((h) => core().ak_set_rate(h, r));
  }
  get state(): PlayState {
    if (!W || !this._h || this._killed) return 'idle';
    return STATES[W.ak_state(this._h)] ?? 'idle';
  }
  /** Current iteration (0-based). */
  get iteration(): number {
    return W && this._h ? W.ak_iteration(this._h) || 0 : 0;
  }

  /** Stop and free everything (the controller becomes inert). */
  kill(): void {
    whenReady(() => {
      if (this._killed) return;
      this._killed = true;
      this._kill();
      if (this._registered) registry!.unregister(this);
      this._finish();
    });
  }
  /** Alias of {@link kill}. */
  stop(): void {
    this.kill();
  }
  protected abstract _kill(): void;

  _finish(): void {
    if (!this._settled) {
      this._settled = true;
      this._resolve();
    }
  }

  /** Mark updated this frame; `onUpdate` fires once per frame after DOM writes. */
  _touch(): void {
    if (!this._dirty && this._cb.onUpdate) {
      this._dirty = true;
      queueUpdate(this);
    }
  }

  /** Dispatch a root event from the core. */
  _event(kind: number): void {
    const cb = this._cb;
    switch (kind) {
      case EV_START:
        cb.onStart?.(this);
        break;
      case EV_REPEAT:
        cb.onRepeat?.(this);
        break;
      case EV_COMPLETE:
        cb.onComplete?.(this);
        break;
      case EV_REVERSE_COMPLETE:
        cb.onReverseComplete?.(this);
        break;
      case EV_FINISH:
        this._finish();
        this._detach();
        break;
    }
  }
}

let pending: Playable[] = [];
let scheduled = false;
import { defer } from './core';
function queueUpdate(p: Playable): void {
  pending.push(p);
  if (!scheduled) {
    scheduled = true;
    defer(runUpdates);
  }
}
function runUpdates(): void {
  scheduled = false;
  const list = pending;
  pending = [];
  for (const p of list) {
    p._dirty = false;
    p._cb.onUpdate?.(p);
  }
}
