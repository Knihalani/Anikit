import { describe, it, expect, beforeAll } from 'vitest';
import { splitText, presence, enter, exit, bindValues, motionValue, animate } from '../src/index';
import { setup, advance } from './setup';

beforeAll(setup);

function el(html: string, tag = 'p'): HTMLElement {
  const e = document.createElement(tag);
  e.innerHTML = html;
  document.body.appendChild(e);
  return e;
}

describe('splitText()', () => {
  it('isolates a right-to-left run inside a left-to-right line, and reverts exactly', () => {
    const p = el('Say שלום עולם to all');
    const before = p.innerHTML;
    const s = splitText(p, { type: 'words,chars' });
    const iso = p.querySelectorAll('span[dir="rtl"]');
    expect(iso.length).toBe(1);
    expect((iso[0] as HTMLElement).style.unicodeBidi).toBe('isolate');
    expect(iso[0].textContent).toBe('שלום עולם');
    expect(s.words.map((w) => w.textContent)).toEqual(['Say', 'שלום', 'עולם', 'to', 'all']);
    s.revert();
    expect(p.innerHTML).toBe(before);
  });

  it('leaves plain left-to-right text without isolates', () => {
    const p = el('Hello big world 123');
    splitText(p, { type: 'words,chars' }).revert();
    const s = splitText(p, { type: 'words' });
    expect(p.querySelector('span[dir]')).toBeNull();
    s.revert();
  });

  it('Arabic chars carry zero-width joiners where the letters join', () => {
    // م (joins both sides) ر (joins only before it) ح ب ا
    const p = el('مرحبا');
    const s = splitText(p, { type: 'chars' });
    expect(s.chars.map((c) => c.textContent)).toEqual(['م‍', '‍ر', 'ح‍', '‍ب‍', '‍ا']);
    s.revert();
    expect(p.innerHTML).toBe('مرحبا');
  });

  it('lines: line wrappers are children of the element and nested markup is kept per word', () => {
    const p = el('Plain <b id="x">bold <i>it</i></b> end');
    const before = p.innerHTML;
    const s = splitText(p, { type: 'lines,words' });
    for (const l of s.lines) expect(l.parentElement).toBe(p);
    const b = p.querySelector('#x')!;
    expect(b.textContent).toBe('bold it');
    expect(b.querySelector('i')!.textContent).toBe('it');
    expect(p.textContent).toBe('Plain bold it end');
    s.revert();
    expect(p.innerHTML).toBe(before);
  });
});

describe('presence (no layout) under rapid add/remove', () => {
  const list = () => {
    const c = document.createElement('div');
    document.body.appendChild(c);
    return c;
  };
  const op = (e: Element) => (e as HTMLElement).style.opacity;

  it('re-adding during an exit that interrupted the entrance finishes the entrance', () => {
    const c = list();
    const p = presence(c, { layout: false, duration: 0.4 });
    const d = document.createElement('div');
    p.add(d);
    advance(0.05);
    p.remove(d);
    advance(0.05);
    p.add(d);
    advance(2);
    expect(d.isConnected).toBe(true);
    expect(op(d)).toBe('1');
  });

  it('add, remove, add, remove, add ends shown', () => {
    const c = list();
    const p = presence(c, { layout: false, duration: 0.3 });
    const d = document.createElement('div');
    for (const k of ['add', 'remove', 'add', 'remove', 'add']) {
      if (k === 'add') p.add(d);
      else p.remove(d);
      advance(0.03);
    }
    advance(2);
    expect(d.isConnected).toBe(true);
    expect(op(d)).toBe('1');
  });

  it('enter() turning around an exit that interrupted it ends at the entrance end', () => {
    const d = document.createElement('div');
    document.body.appendChild(d);
    enter(d, { opacity: [0, 1] }, { duration: 0.4 });
    advance(0.05);
    exit(d, { opacity: 0 }, { duration: 0.3 });
    advance(0.05);
    enter(d);
    advance(2);
    expect(d.isConnected).toBe(true);
    expect(op(d)).toBe('1');
  });
});

describe('bindValues().revert()', () => {
  it('also resets the cached transform, so a later animation starts clean', () => {
    const d = document.createElement('div');
    document.body.appendChild(d);
    const b = bindValues(d, { scale: motionValue(0.5), rotate: motionValue(30) });
    expect(d.style.transform).toContain('scale');
    b.revert();
    animate(d, { x: 10 }, { duration: 0.1 });
    advance(0.2);
    expect(d.style.transform).not.toMatch(/scale|rotate/);
  });
});
