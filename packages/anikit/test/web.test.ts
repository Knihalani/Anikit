import { describe, it, expect, beforeAll } from 'vitest';
import { animate, timeline, flip, presence, enter, exit, engine, viewTransition, compositor, scroll, splitText, mixColor, gesture, transformPath } from '../src/index';
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

describe('viewTransition', () => {
  it('names elements for the transition, times it with any easing, and cleans up', async () => {
    const thumb = box();
    const big = box();
    const seen: Record<string, string> = {};
    let css = '';
    const doc = document as any;
    doc.startViewTransition = (cb: () => Promise<void>) => {
      seen.before = thumb.style.getPropertyValue('view-transition-name');
      css = document.head.querySelector('style[id^="anikit-vt"]')?.textContent ?? '';
      const updateCallbackDone = Promise.resolve().then(cb).then(() => {
        seen.thumbAfter = thumb.style.getPropertyValue('view-transition-name');
        seen.bigAfter = big.style.getPropertyValue('view-transition-name');
      });
      return { updateCallbackDone, ready: updateCallbackDone, finished: updateCallbackDone, skipTransition() {} };
    };
    try {
      let ran = false;
      const vt = viewTransition(() => {
        ran = true;
      }, { duration: 0.5, ease: 'spring(bounce: 0.3, duration: 0.5)', names: { hero: [thumb, () => big] } });
      expect(vt.native).toBe(true);
      await vt;
      expect(ran).toBe(true);
      expect(seen).toEqual({ before: 'hero', thumbAfter: '', bigAfter: 'hero' });
      expect(css).toContain('animation-duration:0.5s');
      expect(css).toMatch(/animation-timing-function:linear\(/);
      // Everything temporary is gone.
      expect(big.style.getPropertyValue('view-transition-name')).toBe('');
      expect(document.head.querySelector('style[id^="anikit-vt"]')).toBe(null);
    } finally {
      delete doc.startViewTransition;
    }
  });

  it('falls back to FLIP when the API is missing', async () => {
    const thumb = box();
    const big = box();
    big.hidden = true;
    layout(thumb, { x: 0, y: 0, w: 50, h: 50 });
    layout(big, { x: 100, y: 100, w: 200, h: 200 });
    const vt = viewTransition(() => {
      thumb.hidden = true;
      big.hidden = false;
    }, { duration: 1, ease: 'linear', names: { hero: [thumb, big] } });
    expect(vt.native).toBe(false);
    await vt.updateCallbackDone;
    await tick();
    expect(big.style.transform).toMatch(/^translate\(-175px,-175px\) scale\(0.25,0.25\)/);
    advance(1.1);
    await vt;
    expect(big.style.transform).toBe('none');
  });
});

/** A minimal WAAPI Animation double (happy-dom doesn't run them). */
function fakeAnimate(el: HTMLElement) {
  const made: any[] = [];
  (el as any).animate = (frames: unknown, timing: any) => {
    const a: any = {
      frames, timing, playState: 'running', currentTime: 0, timeline: null, rangeStart: 'normal', rangeEnd: 'normal',
      effect: { getComputedTiming: () => ({ endTime: timing.duration + timing.delay, progress: 0 }) },
      finished: new Promise(() => {}),
      pause() { a.playState = 'paused'; },
      play() { a.playState = 'running'; },
      cancel() { a.playState = 'idle'; },
      commitStyles() {},
      updatePlaybackRate() {},
    };
    made.push(a);
    return a;
  };
  return made;
}

describe('compositor + scroll', () => {
  it('hands compositor animations to a native ScrollTimeline over the measured range', () => {
    const G = globalThis as any;
    const el = box();
    const made = fakeAnimate(el);
    const timelines: any[] = [];
    G.ScrollTimeline = function (this: any, o: any) {
      this.o = o;
      timelines.push(this);
    };
    try {
      const a = compositor(el, { opacity: [0, 1], x: [0, 100] }, { ease: 'linear', duration: 1 });
      const target = box();
      (target as any).getBoundingClientRect = () => ({ top: 1500, left: 0, width: 100, height: 400, right: 100, bottom: 1900 });
      const stop = scroll(a, { target, offset: ['start end', 'end start'] });
      expect(timelines.length).toBe(1);
      expect(timelines[0].o.axis).toBe('block');
      expect(made[0].timeline).toBe(timelines[0]);
      const vh = window.innerHeight;
      expect(made[0].rangeStart).toBe(1500 - vh + 'px');
      expect(made[0].rangeEnd).toBe('1900px');
      stop();
      expect(made[0].playState).toBe('paused');
    } finally {
      delete G.ScrollTimeline;
    }
  });

  it('scrubs from scroll events where ScrollTimeline is missing', () => {
    const el = box();
    const made = fakeAnimate(el);
    const a = compositor(el, { opacity: [0, 1] }, { ease: 'linear', duration: 2 });
    const scroller = box({ height: '100px', overflow: 'auto' });
    Object.defineProperty(scroller, 'scrollHeight', { value: 1100 });
    Object.defineProperty(scroller, 'clientHeight', { value: 100 });
    scroll(a, { container: scroller });
    scroller.scrollTop = 250;
    scroller.dispatchEvent(new Event('scroll'));
    expect(made[0].playState).toBe('paused');
    expect(made[0].currentTime).toBeCloseTo(500, 6);
  });

  it('compositor(): reduced motion jumps to the end, revert() restores inline styles', () => {
    const el = box({ opacity: '0.5' });
    const made = fakeAnimate(el);
    engine.reducedMotion = 'always';
    try {
      const a = compositor(el, { opacity: [0, 1] }, { duration: 1 });
      expect(made[0].timing.duration).toBe(0);
      el.style.opacity = '1'; // as commitStyles() would
      a.revert();
      expect(el.style.opacity).toBe('0.5');
      expect(made[0].playState).toBe('idle');
    } finally {
      engine.reducedMotion = 'never';
    }
  });
});

describe('splitText masks and autoSplit', () => {
  it('wraps units in clipping masks', () => {
    const el = box();
    el.innerHTML = 'Hello <b>big</b> world';
    const s = splitText(el, { type: 'words', mask: 'words', maskClass: 'm' });
    expect(s.words.length).toBe(3);
    expect(s.masks.length).toBe(3);
    for (let i = 0; i < 3; i++) {
      expect(s.words[i].parentElement).toBe(s.masks[i]);
      expect(s.masks[i].style.overflow).toBe('clip');
      expect(s.masks[i].className).toBe('m');
    }
    expect(el.querySelector('b')!.textContent).toBe('big');
    expect(el.getAttribute('aria-label')).toBe('Hello big world');
    s.revert();
    expect(el.innerHTML).toBe('Hello <b>big</b> world');
    expect(el.hasAttribute('aria-label')).toBe(false);
  });

  it('masks lines', () => {
    const el = box();
    el.textContent = 'one two three';
    const s = splitText(el, { type: 'lines', mask: 'lines' });
    expect(s.lines.length).toBe(1); // no layout in happy-dom: a single line
    expect(s.masks[0].style.display).toBe('block');
    expect(s.lines[0].parentElement).toBe(s.masks[0]);
  });

  it('re-splits on width changes and hands the animation over', async () => {
    const G = globalThis as any;
    const RO = G.ResizeObserver;
    let fire: ((entries: any[]) => void) | null = null;
    G.ResizeObserver = class {
      constructor(cb: (e: any[]) => void) {
        fire = cb;
      }
      observe() {}
      disconnect() {}
    };
    try {
      const el = box();
      el.textContent = 'ab cd';
      const anims: any[] = [];
      const s = splitText(el, {
        type: 'chars', autoSplit: true,
        onSplit: (r) => {
          const a = animate(r.chars, { opacity: [0, 1] }, { duration: 1, ease: 'linear' });
          anims.push(a);
          return a;
        },
      });
      const firstChars = s.chars.slice();
      expect(anims.length).toBe(1);
      advance(0.5);
      fire!([{ target: el, contentRect: { width: 300 } }]);
      fire!([{ target: el, contentRect: { width: 200 } }]);
      await new Promise((r) => setTimeout(r, 50));
      expect(anims.length).toBe(2);
      expect(s.chars.length).toBe(4);
      expect(s.chars[0]).not.toBe(firstChars[0]);
      expect(firstChars[0].isConnected).toBe(false);
      // The new animation continues from the same time.
      expect(anims[1].time).toBeCloseTo(0.5, 1);
      s.revert();
      expect(el.textContent).toBe('ab cd');
      expect(el.children.length).toBe(0);
    } finally {
      G.ResizeObserver = RO;
    }
  });
});

describe('complex values', () => {
  const at = (from: string, to: string, t: number, options: Record<string, unknown> = {}) => {
    const o = { v: from };
    animate(o, { v: [from, to] }, { duration: 1, ease: 'linear', ...options });
    advance(t);
    return o.v;
  };

  it('interpolates colours inside strings in the colour space, premultiplied', () => {
    // Fading in from transparent black stays red (straight RGB would go dark).
    expect(at('0px 0px 0px rgba(0,0,0,0)', '0px 0px 10px rgba(255,0,0,1)', 0.5)).toBe('0px 0px 5px rgba(255,0,0,0.5)');
    const g = at('linear-gradient(90deg, rgb(255,0,0), rgb(0,0,255))', 'linear-gradient(90deg, rgb(0,0,255), rgb(255,0,0))', 0.5);
    const mid = mixColor('rgb(255,0,0)', 'rgb(0,0,255)', 0.5, 'oklab');
    expect(g).toBe(`linear-gradient(90deg, ${mid}, ${mid})`);
    const srgb = at('linear-gradient(red, blue)', 'linear-gradient(blue, red)', 0.5, { colorSpace: 'srgb' });
    expect(srgb).toBe('linear-gradient(rgb(128,0,128), rgb(128,0,128))');
  });

  it('fills filter lists with identity values (none → list, different functions)', () => {
    expect(at('none', 'brightness(1.5) blur(4px)', 0.5)).toBe('brightness(1.25) blur(2px)');
    expect(at('blur(4px)', 'grayscale(100%)', 0.5)).toBe('blur(2px) grayscale(0.5)');
    expect(at('saturate(2) hue-rotate(0.5turn)', 'hue-rotate(0)', 0.5)).toBe('saturate(1.5) hue-rotate(90deg)');
  });

  it('lines up shadow lists (colour first or last, different counts, bare zeros)', () => {
    const v = at('rgba(0, 0, 0, 0.2) 0px 4px 12px 0px', '0 8px 24px rgba(0,0,0,.4), inset 0 0 0 2px rgb(255,0,0)', 0.5);
    expect(v).toBe('0px 6px 18px 0px rgba(0,0,0,0.3), inset 0px 0px 0px 1px rgba(255,0,0,0.5)');
    expect(at('none', '2px 2px red', 0.5)).toBe('1px 1px rgba(255,0,0,0.5)');
  });
});

describe('gesture', () => {
  /** A 200×200 box at the origin whose rect follows its translate/scale. */
  function stage() {
    const el = box();
    (el as any).getBoundingClientRect = () => {
      const m = /translate\(([-\d.]+)px,([-\d.]+)px\)/.exec(el.style.transform);
      const sc = /scale\(([-\d.]+)/.exec(el.style.transform);
      const t = [m ? +m[1] : 0, m ? +m[2] : 0, sc ? +sc[1] : 1];
      const w = 200 * t[2];
      const cx = 100 + t[0];
      const cy = 100 + t[1];
      return { left: cx - w / 2, top: cy - w / 2, width: w, height: w, right: cx + w / 2, bottom: cy + w / 2 } as DOMRect;
    };
    return el;
  }
  const ptr = (type: string, id: number, x: number, y: number, target: EventTarget = window) =>
    target.dispatchEvent(new PointerEvent(type, { pointerId: id, clientX: x, clientY: y, bubbles: true, button: 0 }));

  it('pinches about the fingers, keeping that point fixed', () => {
    const el = stage();
    const changes: number[] = [];
    const [g] = gesture(el, { onChange: (s) => changes.push(s.scale) });
    ptr('pointerdown', 1, 150, 150, el);
    ptr('pointerdown', 2, 170, 150, el);
    ptr('pointermove', 1, 140, 150);
    ptr('pointermove', 2, 180, 150);
    expect(g.scale).toBeCloseTo(2, 9);
    expect(g.x).toBeCloseTo(-60, 9);
    expect(g.y).toBeCloseTo(-50, 9);
    expect(el.style.transform).toBe('translate(-60px,-50px) scale(2,2)');
    ptr('pointerup', 1, 140, 150);
    ptr('pointerup', 2, 180, 150);
    expect(changes.length).toBeGreaterThan(0);
    g.kill();
  });

  it('rotates with two fingers and pans with one', () => {
    const el = stage();
    const [g] = gesture(el, { inertia: false });
    ptr('pointerdown', 1, 100, 50, el);
    ptr('pointerdown', 2, 100, 150, el);
    ptr('pointermove', 1, 50, 100);
    ptr('pointermove', 2, 150, 100);
    expect(g.rotation).toBeCloseTo(-90, 9);
    expect(g.x).toBeCloseTo(0, 9);
    ptr('pointerup', 2, 150, 100);
    // One finger left: it pans from here.
    ptr('pointermove', 1, 80, 130);
    expect(g.x).toBeCloseTo(30, 9);
    expect(g.y).toBeCloseTo(30, 9);
    ptr('pointerup', 1, 80, 130);
    g.kill();
  });

  it('rubber-bands past maxScale and springs back on release', () => {
    const el = stage();
    const [g] = gesture(el, { maxScale: 2, inertia: false });
    ptr('pointerdown', 1, 90, 100, el);
    ptr('pointerdown', 2, 110, 100, el);
    ptr('pointermove', 1, 60, 100);
    ptr('pointermove', 2, 140, 100); // ×4 requested
    expect(g.scale).toBeCloseTo(2 * Math.pow(2, 0.35), 6);
    ptr('pointerup', 1, 60, 100);
    ptr('pointerup', 2, 140, 100);
    advance(1.5);
    expect(g.scale).toBeCloseTo(2, 2);
    expect(el.style.transform).toMatch(/scale\(2,2\)|scale\(1\.99\d*,1\.99\d*\)/);
    g.kill();
  });

  it('zooms with Ctrl + wheel about the pointer', () => {
    const el = stage();
    const [g] = gesture(el, { wheelSpeed: 0.01 });
    const ev = new WheelEvent('wheel', { deltaY: -100, ctrlKey: true, clientX: 100, clientY: 100, cancelable: true });
    // (happy-dom's WheelEvent drops modifier keys)
    Object.defineProperties(ev, { ctrlKey: { value: true }, clientX: { value: 100 }, clientY: { value: 100 } });
    el.dispatchEvent(ev);
    expect(ev.defaultPrevented).toBe(true);
    expect(g.scale).toBeCloseTo(Math.E, 6);
    expect(g.x).toBeCloseTo(0, 9);
    // A plain wheel scrolls the page as usual.
    const plain = new WheelEvent('wheel', { deltaY: -100, cancelable: true });
    el.dispatchEvent(plain);
    expect(plain.defaultPrevented).toBe(false);
    g.kill();
  });
});

describe("reduced motion: 'fade' mode", () => {
  it('holds movement at its end value but still animates opacity and colour', () => {
    engine.reducedMotion = 'always';
    engine.reducedMotionMode = 'fade';
    try {
      const el = box();
      const o = { c: 'rgb(255,0,0)' };
      animate(el, { x: [0, 100], scale: [0.5, 1], opacity: [0, 1], height: [0, 50] }, { duration: 1, ease: 'linear' });
      animate(o, { c: 'rgb(0,0,255)' }, { duration: 1, ease: 'linear', colorSpace: 'srgb' });
      advance(0.5);
      expect(el.style.transform).toBe('translate(100px,0px)');
      expect(el.style.height).toBe('50px');
      expect(el.style.opacity).toBe('0.5');
      expect(o.c).toBe('rgb(128,0,128)');
    } finally {
      engine.reducedMotion = 'never';
      engine.reducedMotionMode = 'instant';
    }
  });

  it("'instant' (default) still jumps everything", () => {
    engine.reducedMotion = 'always';
    try {
      const el = box();
      animate(el, { x: [0, 100], opacity: [0, 1] }, { duration: 1, ease: 'linear' });
      advance(0.05);
      expect(el.style.transform).toBe('translate(100px,0px)');
      expect(el.style.opacity).toBe('1');
    } finally {
      engine.reducedMotion = 'never';
    }
  });
});

describe('timeline onUpdate', () => {
  it('fires once per frame while playing and on seeks, then stops', async () => {
    const el = box();
    const seen: number[] = [];
    const tl = timeline({ onUpdate: (t) => seen.push(Math.round(t.progress * 100)) }).add(el, { x: 100 }, { duration: 1, ease: 'linear' });
    await tick();
    advance(0.5);
    const playing = seen.length;
    expect(playing).toBeGreaterThanOrEqual(29);
    expect(seen[seen.length - 1]).toBe(50);
    tl.pause().seek(0.2);
    expect(seen[seen.length - 1]).toBe(20);
    const n = seen.length;
    advance(0.5); // paused: nothing moves, no updates
    expect(seen.length).toBe(n);
    tl.play();
    advance(1.5);
    expect(seen[seen.length - 1]).toBe(100);
    const done = seen.length;
    advance(0.5);
    expect(seen.length).toBe(done);
  });
});

describe('motion path alignment', () => {
  it('transformPath applies an affine matrix to any path', () => {
    expect(transformPath('M0 0 L10 0', [2, 0, 0, 2, 5, 5])).toBe('M5 5C11.667 5 18.333 5 25 5');
    // 90° rotation
    expect(transformPath('M1 0 L2 0', [0, 1, -1, 0, 0, 0])).toBe('M0 1C0 1.333 0 1.667 0 2');
  });

  it('align maps the path from SVG space onto the target', () => {
    const svgNS = 'http://www.w3.org/2000/svg';
    const svg = document.createElementNS(svgNS, 'svg');
    const path = document.createElementNS(svgNS, 'path');
    path.setAttribute('d', 'M0 0 L100 0');
    svg.appendChild(path);
    document.body.appendChild(svg);
    // The SVG is drawn at (50, 20), scaled ×3 (viewBox).
    (path as any).getScreenCTM = () => ({ a: 3, b: 0, c: 0, d: 3, e: 50, f: 20 });
    const el = box();
    (el as any).getBoundingClientRect = () => ({ left: 10, top: 10, width: 20, height: 20, right: 30, bottom: 30 });
    const a = animate(el, { path: { path, align: true } }, { duration: 1, ease: 'linear', autoplay: false });
    a.seek(0);
    // Centre (20, 20) moves onto (50, 20).
    expect(el.style.transform).toBe('translate(30px,0px)');
    a.seek(0.5);
    expect(el.style.transform).toBe('translate(180px,0px)');
  });
});
