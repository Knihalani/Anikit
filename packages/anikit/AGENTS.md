# AGENTS.md — using Anikit in an app

This is a compact playbook for coding agents (and humans) writing animations with `anikit`. The full API reference ships as `llms-full.txt` (also `docs/api.md` in the repo).

## Install & import

```ts
import { animate, timeline, stagger, spring, scrollTrigger, effects, world, scene3d } from 'anikit';
// 'anikit/lean' streams anikit.wasm instead of embedding it (smaller JS).
```

Calls work immediately. The WebAssembly core loads in the background and queues work until it's ready. For code that reads results synchronously at startup, `await ready` first.

## Rules of thumb

- **Times are in seconds.** `duration: 0.6`, `delay: 0.1`.
- **Properties:**
  - **Transforms:** `x`, `y`, `z`, `rotate`, `rotateX`/`Y`, `scale`, `skew`, `xPercent`, `perspective`. They're composed per element and written once per frame. Use them for motion instead of `left`/`top`.
  - **Other animatable values:** any CSS property in camelCase (`backgroundColor`), CSS variables (`'--hue'`), `attr: {…}`, plain objects, `textContent` counters, `text` typewriter/scramble, `d` shape morphing, `path` motion paths, `draw` stroke drawing, and `transform` (full transform strings interpolated like browsers).
- **Values:**
  - a single value means "from current to this";
  - `[from, to]` or `[a, b, c]` are keyframes, and `null` means the current value;
  - `'+=50'` is relative;
  - `(el, i, count) => v` runs per target, and so does `'random(0, 100)'`.
- **Easing:** CSS (`'ease-out'`, `'cubic-bezier(…)'`, `'steps(4)'`), Penner (`'outCubic'`), GSAP (`'power2.inOut'`, `'back.out(1.7)'`), springs (`'spring(bounce: 0.3, duration: 0.6)'` or `{ stiffness, damping }`), `'custom(M0,0 C…)'`, or a function.
- **Stagger:** `delay: stagger(0.05, { from: 'center', grid: [cols, rows] })`.
- **Overwrite:** a new animation on the same property takes it over (`overwrite: false` to opt out).
- **Cleanup:**
  - `anim.revert()` restores the original inline styles;
  - in components, wrap setup in `context(() => {…}, rootEl)` and call `ctx.revert()` on unmount;
  - `matchMedia()` handles responsive setups.
- **Accessibility:** set `engine.reducedMotion = 'user'` to honour the OS setting.
- **Performance:**
  - **Hundreds of DOM elements:** `animate()` is fine.
  - **Thousands of things on a canvas:** use `batch()` (one Float32Array) or `world()`.
  - **Many points in 3D:** project them with `scene3d()`.
  - **The main thread will be busy:** use `compositor()` for transform/opacity.

## Recipes

```ts
// Entrance with stagger
animate('.card', { y: [24, 0], opacity: [0, 1] }, { duration: 0.6, ease: 'outCubic', delay: stagger(0.06) });

// Sequenced timeline with overlap and labels
timeline({ defaults: { ease: 'outCubic', duration: 0.5 } })
  .add('.title', { y: [30, 0], opacity: [0, 1] })
  .add('.sub', { opacity: [0, 1] }, {}, '-=0.3')
  .label('cta')
  .add('.cta', { scale: [0.8, 1] }, { ease: 'spring(bounce: 0.4)' }, 'cta');

// Scroll-scrubbed, pinned section
scrollTrigger({ trigger: '.story', start: 'top top', end: '+=1200', pin: true, scrub: 0.4,
  animation: timeline({ autoplay: false }).add('.story .a', { x: [0, 400] }).add('.story .b', { opacity: [0, 1] }) });

// Reveal on scroll (play forward on enter, reverse on leave back)
scrollTriggerBatch('.reveal', { start: 'top 85%', toggleActions: 'play none none reverse',
  animation: (el) => animate(el, { y: [40, 0], opacity: [0, 1] }, { autoplay: false }) });

// Pointer-following with momentum
addEventListener('pointermove', (e) => spring('.dot', { x: e.clientX, y: e.clientY }, { stiffness: 300, damping: 25 }));

// Physics: DOM elements that fall, collide and can be thrown
const w = world({ container: '#stage', restitution: 0.4 });
w.bind('#stage .ball', { rotate: true });
w.dragging();
w.pointer({ mode: 'repel' });

// 3D point cloud on a canvas
const cloud = batch(20000, { x: (i) => [0, Math.cos(i) * 300], y: (i) => [0, Math.sin(i * 1.3) * 300], z: (i) => [0, Math.sin(i) * 300] }, { duration: 2 });
const scene = scene3d({ canvas: '#c' }).orbit({ autoRotate: 10 });
engine.onFrame(() => scene.draw(cloud, { size: 1.5, color: '#9cf' }));

// Effects
effects.fadeIn('.toast');
effects.shake('#password');
effects.countUp('#revenue', { to: 128400, format: { style: 'currency', currency: 'USD' } });
```

## Verifying your work

```ts
import { testing, engine } from 'anikit';
await testing.ready;
testing.useFakeTime();             // deterministic: no rAF
const a = animate(el, { x: 100 }, { duration: 1, ease: 'linear' });
testing.advance(0.5);              // el.style.transform === 'translate(50px,0px)'
engine.inspect();                  // [{ type: 'tween', target: 'div#hero', props: ['x'], progress: 0.5, … }]
testing.finishAll();
const warnings = testing.captureWarnings(); // typos come with "did you mean" suggestions
```

## Common mistakes

| Symptom | Fix |
|---|---|
| Nothing moves | Check the warnings: the selector may match no elements, or a property name may be misspelled. In tests, call `testing.advance()`, because nothing advances in manual mode. |
| Element jumps at start | Give an explicit start value: `x: [0, 100]`. |
| Timeline children start from the wrong value | Sequential tweens chain automatically. For parallel tweens on the same property, give explicit `[from, to]`. |
| Scroll animation plays on load | Pass `autoplay: false`. Triggers pause their animation automatically. |
| Styles remain after unmount | Use `context()` and `ctx.revert()`. |
