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
  - **Tens or hundreds of thousands of particles:** `pointCloud()` (simulation) + `glow()` (rendering); both run in WebAssembly. Use `glow({ budget })` so slow machines lower the resolution instead of the frame rate.
  - **Slow motion / pause everything:** `engine.speed` scales tweens, worlds, clouds and `onFrame` `dt` together.
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

// 100k glowing particles that morph between shapes (text → sphere) and part around the pointer
const pc = pointCloud(100_000, { shapes: [textPoints('hello', 100_000, { width: 700 }), (i, n) => {
  const y = 1 - (2 * i) / n, r = Math.sqrt(1 - y * y), t = i * 2.4; return [Math.cos(t) * r * 250, y * 250, Math.sin(t) * r * 250];
}] });
const g = glow('#bg', { trail: 0.5, background: '#07080b' });
const cam = scene3d({ fov: 40, camera: { z: 1000 } });
g.resize(innerWidth, innerHeight);
let ptr = null; addEventListener('pointermove', (e) => (ptr = { x: e.clientX, y: e.clientY }));
engine.onFrame(() => g.render(cam, pc, { size: 1.2, intensity: 0.3, lens: ptr }));
pc.morph(1, { duration: 2.5, ease: 'inOutCubic' });   // or: pc.morph(1); pc.mix = scrollProgress;

// Record any of the above to a GIF, frame-exact
const rec = await record('#stage', { duration: 2, fps: 30 }); rec.download('stage.gif');

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

## Making videos (compositions)

Use this when asked for a video, trailer, title sequence, product film or animated clip. Output is frame-exact MP4/WebM; nothing depends on machine speed.

```bash
npx anikit-render --init film.mjs                    # a working starter: 3D cubes + particles + type + impact + grade
npx anikit-render --init fly.mjs --template journey  # a 3D fly-through: neon city, chrome gates, shattering knot, 3D wordmark
npx anikit-render film.mjs --preview 15 --out pv/    # every 15th frame as JPEG, fast; look at these before rendering
npx anikit-render film.mjs --out film.mp4 --workers 4 [--audio track.wav] [--fade-out 0.5]
```

The module default-exports `async ({ anikit, node }) => composition`. Building blocks, all rendered in WebAssembly:

| Want | Use |
|---|---|
| Scenes, timing | `composition({ width, height, fps, duration, motionBlur, post })`, `.scene(name, start, end, { enter, step, draw, transition, timeScale, motionBlur })`, `.cue(t, fn)` |
| Punch | `.impact(t, { shake, flash, flashOpacity, aberration, decay })` — applied to the finished frame |
| Film look | `post: { bloom, bloomThreshold, bloomRadius, streaks, streakLength, aberration, vignette, grain, zoomBlur, exposure, contrast, saturation, shadows, highlights }` — any value can be `(t) => …` |
| Slow motion / bullet time | scene `timeScale: (local) => 0.1…1` — slows tweens, particles, physics together |
| Solid 3D | `renderer3d({ width, height })`, `mesh.cube() / sphere() / torus() / torusKnot() / plane() / cylinder() / heightfield() / text() / custom()`, `instances(n, i => ({ x, y, z, rx, ry, rz, scale, sx, sy, sz, color }))`, `render(scene3d, [{ mesh, instances, material, cull, blend }], { background, fog, sky, horizon, ground, strips })` |
| Materials that don't look plain | dark bodies + light: `{ metal: 0.9, iridescence: 0.3 }` chrome, `{ lines: 1.5, lineColor, grid: [1, 1] }` glowing edges, `{ pulse: 2, pulsePhase: t }` travelling light, `{ emissive: 1 }` neon; set `strips` for crisp chrome highlights |
| Flying camera ("journey") | `flightPath(points, { lookAhead, bank })`, `path.timing([[sec, frac], …])`, `path.apply(cam, s(t), extraRoll)`; `cam.roll` for barrel rolls |
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
