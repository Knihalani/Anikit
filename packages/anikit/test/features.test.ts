import { describe, it, expect, beforeAll, vi } from 'vitest';
import {
  animate, timeline, sequence, delayedCall, physics, context, quickSetter, quickTo, effects, registerEffect, effectNames,
  interpolate, distribute, shuffle, splitColor, getUnit, VelocityTracker, noise2, noise3, fbm, noiseMany, wander,
  trimPath, pathThrough, simplifyPath, pathArea, flattenPath, pathLength, circle, rect, star, MotionPath, ease, engine,
  morphSequence, resolveRandom, roundTo, pipe,
} from '../src/index';
import { setup, advance, box } from './setup';

beforeAll(setup);
const tf = (el: HTMLElement) => el.style.transform;
const tick = () => new Promise((r) => queueMicrotask(() => r(undefined)));

describe('easings (core additions)', () => {
  it('custom path easing, modifiers and presets', () => {
    expect(ease('custom(M0,0 L1,1)', 0.4)).toBeCloseTo(0.4, 3);
    expect(ease('reverse(inQuad)', 0.5)).toBeCloseTo(0.75, 9);
    expect(ease('mirror(linear)', 0.75)).toBeCloseTo(0.5, 9);
    expect(ease('blend(inQuad, outQuad, 0.5)', 0.5)).toBeCloseTo(0.5, 9);
    for (const e of ['spring.gentle', 'wobbly', 'rough(10, 1, 3)', 'expoScale(1, 5)']) {
      expect(ease(e, 1)).toBeCloseTo(1, 4);
    }
  });
});

describe('values', () => {
  it('snaps, rounds, modifies and formats', () => {
    const o = { x: 0, y: 0, z: 0 };
    animate(o, { x: 100, y: 100, z: 1000 }, {
      duration: 1, ease: 'linear', snap: { x: 25 }, round: ['y'], modifiers: { z: (v: number) => v % 300 },
    });
    advance(0.33);
    expect(o.x % 25).toBe(0);
    expect(Number.isInteger(o.y)).toBe(true);
    expect(o.z).toBeLessThan(300);
    const el = box();
    el.textContent = '0';
    animate(el, { textContent: 1234.5 }, { duration: 1, ease: 'linear', format: { maximumFractionDigits: 0 }, locale: 'en-US' });
    advance(1.1);
    expect(el.textContent).toBe('1,235');
  });

  it('resolves random() strings', () => {
    for (let i = 0; i < 20; i++) {
      const v = resolveRandom('random(10, 20, 5)') as number;
      expect([10, 15, 20]).toContain(v);
      expect(['a', 'b']).toContain(resolveRandom('random([a, b])'));
    }
    expect(String(resolveRandom('random(0px, 10px)'))).toMatch(/px$/);
  });

  it('types and scrambles text', () => {
    const el = box();
    el.textContent = '';
    animate(el, { text: 'Hello' }, { duration: 1, ease: 'linear' });
    advance(0.4);
    expect(el.textContent).toBe('He');
    advance(1);
    expect(el.textContent).toBe('Hello');
    const s = box();
    s.textContent = 'xxxxxxxx';
    animate(s, { text: { value: 'Revealed', type: 'scramble' } }, { duration: 1, ease: 'linear' });
    advance(0.5);
    expect(s.textContent!.slice(0, 4)).toBe('Reve');
    expect(s.textContent!.length).toBe(8);
    advance(1);
    expect(s.textContent).toBe('Revealed');
  });

  it('supports xPercent / yPercent and multi-shape morphs', () => {
    const el = box();
    animate(el, { xPercent: -50, yPercent: [0, -50] }, { duration: 1, ease: 'linear' });
    advance(1.1);
    expect(tf(el)).toBe('translate(-50%,-50%)');
    const p = document.createElementNS('http://www.w3.org/2000/svg', 'path');
    p.setAttribute('d', rect(0, 0, 100, 100));
    animate(p, { d: [null, circle(50, 50, 50), star(50, 50, 5, 50, 20)] }, { duration: 1, ease: 'linear' });
    advance(0.5);
    expect(pathLength(p.getAttribute('d')!)).toBeCloseTo(2 * Math.PI * 50, 0);
    const seq = morphSequence([rect(0, 0, 10, 10), circle(5, 5, 5), star(5, 5, 6, 5, 2)]);
    expect(seq.frames.length).toBe(3);
    expect(new Set(seq.frames.map((f) => f.length)).size).toBe(1);
  });
});

describe('control', () => {
  it('revert() restores original inline values', () => {
    const el = box({ opacity: '0.7' });
    const a = animate(el, { x: 100, opacity: 0, backgroundColor: 'red' }, { duration: 1 });
    advance(0.5);
    a.revert();
    expect(el.style.opacity).toBe('0.7');
    expect(el.style.backgroundColor).toBe('');
    expect(tf(el)).toBe('');
  });

  it('context() tracks, scopes and reverts', () => {
    const root = document.createElement('section');
    const inside = document.createElement('div');
    inside.className = 'ctx-item';
    root.appendChild(inside);
    const outside = box();
    outside.className = 'ctx-item';
    document.body.appendChild(root);
    const cleanup = vi.fn();
    const ctx = context(() => {
      animate('.ctx-item', { x: 50 }, { duration: 0.1 });
      return cleanup;
    }, root);
    advance(0.2);
    expect(tf(inside)).toBe('translate(50px,0px)');
    expect(tf(outside)).toBe('');
    ctx.revert();
    expect(tf(inside)).toBe('');
    expect(cleanup).toHaveBeenCalled();
  });

  it('reduced motion jumps to the end', () => {
    engine.reducedMotion = 'always';
    const el = box();
    animate(el, { x: 100 }, { duration: 5, repeat: Infinity });
    advance(0.05);
    expect(tf(el)).toBe('translate(100px,0px)');
    engine.reducedMotion = 'never';
  });

  it('quickSetter and quickTo', () => {
    const el = box();
    const setX = quickSetter(el, 'x');
    setX(42);
    expect(tf(el)).toBe('translate(42px,0px)');
    const toY = quickTo(el, 'y', { duration: 1, ease: 'linear' });
    toY(100);
    advance(0.5);
    toY(0);
    advance(0.5);
    expect(tf(el)).toBe('translate(42px,25px)');
  });

  it('physics() with gravity and a floor', async () => {
    const o = { y: 0 };
    const p = physics(o, { y: { velocity: 0, acceleration: 1000, max: 100 } }, { restitution: 0.3 });
    advance(0.2);
    expect(o.y).toBeCloseTo(20, 6);
    advance(5);
    expect(o.y).toBe(100);
    await p;
  });
});

describe('timeline additions', () => {
  it('sequence(), labels navigation, tweenTo, remove, delayedCall', async () => {
    const a = box();
    const b = box();
    const tl = sequence([[a, { x: 100 }, { duration: 1, ease: 'linear' }], 'mid', [b, { x: 100 }, { duration: 1, ease: 'linear', at: '<0.5' }], 'end'], { autoplay: false });
    await tick();
    expect(tl.duration).toBe(1.5);
    tl.seek(1.2);
    expect(tl.currentLabel).toBe('mid');
    expect(tl.nextLabel()).toBe('end');
    tl.tweenTo('mid', { duration: 1 });
    advance(1.1);
    expect(tl.time).toBeCloseTo(1, 6);
    const child = tl.getChildren()[1];
    tl.remove(child);
    await tick();
    expect(tl.getChildren().length).toBe(1);
    const fn = vi.fn();
    delayedCall(0.5, fn);
    await tick();
    advance(0.4);
    expect(fn).not.toHaveBeenCalled();
    advance(0.2);
    expect(fn).toHaveBeenCalledTimes(1);
  });

  it('timeline revert restores everything', async () => {
    const el = box({ opacity: '1' });
    const tl = timeline().add(el, { x: 100 }, { duration: 0.2 }).add(el, { opacity: 0 }, { duration: 0.2 });
    await tick();
    advance(1);
    tl.revert();
    expect(tf(el)).toBe('');
    expect(el.style.opacity).toBe('1');
  });
});

describe('effects', () => {
  it('ships many presets and supports custom ones', () => {
    expect(effectNames().length).toBeGreaterThan(30);
    const el = box();
    effects.fadeIn(el, { duration: 1 });
    advance(1.1);
    expect(el.style.opacity).toBe('1');
    registerEffect('nudge', (t, o) => animate(t, { x: o.amount }, { duration: 0.1 }), { amount: 7 });
    effects.nudge(el);
    advance(0.2);
    expect(tf(el)).toBe('translate(7px,0px)');
    const c = box();
    effects.countUp(c, { to: 50, duration: 0.5 });
    advance(0.6);
    expect(c.textContent).toBe('50');
    const t = box();
    t.textContent = '';
    effects.typewriter(t, { text: 'abc', duration: 0.3 });
    advance(0.4);
    expect(t.textContent).toBe('abc');
  });
});

describe('bake to CSS', () => {
  it('produces @keyframes with transforms and colours', () => {
    const el = box();
    const a = animate(el, { x: [0, 100], backgroundColor: ['#f00', '#00f'] }, { duration: 2, ease: 'outCubic', repeat: 1, yoyo: true, autoplay: false });
    const css = a.toCSS({ name: 'move', selector: '.thing', samples: 5 });
    expect(css).toContain('@keyframes move');
    expect(css).toContain('0% { transform: none; background-color: rgb(255,0,0) }');
    expect(css).toContain('100% { transform: translate(100px,0px)');
    expect(css).toMatch(/50% \{ transform: translate\(87\.5px,0px\); background-color: rgb\(/);
    expect(css).toContain('.thing { animation: move 2s linear 0s 2 alternate both; }');
  });
});

describe('utils', () => {
  it('interpolate, distribute, shuffle, colours, velocity', () => {
    const f = interpolate([0, 100, 200], [0, 1, 0]);
    expect(f(50)).toBe(0.5);
    expect(f(150)).toBe(0.5);
    expect(f(-10)).toBe(0);
    expect(interpolate([0, 1], ['0px', '10px'])(0.5)).toBe('5px');
    expect(interpolate([0, 1], ['#000', '#fff'], { colorSpace: 'srgb' })(0.5)).toBe('rgb(128,128,128)');
    const d = distribute({ base: 1, amount: 1 });
    expect([0, 1, 2].map((i) => d(null, i, 3))).toEqual([1, 1.5, 2]);
    expect(shuffle([1, 2, 3, 4, 5], 3).sort()).toEqual([1, 2, 3, 4, 5]);
    expect(splitColor('rgba(1,2,3,.5)')).toEqual([1, 2, 3, 0.5]);
    expect(getUnit('12vh')).toBe('vh');
    expect(roundTo(1.2345, 2)).toBe(1.23);
    expect(pipe((v: number) => v + 1, (v: number) => v * 2)(1)).toBe(4);
    const vt = new VelocityTracker();
    vt.add(0, 0);
    vt.add(10, 50);
    vt.add(20, 100);
    expect(vt.velocity).toBe(200);
  });

  it('noise is smooth, bounded and seeded', () => {
    expect(Math.abs(noise2(0.3, 0.7))).toBeLessThanOrEqual(1);
    expect(noise3(1, 2, 3, 5)).toBe(noise3(1, 2, 3, 5));
    expect(noise2(1.3, 2.1, 1)).not.toBe(noise2(1.3, 2.1, 2));
    expect(Math.abs(noise2(1, 1) - noise2(1.0001, 1))).toBeLessThan(0.01);
    expect(Math.abs(fbm(0.5, 0.5, 0.5, { octaves: 5 }))).toBeLessThanOrEqual(1);
    expect(noiseMany([1, 2, 3, 4, 5, 6], 5)[0]).toBe(noise3(1, 2, 3, 5));
    const w = wander({ amplitude: 10, seed: 3 });
    expect(Math.abs(w(1.5))).toBeLessThanOrEqual(10);
  });

  it('path operations', () => {
    expect(pathLength(trimPath('M0 0 L100 0', 0.2, 0.7))).toBeCloseTo(50, 6);
    expect(pathThrough([[0, 0], [50, 50], [100, 0]])).toMatch(/^M0 0C/);
    expect(pathArea(rect(0, 0, 10, 20))).toBeCloseTo(200, 6);
    const flat = flattenPath(circle(0, 0, 50), 0.1);
    expect(flat[0].length).toBeGreaterThan(20);
    const simple = simplifyPath(circle(0, 0, 50), 1);
    expect(pathLength(simple)).toBeCloseTo(2 * Math.PI * 50, -1);
    const mp = new MotionPath('M0 0 L100 0');
    const c = mp.closest(40, 10);
    expect(c.progress).toBeCloseTo(0.4, 6);
    expect(c.distance).toBeCloseTo(10, 6);
    expect(mp.points(3)).toEqual([[0, 0], [50, 0], [100, 0]]);
    expect(mp.atLength(25).x).toBeCloseTo(25, 6);
    mp.dispose();
  });
});
