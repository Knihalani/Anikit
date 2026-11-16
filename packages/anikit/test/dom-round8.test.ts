import { describe, it, expect, beforeAll, vi } from 'vitest';
import { spring, smoothScroll, scroll, animate, scrollTrigger, VelocityTracker, observer } from '../src/index';
import { setup, advance } from './setup';

beforeAll(setup);

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

describe('spring() listeners', () => {
  it('a new { onUpdate } on every retarget runs once per frame, not once per call', () => {
    const o = { x: 0 };
    let calls = 0;
    for (let i = 0; i < 50; i++) spring(o, { x: 100 + i }, { stiffness: 300, onUpdate: () => calls++ });
    advance(1 / 60);
    calls = 0;
    advance(5 / 60);
    expect(calls).toBe(5);
  });

  it('the latest onUpdate is the one that runs', () => {
    const o = { x: 0 };
    const seen: string[] = [];
    spring(o, { x: 100 }, { onUpdate: () => seen.push('a') });
    spring(o, { x: 200 }, { onUpdate: () => seen.push('b') });
    advance(2 / 60);
    expect(seen.length).toBe(2);
    expect(new Set(seen)).toEqual(new Set(['b']));
  });

  it('every distinct onComplete still fires once at rest; a repeated one fires once', async () => {
    const o = { x: 0 };
    let a = 0;
    let b = 0;
    const onA = () => a++;
    for (let i = 0; i < 10; i++) spring(o, { x: 50 }, { onComplete: onA });
    spring(o, { x: 60 }, { onComplete: () => b++, onUpdate: () => {} });
    advance(5);
    expect(a).toBe(1);
    expect(b).toBe(1);
    expect(o.x).toBeCloseTo(60, 1);
  });

  it('onUpdate stops after rest and a new call after rest adds it again', () => {
    const o = { x: 0 };
    let n = 0;
    spring(o, { x: 10 }, { onUpdate: () => n++ });
    advance(5);
    n = 0;
    advance(0.1);
    expect(n).toBe(0);
    spring(o, { x: 20 }, { onUpdate: () => n++ });
    advance(2 / 60);
    expect(n).toBe(2);
  });
});

describe('smoothScroll()', () => {
  it('wheel events do not pile up per-frame writes', () => {
    const se = (document.scrollingElement || document.documentElement) as HTMLElement;
    Object.defineProperty(se, 'scrollHeight', { value: 10000, configurable: true });
    let n = 0;
    const s = smoothScroll({ onUpdate: () => n++ });
    for (let i = 0; i < 40; i++) window.dispatchEvent(new WheelEvent('wheel', { deltaY: 20, cancelable: true }));
    advance(1 / 60);
    n = 0;
    advance(4 / 60);
    expect(n).toBe(4);
    s.destroy();
    delete (se as any).scrollHeight;
  });
});

describe('smoothed scroll links outside their range', () => {
  it('scroll({ smooth }) still settles on the clamped end after many events past it', () => {
    const s = scroller();
    Object.defineProperty(s, 'scrollHeight', { value: 2000, configurable: true });
    Object.defineProperty(s, 'clientHeight', { value: 1000, configurable: true });
    const seen: number[] = [];
    const off = scroll((p) => seen.push(p), { container: s, smooth: 0.1 });
    for (let y = 1000; y < 1100; y += 5) scrollTo(s, y);
    advance(2);
    expect(seen[seen.length - 1]).toBeCloseTo(1, 3);
    scrollTo(s, 500);
    advance(2);
    expect(seen[seen.length - 1]).toBeCloseTo(0.5, 3);
    off();
  });

  it('a scrubbed trigger with a numeric scrub still follows after events past its end', () => {
    const s = scroller();
    const o = { x: 0 };
    const a = animate(o, { x: [0, 100] }, { ease: 'linear', duration: 1 });
    const st = scrollTrigger({ scroller: s, start: 100, end: 300, animation: a, scrub: 0.2 });
    scrollTo(s, 200);
    advance(2);
    expect(o.x).toBeCloseTo(50, 0);
    for (let y = 400; y < 600; y += 10) scrollTo(s, y);
    advance(2);
    expect(o.x).toBeCloseTo(100, 0);
    scrollTo(s, 150);
    advance(2);
    expect(o.x).toBeCloseTo(25, 0);
    st.kill();
  });
});

describe('VelocityTracker', () => {
  it('matches the pair-list version on long irregular sequences', () => {
    // The previous implementation, kept here as the reference.
    class Ref {
      s: Array<[number, number]> = [];
      add(v: number, t: number) {
        this.s.push([t, v]);
        while (this.s.length > 2 && this.s[0][0] < t - 100) this.s.shift();
      }
      get velocity() {
        const s = this.s;
        if (s.length < 2) return 0;
        const [t0, v0] = s[0];
        const [t1, v1] = s[s.length - 1];
        return t1 > t0 ? ((v1 - v0) / (t1 - t0)) * 1000 : 0;
      }
    }
    let seed = 7;
    const rnd = () => ((seed = (seed * 16807) % 2147483647) / 2147483647);
    const a = new VelocityTracker();
    const b = new Ref();
    let t = 0;
    for (let i = 0; i < 5000; i++) {
      t += rnd() < 0.1 ? 150 : rnd() * 4; // bursts (many samples per window) and gaps
      const v = Math.sin(i / 30) * 300 + rnd();
      a.add(v, t);
      b.add(v, t);
      expect(a.velocity).toBe(b.velocity);
      if (i === 2500) {
        a.reset();
        b.s.length = 0;
      }
    }
  });
});

describe('observer() onStop', () => {
  it('fires once, 150 ms after the last event of a burst, with that event', () => {
    vi.useFakeTimers();
    try {
      const el = document.createElement('div');
      document.body.appendChild(el);
      const stops: number[] = [];
      const ob = observer({ target: el, type: 'wheel', onStop: (e) => stops.push(e.deltaY) });
      for (let i = 1; i <= 20; i++) {
        el.dispatchEvent(new WheelEvent('wheel', { deltaY: i }));
        vi.advanceTimersByTime(40); // a burst: never 150 ms apart
      }
      expect(stops).toEqual([]);
      vi.advanceTimersByTime(109);
      expect(stops).toEqual([]);
      vi.advanceTimersByTime(2);
      expect(stops).toEqual([20]);
      vi.advanceTimersByTime(1000);
      expect(stops).toEqual([20]);
      // A second burst stops again.
      el.dispatchEvent(new WheelEvent('wheel', { deltaY: 5 }));
      vi.advanceTimersByTime(151);
      expect(stops).toEqual([20, 5]);
      // kill() cancels a pending stop.
      el.dispatchEvent(new WheelEvent('wheel', { deltaY: 7 }));
      ob.kill();
      vi.advanceTimersByTime(500);
      expect(stops).toEqual([20, 5]);
    } finally {
      vi.useRealTimers();
    }
  });
});
