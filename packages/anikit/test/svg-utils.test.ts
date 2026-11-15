import { describe, it, expect, beforeAll } from 'vitest';
import {
  animate, morph, pathLength, pathBounds, star, circle, rect, MotionPath, staggerValues, ease, easing, mixColor,
  cssEasing, splitText, normalizePath,
} from '../src/index';
import { setup, advance, box } from './setup';

beforeAll(setup);
const NS = 'http://www.w3.org/2000/svg';

describe('svg', () => {
  it('measures paths and shapes', () => {
    expect(pathLength('M0 0 L100 0 L100 100')).toBeCloseTo(200, 6);
    expect(pathLength(circle(0, 0, 50))).toBeCloseTo(2 * Math.PI * 50, 1);
    expect(pathBounds(rect(10, 20, 30, 40))).toEqual([10, 20, 40, 60]);
    expect(normalizePath('M0 0 L10 0')).toMatch(/^M0 0C/);
    const mp = new MotionPath('M0 0 L100 0 L100 100');
    const pt = mp.at(0.75);
    expect(pt.x).toBeCloseTo(100, 6);
    expect(pt.y).toBeCloseTo(50, 6);
    expect(pt.angle).toBeCloseTo(90, 6);
    mp.dispose();
  });

  it('morphs between arbitrary shapes', () => {
    const f = morph(rect(0, 0, 100, 100), star(50, 50, 5, 50, 20));
    expect(pathLength(f(0))).toBeCloseTo(400, 1);
    expect(f(0.5)).toMatch(/^M[-\d.]+ [-\d.]+C/);
    const p = document.createElementNS(NS, 'path');
    p.setAttribute('d', rect(0, 0, 100, 100));
    animate(p, { d: circle(50, 50, 50) }, { duration: 1, ease: 'linear' });
    advance(1.1);
    expect(pathLength(p.getAttribute('d')!)).toBeCloseTo(2 * Math.PI * 50, 0);
  });

  it('moves along a motion path with auto-rotation', () => {
    const el = box();
    animate(el, { path: { path: 'M0 0 L100 0 L100 100', rotate: true } }, { duration: 1, ease: 'linear' });
    advance(0.75);
    expect(el.style.transform).toBe('translate(100px,50px) rotate(90deg)');
  });

  it('draws strokes', () => {
    const p = document.createElementNS(NS, 'path');
    p.setAttribute('d', 'M0 0 L100 0');
    animate(p, { draw: [0, 1] }, { duration: 1, ease: 'linear' });
    advance(0.5);
    expect((p as any).style.strokeDasharray).toBe('50px 101px');
  });
});

describe('utils', () => {
  it('evaluates easings and exports CSS', () => {
    expect(ease('inQuad', 0.5)).toBe(0.25);
    expect(easing('outQuad')(0.5)).toBe(0.75);
    expect(cssEasing('ease-in-out')).toBe('cubic-bezier(0.42,0,0.58,1)');
    const s = cssEasing('spring(200, 15)');
    expect(s === 'ease-out' || s.startsWith('linear(0,')).toBe(true);
  });
  it('mixes colours and staggers', () => {
    expect(mixColor('red', 'blue', 0.5, 'srgb')).toBe('rgb(128,0,128)');
    expect(mixColor('#000', 'transparent', 0)).toBe('rgb(0,0,0)');
    expect(staggerValues(5, 0.1, { from: 'center' }).map((v) => +v.toFixed(3))).toEqual([0.2, 0.1, 0, 0.1, 0.2]);
    expect(staggerValues(3, 0, { total: 1 })).toEqual([0, 0.5, 1]);
  });
  it('splits text', () => {
    const el = document.createElement('p');
    el.innerHTML = 'Hello <b>big</b> world';
    document.body.appendChild(el);
    const s = splitText(el);
    expect(s.words.length).toBe(3);
    expect(s.chars.map((c) => c.textContent).join('')).toBe('Hellobigworld');
    expect(el.querySelector('b')!.textContent).toBe('big');
    s.revert();
    expect(el.innerHTML).toBe('Hello <b>big</b> world');
  });
});
