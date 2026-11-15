import { describe, it, expect, beforeAll, vi } from 'vitest';
import { animate, from, fromTo, set, stagger, engine, flush } from '../src/index';
import { setup, advance, box } from './setup';

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
    expect(a.map(tf)).toEqual(['translate(50px,0px)', 'none', 'none']);
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
