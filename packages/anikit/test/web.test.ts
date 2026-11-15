import { describe, it, expect, beforeAll } from 'vitest';
import { animate, timeline, flip, presence, enter, exit, engine } from '../src/index';
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

// happy-dom has no layout: give elements a mutable box.
type Box = { x: number; y: number; w: number; h: number };
function layout(el: Element, b: Box): Box {
  const box = { ...b };
  (el as any).getBoundingClientRect = () => {
    const st = (el as HTMLElement).style;
    if (!el.isConnected || ((el as HTMLElement).hidden && !st.display) || st.display === 'none') return { left: 0, top: 0, width: 0, height: 0, right: 0, bottom: 0, x: 0, y: 0 } as DOMRect;
    // Popped-out elements are positioned with left/top relative to the origin.
    const ox = st.position === 'absolute' ? parseFloat(st.left) || 0 : 0;
    const oy = st.position === 'absolute' ? parseFloat(st.top) || 0 : 0;
    const x = st.position === 'absolute' ? ox : box.x;
    const y = st.position === 'absolute' ? oy : box.y;
    return { left: x, top: y, width: box.w, height: box.h, right: x + box.w, bottom: y + box.h, x, y } as DOMRect;
  };
  return box;
}
const settle = () => new Promise((r) => setTimeout(r, 0));

describe('flip', () => {
  it('moves elements with one batched measurement and ends on their own transform', async () => {
    const a = box();
    const b = layout(a, { x: 0, y: 0, w: 100, h: 100 });
    const tl = flip(a, () => {
      b.x = 200;
    }, { duration: 1, ease: 'linear' });
    expect(a.style.transform).toBe('translate(-200px,0px)');
    await tick();
    advance(0.5);
    expect(a.style.transform).toBe('translate(-100px,0px)');
    advance(0.6);
    expect(a.style.transform).toBe('none');
    await settle();
    expect(tl.state).toBe('finished');
  });

  it('scales size changes and keeps border-radius undistorted', async () => {
    const a = box({ borderRadius: '10px' });
    const b = layout(a, { x: 0, y: 0, w: 100, h: 50 });
    flip(a, () => {
      b.w = 200;
      b.h = 100;
    }, { duration: 1, ease: 'linear' });
    expect(a.style.transform).toBe('translate(-50px,-25px) scale(0.5,0.5)');
    expect(a.style.borderRadius).toBe('20px');
    await tick();
    advance(1.1);
    expect(a.style.transform).toBe('none');
    expect(a.style.borderRadius).toBe('10px');
  });

  it('pops leaving elements out, animates them, then removes them', async () => {
    const list = document.createElement('div');
    list.className = 'flip-list';
    document.body.appendChild(list);
    const items = [0, 1, 2].map((i) => {
      const el = document.createElement('div');
      el.className = 'it';
      list.appendChild(el);
      layout(el, { x: 0, y: i * 50, w: 100, h: 40 });
      return el;
    });
    const [, mid, last] = items;
    const lastBox = layout(last, { x: 0, y: 100, w: 100, h: 40 });
    const added = document.createElement('div');
    added.className = 'it';
    layout(added, { x: 0, y: 100, w: 100, h: 40 });
    flip('.flip-list .it', () => {
      mid.remove();
      lastBox.y = 50;
      list.appendChild(added);
    }, { duration: 1, ease: 'linear', fade: true });
    // Leaving: back in place, out of the flow, fading.
    expect(mid.isConnected).toBe(true);
    expect(mid.style.position).toBe('absolute');
    expect(mid.style.top).toBe('50px');
    expect(last.style.transform).toBe('translate(0px,50px)');
    expect(added.style.opacity).toBe('0');
    await tick();
    advance(0.5);
    expect(parseFloat(mid.style.opacity)).toBeLessThan(1);
    expect(parseFloat(added.style.opacity)).toBeGreaterThan(0);
    advance(0.6);
    expect(mid.isConnected).toBe(false);
    expect(mid.getAttribute('style')).toBe(null);
    expect(last.style.transform).toBe('none');
  });

  it('cancels an exit when the element comes back mid-way', async () => {
    const list = document.createElement('div');
    list.className = 'flip-list2';
    document.body.appendChild(list);
    const el = document.createElement('div');
    el.className = 'it';
    list.appendChild(el);
    layout(el, { x: 0, y: 0, w: 100, h: 40 });
    flip('.flip-list2 .it', () => el.remove(), { duration: 1, ease: 'linear', fade: true });
    await tick();
    advance(0.5);
    const op = parseFloat(el.style.opacity);
    expect(op).toBeGreaterThan(0);
    expect(op).toBeLessThan(1);
    flip('.flip-list2 .it', () => list.appendChild(el), { duration: 1, ease: 'linear', fade: true });
    expect(el.style.position).toBe('');
    expect(parseFloat(el.style.opacity)).toBeCloseTo(op, 3);
    await tick();
    advance(1.1);
    expect(el.isConnected).toBe(true);
    expect(el.style.opacity).toBe('1');
  });

  it('keeps removing an element that is removed again while leaving', async () => {
    const list = document.createElement('div');
    list.className = 'flip-list3';
    document.body.appendChild(list);
    const a = document.createElement('div');
    const b = document.createElement('div');
    a.className = b.className = 'it';
    list.append(a, b);
    layout(a, { x: 0, y: 0, w: 100, h: 40 });
    const bb = layout(b, { x: 0, y: 50, w: 100, h: 40 });
    flip('.flip-list3 .it', () => a.remove(), { duration: 1, ease: 'linear', fade: true });
    await tick();
    advance(0.3);
    // A second change while `a` is still fading out.
    flip('.flip-list3 .it', () => {
      bb.y = 0;
    }, { duration: 1, ease: 'linear', fade: true });
    expect(a.isConnected).toBe(true);
    expect(a.style.position).toBe('absolute');
    await tick();
    advance(1.1);
    expect(a.isConnected).toBe(false);
  });

  it('morphs between shared elements (data-flip-id)', async () => {
    const thumb = box();
    const modal = box();
    thumb.dataset.flipId = modal.dataset.flipId = 'photo';
    modal.hidden = true;
    layout(thumb, { x: 10, y: 10, w: 50, h: 50 });
    layout(modal, { x: 100, y: 100, w: 200, h: 200 });
    flip([thumb, modal], () => {
      thumb.hidden = true;
      modal.hidden = false;
    }, { duration: 1, ease: 'linear' });
    // Modal starts exactly over the thumbnail.
    expect(modal.style.transform).toBe('translate(-165px,-165px) scale(0.25,0.25)');
    await tick();
    advance(1.1);
    expect(modal.style.transform).toBe('none');
  });
});

describe('presence', () => {
  it('exit() animates out then removes; enter() turns a running exit around', async () => {
    const a = box();
    exit(a, { opacity: 0 }, { duration: 1, ease: 'linear' });
    advance(1.1);
    expect(a.isConnected).toBe(false);

    const b = box();
    exit(b, { opacity: [1, 0] }, { duration: 1, ease: 'linear' });
    advance(0.5);
    enter(b);
    advance(1.1);
    expect(b.isConnected).toBe(true);
    expect(b.style.opacity).toBe('1');
  });

  it('honours reduced motion (still removes)', () => {
    engine.reducedMotion = 'always';
    try {
      const a = box();
      exit(a, { opacity: 0 });
      advance(0.05);
      expect(a.isConnected).toBe(false);
    } finally {
      engine.reducedMotion = 'never';
    }
  });

  it('adds and removes children without layout animation', () => {
    const c = box();
    const p = presence(c, { layout: false, duration: 1, ease: 'linear', exit: { opacity: [1, 0] } });
    const a = document.createElement('div');
    p.add(a);
    expect(a.parentNode).toBe(c);
    expect(a.style.opacity).toBe('0');
    advance(1.1);
    expect(a.style.opacity).toBe('1');
    p.remove(a);
    advance(0.5);
    expect(a.isConnected).toBe(true);
    advance(0.6);
    expect(a.isConnected).toBe(false);
    p.toggle(a, true);
    expect(a.parentNode).toBe(c);
  });

  it('removes with siblings sliding up, and takes an element back mid-exit', async () => {
    const c = box();
    const els = [0, 1, 2].map((i) => {
      const el = document.createElement('div');
      c.appendChild(el);
      return el;
    });
    const boxes = els.map((el, i) => layout(el, { x: 0, y: i * 50, w: 100, h: 40 }));
    const p = presence(c, { duration: 1, ease: 'linear' });
    p.remove(els[1]);
    boxes[2].y = 50;
    // (the stub's layout is static: move the last box by hand to mimic reflow)
    await tick();
    advance(0.5);
    expect(els[1].isConnected).toBe(true);
    expect(els[1].style.position).toBe('absolute');
    p.add(els[1], els[2]);
    expect(els[1].style.position).toBe('');
    await tick();
    advance(1.1);
    expect(els[1].isConnected).toBe(true);
    expect(els[1].style.opacity).toBe('1');
  });

  it("replace() with mode 'wait' exits before entering", async () => {
    const c = box();
    const a = document.createElement('div');
    c.appendChild(a);
    const b = document.createElement('div');
    const p = presence(c, { layout: false, duration: 0.5, ease: 'linear' });
    const done = p.replace(a, b, { mode: 'wait' });
    advance(0.25);
    expect(b.isConnected).toBe(false);
    advance(0.3);
    await settle();
    expect(a.isConnected).toBe(false);
    expect(b.parentNode).toBe(c);
    advance(0.6);
    await done;
    expect(b.style.opacity).toBe('1');
  });
});
