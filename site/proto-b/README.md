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

- **Simulation:** `pointCloud()` (WebAssembly). Each splat spring-follows a blend between two forms, with its own delay and turbulence. The scroll position follows the target stage through a damped spring integrated in the frame loop, so wheel steps never jump and scrolling allocates nothing.
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

- **Console:** no errors or warnings with the default URL, with `?perf`, and with `prefers-reduced-motion: reduce`.
- **Tests:** `cargo test -p anikit-core` (158 + 1), `cargo clippy -p anikit-core -p anikit-wasm -D warnings` and `vitest` (292/292) pass after the merges.
- **Long tasks:** at 131k with `?nogl` (CPU work only) there are none. Every form-building step is 8–31 ms.
  - With SwiftShader drawing, one ~60–90 ms task remains at load (the full entry's embedded wasm).
  - Occasional build steps also stretch while the software rasteriser saturates the 4 cores.
- **Filmstrips:** made deterministic by ticking the engine at 60 fps in manual mode (`?perf` exposes `__engine`; `__st.noDraw` skips intermediate draws). They cover:
  - the intro: dust converges into "anikit" by ~2 s;
  - the brush and burst;
  - the lettering → flower transition with the caption dissolving into splats;
  - every form at 30k splats.
- **Governor:** under SwiftShader it steps 4 → 0 (glow, shadow, 60% splats, DPR 0.7) without errors.
- **Not measured:** SwiftShader is vertex-bound (~200 ms/frame at 8k instances), so real frame times on an integrated GPU still need a laptop run with `?perf`. The per-frame budget is one 1.5 MB `bufferSubData`, a ~1 ms wasm step and three instanced draws, with splat radius ≤ 4.5 device px.
