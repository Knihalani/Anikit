import { describe, it, expect, beforeAll, beforeEach } from 'vitest';
import {
  animate, timeline, sequence, stagger, spring, inertia, physics, batch, scrollTrigger, scrollTriggerBatch, scrollTo,
  smoothScroll, draggable, observer, scroll, inView, splitText, flip, world, compositor, effects, effectNames, testing,
  onWarning, set, fromTo, from,
} from '../src/index';
import { setup, box } from './setup';

beforeAll(setup);

let warnings: string[] = [];
beforeEach(() => {
  testing.reset();
  document.body.innerHTML = '';
  warnings = testing.captureWarnings();
});
const has = (...parts: string[]) => warnings.some((w) => parts.every((p) => w.includes(p)));
const tf = (el: HTMLElement) => el.style.transform;

describe('animate() guidance', () => {
  it('flags unknown options with a suggestion, in the options and among the properties', () => {
    const el = box();
    animate(el, { x: 10 }, { durration: 1 } as any);
    expect(has('unknown animation option "durration"', 'Did you mean "duration"')).toBe(true);
    animate(el, { x: 10, delya: 0.2 } as any);
    expect(has('"delya" is not a CSS property', 'Did you mean the option "delay"')).toBe(true);
  });

  it('flags times that look like milliseconds', () => {
    const el = box();
    animate(el, { x: 10 }, { duration: 600 });
    expect(has('duration is 600 seconds', 'duration: 0.6')).toBe(true);
    animate(el, { y: 10 }, { delay: 250 });
    expect(has('delay is 250 seconds', 'delay: 0.25')).toBe(true);
    const els = Array.from({ length: 5 }, () => box());
    animate(els, { x: 10 }, { delay: stagger(50) });
    expect(has('use stagger(0.05), not stagger(50)')).toBe(true);
  });

  it('accepts a number as the duration (anime.js habit) and says so', () => {
    const el = box();
    animate(el, { x: [0, 100] }, 1 as any).seek(0.5);
    expect(has('third argument is an options object', '{ duration: 1 }')).toBe(true);
  });

  it('explains empty targets: null, typo selectors, missing class dot', () => {
    animate(null as any, { x: 1 });
    expect(has('received null as its target', 'querySelector()')).toBe(true);
    const hero = box();
    hero.className = 'hero';
    animate('hero', { x: 1 });
    expect(has('selector "hero" matched no elements', 'Did you mean ".hero"?')).toBe(true);
    animate([], { x: 1 });
    expect(has('empty target list')).toBe(true);
  });

  it('throws a readable error for invalid selectors', () => {
    expect(() => animate('#1-bad[', { x: 1 })).toThrow(/is not a valid CSS selector/);
  });

  it('checks units and ranges', () => {
    const el = box();
    animate(el, { rotate: '45px' });
    expect(has('"rotate" is an angle', '45px')).toBe(true);
    animate(el, { x: '10deg' });
    expect(has('"x" is a length', '10deg')).toBe(true);
    animate(el, { scale: '2px' });
    expect(has('"scale" is a unitless factor')).toBe(true);
    animate(el, { opacity: 50 });
    expect(has('opacity goes from 0 to 1', 'Did you mean 0.5')).toBe(true);
    animate(el, { x: '10px 20px' });
    expect(has('"x" needs a single number', 'skipped')).toBe(true);
  });

  it('flags left/top on static elements', () => {
    const el = box({ position: 'static' });   // happy-dom has no default computed position
    animate(el, { left: 100 });
    expect(has('"left" has no effect on div because it is position: static', 'x / y')).toBe(true);
    const abs = box({ position: 'absolute' });
    warnings.length = 0;
    animate(abs, { left: 100 });
    expect(warnings).toEqual([]);
  });

  it('checks enumerated options', () => {
    const el = box();
    animate(el, { color: 'red' }, { colorSpace: 'okl' as any });
    expect(has('unknown colorSpace "okl"')).toBe(true);
    animate(el, { x: 1 }, { direction: 'alternative' as any });
    expect(has('unknown direction "alternative"', 'Did you mean "alternate"')).toBe(true);
    animate(el, { x: 1 }, { fillMode: 'forward' as any });
    expect(has('unknown fillMode "forward"')).toBe(true);
  });

  it('throws on unknown easings with a suggestion', () => {
    expect(() => animate(box(), { x: 1 }, { ease: 'outCubik' })).toThrow(/Did you mean "outCubic"/);
    expect(() => animate(box(), { x: 1 }, { ease: 'power2.outt' })).toThrow(/unknown easing/);
  });

  it('accepts per-target keyframes from a function', () => {
    const els = [box(), box()];
    animate(els, { x: (_el: unknown, i: number) => [i * 10, 100 + i * 10] }, { duration: 1, ease: 'linear' }).seek(0.5);
    expect(els.map(tf)).toEqual(['translate(50px,0px)', 'translate(60px,0px)']);
    expect(warnings).toEqual([]);
  });

  it('suggests effects for a preset name passed as keyframes', () => {
    animate(box(), 'fadeIn' as any);
    expect(has('second argument must be an object', 'effects.fadeIn(targets)')).toBe(true);
  });
});

describe('other APIs', () => {
  it('timeline: unknown options, and GSAP-style positions in the options slot', () => {
    timeline({ defualts: { duration: 1 } } as any);
    expect(has('timeline(): unknown option "defualts"', 'Did you mean "defaults"')).toBe(true);
    const a = box();
    const b = box();
    const tl = timeline({ autoplay: false, defaults: { duration: 1, ease: 'linear' } })
      .add(a, { x: [0, 100] })
      .add(b, { x: [0, 100] }, '-=0.5');
    expect(tl.duration).toBe(1.5);
    tl.seek(1);
    expect(tf(b)).toBe('translate(50px,0px)');
  });

  it('stagger: unknown keys, unknown origin, GSAP object form', () => {
    stagger(0.1, { form: 'center' } as any);
    expect(has('stagger(): unknown option "form"', 'Did you mean "from"')).toBe(true);
    stagger(0.1, { from: 'centre' as any });
    expect(has('unknown from "centre"', 'Did you mean "center"')).toBe(true);
    const fn = stagger({ each: 0.2, from: 'last' } as any);
    expect(has('stagger() takes (each, options)')).toBe(true);
    expect([0, 1, 2].map((i) => fn(null, i, 3))).toEqual([0.4, 0.2, 0]);
  });

  it('spring / inertia / physics: unknown options', () => {
    const el = box();
    spring(el, { x: 100 }, { stifness: 200 } as any);
    expect(has('spring(): unknown option "stifness"', 'Did you mean "stiffness"')).toBe(true);
    spring(el, { x: 100 }, { delay: 1 } as any);
    expect(has('spring(): unknown option "delay"')).toBe(true);
    inertia(el, { x: { velocity: 100 } }, { timeconstant: 0.3 } as any);
    expect(has('inertia(): unknown option "timeconstant"', '"timeConstant"')).toBe(true);
    physics(el, { y: { velocity: 10 } }, { gravity: 900 } as any);
    expect(has('physics(): unknown option "gravity"')).toBe(true);
    spring(null as any, { x: 1 });
    expect(has('spring() received null')).toBe(true);
  });

  it('batch, world, compositor, splitText: unknown options', () => {
    batch(4, { x: [0, 1] }, { durations: 1 } as any);
    expect(has('batch(): unknown option "durations"', '"duration"')).toBe(true);
    world({ gravty: [0, 1] } as any).destroy();
    expect(has('world(): unknown option "gravty"', '"gravity"')).toBe(true);
    world({ container: '#nope' }).destroy();
    expect(has('world(): container "#nope" matched no element')).toBe(true);
    compositor(box(), { opacity: [0, 1] }, { durration: 1 } as any);
    expect(has('compositor(): unknown option "durration"')).toBe(true);
    const p = document.createElement('p');
    p.textContent = 'hello world';
    document.body.appendChild(p);
    splitText(p, { type: 'letters', charsClass: 'c' } as any).revert();
    expect(has('splitText(): unknown type "letters"')).toBe(true);
    expect(has('splitText(): unknown option "charsClass"', '"charClass"')).toBe(true);
  });

  it('scrollTrigger: positions, toggleActions, animation factories', () => {
    const section = box();
    section.className = 'story';
    scrollTrigger({ trigger: '.story', start: 'top middle', toggleActions: 'play none none reversed' });
    expect(has('"middle" in start: "top middle" isn\'t understood')).toBe(true);
    expect(has('toggleActions "play none none reversed"', 'Did you mean "reverse"')).toBe(true);
    scrollTrigger({ trigger: section, scrub: true, toggleActions: 'play none none reverse' });
    expect(has('toggleActions is ignored when scrub is set')).toBe(true);
    scrollTrigger({ trigger: section, onenter: () => {} } as any);
    expect(has('scrollTrigger(): unknown option "onenter"', 'Did you mean "onEnter"')).toBe(true);
    scrollTrigger({ trigger: 'story' });
    expect(has('trigger "story" matched no element', 'Did you mean ".story"')).toBe(true);
    expect(() => scrollTrigger({ trigger: section, animation: (() => animate(section, { x: 1 })) as any })).toThrow(/scrollTriggerBatch/);
    expect(() => scrollTriggerBatch('.story', { animation: animate(section, { x: 1 }) as any })).toThrow(/takes a function/);
    scrollTo('#missing');
    expect(has('scrollTo(): "#missing" matched no element')).toBe(true);
    smoothScroll({ smoothness: 1 } as any).destroy();
    expect(has('smoothScroll(): unknown option "smoothness"')).toBe(true);
  });

  it('interaction and viewport helpers: unknown options and empty targets', () => {
    draggable(box(), { bound: '.board' } as any).forEach((d) => d.kill());
    expect(has('draggable(): unknown option "bound"', 'Did you mean "bounds"')).toBe(true);
    draggable('.nothing-here');
    expect(has('draggable(): selector ".nothing-here" matched no elements')).toBe(true);
    observer({ onUP: () => {} } as any).kill();
    expect(has('observer(): unknown option "onUP"', '"onUp"')).toBe(true);
    scroll(() => {}, { offsets: ['start end', 'end start'] } as any)();
    expect(has('scroll(): unknown option "offsets"', '"offset"')).toBe(true);
    inView(box(), () => {}, { threshold: 0.5 } as any)();
    expect(has('inView(): unknown option "threshold"')).toBe(true);
    expect(() => flip(box(), { duration: 1 } as any)).toThrow(/needs a function that changes the layout/);
  });
});

describe('warning plumbing', () => {
  it('captureWarnings() starts fresh, so repeated messages are captured again', () => {
    animate('.again', { x: 1 });
    expect(warnings.length).toBe(1);
    const second = testing.captureWarnings();
    animate('.again', { x: 1 });
    expect(second.length).toBe(1);
    onWarning(null);
  });

  it('stays quiet for correct code: every effect and the documented patterns', () => {
    const els = Array.from({ length: 4 }, () => box());
    const svg = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
    const path = document.createElementNS('http://www.w3.org/2000/svg', 'path');
    path.setAttribute('d', 'M0,0 L100,0');
    svg.appendChild(path);
    document.body.appendChild(svg);
    const label = box();
    label.textContent = '0';
    for (const name of effectNames()) {
      const target = name.includes('SVG') ? path : name === 'countUp' || name === 'typewriter' || name === 'scramble' ? label : els;
      effects[name](target, name === 'countUp' ? { to: 100 } : name === 'typewriter' || name === 'scramble' ? { text: 'hi' } : { stagger: 0.05 });
    }
    animate(els, { y: [24, 0], opacity: [0, 1] }, { duration: 0.6, ease: 'outCubic', delay: stagger(0.06, { from: 'center' }) });
    animate(els[0], { x: 100, rotate: '1turn', scale: 1.2, backgroundColor: '#0af', '--hue': 120, duration: 0.4 });
    animate(els[1], { x: '50%', rotate: '0.5rad', y: '2em', opacity: '50%' });
    animate(els[2], [{ x: 0 }, { x: 100, offset: 0.2, ease: 'inQuad' }, { x: 0 }]);
    animate({ v: 0 }, { v: 100 }, { onUpdate: () => {}, round: true, snap: 5 });
    animate(label, { textContent: 1200 }, { format: { style: 'currency', currency: 'USD' }, locale: 'en-US' });
    from(els[3], { opacity: 0 }, { duration: 0.2 });
    fromTo(els[3], { x: 0 }, { x: 10 }, { ease: 'spring(bounce: 0.3)' });
    set(els[3], { x: 5 });
    timeline({ defaults: { ease: 'outCubic', duration: 0.5 }, repeat: 1, yoyo: true })
      .add(els[0], { y: [30, 0] })
      .add(els[1], { opacity: [0, 1] }, { delay: stagger(0.05) }, '-=0.3')
      .label('cta')
      .add(els[2], { scale: [0.8, 1] }, { ease: 'spring(bounce: 0.4)' }, 'cta')
      .call(() => {}, '>')
      .set(els[3], { display: 'none' });
    sequence([[els[0], { x: 100 }], [els[1], { opacity: [0, 1] }, { at: '<' }], 'middle', [els[2], { scale: 2 }, { at: 'middle+=0.2', duration: 1 }]]);
    spring(els[0], { x: 200 }, { stiffness: 300, damping: 25 });
    spring(els[1], { x: 0 }, { bounce: 0.3, duration: 0.5 });
    inertia(els[2], { x: { velocity: 500, min: 0, max: 600, snap: 100 } }, { power: 0.8, timeConstant: 0.35 });
    physics(els[3], { y: { velocity: -600, acceleration: 1500, max: 400 } }, { restitution: 0.6, friction: 0.2 });
    const b = batch(100, { x: (i) => [0, i], y: [0, 300] }, { duration: 2, delay: stagger(0.001), onUpdate: () => {} });
    b.kill();
    scrollTrigger({ trigger: els[0], start: 'top 80%', end: '+=600', scrub: 0.5, pin: true, snap: { to: 'labels', duration: 0.4 },
      animation: timeline({ autoplay: false }).add(els[1], { x: [0, 400] }) });
    scrollTrigger({ trigger: els[1], start: 'top+=20 center-=10%', end: 'bottom top', toggleActions: 'play pause resume reverse', toggleClass: 'active', once: true });
    scrollTriggerBatch(els, { start: 'top 85%', animation: (el) => animate(el, { y: [40, 0] }, { autoplay: false }) });
    draggable(els[2], { bounds: { minX: 0, maxX: 100 }, inertia: true, snap: { x: 50 }, edgeResistance: 0.35, lockAxis: true });
    const w = world({ gravity: [0, 980], substeps: 4, restitution: 0.2, friction: 0.05 });
    w.destroy();
    flip(els, () => els.reverse(), { duration: 0.6, ease: 'spring(bounce: .2)' });
    compositor(els[0], { y: [20, 0], opacity: [0, 1] }, { ease: 'spring(bounce: .4)' });
    splitText(label, { type: 'lines,words,chars' }).revert();
    testing.finishAll(5);
    expect(warnings).toEqual([]);
  });
});
