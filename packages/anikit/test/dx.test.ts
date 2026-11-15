import { describe, it, expect, beforeAll } from 'vitest';
import { animate, engine, effect, testing, spring } from '../src/index';
import { setup, box } from './setup';

beforeAll(setup);

describe('developer experience', () => {
  it('suggests fixes for typos', () => {
    const warnings = testing.captureWarnings();
    const el = box();
    animate(el, { x: 10 }, { durration: 1 } as any);
    expect(warnings.some((w) => w.includes('unknown animation option "durration"') && w.includes('Did you mean "duration"'))).toBe(true);
    animate(el, { colour: 'red' } as any, { duration: 0.1 });
    expect(warnings.some((w) => w.includes('"colour" is not a CSS property') && w.includes('"color"'))).toBe(true);
    animate('.does-not-exist', { x: 1 });
    expect(warnings.some((w) => w.includes('selector ".does-not-exist" matched no elements'))).toBe(true);
    expect(() => animate(el, { x: 1 }, { ease: 'outCubik' })).toThrow(/Did you mean "outCubic"/);
    expect(() => effect('fadeInn', el)).toThrow(/Did you mean "fadeIn"/);
  });

  it('inspects running animations as JSON', () => {
    testing.reset();
    const el = box();
    el.id = 'hero';
    animate(el, { x: 100, opacity: 0.5 }, { duration: 2, ease: 'linear' });
    spring({ v: 0 }, { v: 10 });
    testing.advance(0.5);
    const snap = engine.inspect();
    const tween = snap.find((s) => s.type === 'tween')!;
    expect(tween.target).toBe('div#hero');
    expect(tween.props).toEqual(['x', 'opacity']);
    expect(tween.progress).toBe(0.25);
    expect(tween.state).toBe('running');
    expect(snap.some((s) => s.type === 'physics')).toBe(true);
    expect(JSON.parse(JSON.stringify(snap))).toEqual(snap);
    const t = testing.finishAll();
    expect(t).toBeGreaterThan(1);
    expect(engine.stats().active).toBe(0);
  });
});
