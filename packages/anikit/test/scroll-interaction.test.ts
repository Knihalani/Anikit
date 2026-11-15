import { describe, it, expect, beforeAll, vi } from 'vitest';
import { animate, scrollTrigger, draggable, observer, hover, press, inspector, timeline } from '../src/index';
import { setup, advance, box } from './setup';

beforeAll(setup);
const tf = (el: HTMLElement) => el.style.transform;

function scroller(): HTMLElement {
  const s = document.createElement('div');
  document.body.appendChild(s);
  return s;
}
function scrollTo(s: HTMLElement, y: number) {
  s.scrollTop = y;
  if (s.scrollTop !== y) Object.defineProperty(s, 'scrollTop', { value: y, configurable: true, writable: true });
  s.dispatchEvent(new Event('scroll'));
}

describe('scrollTrigger', () => {
  it('scrubs an animation between numeric start/end', () => {
    const s = scroller();
    const el = box();
    const a = animate(el, { x: [0, 100] }, { ease: 'linear', duration: 1 });
    const st = scrollTrigger({ scroller: s, start: 100, end: 300, animation: a, scrub: true });
    scrollTo(s, 200);
    expect(st.progress).toBe(0.5);
    expect(tf(el)).toBe('translate(50px,0px)');
    scrollTo(s, 1000);
    expect(tf(el)).toBe('translate(100px,0px)');
    st.kill();
  });

  it('does not snap back after scrolling past the end', () => {
    vi.useFakeTimers();
    try {
      const s = scroller();
      const st = scrollTrigger({ scroller: s, start: 100, end: 300, snap: 0.5 });
      scrollTo(s, 240); // inside: a snap is scheduled
      scrollTo(s, 900); // ...but we leave before it fires
      vi.advanceTimersByTime(400);
      advance(1);
      expect(s.scrollTop).toBe(900);
      st.kill();
    } finally {
      vi.useRealTimers();
    }
  });

  it('fires callbacks and toggle actions in both directions', () => {
    const s = scroller();
    const el = box();
    const a = animate(el, { x: [0, 100] }, { ease: 'linear', duration: 1 });
    const log: string[] = [];
    const st = scrollTrigger({
      scroller: s, start: 100, end: 200, animation: a, toggleActions: 'play pause resume reverse', toggleClass: { targets: el, className: 'on' },
      onEnter: () => log.push('enter'), onLeave: () => log.push('leave'), onEnterBack: () => log.push('enterBack'), onLeaveBack: () => log.push('leaveBack'),
    });
    expect(a.state).toBe('paused');
    scrollTo(s, 150);
    expect(el.classList.contains('on')).toBe(true);
    advance(0.5);
    expect(tf(el)).toBe('translate(50px,0px)');
    scrollTo(s, 250);
    expect(el.classList.contains('on')).toBe(false);
    scrollTo(s, 150);
    scrollTo(s, 50);
    expect(log).toEqual(['enter', 'leave', 'enterBack', 'leaveBack']);
    st.kill();
  });

  it('smooths scrubbing with a spring', () => {
    const s = scroller();
    const el = box();
    const a = animate(el, { x: [0, 100] }, { ease: 'linear', duration: 1 });
    scrollTrigger({ scroller: s, start: 0, end: 100, animation: a, scrub: 0.3 });
    scrollTo(s, 100);
    advance(1 / 60);
    const mid = parseFloat(tf(el).slice(10));
    expect(mid).toBeGreaterThan(0);
    expect(mid).toBeLessThan(100);
    advance(3);
    expect(tf(el)).toBe('translate(100px,0px)');
  });

  it('works with timelines and once', async () => {
    const s = scroller();
    const el = box();
    const tl = timeline({ autoplay: false }).add(el, { x: 100 }, { duration: 1, ease: 'linear' });
    await new Promise((r) => queueMicrotask(() => r(0)));
    const enter = vi.fn();
    scrollTrigger({ scroller: s, start: 10, end: 20, animation: tl, once: true, onEnter: enter });
    scrollTo(s, 15);
    scrollTo(s, 0);
    scrollTo(s, 15);
    expect(enter).toHaveBeenCalledTimes(1);
  });
});

function pointer(type: string, x: number, y: number, target: EventTarget = window) {
  const e = new (globalThis.PointerEvent ?? MouseEvent)(type, { clientX: x, clientY: y, bubbles: true, button: 0 } as any);
  target.dispatchEvent(e);
}

describe('interaction', () => {
  it('drags with bounds and snaps on release', () => {
    const el = box();
    const [d] = draggable(el, { bounds: { minX: 0, maxX: 200, minY: 0, maxY: 0 }, edgeResistance: 0, snap: { x: 50 } });
    pointer('pointerdown', 10, 10, el);
    pointer('pointermove', 40, 30);
    pointer('pointermove', 82, 60);
    expect(d.isDragging).toBe(true);
    expect(tf(el)).toBe('translate(72px,0px)');
    pointer('pointermove', 400, 10);
    expect(d.x).toBe(200);
    pointer('pointermove', 82, 10);
    pointer('pointerup', 82, 10);
    advance(3);
    expect(d.isDragging).toBe(false);
    expect(tf(el)).toBe('translate(50px,0px)');
    d.kill();
  });

  it('observer reports wheel intents', () => {
    const t = box();
    const down = vi.fn();
    const up = vi.fn();
    const ob = observer({ target: t, type: 'wheel', tolerance: 20, onDown: down, onUp: up });
    t.dispatchEvent(new WheelEvent('wheel', { deltaY: 10 }));
    expect(down).not.toHaveBeenCalled();
    t.dispatchEvent(new WheelEvent('wheel', { deltaY: 15 }));
    expect(down).toHaveBeenCalledTimes(1);
    t.dispatchEvent(new WheelEvent('wheel', { deltaY: -40 }));
    expect(up).toHaveBeenCalledTimes(1);
    ob.kill();
  });

  it('hover and press gestures', () => {
    const el = box();
    const end = vi.fn();
    hover(el, () => end);
    el.dispatchEvent(new (globalThis.PointerEvent ?? MouseEvent)('pointerenter', { pointerType: 'mouse' } as any));
    el.dispatchEvent(new (globalThis.PointerEvent ?? MouseEvent)('pointerleave', { pointerType: 'mouse' } as any));
    expect(end).toHaveBeenCalledTimes(1);
    const rel = vi.fn();
    press(el, () => rel);
    pointer('pointerdown', 0, 0, el);
    pointer('pointerup', 0, 0);
    expect(rel).toHaveBeenCalledTimes(1);
  });

  it('inspector mounts and unmounts', () => {
    const a = animate(box(), { x: 100 }, { duration: 1 });
    const n = document.body.children.length;
    const ins = inspector(a, { title: 'test' });
    expect(document.body.children.length).toBe(n + 1);
    advance(0.1);
    expect(document.body.lastElementChild!.textContent).toContain('test');
    ins.destroy();
    expect(document.body.children.length).toBe(n);
  });
});
