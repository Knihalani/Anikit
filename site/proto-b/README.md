# Prototype B: living splat sculpture

A bright gallery room with one sculpture made of 131,072 Gaussian splats. Scrolling morphs it through six forms, each tied to an anikit feature. The cursor brushes through it like sand, and a click bursts it before it springs back.

Open `site/proto-b/` from a static server at the repo root, after building the package (`cd packages/anikit && node scripts/build-wasm.mjs && node scripts/build.mjs`).

| Form | Built with | Driven by |
|---|---|---|
| "anikit" | `splats.fromText` | intro morph out of a dust cloud |
| flower | `splats.fromMesh` (petal mesh) + `splats.fromSDF` (centre) + `splats.fromPoints` (stem), `splats.merge` | looser per-splat springs (`cloud.set`) |
| flock | wing phases from `staggerValues(72, …, { grid, from: 'center' })` | caption lines enter with `stagger()` |
| wave | sheet of points | swell from a looping `timeline()` (64 → 24, yoyo) |
| galaxy | 3-arm spiral | spin and tilt follow the scroll |
| `npm i anikit` | `splats.fromText` (mono) | |

- **Simulation:** `pointCloud()` (WebAssembly). Each splat spring-follows a blend between two forms, with its own delay and turbulence. The scroll position springs (`spring()`) toward the target stage, so wheel steps never jump.
- **Brush:** `cloud.push()` along the pointer ray, with sweep velocity. Click: `cloud.explode()`, with the springs briefly loosened so the burst blooms.
- **Captions:** each caption is rasterised by `htmlCanvas(el, { target: gl })` into a texture *in the same WebGL2 context*. On exit the DOM caption is swapped for one splat per texel, which drift apart toward the sculpture.
- **Rendering:** one canvas, one WebGL2 context (`splat-gl.js`):
  - instanced Gaussian quads;
  - a half-resolution layer holding the floor shadow (1/8 of the splats) and glow (1/4), composited with a 5-tap tent;
  - splat radius capped at 4.5 device px, DPR capped at 2.
- **Per frame:** one `bufferSubData` of positions, no allocation.
- **`?perf`:** shows frame p50/p95/max and the quality level. The governor steps glow → shadow → splat count (eased fade by index) → resolution (DPR × 0.7), with hysteresis, and steps back up after 6 s of headroom.
- **Debug flags:** `?q=0..4` locks quality, `?n=` sets the splat count, `?test` turns off lag smoothing for slow headless runs, and `?nogl` skips rendering.
- **Startup:** forms are generated in steps under 50 ms after first paint (no long tasks at 131k in Chromium).

## Library gaps and changes made here

- **Added `PointCloud.push(x, y, z, radius, strength, vx, vy, vz, ax, ay, az)`** (core `Cloud::push`, wasm `ak_cloud_push`): a local, allocation-free brush along any axis. `explode()` was the only interaction, and it is global.
- **`splats()` has no dynamic-position path.** Positions live in a packed `RGBA32UI` texture and morph on the GPU between two sets, so per-splat spring physics and the brush can't drive it without repacking and re-sorting 131k splats every frame. This page therefore uses `splats.*` to *build* the forms and `pointCloud()` to *move* them, and draws with its own isotropic splat shader. A `SplatLayer.positions(Float32Array)` hook (centres only, covariance kept) would let it switch over.
- **`anikit/splats` shares a chunk with the full `anikit` entry**, which embeds the wasm, not with `anikit/lean`. Mixing lean + splats would load two wasm instances, so this page imports `anikit.js`. That costs one ~60 ms task at load (base64 decode + compile) before the sculpture appears.
- **`htmlEffect()` creates its own WebGL context per effect.** `htmlCanvas({ target: gl })` was used instead, so the page keeps a single context.

## Checks (headless Chromium, SwiftShader)

No console errors. No long tasks after load. Filmstrips of the intro, brush, burst, caption dissolve and every form are in the session notes. SwiftShader can't show real frame times: it is vertex-bound at ~40k instances, about 200 ms/frame. Real-GPU timing still needs a laptop run with `?perf`.
