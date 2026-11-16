# AGENTS.md — using Anikit in an app

A compact playbook for coding agents (and humans) writing animations with `anikit`. Every `ts` block below type-checks against the real API. More: `llms-full.txt` (this file + task recipes + the full reference, in one file; `docs/recipes.md` and `docs/api.md` in the repo).

## Install & import

```ts
import { animate, timeline, stagger, spring, scrollTrigger, effects, testing } from 'anikit';
// 'anikit/lean' streams anikit.wasm instead of embedding it (smaller JS): `await init()` before synchronous helpers.
```

Calls work immediately: the WebAssembly core loads in the background and queues `animate()`/`timeline()` work until it's ready. Helpers that return a value at once (`ease()`, `morph()`, `pathLength()`, `staggerValues()`, `world()`) need it loaded, so `await ready` first in code that runs at startup.

## Quick start: a polished page in five lines

```ts
import { recipe } from 'anikit';
recipe('hero-reveal', '.hero');                              // masked headline rise + subtitle fade
recipe('magnetic-buttons', '.hero');                         // buttons lean towards the cursor
recipe('scroll-story', 'main', { children: 'section' });     // sections rise in on scroll
recipe('parallax-gallery', '.gallery');                      // images drift at different depths
```

Or start from a working project: `npx anikit new vanilla my-site` (no build step), `npx anikit new vite my-site`, or `npx anikit new react my-site`. `recipeNames()` lists every recipe (`hero-reveal`, `scroll-story`, `magnetic-buttons`, `text-scramble`, `parallax-gallery`, `stagger-in`, `draw-svg`, `count-up`, `tilt-cards`); each returns a handle with `revert()`. Every option of every API is in `docs/api.schema.json` (JSON Schema). Unknown options and recipe names produce an `anikit:` warning or error with a "did you mean" and a `docs/api.md#…` link.

In React, `import { useRecipe, useAnimate, useAnikit, useSpring, useScroll, Presence } from 'anikit/react'`: everything a hook creates is reverted on unmount. Vue: `v-recipe` / `v-anikit` directives from `anikit/vue`; Svelte: `use:recipe` / `use:anikit` actions from `anikit/svelte`.

## Want X → use Y

| Want | Use |
|---|---|
| Animate elements (enter, exit, state change) | `animate(targets, { x, opacity, … }, { duration, ease, delay })` |
| One after another / overlapping steps | `timeline().add(…).add(…, '-=0.2')`, or `sequence([...])` |
| Offset many elements | `delay: stagger(0.05, { from: 'center', grid: [cols, rows] })` |
| Natural, interruptible motion (pointer, toggles) | `spring(el, { x, y }, { stiffness, damping })` (retargets keep velocity) |
| One value driving many properties (progress, drag, slider) | `const p = motionValue(0); bindValues(el, { x: p.map([0, 1], [0, 300]), opacity: p }); p.spring(0.5)` |
| Endless logo / ticker row | `marquee('.logos', { speed: 60, pauseOnHover: true, scrollBoost: 0.5 })` |
| Buttons that lean towards the cursor; layered parallax | `magnetic('.cta', { strength: 0.35 })`; `parallax('.bg', { speed: -0.4 })` |
| A timed spring inside a timeline | `ease: 'spring(bounce: 0.3, duration: 0.6)'` |
| Throw / glide with momentum | `inertia(el, { x: { velocity, min, max, snap } })` or `draggable(el, { inertia: true })` |
| Play when visible | `inView(sel, (el) => { … }, { once: true })`, or `scrollTriggerBatch(sel, { animation: (el) => animate(…, { autoplay: false }) })` |
| Scrub / pin with scrolling | `scrollTrigger({ trigger, start, end, scrub, pin, animation: timeline({ autoplay: false }) })` |
| Progress bar / parallax | `scroll(animate(el, …, { ease: 'linear', autoplay: false }), { target, offset })` |
| Animate a layout change | `flip(targets, () => changeLayout())` |
| Text by characters / words / lines | `splitText(el, { type: 'words,chars' })` then animate `.chars` |
| Typewriter, scramble, counters | `{ text: 'Hi' }`, `{ text: { value, type: 'scramble' } }`, `{ textContent: 1200 }` + `format` |
| SVG shape morph / line draw / follow a path | `{ d: otherPath }`, `{ draw: [0, 1] }`, `{ path: { path: '#track', rotate: true } }` |
| Ready-made entrances / attention seekers | `effects.fadeIn(el)`, `effects.shake(el)`, `effects.countUp(el, { to })` (39 presets) |
| Hover / press feedback | `hover(el, (el) => { …; return () => … })`, `press(…)` |
| Drag, swipe, carousels | `draggable(el, { type: 'x', bounds, inertia: true, snap })`; `observer({ onUp, onDown })` for swipes/wheel |
| Physics: falling, colliding, throwable things | `world({ container })`, `.bind(els)`, `.dragging()` |
| Thousands of things on a canvas | `batch(n, props)` (one Float32Array) or `world().addMany()` |
| 3D points / tilt / transform strings | `scene3d()`, `tilt(el)`, `{ transform: 'rotateY(180deg)' }` |
| 100k particles | `pointCloud()` + `glow()` |
| Off-main-thread transform/opacity | `compositor(el, props, options)` |
| A polished pattern in one call | `recipe('hero-reveal' \| 'scroll-story' \| 'magnetic-buttons' \| 'text-scramble' \| …, el, options)` |
| React | `anikit/react`: `useAnikit(() => { … })` (scoped, reverted on unmount), `useAnimate()`, `useRecipe()`, `useSpring()`, `useScroll()`, `<Presence>` |
| Cleanup in React/Vue/Svelte | `const ctx = context(() => { … }, rootEl); return () => ctx.revert()` |
| Respect "reduce motion" | `engine.reducedMotion = 'user'` once at startup |
| GIF / WebM / PNG of a page | `record(el, { duration, fps })`, `screenshot(el)`, CLI `npx anikit-record` |
| A video file (MP4) rendered in Node | `npx anikit-render --init film.mjs` (see Making videos below) |
| Verify headlessly | `testing.useFakeTime()`, `testing.advance(s)`, `engine.inspect()`, `testing.captureWarnings()` |

## Rules of thumb

- **Times are in seconds**: `duration: 0.6`, `delay: 0.1`, `stagger(0.05)`, `seek(1.2)`. Only the raw clock is in ms: `engine.tick(ms)`, `engine.time`, `engine.onFrame((timeMs, dtMs) => …)`.
- **Properties:**
  - **Transforms:** `x`, `y`, `z`, `rotate`, `rotateX`/`Y`, `scale`, `scaleX`/`Y`, `skew`, `xPercent`/`yPercent`, `perspective`. Composed per element and written once per frame; use them for motion instead of `left`/`top`. Numbers are px / deg / unitless.
  - **Everything else:** any CSS property in camelCase (`backgroundColor`), CSS variables (`'--hue'`), `attr: {…}`, plain-object keys, `textContent` counters, `text` typewriter/scramble, `d` morphing, `path` motion paths, `draw` stroke drawing, and `transform` (whole transform strings).
- **Values:** a single value means "from current to this"; `[from, to]` or `[a, b, c]` are keyframes (`null` = current); `'+=50'` is relative; `(el, i, count) => v` and `'random(0, 100)'` run per target (a function may also return keyframes: `(el, i) => [0, i * 10]`).
- **Options** go in the third argument (`{ duration, ease, delay, repeat, yoyo, autoplay, onComplete }`) or among the properties. GSAP-style `stagger: 0.05` and `loop: true` work too. Unknown keys warn with a suggestion (other libraries' names such as `easing`, `iterations`, `fill` are mapped to the anikit ones).
- **Easing:** CSS (`'ease-out'`, `'cubic-bezier(…)'`, `'steps(4)'`), Penner (`'outCubic'`), GSAP (`'power2.inOut'`, `'back.out(1.7)'`), springs (`'spring(bounce: 0.3, duration: 0.6)'` or `{ stiffness, damping }`), `'custom(M0,0 C…)'`, or a function. Default `'outQuad'`, duration `0.5`.
- **Overwrite:** a new animation on the same property takes it over (`overwrite: false` to opt out), so re-triggering is safe.
- **Controllers** (`animate`, `timeline`, `batch`): `play pause reverse restart seek(s) complete cancel kill revert`, `progress`, `time`, `speed`; `await` them for completion.
- **Cleanup:** `anim.revert()` restores the original inline styles; in components wrap setup in `context(fn, rootEl)` and call `ctx.revert()` on unmount; `matchMedia().add(query, fn)` for responsive setups.
- **Performance:** hundreds of DOM elements are fine with `animate()`; thousands of canvas things → `batch()`/`world()`; many 3D points → `scene3d()`; 100k particles → `pointCloud()` + `glow({ budget })`; a busy main thread → `compositor()`. `engine.speed` scales everything (slow motion).

## Snippets

```ts
import { animate, timeline, stagger, scrollTrigger, scrollTriggerBatch, spring, effects } from 'anikit';

// Entrance with stagger
animate('.card', { y: [24, 0], opacity: [0, 1] }, { duration: 0.6, ease: 'outCubic', delay: stagger(0.06) });

// Sequenced timeline with overlap and labels (position after the props or after the options)
timeline({ defaults: { ease: 'outCubic', duration: 0.5 } })
  .add('.title', { y: [30, 0], opacity: [0, 1] })
  .add('.sub', { opacity: [0, 1] }, '-=0.3')
  .label('cta')
  .add('.cta', { scale: [0.8, 1] }, { ease: 'spring(bounce: 0.4)' }, 'cta');

// Scroll-scrubbed, pinned section (the animation is created paused)
scrollTrigger({ trigger: '.story', start: 'top top', end: '+=1200', pin: true, scrub: 0.4,
  animation: timeline({ autoplay: false }).add('.story .a', { x: [0, 400] }).add('.story .b', { opacity: [0, 1] }) });

// Reveal on scroll: play on enter, reverse when scrolling back above
scrollTriggerBatch('.reveal', { start: 'top 85%', toggleActions: 'play none none reverse',
  animation: (el) => animate(el, { y: [40, 0], opacity: [0, 1] }, { autoplay: false }) });

// Pointer-following with momentum
addEventListener('pointermove', (e) => spring('.dot', { x: e.clientX, y: e.clientY }, { stiffness: 300, damping: 25 }));

// Presets
effects.fadeIn('.toast');
effects.shake('#password');
effects.countUp('#revenue', { to: 128400, format: { style: 'currency', currency: 'USD' } });
```

```ts
import { world, batch, scene3d, engine, pointCloud, textPoints, glow, record } from 'anikit';

// Physics: DOM elements that fall, collide and can be thrown
const w = world({ container: '#stage', restitution: 0.4 });
w.bind('#stage .ball', { rotate: true });
w.dragging();
w.pointer({ mode: 'repel' });

// 3D point cloud on a canvas
const cloud = batch(20000, { x: (i) => [0, Math.cos(i) * 300], y: (i) => [0, Math.sin(i * 1.3) * 300], z: (i) => [0, Math.sin(i) * 300] }, { duration: 2 });
const scene = scene3d({ canvas: '#c' }).orbit({ autoRotate: 10 });
engine.onFrame(() => scene.draw(cloud, { size: 1.5, color: '#9cf' }));

// 100k glowing particles that morph between shapes (text → sphere) and part around the pointer
const pc = pointCloud(100_000, { shapes: [textPoints('hello', 100_000, { width: 700 }), (i, n) => {
  const y = 1 - (2 * i) / n, r = Math.sqrt(1 - y * y), t = i * 2.4; return [Math.cos(t) * r * 250, y * 250, Math.sin(t) * r * 250];
}] });
const g = glow('#bg', { trail: 0.5, background: '#07080b' });
const cam = scene3d({ fov: 40, camera: { z: 1000 } });
g.resize(innerWidth, innerHeight);
let ptr: { x: number; y: number } | undefined;
addEventListener('pointermove', (e) => (ptr = { x: e.clientX, y: e.clientY }));
engine.onFrame(() => g.render(cam, pc, { size: 1.2, intensity: 0.3, lens: ptr }));
pc.morph(1, { duration: 2.5, ease: 'inOutCubic' });   // or: pc.morph(1); pc.mix = scrollProgress;

// Record any of the above to a GIF, frame-exact
const rec = await record('#stage', { duration: 2, fps: 30 });
rec.download('stage.gif');
```

```ts
import { animate, flip, presence, exit, viewTransition, scroll, compositor, splitText, stagger, gesture, engine } from 'anikit';
declare const item: HTMLElement, panel: HTMLElement, open: boolean, thumb: HTMLElement, hero: HTMLElement;
declare function applyFilter(): void;
declare function showDetail(): void;
declare function detailImg(): HTMLElement | null;

// Layout and presence: list changes animate; removed items fade out in place, then go
flip('.card', () => applyFilter(), { fade: true });
const todos = presence('#todos', { enter: { opacity: [0, 1], y: [-8, 0] }, exit: { opacity: 0 } });
todos.add(item); todos.remove(item);
exit('.toast');                                            // animate out, then remove
animate(panel, { height: open ? 'auto' : 0 });             // accordions

// Shared element (thumbnail -> detail) with the View Transitions API, FLIP fallback
viewTransition(() => showDetail(), { ease: 'spring(bounce: 0.2, duration: 0.5)', names: { hero: [thumb, () => detailImg()] } });

// Scroll-linked on the compositor (native ScrollTimeline where available)
scroll(compositor('.hero', { scale: [1, 1.2], opacity: [1, 0] }, { ease: 'linear' }), { target: hero, offset: ['start start', 'end start'] });

// Text lines rising from behind a mask, re-split on resize
splitText('.lede', { type: 'lines', mask: 'lines', autoSplit: true, onSplit: ({ lines }) => animate(lines, { yPercent: [100, 0] }, { delay: stagger(0.08) }) });

// Pinch-zoom / rotate / pan with limits and momentum
gesture('#photo', { minScale: 1, maxScale: 5 });

// Follow an SVG path exactly as drawn (viewBox and page position mapped)
animate('.dot', { path: { path: '#route', align: true, rotate: true } }, { duration: 4 });

// Reduced motion that keeps fades and colour changes
engine.reducedMotion = 'user';
engine.reducedMotionMode = 'fade';
```

Accessibility: with `engine.reducedMotion = 'user'`, add `engine.reducedMotionMode = 'fade'` to keep fades and colour changes while movement (transforms, paths, layout) jumps to its end.

Task-sized versions (hero, list, carousel, page transition, counters, FLIP, React cleanup, recording, Node video, Python) are in the recipes section of `llms-full.txt`.

## Verifying your work

```ts
import { animate, testing, engine } from 'anikit';

await testing.ready;
testing.useFakeTime();                        // deterministic: no requestAnimationFrame
const warnings = testing.captureWarnings();   // BEFORE the code under test
const el = document.createElement('div');
document.body.append(el);
animate(el, { x: 100 }, { duration: 1, ease: 'linear' });
testing.advance(0.5);                         // el.style.transform === 'translate(50px,0px)'
engine.inspect();                             // [{ type: 'tween', target: 'div', props: ['x'], progress: 0.5, … }]
testing.finishAll();                          // run until idle
console.assert(warnings.length === 0, warnings.join('\n'));
testing.reset();                              // between tests
```

In a headless browser, `window.__ANIKIT__` is the engine (`.manual(true)`, `.tick(ms)`, `.inspect()`). Scroll triggers, pinning and dragging need layout: check them in Chromium (Playwright), not happy-dom/jsdom.

## Common mistakes

Most of these print an `anikit:` warning that names the fix; read the console.

| Symptom / warning | Cause → fix |
|---|---|
| Nothing moves; "selector … matched no elements" | Typo, missing `.`/`#` (the warning suggests one), or the script ran before the DOM: run after `DOMContentLoaded` or from a module script. In tests: call `testing.advance()`; nothing moves in fake time otherwise. |
| "received null as its target" | A `querySelector()`/ref returned null. Check the element exists when the code runs. |
| Animation takes minutes; "duration is 600 seconds" | Times are seconds: `duration: 0.6`, `stagger(0.05)`. |
| "unknown animation option" / "unknown option" | Misspelled option (the warning suggests the right one) or an option the API doesn't have, e.g. `spring()` has no `delay`/`ease`. |
| "is not a CSS property" | Misspelled property (`colour` → `color`), or an option put among the properties. |
| "unknown easing" (throws) | Use a listed name (`'outCubic'`, `'power2.out'`, `'ease-in-out'`); the error suggests the nearest. |
| "rotate is an angle" / "opacity goes from 0 to 1" | Wrong unit: `rotate: 45` or `'0.5turn'`; `opacity: 0.5`, not `50`. |
| Element jumps at the start | Give an explicit start value: `x: [0, 100]`, `opacity: [0, 1]`. |
| `left`/`top` don't move it; "has no effect … position: static" | Animate `x`/`y` (transforms, also faster), or give the element `position: relative`. |
| Timeline children start from the wrong value | Sequential tweens of a property chain automatically; for parallel ones give `[from, to]`. |
| Scroll animation runs on load or flashes | Create it with `autoplay: false` and pass it as `animation` (the trigger pauses it, but `autoplay: false` avoids a first frame). |
| `scrollTrigger({ animation: (el) => … })` throws | That form belongs to `scrollTriggerBatch()`; `scrollTrigger` takes one animation. |
| Pinned section has the wrong height after images load | Call `refreshScrollTriggers()` after layout changes. |
| Styles remain after unmount / route change | Use `context()` and `ctx.revert()`. |
| `ease()`/`morph()` throw "core is still loading" | `await ready` (or `await init()` with `anikit/lean`) before synchronous helpers. |
| `onFrame` motion is 1000× too fast | `onFrame` gives `(timeMs, dtMs)`: divide by 1000 for seconds. |
| `draggable(el).x` is undefined | `draggable()` returns an array: `const [drag] = draggable(el)`. |
| Python: `stagger(5)` gives a list; a timeline step lands at the wrong time | In Python an int first argument is a count (`stagger(5, 0.1)`); for a delay pass seconds (`delay=stagger(0.05, from_="center")`). `tl.add(target, props, "-=0.2", duration=0.5)`: the position is the third argument, options are keywords. |

## Making videos (compositions)

Use this when asked for a video, trailer, title sequence, product film or animated clip. Output is frame-exact MP4/WebM; nothing depends on machine speed.

```bash
npx anikit-render --doctor                           # first time: checks Node, @napi-rs/canvas, ffmpeg and the build, with fixes
npx anikit-render --init film.mjs                    # a working starter: 3D cubes + particles + type + impact + grade
npx anikit-render --init fly.mjs --template journey  # a 3D fly-through: neon city, chrome gates, shattering knot, 3D wordmark
npx anikit-render film.mjs --preview 15 --out pv/    # every 15th frame as JPEG, fast; look at these before rendering
npx anikit-render film.mjs --out film.mp4 --workers 4 [--audio track.wav] [--fade-out 0.5]
```

The module default-exports `async ({ anikit, node }) => composition`. Building blocks, all rendered in WebAssembly:

| Want | Use |
|---|---|
| Scenes, timing | `composition({ width, height, fps, duration, motionBlur, post })`, `.scene(name, start, end, { enter, step, draw, transition, timeScale, motionBlur })`, `.cue(t, fn)` |
| Punch | `.impact(t, { shake, flash, flashOpacity, aberration, decay, shockwave, origin })` — applied to the finished frame; `shockwave` sends a refracting ring out from `origin` |
| Film look | `post: { aa, bloom, bloomThreshold, bloomRadius, streaks, streakLength, rays, raysCenter, flares, aberration, vignette, grain, zoomBlur, exposure, contrast, saturation, shadows, highlights }` — any value can be `(t) => …` |
| Slow motion / bullet time | scene `timeScale: (local) => 0.1…1` — slows tweens, particles, physics together; choreograph on `draw({ clock })` (the slowed scene clock) and place hits with `film.impact(film.timeAt('scene', clockSeconds), …)` |
| Solid 3D | `renderer3d({ width, height })`, `mesh.cube() / sphere() / torus() / torusKnot() / plane() / cylinder() / heightfield() / text() / custom()`, `instances(n, i => ({ x, y, z, rx, ry, rz, scale, sx, sy, sz, color }))`, `render(scene3d, [{ mesh, instances, material, cull, blend }], { background, fog, sky, horizon, ground, strips })` |
| Materials that don't look plain | dark bodies + light: `{ metal: 0.9, iridescence: 0.3 }` chrome, `{ lines: 1.5, lineColor, grid: [1, 1] }` glowing edges, `{ pulse: 2, pulsePhase: t }` travelling light, `{ emissive: 1 }` neon; set `strips` for crisp chrome highlights |
| Flying camera ("journey") | `flightPath(points, { lookAhead, bank })`, `path.timing([[sec, frac], …])`, `path.apply(cam, s(t), extraRoll, { shake })` (handheld wobble); `cam.roll` for barrel rolls (unwind the shortest way: wrap to ±180° first); `render(…, { motionBlur: true })` with composition `motionBlur: 1` for ghost-free speed |
| Stars, dust, sparks in 3D | `{ points, colors, size, intensity, streak, fog }` items in `render()` — depth-tested, streak with camera motion; accepts an `emitter()` |
| Explosions | `shatter(mesh, { force, spin, gravity })` → `{ mesh, instances }`; `shards.update(t - hitTime)` |
| Particles as light or ink | `pointCloud(n, { shapes })` + `glow(node.glowTarget(w, h), { mode: 'light' | 'ink' })` |
| Sparks | `emitter({ rate, life, speed, spread, gravity, colors })`, `.burst(n)` |
| Physics | `world({ bounds, gravity })` |
| Type | `typeFx(ctx, text, x, y, { t, start, effect: 'slam' | 'rise' | 'wave' | 'type' | 'scramble' | 'fade' })`; `textPoints(text, n)` for type made of particles |
| Transitions between scenes | `transition: { duration, type: 'whip' | 'push' | 'iris' | 'slice' | 'zoom' | 'flash' | 'mask' | fn }` |
| Music sync | `analyzeAudio(samples, rate)` → `bpm`, `beats`, `onsets`, `at(t)` |

**Rules that keep films correct**
1. Start tweens and simulations in `enter()`; advance simulations in `step(f)` with **`f.dt`** (engine seconds, already scaled by `timeScale`). `draw()` only reads state — with motion blur it runs several times per frame.
2. Don't read wall-clock time. `Math.random` is seeded by `anikit-render`, so parallel workers produce identical frames.
3. Solid 3D: draw the renderer's pixels first (`ctx.putImageData(new ImageData(px, w, h), 0, 0)`), then 2D particles (`glow`) with `globalCompositeOperation = 'lighter'`, then type. Prefer `points` items inside `render()` for particles that must hide behind geometry.
6. Plain flat-coloured shapes look cheap. Use near-black bodies and put the colour in light: chrome + `strips`, glowing `lines`, `pulse`, `emissive`, bloom. Two or three hues per shot.
4. One shared `scene3d` per shot; scenes that overlap in a transition must not share mutable state (cameras, glow targets).
5. Type must contrast with what's behind it — paper on dark, or a scrim (`createLinearGradient`) behind it.

**Verify like a picky editor**
- `npx anikit-render film.mjs --review review/` renders the film once at 10 samples/s and writes `sheet.jpg` (graded frames every 0.5 s with timestamps — the whole film in one image), `pacing.txt` (motion energy, **holds** where little moves for ≥ 1 s, flat frames, cuts) and `pacing.csv`. Look at the sheet; fix every hold the user didn't ask for.
- Preview at `--preview 15` and look at every frame; then render and pull strips around each cut/impact with ffmpeg (`-ss t -frames:v 14 -vf scale=240:-1,tile=14x1`).
- Watch for: empty or flat frames, type over same-tone content, shots that hold still for more than ~1 s.
- `ffprobe` the output: duration, frame count, and whether an audio stream should be there.
- `npx anikit-render --check film.mp4` compares every frame with the last and lists **spikes** (single-frame pops, things switching on in view, worker-slice seams) and frozen frames, with timestamps. Every spike should be a flash, cut or impact you placed; fix the rest (usually an item gated on by camera position while in frame).
- Slow render? `npx anikit-render film.mjs --profile` shows where each frame's time goes (your `step`/`draw` hooks, each `renderer3d` pass, the grade). Fix the biggest line first: fewer motion-blur samples on calm shots, `mirror` only where the floor is visible, gate heavy items by time.
