# Anikit

**A web animation engine with a Rust core.** One `no_std` Rust engine for timing, easing, springs, colours, paths, morphing, timelines, 2D physics, 3D maths, particle clouds and GIF encoding. It compiles to WebAssembly for the browser (TypeScript API) and to a native extension for Python.

The landing page in [`site/`](site/) is built entirely with Anikit: a scroll-driven "poster film" where 150,000 particles print as ink on paper and glow as light on dark, colour fields flood from scene to scene, giant letters fall into a physics world full of sand, every word on screen can be dropped and thrown, and holding Space slows the whole engine down. [`site/demo/record-demo.mjs`](site/demo/record-demo.mjs) renders a directed, frame-exact 1080p60 film of it (virtual time, motion blur, camera moves).

```ts
import { animate, timeline, stagger, spring, scroll, splitText, star, world, pointCloud, textPoints, glow, scene3d, engine, record } from 'anikit';

animate('.card', { y: [40, 0], opacity: [0, 1] }, { delay: stagger(0.05), ease: 'spring(bounce: 0.3)' });
animate('path', { d: star(50, 50, 5, 45, 20) }, { duration: 0.8, ease: 'inOutCubic' }); // shape morph
spring(ball, { x: 300, y: 120 }, { stiffness: 300, damping: 22 });                      // retargetable physics
scroll(animate('.progress', { scaleX: [0, 1] }, { ease: 'linear' }));                   // scroll-linked

const w = world({ container: '#stage' });                 // 2D physics in WebAssembly
w.bind('.letter', { rotate: true }); w.dragging();
const cloud = pointCloud(150_000, { shapes: [textPoints('hello', 150_000), (i) => [i % 400, i % 300, 0]] });
const g = glow('#bg'), scene = scene3d();
engine.onFrame(() => g.render(scene, cloud));             // particles → light, in the core
await record('#hero', { duration: 2 });                   // frame-exact GIF
```

```python
import anikit

box = {"x": 0, "fill": "#ff0000"}
anikit.animate(box, {"x": 100, "fill": "royalblue"}, duration=0.6, ease="outBack")
for frame, t in anikit.render(fps=60):
    draw(box)
```

---

## Why it's built this way

| Layer | What it does |
|---|---|
| **`crates/anikit-core`** (Rust, `no_std`, `#![forbid(unsafe_code)]`) | The engine: WAAPI‑compatible timing, 40+ easings and every common easing syntax, closed‑form springs, inertia, CSS Color 4 parsing with OKLab/OKLCH interpolation, SVG path geometry, shape morphing, stagger, timelines, events. Plus a PBD physics world, 3D transform maths and projection, point clouds, an additive splat renderer and a GIF encoder. |
| **`crates/anikit-wasm`** | A raw C‑ABI WebAssembly build with **no wasm‑bindgen glue**. Inputs go through a scratch buffer; outputs are read **zero‑copy** from linear memory. A frame costs **one** host→WASM call no matter how many animations are running. |
| **`packages/anikit`** (TypeScript) | The DOM layer: per‑element transform composition, one style write per element per frame, dirty‑only updates, property ownership (auto‑overwrite), and a monomorphic binding loop the JIT can inline. |
| **`crates/anikit-py`** (PyO3) | Python bindings and an idiomatic high‑level API over the same engine. Exports to CSS `@keyframes` and WAAPI. |

Memory safety comes from Rust (the only `unsafe` is the single‑threaded static in the WASM shim). Handles are generational, so a stale handle can't touch a reused slot. Finished, unreferenced animations are freed automatically: through a `FinalizationRegistry` in JS, and through reference tracking in Python.

## Features

- **Tweens and keyframes:** arrays, WAAPI‑style keyframe lists, per‑segment easing, `times`, relative values (`+=`, `-=`, `*=`), function values per target, unit conversion (px, %, em, rem, vw, vh, deg, rad, turn, …).
- **Anything animatable:**
  - transforms: `x`, `y`, `z`, `rotate*`, `scale*`, `skew*`, `perspective`, composed per element;
  - any CSS property, CSS variables, SVG attributes (`attr: {…}`) and plain JS/Python objects;
  - colours in any CSS syntax: hex, `rgb`, `hsl`, `hwb`, `lab`, `lch`, `oklab`, `oklch`, `color()`, named. They interpolate in **OKLab** by default (premultiplied, with powerless‑hue handling) or in sRGB, linear‑RGB, OKLCH or HSL;
  - "complex" strings: `box-shadow`, `filter`, `clip-path`, gradients. Every number and colour inside is interpolated, and `none` is lifted to a zero template;
  - discrete values such as `display` and `visibility`.
- **Easings:**
  - CSS: `ease`, `cubic-bezier()`, `steps()` with every jump term, `linear()` with stops;
  - Penner/Motion names: `easeInOutCubic`, `outBack`, `circIn`; GSAP names: `power2.inOut`, `back.out(1.7)`, `elastic.out(1, .3)`;
  - springs, given as stiffness/damping/mass or perceptual `bounce`/`duration`;
  - `wiggle`, `shake`, `slowmo`, `anticipate`, and custom JS functions (baked into a lookup table).
- **Timelines:** GSAP‑style positions (`'<'`, `'>'`, `'+=0.5'`, `'label+=0.2'`, …), labels, `call()` callbacks that fire in both directions, nesting, repeat/yoyo/repeatDelay, timeline‑level easing, and value chaining between sequential tweens. Seeking is exact in both directions: children render in reverse order when going backwards.
- **Physics:**
  - `spring()` keeps velocity when you retarget it. It is solved exactly in closed form, so it stays stable at any frame time;
  - `inertia()` handles momentum, bounds with spring bounce, and snapping.
- **SVG:**
  - shape morphing between any two paths. Subpath and segment counts are matched, and closed shapes are rotated and reversed to minimise point travel;
  - motion paths with arc‑length parameterisation and auto‑rotate;
  - stroke drawing (`draw`);
  - path length and exact bounds, and shape builders (`circle`, `rect`, `star`, `polygon`).
- **Scroll and viewport:** `scroll()` links progress to scroll, using target/container edge offsets and optional spring smoothing. `inView()` fires enter and leave callbacks.
- **Text:** `splitText()` splits into lines, words or characters, keeps nested markup, handles graphemes correctly, preserves an `aria-label`, and can `revert()`.
- **Layout:** `flip()` gives FLIP transitions between any two layouts.
- **Compositor offload:** `compositor()` hands transform and opacity work to the browser's compositor thread through WAAPI. Any easing, springs included, is converted by the core into a minimal native `linear()` easing.
- **Instanced batches:** `batch()` animates up to millions of instances into one contiguous `Float32Array`. It's built for canvas and WebGL, with zero per‑instance JS work.
- **Control:** play, pause, reverse, restart, seek, `progress`, `speed`, `complete`, `cancel`, `kill`. Animations are awaitable (`await animate(...)`) and support `onStart`, `onUpdate`, `onComplete`, `onRepeat` and `onReverseComplete`. There's also a global time scale, lag smoothing, an FPS cap, and a manual tick mode for tests, SSR and video export.
- **Scroll suite:**
  - `scrollTrigger()`: start/end in any unit, scrub with optional spring smoothing, pinning, snapping (including to timeline labels), toggle actions and classes, callbacks for every direction, and debug markers;
  - `scrollTriggerBatch()`, `scrollTo()` and `smoothScroll()` (inertial wheel scrolling);
  - `observer()`: unified wheel, touch, pointer and scroll intents.
- **Interaction:** `draggable()` gives momentum, bounds, edge resistance, axis locking, snapping and rotation knobs. `hover()` and `press()` are gesture helpers, and `VelocityTracker` measures pointer speed.
- **More easings:** draw your own as an SVG path (`custom(M0,0 C…)`), modifiers (`reverse()`, `mirror()`, `blend()`), spring presets (`spring.gentle`, `wobbly`, …), `rough()` and `expoScale()`.
- **More values:**
  - `random(…)` strings resolved per target;
  - `snap`, `round`, `modifiers` (for wrap‑around carousels) and `format` (Intl number counters);
  - `text` typewriter and scramble effects, `xPercent`/`yPercent`;
  - morphing through any number of shapes.
- **Ballistic physics:** `physics()` covers gravity, wind, drag and bouncing bounds. Impacts are resolved at the moment of collision, so it comes to rest the same way at any frame rate.
- **Effects library:** 35+ ready presets (entrances, exits, attention seekers, loops, text, SVG). Add your own with `registerEffect()`.
- **Cleanup and responsiveness:**
  - `revert()` restores original inline styles;
  - `context()` gives scoped, framework‑friendly cleanup;
  - `matchMedia()` runs responsive setups that revert themselves;
  - `engine.reducedMotion` handles accessibility.
- **Workflow:**
  - `sequence()` builds a timeline from an array;
  - timelines gain `tweenTo`/`tweenFromTo`, `remove`, `clear` and label navigation;
  - also `quickTo`, `quickSetter` and `delayedCall`;
  - `toCSS()` bakes any animation into native `@keyframes`;
  - `inspector()` is a dev panel for scrubbing animations.
- **Noise and geometry:** seeded simplex noise and fBm (`noise2`, `noise3`, `fbm`, `wander`). Paths can be trimmed, smoothed through points, simplified, flattened and measured for area, and you can find the closest point on a path.
- **Physics worlds:** `world()` is a 2D rigid‑circle solver in WebAssembly: sub‑stepped position‑based dynamics, a sorted‑grid broad phase, restitution and friction, ropes, cloth with tearing, static boxes from elements, attractors, pointer forces, picking and drag‑to‑throw. DOM elements can be bodies (`bind()`), or draw thousands of bodies to a canvas.
- **3D:**
  - `transform` lists interpolate exactly like browsers do: decomposition, quaternion slerp, recomposition, all in the core;
  - `scene3d()` projects and radix‑sorts point sets in WebAssembly, with an orbit camera; `tilt()` gives spring‑driven pointer tilt.
- **Point clouds and glow:** `pointCloud()` simulates hundreds of thousands of particles, each spring‑following a blend between target shapes through a turbulence field. `glow()` renders them as additive light directly into pixels (bilinear splats, bokeh, fog, a pointer lens, trails, tone mapping), with adaptive resolution to hold a frame budget. `textPoints()` samples shapes from text.
- **Video creation:** `composition()` scenes, cues, transitions (iris, push, whip, slice, zoom, flash, letterform mask or your own) and kinetic type (`typeFx`, including `slam`) render frame-exactly with motion blur; `exportWebM()` encodes in the browser via WebCodecs with a built-in muxer, and `anikit-render` renders in Node (Skia + ffmpeg) across parallel workers with audio. Frames convert to I420 in WebAssembly. The [film](film/) is made this way, transitions included.
- **Particle emitters:** `emitter()` spawns and simulates particles in WebAssembly (shapes, cones, gravity, drag, turbulence, attractors, colour/size over life) and renders through `glow()`.
- **Audio analysis:** `analyzeAudio()` finds tempo, beats and onsets (FFT + spectral flux in WebAssembly) so animation and video cuts land on the music.
- **Capture:** `screenshot()` and `record()` produce frame‑exact GIF, WebM or PNG frames of any element or canvas, with GIFs encoded in the core. The `anikit-record` CLI records any page headlessly to GIF, MP4, WebM or PNG.
- **Built for agents (and humans):** `engine.inspect()` returns live JSON of everything running; `testing` gives fake time, `advance()`, `finishAll()` and warning capture; warnings carry "did you mean" suggestions; `AGENTS.md` files and generated `llms.txt` / `llms-full.txt` describe the repo and the API for coding agents.
- **One clock:** `engine.speed` slows tweens, springs, physics worlds, particle clouds and `onFrame` callbacks together.
- **Utilities:**
  - `ease()`, `easing()`, `cssEasing()`, `mixColor()`, `parseColor()`;
  - `stagger()` (1D/2D grid, any origin, eased spread, seeded random);
  - `interpolate()` for numbers, colours and values with units;
  - `distribute()`, `clamp`, `lerp`, `mapRange`, `wrap`, `snap`, seeded `random`, `shuffle`, `pipe`.

Task recipes (tested): [`docs/recipes.md`](docs/recipes.md). Full TypeScript API: [`docs/api.md`](docs/api.md). Python API: [`crates/anikit-py/README.md`](crates/anikit-py/README.md). Coding agents: [`packages/anikit/AGENTS.md`](packages/anikit/AGENTS.md) and [`llms-full.txt`](llms-full.txt).

## Performance

All numbers are from a shared 4‑core cloud VM, so treat them as indicative. Reproduce them with the commands below.

**Core engine** (native, `cargo run --release -p anikit-core --example bench`), per animation per frame:

| Workload | Items | ms/frame | per item |
|---|---:|---:|---:|
| Tween, 1 track, `outCubic` | 100,000 | 1.5–1.7 | ~16 ns |
| Timeline with 50k children | 50,000 | 0.56 | ~11 ns |
| Springs, 2 channels | 100,000 | 1.2 | ~12 ns |
| x, y, opacity + OKLab colour | 50,000 | 5.9 | ~118 ns |
| 290‑channel shape morph | 2,000 | 0.5 | ~250 ns |

The optimizations behind these numbers were all measured, not assumed:
- a compacted per‑animation memory layout (data and outputs in one allocation): −35%;
- fast paths for single‑segment tracks;
- cold‑path event emission;
- idle timeline children cost a single timing evaluation;
- one spring transition matrix per configuration per frame;
- slice‑wise easing for batches;
- a polynomial `pow` for the sRGB transfer function (error under 1e‑7);
- an O(1) `group_add`.

**Against alternatives.** These are where moving work into WebAssembly pays off. Each row comes from a script in `packages/anikit/scripts/`, run on the same VM.

| Workload | Anikit | Alternative | Speed‑up | Script |
|---|---:|---:|---:|---|
| 2D physics, 250 colliding bodies, per step | 0.09 ms | matter.js 3.9–5.4 ms | 43–60× | `bench-physics.mjs` |
| 2D physics, 1,000 bodies | 0.28–0.47 ms | matter.js 7.2–7.4 ms | 16–26× | 〃 |
| 2D physics, 5,000 bodies | 1.3 ms | matter.js 11.3–11.9 ms | ~9× | 〃 |
| 3D project + depth sort, 100k points | 3.2 ms | optimised plain JS 35.6 ms | 11× | `bench-3d.mjs` |
| 3D project + depth sort, 500k points | 13.8 ms | plain JS 217 ms | 16× | 〃 |
| 2,000 animated DOM elements, real Chromium | 56–59 fps | GSAP 26–27 fps | ~2.2× fps | `bench-dom.mjs` |

Physics numbers vary between runs at small sizes (matter.js's per‑step overhead dominates there), so ranges are shown. In the DOM benchmark at 5,000 elements Anikit ran at 26–32 fps against GSAP's 19, and Motion was 3–5× slower than both. Most of a DOM frame is the browser's own style, layout and paint work, which no library can skip; Anikit wins by doing less JS and skipping unchanged writes.

**Particles** (`bench-cloud.mjs`, Chromium). Per frame: simulation, projection, splat, tone map and `putImageData` at 1080×675. Every row is lit in this test, which is the worst case; sparse shapes skip empty rows.

| Particles | Simulate | Render | Total |
|---:|---:|---:|---:|
| 50,000 | 0.6 ms | 8.4 ms | 9.0 ms |
| 150,000 | 2.1 ms | 11.4 ms | 13.4 ms |
| 300,000 | 4.7 ms | 16.5 ms | 21.2 ms |

On this VM the tone‑map pass is memory‑bandwidth bound (the same loop runs in ~3 ms natively here). Desktop machines with more memory bandwidth are faster, and `glow({ budget })` lowers the resolution automatically where they aren't.

One fix sped up everything that uses square roots or floors: libm's `sqrt`/`floor` compile to software routines on `wasm32` unless nightly‑only features are enabled. The core now calls the stable SIMD intrinsics on one lane, which lower to the native instructions. The particle simulation became 3.4× faster, and the physics speed‑up over matter.js went from 7–27× to 9–44× (up to 60×) in the same benchmark.

**End to end in Node** (`cd packages/anikit && npm run bench`). This is WASM core plus the JS write path, compared with GSAP 3 animating the same plain objects:

| Case | N | Anikit | GSAP 3 |
|---|---:|---:|---:|
| 3 numbers per object | 10,000 | ~1.9 ms | ~0.6–0.9 ms |
| colour string | 10,000 | ~2.0–2.6 ms | ~1.0 ms |
| **`batch()`**, 3 channels | 100,000 | **~1.3 ms** | 7.9 ms (objects) |

These results are mixed, and the reasons matter:

- **Writing a property onto a JS object costs the same for every library.** A good JIT is already close to the hardware limit there. Anikit's per‑object bookkeeping (core handle → record → bindings) currently makes plain‑object tweening **2–3× slower than GSAP** in this synthetic test. With DOM targets, the style writes, style recalc and paint dominate a frame, so this difference is a small share of real frame time.
- **The architecture wins when JS stops touching every object.** `batch()` is ~6× faster than GSAP animating equivalent objects. The heavy‑math features (OKLab colour, morphing, exact springs, long timelines) run in WASM at native‑like speed, and `compositor()` moves work off the main thread entirely.

Bundle sizes: the default entry embeds the WASM module (zero config, 335 KB gz in total). `anikit/lean` is **68 KB gz of JS** that streams a separate **187 KB gz** `.wasm`, which is cacheable and compiles while it downloads. The JS side is ES modules marked side‑effect free, so bundlers drop the features you don't import (scroll, interaction, effects, inspector, …).

## Repository layout

```
crates/anikit-core   Rust engine (no_std) + tests + examples/ (bench, physics_bench, cloud_bench)
crates/anikit-wasm   WebAssembly C-ABI shim (no_std, dlmalloc)
crates/anikit-py     Python package (PyO3 + maturin), tests, README
packages/anikit      TypeScript package: src/, test/ (vitest + happy-dom), scripts/ (build, benchmarks), bin/anikit-record
site/                The landing page (built with Anikit)
examples/            Browser demos (examples/index.html, examples/scroll.html)
docs/                API reference
AGENTS.md, llms.txt  Guides for coding agents
```

## Building and testing

Requirements: Rust (stable) with the `wasm32-unknown-unknown` target, Node 18+, and Python 3.9+.

```bash
cargo test -p anikit-core
```

```bash
cd packages/anikit && npm install && npm test && npm run build
```

```bash
cd crates/anikit-py && python -m venv .venv && . .venv/bin/activate && pip install maturin pytest && maturin develop --release && pytest
```

To see the demos, build the TS package and serve the repo root (for example `python -m http.server`). `/site/` is the landing page, `/examples/` shows the core features and `/examples/scroll.html` the scroll and interaction suite.

Benchmarks: `npm run bench`, `npm run bench:physics`, `npm run bench:3d`, `npm run bench:dom`, `npm run bench:cloud` (in `packages/anikit`) and `cargo run --release -p anikit-core --example bench` / `physics_bench` / `cloud_bench`.

## Status and limitations

This is v0.1 and the API may still change.

- **Repeats skipped in one frame:** if a large frame skips several iterations, one `REPEAT` event is reported, not one per iteration.
- **Handle reuse:** generation counters are 12‑bit, so a handle held through more than 4,096 reuses of the same slot could alias.
- **Physics shapes:** `world()` bodies are circles. Boxes are static colliders; rigid shapes are built from circles held by constraints (the landing page's "gravity" words work this way).
- **Glow rendering is on the CPU:** `glow()` is WebAssembly writing pixels, not WebGL. It's fast because it's simple, but very large canvases at full resolution are memory‑bandwidth bound; use `resolution` / `budget`. For millions of points per frame, hand `cloud.positions` or `batch.data` to WebGL instead.
- **Compositor mode:** `compositor()` needs `linear()` easing support to run springs on the compositor (Chrome 113+, Firefox 112+, Safari 17.2+). Otherwise it falls back to `ease-out`.
- **Path queries:** `MotionPath.closest()` is as precise as the path's arc‑length lookup table, typically within about 0.05 units.
- **Pinning:** `scrollTrigger` pinning uses `position: fixed` for the page and a counter‑transform inside scrolling containers. Pinned elements inside transformed ancestors behave like `fixed` elements always do in CSS, relative to the transformed ancestor.

MIT licensed.
