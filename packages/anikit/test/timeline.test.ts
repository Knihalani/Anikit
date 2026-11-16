import { describe, it, expect, beforeAll, vi } from 'vitest';
import { timeline, animate, engine, flush } from '../src/index';
import { _recordCount } from '../src/core';
import { setup, advance, box } from './setup';

beforeAll(setup);
const tf = (el: HTMLElement) => el.style.transform;
const tick = () => new Promise((r) => queueMicrotask(() => r(undefined)));

describe('timeline', () => {
  it('kill() frees every node and record (children, nested timelines, calls) and children stay killed', async () => {
    flush();
    const nodes0 = engine.stats().nodes, recs0 = _recordCount();
    const objs = Array.from({ length: 40 }, () => ({ x: 0, y: 0 }));
    const inner = timeline({ autoplay: false }).add(objs[0], { y: 5 }, { duration: 0.2 }).add(objs[1], { y: 6 }, { duration: 0.2 });
    const adopted = animate(objs[2], { y: 9 }, { duration: 0.3, autoplay: false });
    const tl = timeline();
    for (let i = 3; i < objs.length; i++) tl.add([objs[i]], { x: i }, { duration: 0.5 }, i * 0.01);
    tl.add(inner, 0).add(adopted, 0.1).call(() => {}, 0.2);
    await tick();
    advance(0.25);
    expect(objs[10].x).toBeGreaterThan(0);
    expect(engine.stats().nodes).toBeGreaterThan(nodes0);
    tl.kill();
    flush();
    expect(engine.stats().nodes).toBe(nodes0);
    expect(_recordCount()).toBe(recs0);
    const x = objs[10].x, y = objs[0].y;
    advance(0.5);
    expect(objs[10].x).toBe(x);
    expect(objs[0].y).toBe(y);
    await expect(inner.finished).resolves.toBeUndefined();
    await expect(adopted.finished).resolves.toBeUndefined();
    // Their own kill()/seek() on released nodes are harmless.
    inner.kill();
    adopted.seek(0.1);
    flush();
    expect(engine.stats().nodes).toBe(nodes0);
  });

  it('sequences, chains values and seeks in both directions', async () => {
    const el = box();
    const tl = timeline({ defaults: { duration: 1, ease: 'linear' } })
      .add(el, { x: 100 })
      .add(el, { x: 200 })
      .add(el, { y: 50 }, {}, '<');
    await tick();
    expect(tl.duration).toBe(2);
    advance(1.5);
    expect(tf(el)).toBe('translate(150px,25px)');
    tl.pause().seek(0.5);
    expect(tf(el)).toBe('translate(50px,0px)');
    tl.seek(2);
    expect(tf(el)).toBe('translate(200px,50px)');
    tl.seek(0);
    expect(tf(el)).toBe('none');
  });

  it('supports position parameters and labels', async () => {
    const a = box();
    const b = box();
    const tl = timeline({ autoplay: false, defaults: { duration: 1, ease: 'linear' } })
      .add(a, { x: 100 })
      .label('mid', '-=0.5')
      .add(b, { x: 100 }, {}, 'mid')
      .add(a, { y: 100 }, {}, 'mid+=1');
    await tick();
    expect(tl.labels.get('mid')).toBe(0.5);
    expect(tl.duration).toBe(2.5);
    tl.seek(1);
    expect(tf(b)).toBe('translate(50px,0px)');
    tl.seek('mid');
    expect(tf(b)).toBe('none');
  });

  it('fires call() in both directions and nests timelines', async () => {
    const fn = vi.fn();
    const el = box();
    const inner = timeline({ defaults: { duration: 1, ease: 'linear' } }).add(el, { x: 100 });
    const outer = timeline({ autoplay: false }).add(inner, 1).call(fn, 1.5);
    await tick();
    expect(outer.duration).toBe(2);
    outer.seek(1.5);
    expect(tf(el)).toBe('translate(50px,0px)');
    expect(fn).toHaveBeenCalledTimes(1);
    outer.seek(1.2);
    expect(fn).toHaveBeenCalledTimes(2);
  });

  it('repeats with yoyo', async () => {
    const el = box();
    const onComplete = vi.fn();
    timeline({ repeat: 1, yoyo: true, onComplete }).add(el, { x: [0, 100] }, { duration: 1, ease: 'linear' });
    await tick();
    advance(1.5);
    expect(tf(el)).toBe('translate(50px,0px)');
    advance(1);
    expect(tf(el)).toBe('none');
    expect(onComplete).toHaveBeenCalledTimes(1);
  });

  it('accepts existing animations', async () => {
    const el = box();
    const a = animate(el, { x: [0, 10] }, { duration: 1, ease: 'linear' });
    const tl = timeline({ autoplay: false }).add(a, 2);
    await tick();
    expect(tl.duration).toBe(3);
    tl.seek(2.5);
    expect(tf(el)).toBe('translate(5px,0px)');
  });
});
