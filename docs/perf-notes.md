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

`bench-wasm.mjs`, ms per call (median of 7 batches), three interleaved runs
each (the splat and emitter-shade rows were re-measured the same way after
their later commits):

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
| splat draw, 1080×675 (all bilinear) | 150k points | 5.3–5.6 | 4.5–5.2 |
| splat draw + tone map (trail 0.5), 1080×675 | 150k points | 10.2–10.8 | 5.8 |
| splat draw + tone map (trail 0.5), 1920×1080 (65% discs) | 120k points | 30.4–31.0 | 17.8–19.0 |
| emitter step (turbulence + attractor) | 100k | 2.64–2.85 | 1.42–1.60 |
| emitter shade (colour + size over life) | 100k | 1.22–1.24 | 0.39–0.40 |
| world step (settled pile) | 1k / 5k / 20k bodies | unchanged | unchanged |
| cloud step | 150k | unchanged (1.57–1.70) | unchanged |

End to end:

| Script | Case | `e73317e` | now |
|---|---|---:|---:|
| `bench-cloud.mjs` (Chromium) | render, 50k particles | 8.6–8.8 ms | 3.1–3.2 ms |
| 〃 | render, 150k | 12.3–12.7 ms | 6.2–6.8 ms |
| 〃 | render, 300k | 18.1–18.3 ms | 10.5–10.6 ms |
| `bench-3d.mjs` | project + sort, 100k | 2.63–2.70 ms | 1.53–1.72 ms |
| 〃 | project + sort, 500k | 13.8–14.4 ms | 8.8–8.9 ms |
| `bench.mjs` | `batch()` 3 channels, 100k | 1.14–1.17 ms | 0.36–0.39 ms |
| 〃 | plain objects (3 numbers / colour / keyframes) | within noise | within noise |
| `bench-physics.mjs` | 250–5,000 bodies | unchanged | unchanged |

`bench.mjs` on plain objects is dominated by the TypeScript per-object write
path (the core's share of a 10k-object tick is ~0.2 ms of ~1.7 ms), which was
out of scope here, so core changes don't move it.
The [second round](#second-round-plain-objects-end-to-end) below takes that
path on.

Wasm size: 397,170 → 403,742 bytes after `wasm-opt` (+6.6 KB, of which 4.8 KB
is the two-phase splat draw, §1).

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

**Two-phase draw.** Each block of 64 points is first resolved into small
"ops" (bilinear: top-left pixel, four weights, colour; disc: box, centre,
1/r², colour), then the read-modify-writes are applied in point order, so
every pixel still sums in the same order (same bit-exact tests). The apply
loop is short, so more of its scattered cache misses are in flight at once.
wasm, against the previous commit: draw 1080×675 5.43–5.50 → 4.48–4.92 ms,
draw + resolve 6.61–6.79 → 5.70–5.90 ms; 1080p (mostly discs) draw + resolve
18.2–19.0 → 17.3–18.3 ms. Native: unchanged. Costs 4.8 KB of wasm.

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

`shade` (colour and size over life, called every rendered frame) walks the
arrays with iterators instead of indexing five of them: wasm 100k
1.22–1.24 → 0.39–0.40 ms (`shade_matches_reference_bit_for_bit`).

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

## Second round: plain objects end to end

Goal: `scripts/bench.mjs` (N plain objects animated by one `animate()` call
vs GSAP 3, ms per frame) at or below GSAP on every row. Starting point:
`fc71751`. Same VM and caveats as above; every before/after pair below was
measured interleaved, usually with a split harness that times `ak_tick` and
`flush()` separately (wasm in Node, 50k objects, 60 frames, median of 5
runs, several interleaved rounds).

Where the time went at the start (50k objects x 3 numbers): `ak_tick` ~2.1 ms,
`flush()` ~3.1 ms. In the core, ~250 instructions per object (callgrind):
timing evaluation, phase bookkeeping, three easing calls and three track
structs for three lerps. In JS, the keyed store `t[k[i]] = f[at + o[i]]`
sees every key, goes megamorphic, and V8 boxes each double.

### Results

`bench.mjs`, ms per frame, median of 5 runs after 100 warm-up frames (the
script used to warm up for 5 frames, see below). `fc71751`: one run; now:
three runs.

| Case | N | `fc71751` | now | GSAP 3 (now runs) | now vs GSAP |
|---|---:|---:|---:|---:|---:|
| 3 numbers, `outCubic` | 1k | 0.085 | 0.021–0.057 | 0.067–0.078 | 1.4–3.3x faster |
| 〃 | 10k | 0.969 | 0.372–0.439 | 0.509–0.687 | 1.3–1.9x faster |
| 〃 | 50k | 4.633 | 1.854–2.032 | 2.529–2.756 | 1.3–1.4x faster |
| colour string | 1k | 0.075 | 0.052–0.060 | 0.088–0.123 | 1.5–2.4x faster |
| 〃 | 10k | 0.941 | 0.688–0.727 | 0.886–0.978 | 1.2–1.4x faster |
| 〃 | 50k | 4.676 | 3.390–3.547 | 4.543–4.944 | 1.3–1.5x faster |
| 8 keyframes, `inOutSine` | 1k | 0.053 | 0.031–0.036 | 0.024–0.026 | 0.7–0.8x |
| 〃 | 10k | 0.658 | 0.435–0.461 | 0.270–0.305 | 0.6–0.7x |
| 〃 | 50k | 3.321 | 2.189–2.314 | 1.206–1.449 | 0.5–0.7x |

At 100k (one run): 3 numbers 3.68 vs GSAP 5.85 ms, colour 7.85 vs 11.51 ms,
8 keyframes 4.77 vs 2.61 ms, `batch()` 0.37 ms.

**Keyframes stay behind GSAP.** After these changes a 50k-object keyframe
frame is ~1.1 ms of wasm tick (~20 ns per object: the node, its track and its
keyframe data are separate allocations, and the size sweep shows ~14 ns per
object even when everything is cache resident) plus ~1.0–1.2 ms of JS writes.
Part of the write cost is not ours to remove: the benchmark's props literal
`{ x: [0, 50, …] }` shares V8's `{ x }` shape with the targets and makes its
`x` field tagged (`--trace-generalization` shows it at `bench.mjs:46`), so
every double written to a target allocates a heap number (32 scavenges vs 8
for a single-segment `{ x: 500 }` over the same run). GSAP's targets share
that shape too.

**Warm-up.** `bench.mjs` now runs 100 untimed frames before each measurement
for both libraries (`WARMUP=5` gives the old method). With 5 frames the first
(1k) rows timed code V8 had not tiered up yet; the 50k rows are within noise
either way (`fc71751`, 3 numbers 50k: 4.13 ms with 5, 4.63 ms with 100).

**DOM** (`bench-dom.mjs`, Chromium, script ms per frame, two interleaved runs
each, `fc71751` vs now): 500 elements 0.74 / 0.83 vs 0.76 / 0.87; 2,000
3.29 / 2.58 vs 2.73 / 2.45; 5,000 8.75 / 7.59 vs 8.01 / 8.00, i.e. unchanged
within noise (elements write through transform bindings, not the plain-object
writers below).

Kernel benchmarks, `fc71751` vs now (one run each, except the springs,
batch, cloud and emitter rows: two interleaved runs each, which differ by
VM noise only, as those code paths did not change; the rows marked * have
every animation at the same progress or colour, so the memos hit on every
animation after the first; staggered work doesn't get that):

| Kernel | Size | `fc71751` | now |
|---|---:|---:|---:|
| `bench-wasm.mjs` tick: tween 1 number, `outCubic` * | 100k | 2.21 ms | 1.84 ms |
| 〃 tween 1 number, cubic-bezier * | 100k | 5.09 ms | 1.99 ms |
| 〃 x, y, opacity + OKLab colour * | 50k | 3.66 ms | 2.39 ms |
| 〃 8 keyframes, spring easing * | 50k | 3.18 ms | 1.41 ms |
| 〃 springs: retarget 2 ch + tick | 10k | 0.65–0.72 ms | 0.68–0.72 ms |
| 〃 batch: 3 channels, `outCubic` | 100k | 0.35–0.38 ms | 0.38–0.50 ms |
| 〃 cloud: step | 150k | 1.66–1.67 ms | 1.59–1.98 ms |
| 〃 emitter: step | 100k | 1.42–1.43 ms | 1.45–1.47 ms |
| native `bench`: tween 1 number, `outCubic` * | 100k | 1.48 ms | 1.38 ms |
| 〃 x, y, opacity + OKLab colour * | 50k | 3.33 ms | 1.40 ms |
| 〃 keyframes: 8 kf, spring * | 50k | 2.32 ms | 0.93 ms |
| 〃 timeline: 1 group x 50k children (staggered starts) | 50k | 0.53 ms | 0.58 ms |
| 〃 animate(): group of 50k x 3 tracks (new row) | 50k | — | 0.96 ms |
| 〃 animate(): group of 50k, 8 kf (new row) | 50k | — | 0.77 ms |

Wasm size: 657,280 → 665,608 bytes after `wasm-opt` (+8.3 KB).

### 1. One generated writer per plain-object key set (JS)

Parts whose bindings all write plain properties of one target already shared
key/offset arrays. Each distinct key/offset signature now also gets a writer
`t["x"]=f[a+0];t["y"]=f[a+1];…`, compiled once with `new Function` and shared
by every Part with that signature, so each store is a monomorphic named store.
Same values, same order. Code generation is probed once; under a CSP without
`'unsafe-eval'` the first attempt fails, is remembered, and the generic loop
is used (one violation report, not one per key set). This is one function per
property-set shape, not a closure per binding: the `Binding` loop is
untouched. Test: `animate: plain-object write path` runs one scenario
(including a mid-animation overwrite that drops a target out of its key set,
a seek and a second key set) through both paths and compares every value.

| `flush()`, 50k x 3 numbers | before | after |
|---|---:|---:|
| one key set in the process | 3.07 ms | 0.89–0.92 ms |
| four other key sets used first (polymorphic writer call) | 2.54–2.78 ms | 1.00–1.06 ms |
| 8 keyframes, four other key sets used first | 1.43 ms | 1.23–1.25 ms |

### 2. Easing memo (core)

The children of one `animate()` call share their timing, so each frame every
child and track evaluates the same easing at the same progress. The frame loop
keeps the last `(easing id, bits of t) → result`; `Easing::ease` is pure and
the registry append-only, so this is exact (bits, so `-0.0`/`0.0` and NaNs
never alias). Test: `memoised_easing_matches_direct_evaluation_bit_for_bit`
(90 children with staggered starts and delays, timing easings, repeats,
alternate direction, five easing ids interleaved, vs a direct evaluation).

| | before | after |
|---|---:|---:|
| `ak_tick`, 50k x 3 numbers | 2.08–2.26 ms | 1.66–1.69 ms |
| `ak_tick`, 50k x 8 keyframes | 1.78–1.87 ms | 1.58–1.64 ms |

`bench-wasm.mjs` rows whose anims all share one progress hit the memo too
(cubic-bezier 100k 5.09 → 1.82 ms, 8 keyframes spring 50k 3.18 → 1.53 ms);
staggered work doesn't get this.

### 3. Timing lane for group children (core)

After a group child is rendered in full in its active phase, the next child
whose local time and timing are bit-identical and which is already steady in
the same active iteration only sets its time, evaluates its tracks at the
known eased progress and is reported updated: no timing evaluation, no phase
or event bookkeeping (there would be no events). After 8 consecutive misses
the lane stops being checked for the rest of that group. `render()` returns
the progress as an `f64` (NaN = no lane) rather than an `Option`, which wasm32
returns through memory. Test: `group_lane_matches_full_render` runs one
scenario on two engines, one with the lane disabled, and compares values,
updated lists, events and node state every frame (seek, reverse, rate
changes, repeats, fill modes, a callback child); dropping the timing or the
iteration check from the lane fails it.

| `ak_tick`, 50k | before | after |
|---|---:|---:|
| 3 numbers | 1.71 (1.65–1.82) ms | 1.49 (1.45–1.55) ms |
| 8 keyframes | 1.54–1.64 ms | 1.28–1.34 ms |
| 3 numbers, staggered delays (never hits) | 1.52 (1.47–2.09) ms | 1.57 (1.54–2.13) ms |

The staggered case came out slower in all three A/B rounds (+8%, +6%, then
+3.6% median / +4.7% minimum for the committed variant), so count it as ~4%
slower, not noise. Natively the staggered `timeline: 1 group x 50k children`
row measured 0.54–0.59 ms before and 0.54–0.72 ms after.

### 4. Inline path for keyframed single numbers (core)

A one-channel numeric track with keyframes went through the out-of-line
general `eval_track` (~140 instructions per track: slicing, kind dispatch, a
zip loop for one value). `eval_into` now handles it next to the
single-segment path with the same cursor search, local progress, easing
choice and lerp. Test: `keyframed_number_fast_path_matches_eval_track`
(per-segment easings, duplicate and unsorted offsets, two keyframes off
[0, 1], forward/backward/random/edge progress incl. -0 and NaN; outputs and
cursors bit for bit). `ak_tick`, 50k x 8 keyframes: 1.34 (1.30–1.40) →
1.08 (1.05–1.18) ms.

### 5. Colour conversion memo (core)

Targets animated between the same colours interpolate to the same colour in
the track's space, and each converted it back to RGBA. The frame memo keeps
the last conversion (space + bits of the four channels → RGBA); `from_space`
is pure, so this is exact. Test:
`memoised_colour_conversion_matches_from_space` (five spaces interleaved,
shared and per-target colours, alpha 0). `ak_tick`, 50k colour strings:
2.20 (2.16–2.28) → 1.12 (1.06–1.17) ms; `bench-wasm.mjs` "x,y,opacity +
OKLab colour" (same colours): 3.56 → 2.09–2.39 ms.

### 6. Simple from/to animations skip the track table (core)

When every track is an enabled single-segment number of one channel with the
same easing (`{ x: 500, y: 300, opacity: 0 }`), the data layout is fixed
(track k: values at 4k + 2, output k), so `Anim::eval` evaluates the easing
once and lerps straight from the buffer without reading the 56-byte track
structs. The flag is set at creation and re-checked when a track is toggled.
Test: `simple_anims_match_track_evaluation`. `ak_tick`, 3 numbers: 50k
1.51 (1.42–1.52) → 0.99 (0.86–1.01) ms, 10k 0.27–0.32 → 0.19 ms.

### Tried and not kept (second round)

| Idea | Result |
|---|---|
| Direct-write table in `flush()`: target, writer and value offset in parallel arrays indexed by handle, so the loop never touches the record (Part) object | no gain (kf flush 0.99 → 1.06, 3 numbers 0.96 → 0.94 ms, interleaved): the Part load isn't the bottleneck. |
| Hot `Part` fields first | inconclusive / within noise. |
| Segment memo for evenly spaced keyframes (segment index and local progress shared across targets, exact) | 3–4% on the 8-keyframe tick, not worth the extra state. |
| Outputs first in the animation buffer (to share a cache line with the first values) | slower: 3 numbers 0.93 → 1.02, keyframes 1.08 → 1.25, colour 1.16 → 1.28 ms. |
| Indexed instead of `chunks_exact` loop for simple anims | within noise natively. |
| Generated per-key-set loop over runs of same-writer records in `flush()` | not attempted: the direct-write table above removed the same indirection without a gain. |

