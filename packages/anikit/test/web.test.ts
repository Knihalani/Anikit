import { describe, it, expect, beforeAll } from 'vitest';
import { animate, timeline } from '../src/index';
import { setup, advance, box } from './setup';

beforeAll(setup);
const tick = () => new Promise((r) => queueMicrotask(() => r(undefined)));

const fixedSize = (el: HTMLElement, h: number, w = 200) => {
  Object.defineProperty(el, 'scrollHeight', { configurable: true, get: () => h });
  Object.defineProperty(el, 'scrollWidth', { configurable: true, get: () => w });
};

describe("size: 'auto'", () => {
  it('animates to the measured auto height and ends on auto', () => {
    const el = box({ height: '0px', overflow: 'hidden' });
    fixedSize(el, 120);
    const seen: string[] = [];
    animate(el, { height: 'auto' }, { duration: 1, ease: 'linear', onUpdate: () => seen.push(el.style.height) });
    advance(0.5);
    expect(el.style.height).toBe('60px');
    advance(0.6);
    expect(el.style.height).toBe('auto');
    expect(seen).toContain('60px');
  });

  it('collapses from auto and reverses back to auto', () => {
    const el = box({ height: 'auto' });
    fixedSize(el, 80);
    const a = animate(el, { height: 0 }, { duration: 1, ease: 'linear' });
    advance(0.25);
    expect(el.style.height).toBe('60px');
    advance(1);
    expect(el.style.height).toBe('0px');
    a.reverse();
    advance(1.1);
    expect(el.style.height).toBe('auto');
  });

  it('works for widths and max sizes, and in timelines', async () => {
    const el = box({ width: '10px', maxHeight: '0px' });
    fixedSize(el, 50, 300);
    timeline()
      .add(el, { width: 'auto' }, { duration: 0.5, ease: 'linear' })
      .add(el, { maxHeight: 'auto' }, { duration: 0.5, ease: 'linear' });
    await tick();
    advance(0.25);
    expect(el.style.width).toBe('155px');
    advance(0.5);
    expect(el.style.width).toBe('auto');
    expect(el.style.maxHeight).toBe('25px');
    advance(0.5);
    expect(el.style.maxHeight).toBe('none');
  });
});
