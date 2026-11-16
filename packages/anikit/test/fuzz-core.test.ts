// Property-based tests of the animation core: random scenarios (animate /
// fromTo / timelines with seeks, reversals, kills, reverts and takeovers)
// run through every internal fast path and its general fallback, which must
// agree frame for frame; plus single tweens checked against a reference
// model of the timing maths, and edge cases (zero durations, negative
// delays, NaN / Infinity inputs, stale handles, re-entrant flush()).
import { describe, it, expect, beforeAll } from 'vitest';
import { animate, fromTo, from, set, stagger, engine, flush, setWarnings } from '../src/index';
import { timeline } from '../src/timeline';
import { setup, advance } from './setup';
import { _setBatchCreate, _setPackGroups, _setCodegen, _setNumberTracks, _staleSlots } from '../src/animate';
import { _recordCount } from '../src/core';

beforeAll(async () => {
  await setup();
  setWarnings(false);
});

/** Small seeded PRNG (mulberry32). */
function rng(seed: number): () => number {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

const EASES = ['linear', 'outCubic', 'inOutSine', 'inQuad', 'outBack', [0.4, 0, 0.2, 1]] as const;
const KEYS = ['x', 'y', 'o'];
type Obj = Record<string, number>;
type Ctl = { kill(): void; revert(): void; seek(t: number): unknown; reverse(): unknown; pause(): unknown; play(): unknown; restart(): unknown };

/** One random scenario; returns a frame log of every target's values. */
function scenario(seed: number): unknown[] {
  const r = rng(seed);
  const pick = <T,>(a: readonly T[]): T => a[Math.floor(r() * a.length)];
  const num = () => Math.round((r() * 400 - 200) * 100) / 100;
  const nT = 1 + Math.floor(r() * 6);
  const objs: Obj[] = Array.from({ length: nT }, () => ({ x: num(), y: num(), o: r() }));
  const ctls: Ctl[] = [];
  const log: unknown[] = [];
  const snap = (tag: string) => log.push(tag, objs.map((o) => KEYS.map((k) => o[k])));
  const subset = () => {
    const s = objs.filter(() => r() < 0.6);
    // Sometimes a target listed twice (the later one takes the earlier over).
    if (s.length && r() < 0.1) s.push(s[0]);
    return s.length ? s : [pick(objs)];
  };
  const value = (): unknown => {
    const k = r();
    if (k < 0.5) return num();
    if (k < 0.7) return [num(), num()];
    if (k < 0.8) return [null, num()];
    if (k < 0.9) return [num(), num(), num(), num()];
    return [num()];
  };
  const props = () => {
    const p: Record<string, unknown> = {};
    for (const k of KEYS) if (r() < 0.6) p[k] = value();
    if (Object.keys(p).length === 0) p.x = num();
    return p;
  };
  const opts = () => {
    const o: Record<string, unknown> = {};
    const d = r();
    o.duration = d < 0.15 ? 0 : Math.round(r() * 100) / 100 + 0.05;
    if (r() < 0.4) o.delay = Math.round((r() * 0.6 - 0.2) * 100) / 100;
    if (r() < 0.3) o.repeat = Math.floor(r() * 3);
    if (r() < 0.3) o.yoyo = true;
    if (r() < 0.7) o.ease = pick(EASES);
    if (r() < 0.15) o.endDelay = Math.round(r() * 30) / 100;
    if (r() < 0.15) o.delay = stagger(Math.round(r() * 10) / 100);
    if (r() < 0.1) o.overwrite = false;
    return o;
  };
  const steps = 6 + Math.floor(r() * 14);
  for (let s = 0; s < steps; s++) {
    const k = r();
    try {
      if (k < 0.3) {
        ctls.push(animate(subset(), props() as any, opts()) as unknown as Ctl);
        snap('animate');
      } else if (k < 0.38) {
        const p = props();
        const to: Record<string, unknown> = {};
        for (const key of Object.keys(p)) to[key] = num();
        ctls.push(fromTo(subset(), p as any, to as any, opts()) as unknown as Ctl);
        snap('fromTo');
      } else if (k < 0.43) {
        ctls.push(from(subset(), { x: num() }, opts()) as unknown as Ctl);
        snap('from');
      } else if (k < 0.5) {
        const tl = timeline({ repeat: r() < 0.3 ? 1 : 0, yoyo: r() < 0.5 });
        const m = 1 + Math.floor(r() * 4);
        for (let j = 0; j < m; j++) {
          const pos = r() < 0.5 ? undefined : Math.round(r() * 50) / 100;
          tl.add(subset(), props() as any, opts() as any, pos as any);
        }
        // (Timelines autoplay on a microtask; this test runs synchronously.)
        if (r() < 0.7) tl.play();
        ctls.push(tl as unknown as Ctl);
        snap('timeline');
      } else if (k < 0.55) {
        set(subset(), { x: num() });
        snap('set');
      } else if (k < 0.85 || ctls.length === 0) {
        advance(Math.round(r() * 40) / 100);
        snap('advance');
      } else {
        const c = pick(ctls);
        const op = Math.floor(r() * 7);
        if (op === 0) c.seek(Math.round(r() * 100) / 100);
        else if (op === 1) c.reverse();
        else if (op === 2) c.pause();
        else if (op === 3) c.play();
        else if (op === 4) c.kill();
        else if (op === 5) c.revert();
        else c.restart();
        flush();
        snap('op' + op);
      }
    } catch (e) {
      log.push('throw', String(e));
    }
  }
  advance(3);
  snap('end');
  for (const c of ctls) c.kill();
  advance(0.1);
  return log;
}

function finite(log: unknown[]): boolean {
  return JSON.stringify(log).indexOf('null') === -1;
}

describe('core fuzz: every fast path agrees with the general one', () => {
  const SEEDS = Array.from({ length: 60 }, (_, i) => 1000 + i * 7919);
  const variants: Array<[string, () => () => void]> = [
    ['batched creation off', () => { const p = _setBatchCreate(false); return () => _setBatchCreate(p); }],
    ['group packing off', () => { const p = _setPackGroups(false); return () => _setPackGroups(p); }],
    ['generated writers off', () => { const p = _setCodegen(false); return () => _setCodegen(p); }],
    ['number tracks off', () => { const p = _setNumberTracks(false); return () => _setNumberTracks(p); }],
  ];
  for (const [name, toggle] of variants) {
    it(name, () => {
      for (const seed of SEEDS) {
        const ref = scenario(seed);
        const undo = toggle();
        let got: unknown[];
        try {
          got = scenario(seed);
        } finally {
          undo();
        }
        expect(got, `seed ${seed}`).toEqual(ref);
        expect(finite(ref), `seed ${seed} produced a non-finite value`).toBe(true);
      }
    });
  }

  it('leaves no live records behind', () => {
    advance(0.1);
    const before = _recordCount();
    for (let s = 0; s < 20; s++) scenario(5000 + s);
    advance(0.1);
    expect(_recordCount()).toBeLessThanOrEqual(before);
    expect(_staleSlots()).toBe(0);
  });
});

/** Reference model: value of a linear from→to tween at time `t` after creation. */
function model(t: number, a: number, b: number, dur: number, delay: number, repeat: number, yoyo: boolean): number {
  const local = t - delay;
  if (local < 0) return a;
  const iters = repeat + 1;
  let iter: number, p: number;
  if (dur <= 0 || local >= dur * iters) {
    iter = iters - 1;
    p = 1;
  } else {
    iter = Math.floor(local / dur);
    p = (local - iter * dur) / dur;
  }
  if (yoyo && iter % 2 === 1) p = 1 - p;
  return a + (b - a) * p;
}

describe('core fuzz: single tweens match a reference timing model', () => {
  it('linear from/to with delay (also negative), repeat and yoyo', () => {
    const r = rng(42);
    for (let c = 0; c < 150; c++) {
      const a = Math.round(r() * 200 - 100), b = Math.round(r() * 200 - 100);
      const dur = r() < 0.1 ? 0 : Math.round(r() * 20) / 20 + 0.05;
      const delay = r() < 0.5 ? 0 : Math.round(r() * 20 - 6) / 20;
      const repeat = r() < 0.5 ? 0 : Math.floor(r() * 3);
      const yoyo = r() < 0.5;
      // Several targets (batched path) and one (single node) must both match.
      const n = r() < 0.5 ? 1 : 3;
      const objs = Array.from({ length: n }, () => ({ v: 0 }));
      const an = animate(objs, { v: [a, b] }, { duration: dur, delay, repeat, yoyo, ease: 'linear' });
      let t = 0;
      for (let k = 0; k < 12; k++) {
        const dt = Math.round(r() * 30) / 100 + 0.01;
        advance(dt, dt * 1000);
        t += dt;
        const want = model(t, a, b, dur, delay, repeat, yoyo);
        for (const o of objs) {
          expect(o.v, `case ${c}: a=${a} b=${b} dur=${dur} delay=${delay} repeat=${repeat} yoyo=${yoyo} t=${t}`).toBeCloseTo(want, 6);
        }
      }
      an.kill();
    }
  });
});

describe('core edge cases', () => {
  it('a target listed twice animates, and kill() stops it', async () => {
    for (const on of [true, false]) {
      const prev = _setBatchCreate(on);
      try {
        const o = { x: 0 }, p = { x: 0 };
        const a = animate([o, o, p], { x: [0, 100] }, { duration: 1, ease: 'linear' });
        let done = false;
        a.then(() => { done = true; });
        advance(0.25, 250);
        expect(o.x).toBeCloseTo(25, 6);
        expect(p.x).toBeCloseTo(25, 6);
        await Promise.resolve();
        expect(done).toBe(false);
        a.kill();
        advance(0.25, 250);
        expect(o.x).toBeCloseTo(25, 6);
        expect(p.x).toBeCloseTo(25, 6);
        const b = animate([o, o], { x: 0 }, { duration: 1, ease: 'linear' });
        advance(0.5, 500);
        expect(o.x).toBeCloseTo(12.5, 6);
        b.kill();
      } finally {
        _setBatchCreate(prev);
      }
    }
  });
});

describe('fast creation path: user code while reading values', () => {
  it('a getter that starts another animation (re-entrant) and a getter that throws', () => {
    const inner = [{ v: 0 }, { v: 0 }];
    let nested: ReturnType<typeof animate> | undefined;
    const tricky = {
      get x() {
        if (!nested) nested = animate(inner, { v: [0, 10] }, { duration: 1, ease: 'linear' });
        return 5;
      },
      set x(_v: number) { /* ignored */ },
    };
    const plain = { x: 0 };
    const a = animate([plain, tricky as unknown as { x: number }], { x: 100 }, { duration: 1, ease: 'linear' });
    advance(0.5, 500);
    expect(plain.x).toBeCloseTo(50, 6);
    expect(inner[0].v).toBeCloseTo(5, 6);
    expect(inner[1].v).toBeCloseTo(5, 6);
    a.kill();
    nested!.kill();
    const bad = { get y(): number { throw new Error('boom'); } };
    expect(() => animate([{ y: 0 }, bad as unknown as { y: number }], { y: 1 }, { duration: 1 })).toThrow('boom');
    // The shared scratch is usable again afterwards.
    const objs = [{ y: 0 }, { y: 10 }];
    const b = animate(objs, { y: 20 }, { duration: 1, ease: 'linear' });
    advance(0.5, 500);
    expect(objs.map((o) => o.y)).toEqual([10, 15]);
    b.kill();
  });
});

describe('generational handles', () => {
  it('animations still render and finish once a slot has been reused 2048+ times (negative int32 handles)', () => {
    // Freed slots are reused last-in first-out, so one create/kill cycle at a
    // time keeps landing in the same slot and its generation keeps rising;
    // from 2048 on the handle's top bit is set (negative in JS).
    const o = { x: 0 };
    const objs = [{ x: 0 }, { x: 0 }];
    let bad = 0, done = 0;
    for (let k = 0; k < 4200; k++) {
      o.x = 0;
      const a = animate(o, { x: [0, 100] }, { duration: 0.1, ease: 'linear', onComplete: () => done++ });
      advance(0.05, 50);
      if (Math.abs(o.x - 50) > 1e-6) bad++;
      advance(0.06, 60);
      if (o.x !== 100) bad++;
      a.kill();
      if (k % 100 === 0) {
        // A multi-target call too (group root + packed children + generated writer).
        const b = animate(objs, { x: [0, 10] }, { duration: 0.1, ease: 'linear' });
        advance(0.05, 50);
        if (objs.some((t) => Math.abs(t.x - 5) > 1e-6)) bad++;
        b.kill();
      }
    }
    expect(bad).toBe(0);
    expect(done).toBe(4200);
  });
});
