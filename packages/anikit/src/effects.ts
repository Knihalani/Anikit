/**
 * Effects: ready-made, configurable animations (entrances, exits, attention
 * seekers, loops, text and SVG effects) plus `registerEffect()` for your own.
 *
 * ```ts
 * effects.fadeIn('.card', { stagger: 0.05 });
 * effects.slideIn('.menu', { from: 'left', distance: 80 });
 * effects.shake(input);
 * effects.countUp('#total', { to: 12500, format: { style: 'currency', currency: 'USD' } });
 * registerEffect('glow', (t, o) => animate(t, { filter: ['drop-shadow(0 0 0px gold)', `drop-shadow(0 0 ${o.size}px gold)`] }, o), { size: 12 });
 * effects.glow('.star');
 * ```
 */
import { animate, AnimateOptions, Animation, Props } from './animate';
import { Targets } from './props';
import { stagger } from './stagger';
import { Playable } from './playable';
import { didYouMean } from './debug';

export interface EffectOptions extends AnimateOptions {
  /** Seconds between targets (shorthand for `delay: stagger(n)`). */
  stagger?: number;
  /** Travel distance for slide/drop effects (px or any length). */
  distance?: number | string;
  /** Direction for slide effects. */
  from?: 'up' | 'down' | 'left' | 'right';
  /** Strength multiplier for attention effects (default 1). */
  intensity?: number;
  /** Loop forever (attention / looping effects). */
  loop?: boolean;
  /** `countUp`: number to count to (default 100). */
  to?: number;
  /** `countUp`: number to start from (default 0). */
  start?: number;
  /** `typewriter` / `scramble`: the text to reveal. */
  text?: string;
  /** `typewriter`: seconds per character (default 0.04). */
  charDelay?: number;
  /** `typewriter`: reveal per word with `' '`. */
  delimiter?: string;
  /** `scramble`: glyph set, `'upper' | 'lower' | 'digits' | 'symbols' | 'binary'` or your own characters. */
  chars?: string;
  /** `highlight`: flash colour (default `'#fff3a0'`). */
  color?: string;
  /** `flipIn` / `flipOut`: rotation axis (default `'x'`). */
  axis?: 'x' | 'y';
  /** Options of your own effects (see `registerEffect`). */
  [key: string]: any;
}

/** Names of the built-in effects. */
export type EffectName =
  | 'fadeIn' | 'fadeOut' | 'slideIn' | 'slideOut' | 'scaleIn' | 'scaleOut' | 'zoomIn' | 'zoomOut' | 'pop'
  | 'bounceIn' | 'bounceOut' | 'dropIn' | 'flipIn' | 'flipOut' | 'rotateIn' | 'blurIn' | 'blurOut' | 'revealClip' | 'cascade'
  | 'shake' | 'headShake' | 'wobble' | 'jello' | 'rubberBand' | 'swing' | 'tada' | 'pulse' | 'heartbeat' | 'flash' | 'highlight'
  | 'float' | 'breathe' | 'spin' | 'wiggleLoop'
  | 'typewriter' | 'scramble' | 'countUp'
  | 'drawSVG' | 'undrawSVG';

export type EffectFn = (targets: Targets, options: EffectOptions) => Playable;

const registry = new Map<string, { fn: EffectFn; defaults: EffectOptions }>();

/** Register a reusable effect, callable as `effects.name(targets, options)` or `effect('name', …)`. */
export function registerEffect(name: string, fn: EffectFn, defaults: EffectOptions = {}): void {
  registry.set(name, { fn, defaults });
}

/** Run a registered effect by name. */
export function effect(name: string, targets: Targets, options: EffectOptions = {}): Playable {
  const e = registry.get(name);
  if (!e) throw new Error(`anikit: unknown effect "${name}".${didYouMean(name, registry.keys())} Available: ${Array.from(registry.keys()).join(', ')}.`);
  return e.fn(targets, { ...e.defaults, ...options });
}

/** Strip effect-only keys and turn `stagger` into a delay. */
function opts(o: EffectOptions, defaults: AnimateOptions = {}): AnimateOptions {
  const { stagger: st, distance, from, intensity, loop, to, size, color, text, ...rest } = o;
  void distance; void from; void intensity; void to; void size; void color; void text;
  const out: AnimateOptions = { ...defaults, ...rest };
  if (st !== undefined && rest.delay === undefined) out.delay = stagger(st);
  if (loop) out.repeat = Infinity;
  return out;
}

const dist = (o: EffectOptions, d: number) => o.distance ?? d;
const k = (o: EffectOptions) => o.intensity ?? 1;
const axisFor = (from: EffectOptions['from']): ['x' | 'y', number] =>
  from === 'left' ? ['x', -1] : from === 'right' ? ['x', 1] : from === 'down' ? ['y', -1] : ['y', 1];
const neg = (v: number | string, s: number): number | string =>
  typeof v === 'number' ? v * s : s < 0 ? '-' + v : v;

const builtins: Record<EffectName, [EffectFn, EffectOptions?]> = {
  // Entrances / exits
  fadeIn: [(t, o) => animate(t, { opacity: [0, 1] }, opts(o, { ease: 'outQuad' }))],
  fadeOut: [(t, o) => animate(t, { opacity: 0 }, opts(o, { ease: 'inQuad' }))],
  slideIn: [(t, o) => {
    const [ax, s] = axisFor(o.from);
    return animate(t, { [ax]: [neg(dist(o, 40), s), 0], opacity: [0, 1] } as Props, opts(o, { ease: 'outCubic', duration: 0.6 }));
  }],
  slideOut: [(t, o) => {
    const [ax, s] = axisFor(o.from);
    return animate(t, { [ax]: neg(dist(o, 40), -s), opacity: 0 } as Props, opts(o, { ease: 'inCubic', duration: 0.5 }));
  }],
  scaleIn: [(t, o) => animate(t, { scale: [0.85, 1], opacity: [0, 1] }, opts(o, { ease: 'outCubic' }))],
  scaleOut: [(t, o) => animate(t, { scale: 0.85, opacity: 0 }, opts(o, { ease: 'inCubic' }))],
  zoomIn: [(t, o) => animate(t, { scale: [0.3, 1], opacity: [0, 1] }, opts(o, { ease: 'outBack', duration: 0.6 }))],
  zoomOut: [(t, o) => animate(t, { scale: 0.3, opacity: 0 }, opts(o, { ease: 'inBack', duration: 0.5 }))],
  pop: [(t, o) => animate(t, { scale: [0, 1.15, 1], opacity: [0, 1, 1] }, opts(o, { ease: 'outQuad', duration: 0.45, times: [0, 0.7, 1] }))],
  bounceIn: [(t, o) => animate(t, { scale: [0.3, 1.1, 0.9, 1.03, 0.97, 1], opacity: [0, 1, 1, 1, 1, 1] }, opts(o, { ease: 'outSine', duration: 0.75 }))],
  bounceOut: [(t, o) => animate(t, { scale: [1, 0.9, 1.1, 0.3], opacity: [1, 1, 1, 0] }, opts(o, { ease: 'inSine', duration: 0.6 }))],
  dropIn: [(t, o) => animate(t, { y: [neg(dist(o, 120), -1), 0], opacity: [0, 1] } as Props, opts(o, { ease: 'bounce.out', duration: 0.9 }))],
  flipIn: [(t, o) => animate(t, { [o.axis === 'y' ? 'rotateY' : 'rotateX']: [90, 0], perspective: [600, 600], opacity: [0, 1] } as Props, opts(o, { ease: 'outBack', duration: 0.7 }))],
  flipOut: [(t, o) => animate(t, { [o.axis === 'y' ? 'rotateY' : 'rotateX']: 90, perspective: [600, 600], opacity: 0 } as Props, opts(o, { ease: 'inBack', duration: 0.5 }))],
  rotateIn: [(t, o) => animate(t, { rotate: [-200 * k(o), 0], opacity: [0, 1] }, opts(o, { ease: 'outCubic', duration: 0.7 }))],
  blurIn: [(t, o) => animate(t, { filter: [`blur(${12 * k(o)}px)`, 'blur(0px)'], opacity: [0, 1] }, opts(o, { ease: 'outQuad', duration: 0.6 }))],
  blurOut: [(t, o) => animate(t, { filter: `blur(${12 * k(o)}px)`, opacity: 0 }, opts(o, { ease: 'inQuad', duration: 0.5 }))],
  revealClip: [(t, o) => {
    const from: Record<string, string> = { left: 'inset(0 100% 0 0)', right: 'inset(0 0 0 100%)', up: 'inset(100% 0 0 0)', down: 'inset(0 0 100% 0)' };
    return animate(t, { clipPath: [from[o.from ?? 'left'], 'inset(0% 0% 0% 0%)'] }, opts(o, { ease: 'inOutCubic', duration: 0.8 }));
  }],
  cascade: [(t, o) => animate(t, { y: [dist(o, 24), 0], opacity: [0, 1] } as Props, opts(o, { ease: 'outCubic', duration: 0.5, delay: stagger(o.stagger ?? 0.06) }))],

  // Attention seekers
  shake: [(t, o) => { const a = 10 * k(o); return animate(t, { x: [0, -a, a, -a, a, -a * 0.6, a * 0.6, 0] }, opts(o, { ease: 'linear', duration: 0.5 })); }],
  headShake: [(t, o) => { const a = 6 * k(o); return animate(t, { x: [0, -a, a * 0.8, -a * 0.5, a * 0.3, 0], rotateY: [0, -9, 7, -5, 3, 0] }, opts(o, { ease: 'inOutSine', duration: 0.8 })); }],
  wobble: [(t, o) => { const a = k(o); return animate(t, { x: [0, -25 * a, 20 * a, -15 * a, 10 * a, -5 * a, 0], rotate: [0, -5 * a, 3 * a, -3 * a, 2 * a, -1 * a, 0] }, opts(o, { ease: 'inOutSine', duration: 0.9 })); }],
  jello: [(t, o) => { const a = 12.5 * k(o); return animate(t, { skewX: [0, -a, a * 0.5, -a * 0.25, a * 0.125, -a * 0.06, 0], skewY: [0, -a, a * 0.5, -a * 0.25, a * 0.125, -a * 0.06, 0] }, opts(o, { ease: 'inOutSine', duration: 0.9 })); }],
  rubberBand: [(t, o) => animate(t, { scaleX: [1, 1.25, 0.75, 1.15, 0.95, 1.05, 1], scaleY: [1, 0.75, 1.25, 0.85, 1.05, 0.95, 1] }, opts(o, { ease: 'inOutSine', duration: 0.8 }))],
  swing: [(t, o) => { const a = 15 * k(o); return animate(t, { rotate: [0, a, -a * 0.67, a * 0.33, -a * 0.33, 0] }, opts(o, { ease: 'inOutSine', duration: 0.9 })); }],
  tada: [(t, o) => animate(t, { scale: [1, 0.9, 0.9, 1.1, 1.1, 1.1, 1.1, 1], rotate: [0, -3, -3, 3, -3, 3, -3, 0] }, opts(o, { ease: 'linear', duration: 0.9 }))],
  pulse: [(t, o) => animate(t, { scale: [1, 1 + 0.06 * k(o), 1] }, opts(o, { ease: 'inOutSine', duration: 0.8 }))],
  heartbeat: [(t, o) => { const a = 0.15 * k(o); return animate(t, { scale: [1, 1 + a, 1, 1 + a, 1] }, opts(o, { ease: 'inOutSine', duration: 1.2, times: [0, 0.14, 0.28, 0.42, 0.7] })); }],
  flash: [(t, o) => animate(t, { opacity: [1, 0, 1, 0, 1] }, opts(o, { ease: 'linear', duration: 0.8 }))],
  highlight: [(t, o) => animate(t, { backgroundColor: [o.color ?? '#fff3a0', 'rgba(255,255,255,0)'] }, opts(o, { ease: 'outQuad', duration: 1.2 }))],

  // Loops
  float: [(t, o) => animate(t, { y: [0, neg(dist(o, 10), -1)] } as Props, opts({ ...o }, { ease: 'inOutSine', duration: 1.6, repeat: Infinity, yoyo: true }))],
  breathe: [(t, o) => animate(t, { scale: [1, 1 + 0.04 * k(o)] }, opts(o, { ease: 'inOutSine', duration: 2, repeat: Infinity, yoyo: true }))],
  spin: [(t, o) => animate(t, { rotate: [0, o.from === 'left' ? -360 : 360] }, opts(o, { ease: 'linear', duration: 1.2, repeat: Infinity }))],
  wiggleLoop: [(t, o) => animate(t, { rotate: [0, 4 * k(o)] }, opts(o, { ease: 'wiggle(6)', duration: 1, repeat: Infinity, repeatDelay: 1 }))],

  // Text & numbers
  typewriter: [(t, o) => animate(t, { text: { value: String(o.text ?? ''), delimiter: o.delimiter } } as Props, opts(o, { ease: 'linear', duration: Math.max(0.3, String(o.text ?? '').length * (o.charDelay ?? 0.04)) }))],
  scramble: [(t, o) => animate(t, { text: { value: String(o.text ?? ''), type: 'scramble', chars: o.chars } } as Props, opts(o, { ease: 'linear', duration: 1 }))],
  countUp: [(t, o) => animate(t, { textContent: [o.start ?? 0, o.to ?? 100] } as Props, opts(o, { ease: 'outCubic', duration: 1.5, round: o.format ? undefined : true }))],

  // SVG
  drawSVG: [(t, o) => animate(t, { draw: [0, 1] } as Props, opts(o, { ease: 'inOutCubic', duration: 1.2 }))],
  undrawSVG: [(t, o) => animate(t, { draw: [1, 0] } as Props, opts(o, { ease: 'inOutCubic', duration: 1 }))],
};

for (const [name, [fn, d]] of Object.entries(builtins)) registerEffect(name, fn, d ?? {});

/** Built-in effects by name (with autocomplete), plus any added with `registerEffect()`. */
type Effects = { readonly [K in EffectName]: (targets: Targets, options?: EffectOptions) => Animation } &
  Record<string, (targets: Targets, options?: EffectOptions) => Animation | Playable>;

/** All registered effects as functions: `effects.fadeIn('.a')`. */
export const effects: Effects = new Proxy({} as Effects, {
  get: (_o, name: string) => (targets: Targets, options?: EffectOptions) => effect(name, targets, options),
  has: (_o, name: string) => registry.has(name),
  ownKeys: () => Array.from(registry.keys()),
  getOwnPropertyDescriptor: (_o, name: string) =>
    registry.has(name) ? { enumerable: true, configurable: true, value: (t: Targets, o?: EffectOptions) => effect(name, t, o) } : undefined,
});

/** Names of all registered effects. */
export const effectNames = (): string[] => Array.from(registry.keys());
