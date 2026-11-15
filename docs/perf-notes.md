# Performance notes

Before/after numbers for optimisations of the Rust core and its WebAssembly
boundary. Every number here comes from a script in the repo:

| Script | What it measures |
|---|---|
| `cargo run --release -p anikit-core --example bench` | engine tick, native |
| `cargo run --release -p anikit-core --example physics_bench` | 2D world step, native |
| `cargo run --release -p anikit-core --example cloud_bench` | cloud step, projection (+ radix sort), splat, tone map, emitter, native |
| `node packages/anikit/scripts/bench-wasm.mjs` | the same kernels through the `ak_*` exports of `dist/anikit.wasm` in Node (V8), no JS write path |
| `node packages/anikit/scripts/bench-cloud.mjs` | point cloud + glow end to end in headless Chromium |
| `node packages/anikit/scripts/bench-physics.mjs`, `bench-3d.mjs`, `bench.mjs` | Node comparisons against matter.js / plain JS / GSAP |

**Machine:** shared 4-core cloud VM (Intel Xeon, Sapphire Rapids class, 2.1 GHz,
2 MB L2 per core, KVM), Node 22, Chromium via Playwright. Other jobs ran on the
same VM during these measurements, so single runs vary by 10–30%. Every
before/after pair below was measured **interleaved** (baseline build, new
build, baseline, new, …) against a baseline built from the starting commit
(`e73317e`), and ranges are given where runs disagreed. `bench-wasm.mjs`
accepts `WASM=path/to/build.wasm` for exactly this.

Native numbers use the default `x86_64` target (SSE2 only); the shipped
WebAssembly build uses `simd128`. Where the two disagree, the WebAssembly
number is the one users see.

## Baseline (`e73317e`)

`bench-wasm.mjs` (V8, ms per call, median of 7 batches):

| Kernel | Size | ms |
|---|---:|---:|
| tick: tween 1 number, `outCubic` | 100k anims | 2.42–2.50 |
| tick: tween 1 number, `cubic-bezier` | 100k anims | 5.59–6.03 |
| tick: x, y, opacity + OKLab colour | 50k anims | 5.56–5.67 |
| tick: 8 keyframes, spring easing | 50k anims | 4.32–4.58 |
| world: 4 substeps, settled pile | 1k / 5k bodies | 0.29 / 1.53–1.59 |
| world: 2 substeps, settled pile | 20k bodies | 4.45–4.68 |
| cloud step | 150k | 1.60–2.06 |
| project / project + depth sort | 150k | 1.69–1.70 / 4.08–4.10 |
| splat draw 1080×675 (0% discs) | 150k points | 5.3–6.2 |
| splat draw + resolve (trail 0.5) 1080×675 | 150k points | 10.5–11.5 |
| splat draw 1920×1080 (65% discs) | 120k points | 14.3–15.3 |
| splat draw + resolve (trail 0.5) 1920×1080 | 120k points | 29.1–31.5 |
| emitter step (turbulence + attractor) | 100k | 2.53–2.60 |

`bench-cloud.mjs` (Chromium; render = project + splat + tone map + `putImageData` at 1080×675):
50k particles render 8.8–10.3 ms, 150k render 12.5–14.2 ms.

`bench.mjs` (Node, JS write path included, vs GSAP): 3 numbers × 10k objects
1.98 ms (GSAP 0.58 ms). This case is dominated by the TypeScript per-object
write path (the core's share of the 10k tick is ~0.2 ms), which is outside the
scope of this work, so core changes barely move it.

## Optimisations

### 1. Splat: interleaved accumulator, select-based clamps, SIMD tone map

`splat.rs` (the glow renderer's hot code).

* **Clamps as compare + select.** `f32::max`/`f32::min` have IEEE `maxNum`
  semantics that wasm's `f32x4.max/min` don't implement, so LLVM scalarised
  them inside the vectorised tone-map loop (60 `extract_lane`s, scalar
  `max`es). Writing them as `if x > 0 { x } else { 0 }` gives the same result
  for every input (NaN included) and lowers to `pmax`/`pmin`/`gt`+`and`.
* **"Any light in this row"** is tracked as an integer max over the float
  bits instead of a float max reduction, which vectorises.
* **Interleaved `[r, g, b]` accumulator** instead of three planes: a splat
  touches one cache line per row instead of three. The disc path keeps its
  branch (a branch-free select variant measured ~20% *slower* on wasm: it
  turns every bounding-box pixel into a store).
* **Explicit `simd128` tone map** (`resolve_simd`, safe intrinsics, cfg-gated
  with a scalar fallback): four pixels as three `f32x4`
  (`[r0 g0 b0 r1] [g1 b1 r2 g2] [b2 r3 g3 b3]`) against per-lane channel
  constants, then one narrow + shuffle packs 4 RGBA words.

Equivalence: bit-identical. `splat::tests::matches_planar_reference_bit_for_bit`
runs the old planar implementation (kept verbatim in the test module) against
the new one over 12 frames of mixed bilinear/disc/fogged/lensed/off-screen
points, trails, clears, background changes and ink mode, comparing RGBA and the
accumulators exactly. `golden_scene_matches_reference` pins a scene hash, and
`test/cloud.test.ts` renders the same scene through the wasm build (the SIMD
path) and expects the same hash (a deliberately broken SIMD constant makes it
fail). The only behavioural difference is for invalid input: negative or NaN
light used to wrap into neighbouring bytes of the RGBA word; the SIMD path
saturates instead.

| Kernel (wasm, `bench-wasm.mjs`) | before | after |
|---|---:|---:|
| draw 1080×675, 150k (bilinear) | 5.3–5.9 ms | 4.6–4.8 ms |
| draw + resolve 1080×675, 150k | 10.5–10.9 ms | 6.2–7.3 ms |
| draw 1920×1080, 120k (65% discs) | 15.1–15.3 ms | 12.1–13.0 ms |
| draw + resolve 1920×1080, 120k | 29.1–29.6 ms | 16.7–19.6 ms |

`bench-cloud.mjs` (Chromium, interleaved A/B): render 50k **8.8–10.3 → 4.3–4.8 ms**,
150k **12.5–14.2 → 7.3–8.5 ms**.

Native (`cloud_bench`, SSE2): draw 1080×675/150k 3.8 → 3.2–3.5 ms, 1080p
12.6–15.7 → 11.5–12.1 ms; the tone map is ~10–25% *slower* natively
(2.6–2.8 → 3.0–3.3 ms at 1080×675) because SSE2 has no cheap stride-3
de-interleave and the SIMD path is wasm-only. Splat is only used through
WebAssembly (browser glow and the video renderer), so the trade is taken.
