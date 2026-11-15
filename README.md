# Anikit

**A web animation engine with a Rust core.** One `no_std` Rust engine for timing, easing, springs, colours, paths, morphing and timelines. It compiles to WebAssembly for the browser (TypeScript API) and to a native extension for Python.

```ts
import { animate, timeline, stagger, spring, scroll, splitText, star } from 'anikit';

animate('.card', { y: [40, 0], opacity: [0, 1] }, { delay: stagger(0.05), ease: 'spring(bounce: 0.3)' });
animate('path', { d: star(50, 50, 5, 45, 20) }, { duration: 0.8, ease: 'inOutCubic' }); // shape morph
spring(ball, { x: 300, y: 120 }, { stiffness: 300, damping: 22 });                      // retargetable physics
scroll(animate('.progress', { scaleX: [0, 1] }, { ease: 'linear' }));                   // scroll-linked
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
| **`crates/anikit-core`** (Rust, `no_std`, `#![forbid(unsafe_code)]`) | The engine: WAAPI‑compatible timing, 40+ easings and every common easing syntax, closed‑form springs, inertia, CSS Color 4 parsing with OKLab/OKLCH interpolation, SVG path geometry, shape morphing, stagger, timelines, events. |
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
- **Utilities:**
  - `ease()`, `easing()`, `cssEasing()`, `mixColor()`, `parseColor()`;
  - `stagger()` (1D/2D grid, any origin, eased spread, seeded random);
  - `interpolate()` for numbers, colours and values with units;
  - `distribute()`, `clamp`, `lerp`, `mapRange`, `wrap`, `snap`, seeded `random`, `shuffle`, `pipe`.

Full TypeScript API: [`docs/api.md`](docs/api.md). Python API: [`crates/anikit-py/README.md`](crates/anikit-py/README.md).

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

**End to end in Node** (`cd packages/anikit && npm run bench`). This is WASM core plus the JS write path, compared with GSAP 3 animating the same plain objects:

| Case | N | Anikit | GSAP 3 |
|---|---:|---:|---:|
| 3 numbers per object | 10,000 | ~1.9 ms | ~0.6–0.9 ms |
| colour string | 10,000 | ~2.0–2.6 ms | ~1.0 ms |
| **`batch()`**, 3 channels | 100,000 | **~1.3 ms** | 7.9 ms (objects) |

These results are mixed, and the reasons matter:

- **Writing a property onto a JS object costs the same for every library.** A good JIT is already close to the hardware limit there. Anikit's per‑object bookkeeping (core handle → record → bindings) currently makes plain‑object tweening **2–3× slower than GSAP** in this synthetic test. With DOM targets, the style writes, style recalc and paint dominate a frame, so this difference is a small share of real frame time.
- **The architecture wins when JS stops touching every object.** `batch()` is ~6× faster than GSAP animating equivalent objects. The heavy‑math features (OKLab colour, morphing, exact springs, long timelines) run in WASM at native‑like speed, and `compositor()` moves work off the main thread entirely.

Bundle sizes: the default entry embeds the WASM module (zero config, 148 KB gz in total). `anikit/lean` is **31 KB gz of JS** that streams a separate **88 KB gz** `.wasm`, which is cacheable and compiles while it downloads. The JS side is ES modules marked side‑effect free, so bundlers drop the features you don't import (scroll, interaction, effects, inspector, …).

## Repository layout

```
crates/anikit-core   Rust engine (no_std) + tests + examples/bench.rs
crates/anikit-wasm   WebAssembly C-ABI shim (no_std, dlmalloc)
crates/anikit-py     Python package (PyO3 + maturin), tests, README
packages/anikit      TypeScript package: src/, test/ (vitest + happy-dom), scripts/
examples/            Browser demo (examples/index.html)
docs/                API reference
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

To see the demos, build the TS package and serve the repo root (for example `python -m http.server`). `/examples/` shows the core features and `/examples/scroll.html` the scroll and interaction suite.

## Status and limitations

This is v0.1 and the API may still change.

- **Repeats skipped in one frame:** if a large frame skips several iterations, one `REPEAT` event is reported, not one per iteration.
- **Handle reuse:** generation counters are 12‑bit, so a handle held through more than 4,096 reuses of the same slot could alias.
- **Morph keyframes:** shape morphs interpolate between two shapes. For more, chain tweens in a timeline.
- **Compositor mode:** `compositor()` needs `linear()` easing support to run springs on the compositor (Chrome 113+, Firefox 112+, Safari 17.2+). Otherwise it falls back to `ease-out`.
- **Path queries:** `MotionPath.closest()` is as precise as the path's arc‑length lookup table, typically within about 0.05 units.
- **Pinning:** `scrollTrigger` pinning uses `position: fixed` for the page and a counter‑transform inside scrolling containers. Pinned elements inside transformed ancestors behave like `fixed` elements always do in CSS, relative to the transformed ancestor.

MIT licensed.
