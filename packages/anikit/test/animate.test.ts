import { describe, it, expect, beforeAll, vi } from 'vitest';
import { animate, from, fromTo, set, stagger, engine, flush, FrameBuffer } from '../src/index';
import { setup, advance, box } from './setup';
import { _setCodegen, _setNumberTracks, _setCompactFmt } from '../src/animate';
import { timeline } from '../src/timeline';
import { _recordCount } from '../src/core';
import { r3, s3 } from '../src/values';

beforeAll(setup);

const tf = (el: HTMLElement) => el.style.transform;

describe('animate: values', () => {
  it('interpolates transforms and unitless styles with linear easing', () => {
    const el = box({ opacity: '1' });
    animate(el, { x: 100, rotate: 90, scale: 2, opacity: [0, 1] }, { duration: 1, ease: 'linear' });
    advance(0.5);
    expect(tf(el)).toBe('translate(50px,0px) rotate(45deg) scale(1.5,1.5)');
    expect(parseFloat(el.style.opacity)).toBeCloseTo(0.5, 3);
    advance(1);
    expect(tf(el)).toBe('translate(100px,0px) rotate(90deg) scale(2,2)');
  });

  it('respects easings', () => {
    const el = box();
    animate(el, { x: [0, 100] }, { duration: 1, ease: 'inQuad' });
    advance(0.5);
    expect(tf(el)).toBe('translate(25px,0px)');
    const el2 = box();
    animate(el2, { x: [0, 100] }, { duration: 1, ease: [0.42, 0, 1, 1] });
    advance(1.1);
    expect(tf(el2)).toBe('translate(100px,0px)');
    const el3 = box();
    animate(el3, { x: [0, 100] }, { duration: 1, ease: (t: number) => t * t * t });
    advance(0.5);
    expect(parseFloat(tf(el3).slice(10))).toBeCloseTo(12.5, 1);
  });

  it('handles px lengths, colours and complex strings', () => {
    const el = box({ width: '10px', backgroundColor: 'rgb(255, 0, 0)' });
    animate(
      el,
      { width: 110, backgroundColor: '#0000ff', boxShadow: ['0px 0px 0px rgba(0,0,0,0)', '10px 20px 30px rgba(0,0,0,0.5)'] },
      { duration: 1, ease: 'linear', colorSpace: 'srgb' },
    );
    advance(0.5);
    expect(el.style.width).toBe('60px');
    expect(el.style.backgroundColor.replace(/\s/g, '')).toBe('rgb(128,0,128)');
    expect(el.style.boxShadow).toMatch(/5px/);
    advance(1);
    expect(el.style.width).toBe('110px');
  });

  it('interpolates colours in OKLab by default (no muddy midpoint)', () => {
    const el = box({ color: 'rgb(255, 0, 0)' });
    animate(el, { color: ['#ff0000', '#00ff00'] }, { duration: 1, ease: 'linear' });
    advance(0.5);
    const [r, g] = el.style.color.match(/\d+/g)!.map(Number);
    // sRGB lerp would give (128,128,0); OKLab keeps it brighter.
    expect(r + g).toBeGreaterThan(300);
  });

  it('supports relative values, keyframe arrays with times, and keyframe lists', () => {
    const el = box();
    set(el, { x: 50 });
    animate(el, { x: '+=50' }, { duration: 1, ease: 'linear' });
    advance(1.1);
    expect(tf(el)).toBe('translate(100px,0px)');

    const k = box();
    animate(k, { y: [0, 100, 0] }, { duration: 1, ease: 'linear', times: [0, 0.25, 1] });
    advance(0.25);
    expect(tf(k)).toBe('translate(0px,100px)');

    const l = box();
    animate(l, [{ x: 0 }, { x: 100, offset: 0.5 }, { x: 0 }], { duration: 1, ease: 'linear' });
    advance(0.25);
    expect(tf(l)).toBe('translate(50px,0px)');
  });

  it('converts units', () => {
    const el = box({ fontSize: '10px' });
    animate(el, { x: ['0em', '2em'] }, { duration: 1, ease: 'linear' });
    advance(1.1);
    expect(tf(el)).toBe('translate(2em,0px)');
    const r = box();
    animate(r, { rotate: ['0turn', '0.5turn'] }, { duration: 1, ease: 'linear' });
    advance(1.1);
    expect(tf(r)).toBe('rotate(180deg)');
  });

  it('animates plain objects, CSS variables, attributes and discrete values', () => {
    const obj = { value: 0, label: 'a' };
    animate(obj, { value: 10 }, { duration: 1, ease: 'linear' });
    advance(0.5);
    expect(obj.value).toBeCloseTo(5, 6);

    const el = box();
    animate(el, { '--hue': [0, 360] }, { duration: 1, ease: 'linear' });
    advance(0.5);
    expect(el.style.getPropertyValue('--hue')).toBe('180');

    const svg = document.createElementNS('http://www.w3.org/2000/svg', 'circle');
    svg.setAttribute('r', '10');
    animate(svg, { attr: { r: 20 } }, { duration: 1, ease: 'linear' });
    advance(0.5);
    expect(svg.getAttribute('r')).toBe('15');

    const d = box();
    animate(d, { display: ['none', 'block'] }, { duration: 1, ease: 'linear' });
    advance(0.25);
    expect(d.style.display).toBe('none');
    advance(0.5);
    expect(d.style.display).toBe('block');
  });

  it('resolves function values and staggers', () => {
    const els = [box(), box(), box()];
    animate(els, { x: (_el: unknown, i: number) => (i + 1) * 10 }, { duration: 0.1, delay: stagger(0.1) });
    advance(1);
    expect(els.map(tf)).toEqual(['translate(10px,0px)', 'translate(20px,0px)', 'translate(30px,0px)']);
    const a = [box(), box(), box()];
    animate(a, { x: [0, 100] }, { duration: 1, ease: 'linear', delay: stagger(0.5) });
    advance(0.5);
    expect(a.map(tf)).toEqual(['translate(50px,0px)', '', '']); // unstarted: nothing written
  });

  it('immediately renders explicit from values when delayed', () => {
    const els = [box({ opacity: '1' }), box({ opacity: '1' })];
    animate(els, { opacity: [0, 1] }, { delay: stagger(0.5, { start: 0.1 }) });
    expect(els.map((e) => e.style.opacity)).toEqual(['0', '0']);
  });

  it('from / fromTo / set', () => {
    const el = box({ opacity: '1' });
    from(el, { opacity: 0 }, { duration: 1, ease: 'linear' });
    expect(el.style.opacity).toBe('0');
    advance(0.5);
    expect(parseFloat(el.style.opacity)).toBeCloseTo(0.5, 3);
    const f = box();
    fromTo(f, { x: 10 }, { x: 20 }, { duration: 1, ease: 'linear' });
    advance(0.5);
    expect(tf(f)).toBe('translate(15px,0px)');
    const s = box();
    set(s, { x: 5, y: 6, opacity: 0.3 });
    expect(tf(s)).toBe('translate(5px,6px)');
    expect(s.style.opacity).toBe('0.3');
  });
});

describe('animate: plain-number track path', () => {
  // Number keyframes (and `null` = current value) skip the general value
  // parser; every scenario must build the same tracks either way.
  const run = (): unknown[] => {
    const out: unknown[] = [];
    const objs = Array.from({ length: 4 }, (_, i) => ({ x: i * 10, y: -i, s: '5px', z: i === 2 ? '7em' : 3, q: 1 }));
    const els = [
      box({ transform: 'translate(12px, 4px) rotate(30deg) scale(1.5)', opacity: '0.4', width: '40px' }),
      box({ transform: 'translateX(50%)', marginLeft: '2em' }),
    ];
    const anims = [
      animate(objs, { x: 100, y: [null, 50, -20], s: 40, z: 10, q: [2, null] }, { duration: 1, ease: 'outCubic' }),
      animate(els, { x: 100, y: [0, 30], rotate: 270, scale: 2, skewX: [null, 15], opacity: 1, width: 200, marginLeft: 10 }, { duration: 1, ease: 'linear', repeatDelay: 0.2, repeat: 1 }),
      animate(objs[3], { x: '+=5', y: (_: unknown, i: number) => i + 1, q: 'random(3, 3)' }, { duration: 0.5, delay: 0.1 }),
      animate([{ v: NaN }, { v: -0 }, { v: undefined as unknown as number }], { v: [null, 1] }, { duration: 0.5 }),
    ];
    // Timeline children read the value the previous step left (`chain`).
    const solo = { x: 1 };
    const el3 = box({ transform: 'rotate(10deg)' });
    const tl = timeline({ autoplay: false }).add(solo, { x: 5 }, { duration: 0.2 }).add(solo, { x: [null, 9] }, { duration: 0.2 }).add(el3, { x: [null, 40], rotate: 2 }, { duration: 0.2 });
    tl.play();
    for (let f = 0; f < 6; f++) {
      advance(0.27);
      out.push(JSON.stringify(objs), els.map((e) => e.getAttribute('style')), solo.x, el3.getAttribute('style'));
    }
    for (const a of anims) {
      a.revert();
    }
    tl.revert();
    flush();
    out.push(JSON.stringify(objs), els.map((e) => e.getAttribute('style')));
    return out;
  };
  it('builds the same tracks as the general path', () => {
    const prev = _setNumberTracks(false);
    const general = run();
    _setNumberTracks(true);
    const fast = run();
    _setNumberTracks(prev);
    expect(fast).toEqual(general);
    expect(general[0]).not.toBe(general[2]);
  });
});

describe('s3 (output number formatting)', () => {
  it('equals String(r3(v)) everywhere', () => {
    const edge = [0, -0, 1, -1, 0.0004, -0.0004, 0.0005, -0.0005, 0.0015, 2.5e-4, 0.1 + 0.2, 1 / 3, -2 / 3, 999.9995,
      2147483.647, 2147483.648, -2147483.647, -2147483.648, 1e6 + 0.001, 1e9, 1e21, -1e21, 1e300, 5e-324,
      NaN, Infinity, -Infinity, Number.MAX_SAFE_INTEGER, 0.1, 0.01, 0.001, 12.34, 300, -300.05];
    for (const v of edge) expect(s3(v)).toBe(String(r3(v)));
    let seed = 7;
    const rnd = () => ((seed = (seed * 1103515245 + 12345) % 2147483648) / 2147483648);
    for (let i = 0; i < 200000; i++) {
      const v = (rnd() - 0.5) * Math.pow(10, (i % 12) - 3);
      if (s3(v) !== String(r3(v))) expect(s3(v)).toBe(String(r3(v)));
    }
  });
});

describe('animate: formatted single-property path', () => {
  // Parts writing one formatted plain property (colours, '12px', complex
  // strings) skip the binding loop; same writes, same order, same skipping
  // of unchanged values either way.
  const run = (): unknown[] => {
    const out: unknown[] = [];
    const log: unknown[] = [];
    const cols = Array.from({ length: 5 }, () => ({ c: '#ff0000' }));
    const pxs = Array.from({ length: 5 }, (_, i) => ({ w: i + 'px' }));
    const cx = [{ f: 'blur(0px) brightness(1)' }, { f: 'blur(2px) brightness(0.5)' }];
    const setter = { get c() { return '#00ff00'; }, set c(v: string) { log.push(v); } };
    let updates = 0;
    const a = animate([...cols.slice(0, 2), setter, ...cols.slice(2)], { c: '#0000ff' }, { duration: 1, ease: 'linear' });
    const b = animate(pxs, { w: '100px' }, { duration: 1, ease: 'outCubic', onUpdate: () => updates++ });
    const c = animate(cx, { f: 'blur(8px) brightness(2)' }, { duration: 1, ease: 'linear' });
    // Steps: the value repeats for whole frames, so unchanged writes are skipped.
    const d = animate([{ s: 'a' }, setter], { c: ['#000000', '#ffffff'] }, { duration: 1, ease: 'steps(3)' });
    const snap = () => out.push(JSON.stringify([cols, pxs, cx]), log.length, log[log.length - 1], updates);
    advance(0.2); snap();
    animate(cols[1], { c: '#00ff00' }, { duration: 0.5 }); // takes c over on one target mid-run
    animate(pxs.slice(2), { w: ['5px', '7px'] }, { duration: 0.3, ease: 'linear' });
    advance(0.2); snap();
    a.seek(0.1); b.seek(0.9); flush(); snap();
    advance(0.5); snap();
    advance(1); snap();
    for (const x of [a, b, c, d]) x.revert();
    flush(); snap();
    out.push(log.slice());
    return out;
  };
  it('writes exactly what the binding loop writes', () => {
    const prev = _setCompactFmt(false);
    const general = run();
    _setCompactFmt(true);
    const fast = run();
    _setCompactFmt(prev);
    expect(fast).toEqual(general);
    expect((general[general.length - 1] as unknown[]).length).toBeGreaterThan(3); // the setter was written to
    expect(general[0]).not.toEqual(general[4]);
  });
});

describe('animate: memory', () => {
  it('kill() unregisters every record, the group record of a multi-target animation included', () => {
    flush();
    const before = _recordCount();
    const a = animate(Array.from({ length: 4 }, () => ({ x: 0 })), { x: 10 }, { duration: 1 });
    advance(0.1);
    expect(_recordCount()).toBe(before + 5); // 4 parts + the group
    a.kill();
    flush();
    expect(_recordCount()).toBe(before);
    const b = animate({ x: 0 }, { x: 10 }, { duration: 1 });
    advance(0.1);
    b.revert();
    flush();
    expect(_recordCount()).toBe(before);
  });
});

describe('animate: plain-object write path', () => {
  // Plain-property parts write through a generated per-key-set writer when
  // code generation is allowed, else through a generic loop (CSP fallback).
  const run = (): number[][] => {
    const objs = Array.from({ length: 6 }, (_, i) => ({ x: 0, y: i, o: 1, 'a-b': 0 }));
    // A setter in the middle of a run of same-key-set targets: same writes, same order.
    const log: number[] = [];
    const acc = { get x() { return 0; }, set x(v: number) { log.push(v); }, y: 0, o: 1, 'a-b': 0 };
    let updates = 0;
    const frames: number[][] = [];
    const snap = () => frames.push([...objs.flatMap((o) => [o.x, o.y, o.o, o['a-b']]), ...log, -1, updates]);
    const a = animate([...objs.slice(0, 3), acc, ...objs.slice(3)], { x: 500, y: [10, 300], o: 0, 'a-b': 3 }, { duration: 1, ease: 'outCubic' });
    advance(0.25);
    snap();
    // Parts with `onUpdate` interleaved with the run (they take `o` over on two targets).
    animate([objs[1], objs[4]], { o: [0.5, 1] }, { duration: 0.3, ease: 'linear', onUpdate: () => updates++ });
    advance(0.1);
    snap();
    // Take `y` over on one target: its part leaves the shared key set mid-animation.
    animate(objs[2], { y: -50 }, { duration: 0.5, ease: 'linear' });
    advance(0.25);
    snap();
    a.seek(0.9);
    flush();
    snap();
    animate(objs.slice(3), { x: [1, 2, 3, 4] }, { duration: 0.4, ease: 'inOutSine' });
    advance(0.6);
    snap();
    return frames;
  };
  it('generated writers and the generic loop write identical values', () => {
    const prev = _setCodegen(false);
    const loop = run();
    _setCodegen(undefined);
    const gen = run();
    expect(_setCodegen(prev)).toBe(true); // the generated path was really used
    expect(gen).toEqual(loop);
    expect(loop[0][0]).toBeGreaterThan(0);
    expect(loop[2][2 * 4 + 1]).toBeLessThan(loop[2][1 * 4 + 1]);
    expect(loop[0][24]).toBeGreaterThan(0); // the setter was called
    expect(loop[1][loop[1].length - 1]).toBeGreaterThan(0); // and onUpdate
  });
  it('single-property runs (dense slot tables) match the generic loop through takeovers, kills and reuse', () => {
    const runOne = (): number[][] => {
      const objs = Array.from({ length: 8 }, () => ({ x: 0 }));
      const log: number[] = [];
      const acc = { get x() { return 0; }, set x(v: number) { log.push(v); } };
      let updates = 0;
      const frames: number[][] = [];
      const snap = () => frames.push([...objs.map((o) => o.x), ...log, -1, updates]);
      const kf = [0, 50, 10, 80, 20, 90, 30, 100];
      const a = animate([...objs.slice(0, 4), acc, ...objs.slice(4)], { x: kf }, { duration: 1, ease: 'inOutSine' });
      advance(0.2);
      snap();
      // `onUpdate` Parts on the same key set interleaved with the run.
      const u = animate([objs[1], objs[5]], { x: [5, 6] }, { duration: 0.3, ease: 'linear', onUpdate: () => updates++ });
      advance(0.1);
      snap();
      // Take one target over without `onUpdate`, then kill the taker: its slot is cleared.
      const b = animate(objs[2], { x: -50 }, { duration: 0.5, ease: 'linear' });
      advance(0.2);
      snap();
      b.kill();
      u.kill();
      advance(0.1);
      snap();
      // Freed node indices reused by new Parts (other generations), staggered.
      a.kill();
      const c = animate(objs, { x: [1, 2, 3] }, { duration: 0.5, ease: 'linear', delay: stagger(0.05) });
      advance(0.2);
      snap();
      a.seek(0.5); // a killed controller: nothing to write
      advance(0.1);
      snap();
      c.kill();
      return frames;
    };
    const prev = _setCodegen(false);
    const loop = runOne();
    _setCodegen(undefined);
    const gen = runOne();
    expect(_setCodegen(prev)).toBe(true);
    expect(gen).toEqual(loop);
    expect(loop[0][0]).toBeGreaterThan(0);
    expect(loop[1][loop[1].length - 1]).toBeGreaterThan(0); // onUpdate ran
    expect(loop[2][2]).toBeLessThan(0); // the takeover wrote objs[2]
  });
  it('both paths are strict: a frozen target throws the same way', () => {
    const attempt = (): string => {
      const objs = [{ p: 0, q: 0 }, Object.freeze({ p: 0, q: 0 })];
      let a: ReturnType<typeof animate> | undefined;
      try {
        a = animate(objs, { p: 10, q: 5 }, { duration: 1, ease: 'linear' });
        advance(0.1);
        return 'no error ' + objs[0].p;
      } catch (e) {
        return (e as Error).constructor.name;
      } finally {
        a?.kill();
      }
    };
    const prev = _setCodegen(false);
    const loop = attempt();
    _setCodegen(undefined);
    const gen = attempt();
    _setCodegen(prev);
    expect(loop).toBe('TypeError');
    expect(gen).toBe(loop);
  });
});

describe('animate: playback', () => {
  it('pauses, seeks, reverses and reports progress', async () => {
    const el = box();
    const a = animate(el, { x: [0, 100] }, { duration: 1, ease: 'linear' });
    advance(0.25);
    a.pause();
    advance(0.5);
    expect(tf(el)).toBe('translate(25px,0px)');
    a.seek(0.8);
    expect(tf(el)).toBe('translate(80px,0px)');
    expect(a.progress).toBeCloseTo(0.8, 6);
    a.progress = 0.1;
    expect(tf(el)).toBe('translate(10px,0px)');
    a.play();
    advance(1);
    expect(a.state).toBe('finished');
    await a;
    a.reverse();
    advance(0.5);
    expect(tf(el)).toBe('translate(50px,0px)');
    advance(1);
    expect(tf(el)).toBe('none');
  });

  it('repeats, yoyos and honours repeatDelay', () => {
    const el = box();
    const onRepeat = vi.fn();
    animate(el, { x: [0, 100] }, { duration: 1, ease: 'linear', repeat: 1, yoyo: true, onRepeat });
    advance(1.5);
    expect(tf(el)).toBe('translate(50px,0px)');
    expect(onRepeat).toHaveBeenCalledTimes(1);
    advance(1);
    expect(tf(el)).toBe('none');

    const r = box();
    animate(r, { x: [0, 100] }, { duration: 1, ease: 'linear', repeat: 1, repeatDelay: 1 });
    advance(1.5);
    expect(tf(r)).toBe('translate(100px,0px)'); // holding
    advance(1);
    expect(tf(r)).toBe('translate(50px,0px)');
  });

  it('fires callbacks once per frame and resolves `finished`', async () => {
    const els = [box(), box(), box()];
    const onUpdate = vi.fn();
    const onStart = vi.fn();
    const onComplete = vi.fn();
    const a = animate(els, { x: 10 }, { duration: 0.1, onUpdate, onStart, onComplete });
    engine.tick(16);
    expect(onUpdate).toHaveBeenCalledTimes(1);
    expect(onStart).toHaveBeenCalledTimes(1);
    advance(0.2);
    expect(onComplete).toHaveBeenCalledTimes(1);
    await expect(a.finished).resolves.toBeUndefined();
  });

  it('completion promises: pending before the end, one per run, re-armed by a replay', async () => {
    const o = { x: 0 };
    const a = animate(o, { x: 10 }, { duration: 0.1 });
    const early = a.finished;
    expect(a.finished).toBe(early);
    let done = 0;
    void early.then(() => done++);
    void a.then(() => done++);
    await Promise.resolve();
    expect(done).toBe(0);
    advance(0.2);
    await early;
    expect(done).toBe(2);
    expect(a.finished).toBe(early);
    // Never awaited before finishing: still resolves.
    const b = animate({ y: 0 }, { y: 1 }, { duration: 0.05 });
    advance(0.1);
    await expect(b.finished).resolves.toBeUndefined();
    // A replay re-arms: a new promise that is pending until the new end.
    a.restart();
    const again = a.finished;
    expect(again).not.toBe(early);
    let redone = false;
    void again.then(() => (redone = true));
    await Promise.resolve();
    expect(redone).toBe(false);
    advance(0.2);
    await again;
    expect(redone).toBe(true);
  });

  it('overwrites properties and frees fully replaced animations', () => {
    const el = box();
    const before = engine.stats().nodes;
    const a = animate(el, { x: [0, 100], y: [0, 100] }, { duration: 1, ease: 'linear' });
    advance(0.5);
    animate(el, { x: [0, -100] }, { duration: 1, ease: 'linear' });
    advance(0.25);
    expect(tf(el)).toBe('translate(-25px,75px)');
    animate(el, { y: 0 }, { duration: 0.1 });
    flush();
    expect(a.state).toBe('idle'); // killed: all props taken over
    advance(2);
    expect(engine.stats().nodes).toBeLessThanOrEqual(before + 2);
  });

  it('a multi-target animation is freed once every target was taken over, not before', () => {
    const objs = Array.from({ length: 30 }, () => ({ x: 0, y: 0 }));
    const a = animate(objs, { x: 100, y: 50 }, { duration: 1, ease: 'linear' });
    advance(0.1);
    // Take x over everywhere: y still runs on every target.
    animate(objs, { x: -1 }, { duration: 1 });
    advance(0.1);
    expect(a.state).toBe('running');
    // Take y over on all but the last target, in an order other than the parts'.
    animate(objs.slice(0, 29).reverse(), { y: -1 }, { duration: 1 });
    advance(0.1);
    expect(a.state).toBe('running');
    const y = objs[29].y;
    advance(0.1);
    expect(objs[29].y).toBeGreaterThan(y);
    // The last one: now nothing is left.
    animate(objs[29], { y: -2 }, { duration: 1 });
    flush();
    expect(a.state).toBe('idle');
    // The same target twice in one call: the second part takes it over from the first.
    const o = { x: 0 };
    const b = animate([o, { x: 0 }, o], { x: 10 }, { duration: 1, ease: 'linear' });
    advance(0.5);
    expect(b.state).toBe('running');
    expect(o.x).toBeCloseTo(5, 5);
  });

  it('kill() releases engine nodes', () => {
    const start = engine.stats().nodes;
    const list = Array.from({ length: 50 }, () => box());
    const a = animate(list, { x: 100, opacity: 0.5 }, { duration: 1 });
    expect(engine.stats().nodes).toBe(start + 51);
    a.kill();
    expect(engine.stats().nodes).toBe(start);
  });
});

describe('animate: name collisions', () => {
  it('treats `fill` in keyframes as the SVG/CSS property', () => {
    const p = document.createElementNS('http://www.w3.org/2000/svg', 'path');
    document.body.appendChild(p);
    animate(p, { fill: ['#ff0000', '#0000ff'] }, { duration: 1, ease: 'linear', colorSpace: 'srgb' });
    advance(1.1);
    expect((p as any).style.fill.replace(/\s/g, '')).toBe('rgb(0,0,255)');
  });
});

describe('generated flush loops survive engine changes mid-flush', () => {
  it('keeps writing correct values when a setter creates animations (array moves, memory may grow)', () => {
    const N = 64;
    const targets: Array<{ x: number }> = [];
    let spawned = 0;
    const extra: Array<{ v: number }> = [];
    const buffers: FrameBuffer[] = [];
    for (let i = 0; i < N; i++) {
      if (i === 20) {
        // A class whose prototype setter starts thousands of animations the first
        // time it is written: the generated loop sees a plain-looking target.
        class Spawner {
          _x = 0;
          get x() { return this._x; }
          set x(v: number) {
            this._x = v;
            if (!spawned++) {
              for (let k = 0; k < 2000; k++) { const o = { v: 0 }; extra.push(o); animate(o, { v: 1 }, { duration: 1, ease: 'linear' }); }
              // …and grows WebAssembly memory (detaching every typed-array view) mid-flush.
              buffers.push(new FrameBuffer(4096, 4096));
            }
          }
        }
        targets.push(new Spawner());
      } else targets.push({ x: 0 });
    }
    animate(targets, { x: [0, 100] }, { duration: 1, ease: 'linear' });
    for (const [s, want] of [[0.25, 25], [0.25, 50], [0.25, 75]] as const) {
      advance(s);
      for (let i = 0; i < N; i++) expect(targets[i].x).toBeCloseTo(want, 6);
    }
    expect(spawned).toBeGreaterThan(0);
    expect(extra[1999].v).toBeGreaterThan(0);
  });
});
