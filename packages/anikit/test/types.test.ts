import { describe, it, expect, expectTypeOf, beforeAll } from 'vitest';
import { easing, effects, effectNames } from '../src/index';
import type { EasingName, EasingInput, EffectName, Props } from '../src/index';
import { setup } from './setup';

beforeAll(setup);

// Mirrors the template-literal parts of `EasingName` (src/core.ts). If the
// type gains a member the parser rejects, this test fails.
const curves = ['Quad', 'Cubic', 'Quart', 'Quint', 'Sine', 'Expo', 'Circ', 'Back', 'Elastic', 'Bounce'];
const gsap = ['power1', 'power2', 'power3', 'power4', 'sine', 'expo', 'circ', 'back', 'elastic', 'bounce'];
const presets = ['gentle', 'wobbly', 'stiff', 'slow', 'molasses', 'snappy', 'bouncy'];
const names: EasingName[] = [
  'linear', 'ease', 'ease-in', 'ease-out', 'ease-in-out', 'step-start', 'step-end', 'spring', 'anticipate', 'wiggle', 'shake', 'slowmo', 'rough',
  ...curves.flatMap((c) => ['in', 'out', 'inOut', 'outIn'].map((m) => `${m}${c}` as EasingName)),
  ...curves.flatMap((c) => ['In', 'Out', 'InOut'].map((m) => `ease${m}${c}` as EasingName)),
  ...gsap.flatMap((g) => [g, `${g}.in`, `${g}.out`, `${g}.inOut`] as EasingName[]),
  ...presets.flatMap((p) => [p, `spring.${p}`] as EasingName[]),
];

describe('types match the runtime', () => {
  it('every EasingName parses', () => {
    for (const n of names) expect(() => easing(n), n).not.toThrow();
    expect(names.length).toBeGreaterThan(130);
    expect(() => easing('outCubik')).toThrow(/Did you mean/);
  });

  it('EasingInput still accepts any string and the other forms', () => {
    expectTypeOf<'cubic-bezier(.2,.8,.2,1)'>().toMatchTypeOf<EasingInput>();
    expectTypeOf<[number, number, number, number]>().toMatchTypeOf<EasingInput>();
    expectTypeOf<{ bounce: number; duration: number }>().toMatchTypeOf<EasingInput>();
    expectTypeOf<(t: number) => number>().toMatchTypeOf<EasingInput>();
  });

  it('every built-in effect is typed, and every typed effect exists', () => {
    const typed: EffectName[] = [
      'fadeIn', 'fadeOut', 'slideIn', 'slideOut', 'scaleIn', 'scaleOut', 'zoomIn', 'zoomOut', 'pop', 'bounceIn', 'bounceOut',
      'dropIn', 'flipIn', 'flipOut', 'rotateIn', 'blurIn', 'blurOut', 'revealClip', 'cascade', 'shake', 'headShake', 'wobble',
      'jello', 'rubberBand', 'swing', 'tada', 'pulse', 'heartbeat', 'flash', 'highlight', 'float', 'breathe', 'spin',
      'wiggleLoop', 'typewriter', 'scramble', 'countUp', 'drawSVG', 'undrawSVG',
    ];
    expect([...effectNames()].sort()).toEqual([...typed].sort());
    expectTypeOf(effects.fadeIn).toBeFunction();
  });

  it('Props documents the special keys and still takes any CSS property', () => {
    const p: Props = { x: 100, rotate: '1turn', backgroundColor: '#0af', '--hue': 120, attr: { r: 20 }, path: { path: '#track', rotate: true } };
    expect(Object.keys(p).length).toBe(6);
  });
});
