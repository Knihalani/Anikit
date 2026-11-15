# Performance notes

Before/after numbers for a round of optimisations of the Rust core and its
WebAssembly boundary. Every number here comes from a script in the repo:

| Script | What it measures |
|---|---|
| `cargo run --release -p anikit-core --example bench` | engine tick and per-easing cost, native |
| `cargo run --release -p anikit-core --example physics_bench` | 2D world step, native |
| `cargo run --release -p anikit-core --example cloud_bench` | cloud step, projection (+ radix sort), splat, tone map, emitter, native |
| `node packages/anikit/scripts/bench-wasm.mjs` | the same kernels through the `ak_*` exports of `dist/anikit.wasm` in Node (V8), no JS write path |
| `node packages/anikit/scripts/bench-cloud.mjs` | point cloud + glow end to end in headless Chromium |
| `node packages/anikit/scripts/bench-3d.mjs`, `bench-physics.mjs`, `bench.mjs` | Node comparisons against plain JS / matter.js / GSAP |

**Machine:** shared 4-core cloud VM (Intel Xeon, Sapphire Rapids class, 2.1 GHz,
2 MB L2 per core, KVM), Node 22, Chromium via Playwright. Other jobs ran on
the same VM throughout, so single runs vary by 10–30%.

**Method:** the baseline is the starting commit `e73317e`, built separately
(wasm with the same flags and `wasm-opt -O3` as `scripts/build-wasm.mjs`).
Every before/after pair was measured **interleaved** (baseline, new,
baseline, new, …); ranges are given where runs disagreed. `bench-wasm.mjs`
takes `WASM=path/to/build.wasm` for this, and the Node/Chromium scripts were
run against the two builds embedded in turn. V8 tiers wasm up lazily, so every
case warms up first (120 iterations in `bench-wasm.mjs`).

Native numbers use the default `x86_64` target (SSE2 only); the shipped
WebAssembly build uses `simd128`. Where they disagree, the WebAssembly number
is what users get.

**Equivalence:** every change except the sRGB encode (§5) produces
bit-identical results. Each has a test that runs the previous code (kept
verbatim in the test module) or a recorded golden hash against the new code;
for the wasm-specific SIMD paths the same scene is hashed natively and through
the wasm build in vitest, so the two targets are checked against each other.

## Summary

`bench-wasm.mjs`, ms per call, three interleaved runs each:

| Kernel | Size | `e73317e` | now |
|---|---:|---:|---:|
| tick: tween, 1 number, `outCubic` | 100k anims | 2.44–3.13 | 2.17–2.35 |
| tick: tween, 1 number, `cubic-bezier` | 100k anims | 5.31–6.90 | 5.01–5.65 |
| tick: x, y, opacity + OKLab colour | 50k anims | 5.28–5.87 | 3.61–3.78 |
| tick: 8 keyframes, spring easing | 50k anims | 4.27–4.31 | 3.24–3.58 |
| springs: retarget 2 ch every frame + tick | 10k | 0.79–0.80 | 0.61–0.66 |
| batch: 3 channels, `outCubic` | 100k instances | 1.13–1.18 | 0.35–0.39 (one 0.67 outlier) |
| batch: same, staggered delays | 100k instances | 1.13–1.21 | 0.48–0.51 |
| batch: `inOutQuad`, staggered delays | 100k instances | 1.25–1.30 | 0.63–0.71 |
| project | 150k points | 1.61–1.74 | 0.37–0.40 |
| project + depth sort | 150k points | 3.87–4.07 | 2.21–2.41 |
| splat draw + tone map (trail 0.5), 1080×675 | 150k points | 10.4–11.3 | 6.6–6.9 |
| splat draw + tone map (trail 0.5), 1920×1080 | 120k points | 30.4–30.6 | 18.2–19.4 |
| emitter step (turbulence + attractor) | 100k | 2.64–2.85 | 1.42–1.60 |
| world step (settled pile) | 1k / 5k / 20k bodies | unchanged | unchanged |
| cloud step | 150k | unchanged (1.57–1.70) | unchanged |

End to end:

| Script | Case | `e73317e` | now |
|---|---|---:|---:|
| `bench-cloud.mjs` (Chromium) | render, 50k particles | 8.6–8.8 ms | 3.2–3.6 ms |
| 〃 | render, 150k | 12.3–12.7 ms | 6.6–7.2 ms |
| 〃 | render, 300k | 18.1–18.3 ms | 11.7–12.0 ms |
| `bench-3d.mjs` | project + sort, 100k | 2.63–2.70 ms | 1.53–1.72 ms |
| 〃 | project + sort, 500k | 13.8–14.4 ms | 8.8–8.9 ms |
| `bench.mjs` | `batch()` 3 channels, 100k | 1.14–1.17 ms | 0.36–0.39 ms |
| 〃 | plain objects (3 numbers / colour / keyframes) | within noise | within noise |
| `bench-physics.mjs` | 250–5,000 bodies | unchanged | unchanged |

`bench.mjs` on plain objects is dominated by the TypeScript per-object write
path (the core's share of a 10k-object tick is ~0.2 ms of ~1.7 ms), which was
out of scope here, so core changes don't move it.

Wasm size: 397,170 → 398,281 bytes after `wasm-opt` (+1.1 KB).

## 1. Splat tone map: clamps as compare + select

`splat.rs`, the glow renderer's hottest loop. `f32::max`/`f32::min` have IEEE
`maxNum` semantics, which wasm's `f32x4.max/min` don't implement, so LLVM
scalarised them inside the vectorised tone-map loop (60 `extract_lane`s per
iteration). Written as `if x > 0 { x } else { 0 }` / `if x < m { x } else { m }`
they give the same result for every input, NaN included, and lower to vector
compares and selects. The row's "any light left" flag became an integer max
over the float bits (a float max reduction doesn't vectorise). The planes stay
planar, as AGENTS.md asks.

Tests: `splat::tests::matches_planar_reference_bit_for_bit` (the previous code
verbatim vs the new, 12 frames of bilinear/disc/fogged/lensed/off-screen
points, trails, clears, background changes, ink mode, odd sizes; RGBA and
accumulators compared exactly) and `golden_scene_matches_reference` + the
matching vitest in `test/cloud.test.ts` (wasm vs native).

| | before | after |
|---|---:|---:|
| wasm draw + resolve 1080×675 | 10.4–11.3 ms | 6.6–6.9 ms |
| wasm draw + resolve 1920×1080 | 30.4–30.6 ms | 18.2–19.4 ms |
| native tone map | unchanged | unchanged |

**Tried, kept in history, reverted:** an interleaved `[r, g, b]` accumulator
plus a hand-written `simd128` tone map (commit `ed6dde0`). It was faster on
wasm (draw + resolve 6.2 ms / 16.7–17.2 ms; the interleaved draw touches one
cache line per row instead of three), but it breaks the planar-slices
invariant and made the native tone map 10–25% slower. A branch-free disc
loop was ~20% slower (every bounding-box pixel becomes a store).

## 2. Projection and depth sort

`space.rs`. Packed xyz sources (clouds, emitters, point buffers) project
through a bounds-check-free path, four points per step with `simd128` on wasm
(de-interleave, project, cull with a select, 4×4 transpose back). The radix
depth sort builds every histogram in the key pass, rebases keys onto the range
actually present (`key - min + 1`, culled points stay 0) and splits only the
significant bits into equal digits of at most 11 bits: a typical cloud takes
three 8-bit passes, which scatter better than three 11-bit ones (fewer write
streams; two 12-bit passes measured slower). Passes whose digit is uniform are
skipped.

Tests: `projector_matches_reference_bit_for_bit` (previous projector verbatim;
strides, offsets, culled/NaN/∞ points, wide and narrow depth ranges,
all-culled) and `projector_golden_hash` + vitest in `test/world-3d.test.ts`.

| | before | after |
|---|---:|---:|
| wasm project, 150k | 1.61–1.74 ms | 0.37–0.40 ms |
| wasm project + sort, 150k | 3.87–4.07 ms | 2.21–2.41 ms |
| native project / sort, 150k | 0.60 / 2.2 ms | 0.32 / 1.6 ms |

## 3. Batches

`engine.rs`. Without per-instance delays (flag cached at creation) a
single-segment batch's instances share one progress, so it is timed and eased
once instead of per instance. With delays, one forward iteration (the common
stagger) uses the closed form of `Timing::eval` (before 0, active
`at / duration`, after 1) as selects, so the loop vectorises; other timings
keep `Timing::eval`. Lerp loops walk chunks (no bounds checks).

Tests: `uniform_batches_match_per_instance_eval_bit_for_bit` (previous
per-instance evaluation verbatim, with timing easing, segment easing, repeats,
alternate direction) and `staggered_batch_progress_matches_timing_eval`.

| | before | after |
|---|---:|---:|
| wasm 100k × 3, uniform | 1.13–1.18 ms | 0.35–0.39 ms |
| wasm 100k × 3, staggered | 1.13–1.21 ms | 0.48–0.51 ms |
| `bench.mjs` `batch()` 100k | 1.14–1.17 ms | 0.36–0.39 ms |

## 4. Emitter

`emitter.rs`. One loop aged, swap-removed and integrated with indexed access
to six arrays. A particle is only ever moved into the slot being examined, so
it ends at the slot index the fused loop integrated it at: an age/remove pass
followed by a chunked integrate pass (turbulence/attractor variants as
separate loops) is bit-identical (`step_matches_reference_bit_for_bit`, with
emission, deaths, variable `dt` and all four variants).

wasm 100k: 2.64–2.85 → 1.42–1.60 ms; native: 2.34–2.48 → 1.95–2.16 ms.

## 5. sRGB encode (the only non-bit-exact change)

`color.rs` / `math.rs`. `linear_to_srgb` ran a generic `fast_pow` (log2 with a
division, exp2 with a floor and a degree-9 series) per channel per frame.
`math::pow_5_12` splits `x = m·2^e`, so `x^(5/12) = m^(5/12) · 2^q · 2^(r/12)`
with `5e = 12q + r` (a 12-entry table), and fits `m^(5/12)` on `[1, 2)` with a
degree-8 Chebyshev polynomial evaluated by Estrin's scheme. Relative error
< 5e-9 (was ~1e-7), i.e. about 1e-6 of an 8-bit step; values change only at
that level. Tests compare against `libm::pow` over `[0.0031, 1e6]` and OKLab
interpolation end to end.

native `linear_to_srgb` 20.6 → 7.0 ns, OKLab `from_space` 48 → 16 ns;
wasm OKLab tween tick (50k): 5.28–5.87 → 3.61–3.78 ms.

## 6. Smaller ones

* **Spring easing:** `SpringParams::matrix` uses `libm::sincos` (one argument
  reduction; bit-identical to `sin` + `cos`, tested over 200k arguments).
  wasm 8-keyframe spring tick: 4.27–4.31 → 3.24–3.58 ms; native
  `ease(spring)` 37–39 → 34–35 ns (shuffled inputs 52–56 → 47–48 ns).
* **Tween tick:** the single-segment fast path of `Anim::eval_into` is inlined
  into the frame loop and the general track evaluation moved out of line
  (`eval_track`). Callgrind, 10k `outCubic` tweens × 200 ticks including
  setup: 636M → 570M instructions (−10%). wasm
  100k `outCubic` tweens: 2.37–2.50 → 2.12–2.20 ms against the previous
  commit. Natively within noise (the 8-keyframe spring case may be a few %
  slower natively; unchanged on wasm).
* **Wasm boundary:** `ak_sim_target`, `ak_sim_position` and
  `ak_sim_acceleration` copied their inputs into a fresh `Vec` on every call;
  `ak_world_constraints` built a `Vec` and copied it. All now read/write the
  scratch buffer in place. 10k springs retargeted per frame + tick:
  0.79–0.85 → 0.61–0.71 ms.

## Tried and not kept

| Idea | Result |
|---|---|
| `simd128` cloud step (4 particles per step, exact) | no gain: 10.0–10.9 ns/particle both ways even at 10k (cache-resident). LLVM turned the de-interleave into 64 `load32_lane`s; at 150k the step is memory-bound anyway (~76 bytes per particle). |
| Bezier: stop Newton at a fixed point (exact) | 21.8 → 40.6 ns/call natively: the data-dependent exit mispredicts, the fixed 4 iterations don't. |
| Bezier: branch-free interval count (exact) | slower for both monotonic and shuffled inputs (27–30 vs 21–26 ns). |
| Physics: dense pair loop per body against contiguous neighbour runs; bounds-check-free integrate; `[x, y, r, w]` gather (all exact) | within noise on wasm, slower natively at 1k bodies (cells hold ~one body, so the per-run setup dominates). The world is pinned by a golden scenario test (native + wasm) for future attempts. |
| `ease_slice` compiled per polynomial curve/mode (exact) | ~3% on a staggered `inOutQuad` batch for +3.3 KB of wasm. |
| Inlining all of `eval_into` | same speed-up as the split, but +6 KB of wasm (two copies). |
