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


## Third round: keyframes on plain objects

Goal: the "8 keyframes" row of `scripts/bench.mjs`. Starting point:
`067c4a2`. What the row compares: Anikit evaluates one keyframed track per
target with `inOutSine` on each of the 7 evenly spaced segments; GSAP turns
`keyframes: { x: [...] }` into a timeline of 8 tweens over all targets (one
per segment, `power1.inOut` each by default, `ease: 'none'` over the whole),
so per frame only the active segment's tween renders: one ease call, then
`start + change * ratio` and a store per target.

Where the time went (50k targets, `ak_tick` and `flush()` timed separately,
Node): tick ~1.05 ms (~187 instructions per child natively, callgrind, of
which ~45 are the segment search and lerp), flush ~0.95 ms when the
keyframes run first in the process and ~1.2 ms after the "3 numbers" case
(see 2). Flush experiments on one build: the record lookup loop alone
0.27 ms; plus reading each value from its animation's buffer 0.49 ms; with a
Smi store instead of the value 0.36 ms; the real store 0.88–0.93 ms (the
boxed heap number, see above, is ~0.4 ms of it, and GSAP pays it too).
Calling the writer from `flush()` or writing `r.pt.x` inline instead of
`r.render()` made no difference.

### Results

`bench.mjs`, ms per frame, three runs each:

| Case | N | `067c4a2` | GSAP | now | GSAP | now vs GSAP |
|---|---:|---:|---:|---:|---:|---:|
| 3 numbers, `outCubic` | 1k | 0.019–0.039 | 0.066–0.068 | 0.019–0.022 | 0.068–0.083 | 3.5–3.8x faster |
| 〃 | 10k | 0.374–0.387 | 0.489–0.531 | 0.363–0.377 | 0.475–0.535 | 1.3–1.5x faster |
| 〃 | 50k | 1.860–1.968 | 2.509–2.643 | 1.775–1.975 | 2.542–2.956 | 1.4–1.5x faster |
| colour string | 1k | 0.047–0.051 | 0.074–0.087 | 0.055–0.062 | 0.084–0.105 | 1.5–1.9x faster |
| 〃 | 10k | 0.678–0.700 | 0.841–0.916 | 0.717–0.885 | 0.896–1.053 | 1.0–1.4x faster |
| 〃 | 50k | 3.341–3.683 | 4.348–5.495 | 3.652–4.679 | 4.542–5.069 | 1.0–1.4x faster |
| 8 keyframes, `inOutSine` | 1k | 0.030–0.033 | 0.023 | 0.021–0.026 | 0.025–0.028 | 1.0–1.3x |
| 〃 | 10k | 0.442–0.485 | 0.245–0.282 | 0.330–0.351 | 0.230–0.329 | 0.65–1.0x |
| 〃 | 50k | 2.253–2.380 | 1.249–1.475 | 1.421–1.497 | 1.224–1.266 | 0.83–0.88x |
| `batch()` 3 channels | 50k | 0.182–0.215 | — | 0.180–0.195 | — | |

The keyframes row is ~35% faster at 50k and still behind GSAP above 1k.

Interleaved with the `067c4a2` build (three rounds, base / now, ms):
keyframes 1k 0.029–0.034 / 0.021–0.034, 10k 0.425–0.462 / 0.318–0.333, 50k
2.11–2.19 / 1.34–1.45; 3 numbers 50k 1.83–1.97 / 1.70–1.83; colour 50k
3.36–3.59 / 3.36–3.60, colour 1k 0.047–0.057 / 0.053–0.060, **colour 10k
0.688–0.701 / 0.734–0.873**. The colour 10k row is slower (still faster
than GSAP in every run, 1.07–1.41x). In 10k-only runs (three rounds, base /
after change 2 / now): 0.686–0.739 / 0.734–0.843 / 0.701–0.793, so it came
with change 2's `flush()` loop (colour Parts take `Part.render` as before;
the loop's extra `r.run` check, or the larger `flush()` being optimised
differently, are the suspects); not resolved.

Kernels, one run each, `067c4a2` vs now: `bench-wasm.mjs` "8 keyframes,
spring easing" 1.25 → 1.16 ms, cubic-bezier tween 1.96 → 1.98 ms, "x, y,
opacity + OKLab colour" 2.07 → 2.26 ms (this row moved 2.09–2.39 between
runs before). Native `bench`, two interleaved runs (very noisy on this VM):
"animate(): group of 50k, 8 kf" 0.76–0.80 → 0.71–0.77 ms, "group of 50k x 3
tracks" 0.80–0.92 → 0.84–0.93 ms. Standalone tweens (no group), which also
write the first-output array of 3 below: `bench-wasm.mjs` "tween 1 number,
outCubic" 100k, two interleaved rounds, 1.82–2.00 → 1.86–1.99 ms (no
change); natively, three interleaved rounds, 100k 1.39–1.50 → 1.41–1.89 ms
and 10k 0.127–0.146 → 0.133–0.156 ms (overlapping, at most a few % slower).

### 1. Keyframed single numbers skip the track table (core)

Like the simple from/to animations of the second round: an animation whose
only track is an enabled keyframed number with at most one segment easing
has a fixed layout (offsets at 0, values at `n`, output 0), so `Anim::eval`
handles it without reading the track struct. When the engine generated the
offsets (`k / (n - 1)`, strictly increasing), the segment and its eased
local progress depend only on (bits of the progress, keyframe count,
easing), so the frame memo keeps the last one and the targets of one
`animate()` call share it. NaN progress never uses it (the search then stops
wherever each cursor is). The cursor lives in the `Anim` while this path is
on and goes back to the track on reclassification and around `sample()`.
Test: `kf_anims_match_track_evaluation` (outputs and cursors vs `eval_into`
on private track copies, one memo shared across animations of different
keyframe counts and easings, per-animation progress before NaN steps,
`sample()` calls, track toggles; dropping the NaN guard or the count from
the memo key fails it). `ak_tick`, 50k x 8 keyframes: 1.03–1.07 → 0.74–0.75
ms (three interleaved rounds); callgrind 187 → 129 instructions per child;
"3 numbers" and colour unchanged.

### 2. A generated loop per key set in `flush()` (JS)

The writer call in `Part.render` (`w(this.pt, f, at)`) went polymorphic once
two key sets had been used in a process: the 8-keyframe flush took ~1.2 ms
after the "3 numbers" case instead of ~0.95 ms (also with differently named
keys, so it is not V8 shape sharing). Each key set now also gets a generated
loop over consecutive updated records that use it (compact path, no
`onUpdate`); `flush()` calls it once per run, and anything else ends the run
and goes through `Part.render`. Same values, same order. (This is the
"generated per-key-set loop" the second round did not attempt.) Flush after
the other cases, three interleaved rounds: keyframes 1.12–1.23 → 0.88–0.93
ms, 3 numbers 0.89–0.92 → 0.81–0.91 ms. The write-path test now has a setter
target inside a run and `onUpdate` Parts interleaved with it (dropping
either guard from the loop fails it).

### 3. First outputs in one array (core + JS)

Each animation's outputs sit at the end of its own buffer, so the flush read
one cache line per target for one number. The engine now also keeps the
first output of every animation node in one array indexed by node index
(`Engine::first_outputs`, `ak_out0_ptr`), written next to every output write,
so it always equals `values(h)[0]` (test: `first_outputs_track_values`: full
renders, the lane, seek, reverse, node reuse; dropping the lane's write
fails it). The targets of one call get consecutive node indices, so the
generated loop for a single property at offset 0 reads consecutive numbers.
It stops if that array moved (a counter at `ak_out0_epoch_ptr`, bumped when
it reallocates) or memory grew. Keyframes flush 0.90 → 0.65 ms; `bench.mjs`
interleaved with the build before it: 8 keyframes 50k 1.59–1.65 → 1.41–1.52
ms. The first version fetched the two pointers in `flush()` itself, and the
colour row went from 3.5–3.8 to 4.4–4.9 ms (interleaved, twice) with no
change in its own code path; fetching them inside the generated loop fixed
that, which points at `flush()`'s optimisation budget.

### Tried and not kept (third round)

| Idea | Result |
|---|---|
| Lane leader's time, iteration and timing copied into locals instead of re-read from its node per child | 10 fewer instructions per child natively, no measurable change in wasm (`ak_tick` 0.73–0.82 ms vs 0.74–0.96 ms before, seven interleaved rounds). |
| `r.pt.x = F64[r.at]` (or `r.pw(...)`) inline in `flush()` instead of `r.render()` | no change (0.88–1.11 vs 0.96–1.01 ms): the call isn't the cost. |
| Matching GSAP's structure (one tween per segment) | not attempted: with the segment memo the per-target core work is a compare, two loads and a lerp; what remains is per-node bookkeeping (lane checks, the updated list) and the JS store. |

## Fourth round (a): a landscape film (renderer3d)

The next film is a landscape, not the neon city: a 401×401 textured terrain
(320k triangles, 4096² albedo), a coarse far terrain, a mirror lake, thousands
of instanced trees, an emissive textured sky dome, camera motion blur with
`prevCamera`, `depth()` read-back and the grade, with two renders per frame in
transitions. `scripts/bench-terrain.mjs` reproduces it synthetically (12,000
cone trees of 48 triangles, a 4096×2048 sky, a low flight over the lake) and
prints ms per `renderer3d` pass from `r3.stats`, the depth read-back, and
whole composition frames with the draw / read / grade split.

**Method:** the baseline is `adcffe5`'s `dist/` saved before any change. All
three Node benches take `DIST=path/to/dist`, and the runs below are
interleaved (base, new, base, new). `--hash` (in `bench-terrain.mjs` and
`bench-raster.mjs`) prints FNV hashes of every frame's colour and depth; every
change in this round keeps them identical:

| Bench | Hash (base = new) |
|---|---|
| `bench-terrain.mjs --hash --frames 12` | `ce3c2950` |
| `bench-raster.mjs --hash` knot / towers / cubes / textures+shadows+additive / film-like | `bcfa8f8e/06cb0df2` · `a3adee0a/8eb73fb5` · `cccf64fd/7f3d856d` · `9fc65e43/8661ce9e` · `77424f03/fdb7e70e` |

### Results

`node scripts/bench-terrain.mjs --frames 12 --hash`, 1920×1080, wasm in Node,
two interleaved rounds (ms per frame):

| | base | new |
|---|---|---|
| one render | 638 · 608 | 443 · 493 |
| … mirror pass | 127 · 121 | 93 · 100 |
| … opaque pass | 453 · 429 | 295 · 332 |
| … motion blur | 55 · 56 | 52 · 59 |
| `depth(cam)` read-back | 6.9 · 6.8 | 6.9 · 7.3 |
| full frame (render + depth + grade) | 648 · 633 | 487 · 476 |
| transition frame (2 renders + grade) | 971 · 921 | 710 · 702 |
| grade alone (within the full frame) | 35 · 33 | 33 · 32 |

Per item (`--only <item> --frames 6`, opaque pass, single runs): sky dome
128 → 67 ms; near terrain 127 → 97 ms; far terrain 103 → 104 ms and trees
51 → 55 ms (both within noise). `bench-raster.mjs`: the textured / shadowed /
additive scene 138 → 114 ms. **Untextured shading got slightly slower in
wasm:** the film-like frame was slower in all six interleaved pairs (by
0.5–14%; 227–273 base vs 256–312 new) and the towers in five of six, about
3–5% overall. In the one pair with a per-pass split, the gap sits in the mesh
passes (opaque 83 → 101, mirror 31 → 36 ms); sky, light, DOF and motion blur
are flat. Natively, `raster_bench` moved both ways (knot 29.5 → 25.5, plain
knot 18.4 → 20.9, towers 65.9 → 66.1 ms). Not bisected yet; the suspects are
the larger queue (per-lane aniso gradients) and the two extra branches
(`unlit`, `aniso`) in the inlined shader.

### 1. Unlit fast path (core)

A material with zero diffuse, specular and rim and no reflections, lines or
pulses (the emissive textured sky dome) makes every lighting term exactly
zero, so `px4` now returns base × (ambient + emissive), tinted by or plus the
texture, then fogged, without the lighting maths. Test
`unlit_fast_path_matches_the_full_shader` renders such materials with every
texture mode, fog, ground fog, a point light, shadows and opaque / additive /
gloss blending through both paths and requires identical colour and depth.
A finite, positive shininess is part of the condition: it keeps the Schlick
quotient finite, so `0 × it` stays 0. Sky dome alone: 128 → 67 ms.

### 2. Bilinear sampler: SIMD across channels, no division to wrap (core)

`Texture::sample` blended each channel in scalar and wrapped every index with
`rem_euclid` and `%` (four integer divisions per sample). It now loads each
texel's four bytes into lanes (`V4::bytes`) and blends all channels at once,
with the same operations per channel; indices wrap with compares, as
`u − floor(u)` already puts them in −1…n−1 (`rem_euclid` remains for the
rest). `texture_sampling_matches_the_reference_bit_for_bit` checks 20,000
samples, including wrap edges, against the old sampler. Four lane samples
are transposed with shuffles. Textured scenes: −15–20% on the opaque pass.

### 3. Tiny and thin triangles (core)

A triangle whose bounds hold no pixel centre (with a conservative 1/16 px
margin, so rounding in the span solve can't disagree) now returns before
setup, rows outside the centre range are not walked, and the shading
gradients (with six divisions for the UV footprint) are set up only when a
row has a covered span. Triangle counts are unchanged. Tree pass in the
mirror (half-res) 44 → 39 ms; little change at full resolution.

`examples/trees_prof.rs` (12,000 cones) under callgrind shows where the
trees' time goes: about 1,000 instructions per front-facing triangle in
`tri`, mostly the per-row span solve (an edge-function evaluation, a division
and a floor/ceil per edge per row) on tall, thin cone sides, and about 20% in
the vertex stage. Stepping the edge functions incrementally would make it
cheap, but it rounds differently from the direct evaluation, so it can't be
bit-identical; it is the next step if a small output change is acceptable.

### 4. Anisotropic filtering (opt-in, `textureFilter: 'aniso'`)

New: per-pixel UV derivatives from the triangle's screen gradients of 1/w and
uv/w (exact under perspective, unlike the per-triangle footprint used for
line antialiasing). The mip follows the shorter footprint axis (ratio capped
at 8), and up to 4 bilinear taps sit along the longer one, at most 1.5
footprints apart (one tap up to a 1.5 ratio). Default output is unchanged.
Its cost on `bench-terrain.mjs --aniso` (both terrains): opaque 295–332 →
515 ms. On the near terrain alone it adds ~90 ms (the first version, one tap
per footprint and up to 8, added 164); on the far ring, where almost every
pixel is grazing, it adds ~100 ms (from 270). Use it on the near ground only.

### Not done or not kept (fourth round)

| Idea | Why not |
|---|---|
| Draw opaque backdrops (the sky dome) last, or sort opaque items front to back | Measured from the film side: `bench-terrain.mjs --order near` (trees, terrain, far, sky) takes the opaque pass 297 → 200 ms with **identical depth** but different colour hashes (`74919321` vs `ce3c2950`), so some pixels change (not yet located: exact depth ties or queue order). `docs/api.md` says items draw in order, so a film can list opaque items nearest first itself; doing it inside `render()` would change output. A deferred item compared with `<=` gets ties against other items right but flips ties *within* the item (shared edges). |
| `depth(cam)` in wasm | 7 ms per call, and the loop is division-bound in either place; a gain of ~5 ms per frame wasn't worth a new export. |
| Caching instance uploads / the `vals` array in `drawMesh` | Setup is ~2 ms per render, and `Instances.data` is writable in place, so "unchanged" can't be detected safely. |
| Clipping the mirror pass at the water plane | It would skip reflected geometry below the water but change output wherever that geometry shows in the reflection today (mostly culled as back faces already). |
| ffmpeg piping / readback | 1.2 ms of a ~480 ms frame. |

## Fourth round (b): the web layer end to end

Goal: everything a page uses, not only plain-object frames: creation cost,
memory kept and churned, DOM writes, timelines, motion values, scroll links,
startup and bundle size. Starting point: `adcffe5`. No Rust changed in this
round (the wasm binary is byte-identical, 567.7 KB); everything is in
`packages/anikit/src`.

**Noise.** Same VM as above, but shared throughout with another agent's 3D
renders (one or two of the four cores busy). The same build measured
anywhere from 0.6 to 1.6 ms per frame for one case in consecutive processes,
and GSAP's own numbers moved up to 2x between rounds. Every before/after
below is interleaved with a build of `adcffe5` (base, new, base, new, …) and
given as the range over the rounds. The code of `batch()` did not change and
its 50k row moved 0.17–0.23 → 0.22–0.44 ms: that is the noise floor for
single rows. Changes that did not clear the noise were dropped ("Tried and
not kept").

New and extended scripts:

| Script | What it measures |
|---|---|
| `node --expose-gc scripts/bench-create.mjs [N]` | creation + first frame (GSAP defers its set-up to the first render, so the first frame is included for both), heap retained per target, re-triggering (a new `animate()` taking over a running one), `kill()` of a long timeline, staggered/timeline/motion-value frames, import-to-ready startup; every case checks that something moved |
| `MANUAL=1 node scripts/bench-dom.mjs` | scripted DOM frames in Chromium: the time of `engine.tick` (anikit) or `gsap.updateRoot` alone, style recalc forced outside the timed region; much steadier than the rAF metrics, which move with the frame rate |
| `node scripts/bench-scroll.mjs` | scroll-linked updates in Chromium: scrubbed `scrollTrigger`s and `scroll()` links, ms per scroll event |
| `node scripts/bench.mjs` | new "unit string" case (`'0px'` → `'100px'` on objects) |

### Results

`bench-create.mjs` (N = 10k, two interleaved rounds, ms; retained = heap
per target kept by a live animation):

| Case | `adcffe5` | now | GSAP 3 (now runs) |
|---|---:|---:|---:|
| one `animate()`, 3 numbers | 59.0–65.1 | 49.8–54.1 | 3.6 |
| 10k `animate()` calls, 3 numbers | 65.6–71.6 | 43.4–50.4 | 12.7–12.8 |
| one `animate()`, stagger delay | 23.1–23.5 | 14.5–20.6 | 7.5–14.1 |
| one `animate()`, 8 keyframes | 29.4–35.2 | 23.2–26.8 | 37.2–50.1 |
| one `animate()`, colour string | 33.0–35.5 | 25.6–39.9 | 32.8–51.9 |
| timeline of 1000 steps | 6.2–7.8 | 5.2–8.4 | 1.1–2.0 |
| **`animate()` taking over a running one** | **1253–1802** | **40.6–41.2** | 353–390 |
| **`kill()`, timeline of 5000 steps** | **45.4–47.0** | **0.85–1.05** | 0.01–0.02 |
| retained, 10k `animate()` calls | 5.50 KB | 2.44 KB | 1.14 KB |
| retained, one `animate()` of 10k / 1000-step timeline | ~0 KB* / 4.79 KB | 1.49 KB / 2.61–2.63 KB | 0.47 / 1.09 KB |
| frame: 10k targets, stagger delay | 0.187–0.365 | 0.189–0.288 | 0.79–0.82 |
| frame: timeline of 1000 steps x 10 targets | 0.423–0.509 | 0.530–0.704 | 0.80–1.03 |
| frame: `motionValue` → 1000 derived + listeners | 0.083–0.110 | 0.068–0.103 | — |
| startup: import → `ready` (Node) | 17.4–19.9 | 14.9–16.0 | — |

\* the base build never frees a killed multi-target animation (§1), so the
"alive minus released" difference reads ~0.

`MANUAL=1 bench-dom.mjs` (Chromium, x/y/rotate/opacity on N boxes, ms in the
frame function, three interleaved rounds):

| N | `adcffe5` | now | GSAP 3 (all runs) |
|---:|---:|---:|---:|
| 500 | 0.58–0.60 | 0.37–0.50 | 0.63–0.95 |
| 2000 | 2.04–2.29 | 1.78–1.86 | 2.48–3.41 |
| 5000 | 5.72–8.65 | 4.31–5.34 | 5.50–6.95 |

Creation on elements (same runs, `MANUAL=1` also prints the time of the
page's one `animate()` call on the N boxes: four properties, a delay function,
current transforms read; cold, so it includes JIT warm-up), three interleaved
rounds: 500 boxes 25.6–37.0 → 22.5–24.1 ms, 2000 51.6–72.3 → 35.6–55.5 ms,
5000 96.5–98.6 → 69.5–99.4 ms.

The rAF metrics of `bench-dom.mjs` (one run each, script ms per frame):
anikit 0.68 / 2.52 / 8.18 → 0.84 / 2.36 / 6.07 at 500 / 2000 / 5000, GSAP in
the same runs 0.97–1.07 / 4.65–5.65 / 7.15–8.17; within their noise except
perhaps at 5000.

`bench.mjs` (plain objects, ms per frame, the last two interleaved rounds,
after every change):

| Case | N | `adcffe5` | now | GSAP 3 (now runs) | now vs GSAP |
|---|---:|---:|---:|---:|---:|
| 3 numbers, `outCubic` | 1k | 0.016–0.032 | 0.016–0.018 | 0.038–0.046 | 2.4–2.6x faster |
| 〃 | 10k | 0.641–0.708 | 0.661–0.712 | 0.637–0.652 | 0.89–0.99x |
| 〃 | 50k | 2.390–3.066 | 2.617–2.843 | 2.322–2.678 | 0.89–0.94x |
| colour string | 1k | 0.051 | 0.037–0.049 | 0.098–0.127 | 2.6x faster |
| 〃 | 10k | 1.218–1.261 | **0.521–0.593** | 0.698–0.801 | 1.2–1.5x faster |
| 〃 | 50k | 4.699–5.427 | **2.263–2.271** | 3.975–4.653 | 1.8–2.1x faster |
| unit string (new) | 1k | 0.083–0.090 | 0.047–0.050 | 0.033–0.036 | 0.70–0.72x |
| 〃 | 10k | 1.647–1.666 | 0.899–1.006 | 0.444–0.473 | 0.47–0.49x |
| 〃 | 50k | 6.598–8.742 | 3.817–4.754 | 1.895–2.123 | 0.45–0.50x |
| 8 keyframes, `inOutSine` | 1k | 0.018 | 0.017–0.018 | 0.020 | 1.1–1.2x faster |
| 〃 | 10k | 0.294–0.422 | 0.434–0.479 | 0.251–0.271 | 0.57–0.58x |
| 〃 | 50k | 2.181–2.185 | 1.767–2.165 | 1.146–1.216 | 0.56–0.65x |

The number and keyframe paths did not change in this round; their rows move
with the VM (in the same rounds base itself read 0.72–1.36x GSAP on 3
numbers, and the three earlier interleaved rounds gave 8 keyframes 10k
0.39–0.50 base vs 0.27–0.44 new, the other way round). A CPU-time A/B of 8
keyframes against `dc17c46` (before the commits that added fields to
`Part`; a development harness timing CPU time, minimum of 40 blocks per
process) gave, over five interleaved triples at 50k, minima of 1.19–1.32
(`dc17c46`) vs 1.20–1.26 ms (now), and at 10k 0.24–0.28 (`dc17c46`) vs
0.24–0.37 (now) vs 0.24–0.39 ms (`adcffe5`): the processes split into a fast
and a slow mode, now and `adcffe5` each landed in the slow one in some runs
and `dc17c46` in none, so a small cost of the larger `Part` at 10k can't be
excluded. The `frame: timeline` row of `bench-create.mjs`, which read slower
in the two full rounds, was faster in three further interleaved pairs
(`RUNS=1 WARM=1`): 0.656 / 0.429, 0.789 / 0.597, 0.736 / 0.566 ms (base /
now).

`bench-scroll.mjs` (ms per scroll event; base / now, one run each, the code
did not change): 200 scrubbed triggers 0.176 / 0.146, 200 `scroll()` links
0.292 / 0.290. Below a microsecond per link per event: nothing to fix.

Bundle (`node scripts/build.mjs`): `anikit.min.js` 953.5 → 956.3 KB (gzip
367.6 → 368.7 KB), `anikit.lean.js` 196.6 → 199.4 KB (gzip 73.5 → 74.5 KB),
`anikit.wasm` unchanged. Native `cargo run --release -p anikit-core
--example bench`: no Rust changes, not re-measured.

### 1. Killed multi-target animations were never collected (memory leak)

`Animation._kill()` unregistered its group's node record with
`unregister({ h: this._group, at: 0 })`, a fresh object, but `unregister`
matches the record itself, so the group record (whose `ctl` is the
animation) stayed in the record table until its slot was reused, keeping
the killed animation, its Parts, bindings and restore closures alive. It
now unregisters the record it registered. Separately, `Part.onEmpty` was a
closure created inside the per-target build loop; V8 gives that loop body
one context per iteration holding every variable any closure in it
captures, and `addTrack` captured the target's track list, so each Part kept
its track definitions (value arrays, formatters, first/last values) alive.
It is now built by a module-level helper. Test: `kill() unregisters every
record` (fails on the old code).

### 2. Creation: plain-number tracks, per-call hoisting, shared arrays

Creating an animation spent ~45% of its time in GC (development CPU profile
of 10k targets x 3 numbers) and allocated ~7.6 KB per target (development
allocation profile with collected objects included; ~4.3 KB after these
changes). All changes produce the same tracks:

- `numberTrack()`: keyframes that are numbers or `null` (the current value:
  a number, or a plain number string such as `'0px'` from a transform read)
  build the `TrackDef` without the parser's per-value objects and the
  per-property `read`/`convert` closures. Anything else (relative values,
  colours, strings, functions, `random()`) takes `buildTrack`, which also
  runs whenever the fast path declines. `_setNumberTracks(false)` forces the
  general path; the test runs objects (numbers, `null` keyframes, unit
  strings, NaN/-0/undefined current values), elements (existing matrices,
  scale/skew, opacity, px/em styles, `repeatDelay`), relative/function/
  random values and a chained timeline through both and compares every
  frame and the reverted state; dropping or changing the current value's
  unit fails it.
- One formatter per unit for number tracks instead of one closure per track;
  one `easings` array per easing id (tracks only read them; the one append,
  for `repeatDelay`, only touches lists of more than one).
- Per call instead of per target: the property list (no `Map` iterator
  entries per target), `postFor()` (which built an `Intl.NumberFormat` per
  target when `format` was set) and whether values need resolving at all.
- `Part.prep()` reuses the last key/offset entry when the bindings match
  instead of building a signature string per Part.
- The completion promise (`finished`/`then`) is created on first use: most
  animations are never awaited. Same object for the rest of a run, already
  resolved when asked after the end, re-armed by a replay (the test passes
  on the old code too).

### 3. Two O(n²) paths

- **Taking over a running animation.** When another animation takes a
  property over, each emptied Part asked its animation whether all parts
  were empty by scanning `_parts` from the start, past every part already
  emptied, so a new `animate()` over the same n targets and properties (any
  re-triggered staggered list) cost O(n²): 10k targets 1.25–1.80 s. The
  animation now counts the parts that still write something (bindings only
  ever turn off): ~41 ms, 9x faster than GSAP's `overwrite: 'auto'`. Test:
  partial takeovers keep it running, the last one frees it, the same target
  twice in one call (passes on the old code too).
- **Killing a timeline.** `kill()` killed each child before releasing the
  root; each child's `ak_release` detached it from the group (`Vec::retain`)
  and recomputed the group's auto duration over the remaining children. The
  root is now released first (one recursive pass frees the subtree); the
  children's own kills then hit stale handles, which are harmless, and
  still dispose their records and claims. Test: nodes and records all freed
  (children, a nested timeline, an adopted animation, a `call()`), later
  `kill()`/`seek()` on the children harmless (passes on the old order too).

### 4. DOM writes

A CPU profile of the DOM bench at 5000 boxes put Blink's `style.transform`
setter (CSS parsing) first, which is not ours to remove; next came building
the transform strings (double-to-string conversions), the `opacity` writes
and the `el.style` getter, called twice per element per frame. Small
Chromium experiments (not kept as scripts) chose between the alternatives;
the effect of the whole change is the `MANUAL=1 bench-dom.mjs` table above.

- `s3(v)`: `String(r3(v))` built from the integer `Math.round(v * 1000)`
  and a table of the 1000 fraction strings, instead of V8's general
  double-to-string conversion. Identical output: below 2^31 thousandths the
  shortest representation of `k / 1000` is exactly those digits; anything
  else (large, NaN, infinite) takes the general conversion. Test: edge
  cases and 200k values over 12 magnitudes against `String(r3(v))`. Used for
  transforms and every per-frame number-with-unit string (number, complex
  and colour-alpha formats, styles with units, springs, `quickTo`, morph
  paths, FLIP radii, stroke drawing); it is most of the "unit string" gain
  above.
- Unitless numeric styles (`opacity`, `z-index`, …) are written as numbers:
  the setter makes the same string, at about half the cost in Chromium.
- `TargetState` keeps the element's style declaration (`el.style` returns
  the same object for the element's lifetime): no getter call per element
  per frame.
- `composeTransform` joins parts as it appends them instead of slicing off
  a trailing space (which flattened and copied the string once more).

### 5. Motion values, reduced motion, startup

- `MotionValue.write()` replaced its velocity record with a new object on
  every change, and `derived()` mapped its sources into an array and spread
  it on every update. The record is updated in place; one- and two-source
  values (every `map()`) call `fn` directly; three or more keep the general
  path (test added). The `bench-create.mjs` row moved 0.083–0.110 →
  0.068–0.103 ms (overlapping).
- With `engine.reducedMotion = 'user'`, `prefersReduced()` called
  `matchMedia(...)` on every check (each `animate()` and every frame of every
  marquee), making a `MediaQueryList` and parsing the query each time. The
  list (its `matches` is live) is kept and re-made only if `matchMedia` is
  replaced. Test: follows a live stub, one query object.
- The embedded module was decoded with `atob()` plus a copy loop, cold code
  at startup (in Node `atob` is JavaScript). `Uint8Array.fromBase64`
  (current Chromium, Firefox, Safari) or Node's own `Buffer` decode it
  natively; the loop stays as the fallback (a bundler's `Buffer` polyfill is
  not used). Node import → `ready` 17.4–19.9 → 14.9–16.0 ms. In Chromium the
  `MANUAL=1 bench-dom.mjs` page's navigation → `ready` time (50–90 ms, bundle
  parse included) did not move measurably. Test: every decoder gives the
  same bytes; the native one was also checked against `Buffer` on the
  module under `node --js-base-64`, and the recipes run on it in Chromium.

### 6. One formatted plain property per Part skips the binding loop

Profiling the colour and unit-string rows showed about half of the frame at
50k in the dependent loads Part → bindings array → Binding → target (the
number path avoids them through the compact writers of the second round).
A Part with exactly one formatted plain-property binding (colours, `'12px'`,
complex strings on objects) now keeps the key, output offset, formatter and
last written value itself; the last value goes back to the binding on
`release`. Same writes, same order, same skipping of unchanged values
(`_setCompactFmt(false)` forces the binding loop; the test runs colour, px,
complex and setter targets, `onUpdate`, a mid-run takeover, a seek, a
`steps()` easing that repeats values, and revert, through both; always
writing, or writing twice, fails it; a development in-process A/B toggling
the path gave medians 4.08 → 2.77 ms for colours and 4.38 → 3.36 ms for
unit strings at 50k). This is the colour gain in the
`bench.mjs` table: 10k 1.22–1.26 → 0.52–0.59 ms, 50k 4.70–5.43 →
2.26–2.27 ms, from 0.86–1.00x GSAP to 1.2–2.1x faster.

### Weak spots from the third round, re-checked

- **8 keyframes above 1k:** still behind GSAP (0.56–0.65x at 50k, ~0.58x at
  10k in the final rounds; 0.6–0.9x over all five). Not worked on: that path
  (core tick + generated runner + V8's boxed stores, see the third round) is
  unchanged, and a frame profile at 10k split it about evenly between
  `ak_tick` and the runner's stores.
- **Colour strings at 10k:** resolved, 2.1–2.3x faster than before (§6).
- **Staggered tweens (~4% from the third round's lane):** not verified and
  not addressed (the core lane is unchanged). For reference, `frame: 10k
  targets, stagger delay` read 0.187–0.365 → 0.189–0.288 ms, 2.7–4.4x faster
  than GSAP.
- **New: unit strings on objects** (not benchmarked before): 1.7–2.2x
  faster than before, still 0.45–0.72x GSAP. What remains per target is the
  formatter call and the string allocation (`s3` + unit).

### Tried and not kept (fourth round)

| Idea | Result |
|---|---|
| A generated store per key for formatted plain properties (`t["c"] = v` instead of `t[b.key] = v`) | no change in a development in-process A/B (median 0.95 vs 0.93 ms, 10k colours): with one key per process the keyed store site is already monomorphic. |
| `claim`/`unclaim` with one state lookup per target, and `set(prop, undefined)` instead of `Map.delete` | 0–5% slower creation in three interleaved rounds of a development creation loop. |
| Pinning benchmark processes to one CPU (`taskset`) | did not remove the 2x process-to-process modes under the shared load. |
| Typed OM transforms (`setTypedOM(true)`) | slower than the string path now: `MANUAL=1 LIBS=anikit,anikit-typed,gsap bench-dom.mjs`, one run, 0.41 / 1.85 / 4.31 (strings) vs 0.77 / 2.87 / 9.66 ms (Typed OM) at 500 / 2000 / 5000. Still off by default. |

## Fifth round (video)

Goal: the whole path from a `renderer3d` frame to an encoded video file, on
the real films (`film/film.mjs`) and the synthetic benches. The machine is
4 cores shared with other agents (load 7–9 throughout), so every number below
comes from interleaved base/new runs. Single runs moved by up to ±15% on
their own (`bench-post` swung 93 → 80 ms with no post change), so only the
large differences count.

**Baseline:** `86b471c`'s `dist/`, `node/` and `bin/`, saved before any
change. `film/film.mjs` at HEAD was copied as `film-base.mjs` for the base
CLI.

**Output identity** (base = new, every change in this round):

| Check | Hash |
|---|---|
| `bench-terrain.mjs --frames 12 --hash`, also with `--sort near` | `ce3c2950` |
| `bench-raster.mjs --hash` (5 scenes) | `bcfa8f8e/06cb0df2` · `a3adee0a/8eb73fb5` · `cccf64fd/7f3d856d` · `9fc65e43/8661ce9e` · `77424f03/fdb7e70e` |
| `film/film.mjs`, frames 0, 240, …, 1200, RGBA after the grade (base film vs the film with `sort: 'near'` and `pixels()`) | `791476f7 41b78629 04626768 d9b92636 7dfdebe2 db144d17` |
| `film.mjs` frames 960–965: `--preroll 0` vs the default 0.5 | `7dfdebe2 c0d32803 86c7991b ae7e6d89 ea02b3c1 9d2b3772` (same) |
| CLI 16–16.5 s, one worker: base vs `--preroll 0` | byte-identical mp4 |

### Results

`bench-terrain.mjs --frames 12 --hash` (ms per frame, interleaved pairs;
"new" is the first commit without `sort`, then with it; "final" adds the
mirror change):

| | base | new | new, `sort: 'near'` | final, `sort: 'near'` |
|---|---|---|---|---|
| one render | 479 · 512 | 449 · 451 | 355 · 335 | 323 · 323 |
| … opaque pass | 316 · 341 | 297 · 303 | 199 · 188 | 188 · 192 |
| … mirror pass | 100 · 108 | 95 · 93 | 97 · 87 | 75 · 75 |
| … motion blur | 60 · 59 | 54 · 53 | 57 · 57 | 57 · 53 |
| `depth(cam)` | 8.0 · 8.1 | 7.4 · 7.2 | 7.7 · 7.7 | 5.5 · 5.4 |

The final column comes from its own two pairs against the second commit's
dist (mirror first), both with `--sort near`: one render 365 · 379 → 323 ·
323, mirror 93 · 99 → 75 · 75, opaque 209 · 217 → 188 · 192. `depth(cam)`
also dropped from 7.6 · 8.1 to 5.5 · 5.4 ms in those pairs, although its code
didn't change. That drop is unexplained (perhaps the depth buffer is still in
cache after the reflection's mask pass reads it). The
first commit's message gave mirror 140 → 110 from single runs; the pairs show that part as
noise.

`bench-raster.mjs`: flat (film-like frame 266 · 282 base vs 270 · 278 new).
The 3–5% untextured regression from the fourth round doesn't show above this
noise; it is still unbisected.

`film/film.mjs` through `anikit-render --profile --every 240` (frames at
0, 4, …, 20 s; the base film against the film with `sort: 'near'` and
`pixels()`). Means per frame in ms, in two interleaved rounds. "c2" is the
second commit (sort + pixels + pipeline); "final" adds the mirror change:

| | base | c2 | final |
|---|---|---|---|
| frame total (step + draw + read + post) | 1013 · 1059 | 872 · 921 | 929 · 874 |
| … draw (all 3D passes + the film's JS) | 950 · 994 | 825 · 870 | 871 · 824 |
| … 3d: mirror + opaque | 687 · 691 | 581 · 602 | 630 · 603 |
| … read (canvas → frame buffer) | 7.2 · 6.8 | 1.4 · 1.5 | 1.5 · 1.5 |
| … post (grade) | 56 · 59 | 46 · 50 | 57 · 48 |

So about −13% per film frame, from `sort: 'near'` and the canvas round
trip. The mirror change helps `bench-terrain` (mirror 93–99 → 75 ms) and is
flat on the film: mirror 234 · 237 → 244 · 224 ms, opaque 347 · 365 → 386 ·
379 ms, within this machine's noise. The film's lake fills a small part of
the frame, and its reflection was already cheap. (The `--profile` columns
follow the pass order, so after this change `3d:opaque` comes before
`3d:mirror`.)

End to end (`anikit-render`, one worker, `film.mjs` with the base CLI and
dist against the new CLI with `--preroll 0`; wall time including world setup
and x264 medium):

| Slice | base | new | Output |
|---|---|---|---|
| 16–16.5 s (30 frames, plain flight; before the mirror commit) | 68.3 s | 26.9 s | byte-identical mp4 |
| 2–2.5 s (30 frames, over the lake) | 52.7 s | 34.0 s | byte-identical mp4 |

On a full three-worker render, the preroll is 30 of ~430 frames per worker.
A 2-second `--chunk` with `--preroll 0` costs one world setup (~2.5 s) per
120 frames and evens out the 0.45–3 s frame costs across workers. The tease
at the start no longer holds up the first worker while the others finish.
(Not measured on a full render: the main checkout was rendering, and the
machine is shared.)

### 1. Opaque items in any order, list-order pixels (`sort: 'near'`)

Before this change, drawing the sky dome last changed the picture. The fourth
round measured it (`--order near`, opaque 297 → 200 ms, colour hash
`74919321`). The cause is exact depth ties between items. With `<`, the item
drawn first wins a tie, so reordering the list changes which item wins.

Now an opaque draw can carry its list index (flag bit 4, index in bits 8…).
In that mode (`SHADE_OWN`, its own const `MODE`, so the other modes compile
as before), the core keeps a per-pixel owner. A fragment wins with `z < d`,
or with `z == d` when the pixel's owner is later in the list. Ties inside an
item keep first-drawn-wins. The final pixel is therefore the minimum depth,
then the lowest list index, then the item's own first triangle, whatever the
draw order.

`render(…, { sort: 'near' })` uses this in the main and mirror passes. It
orders opaque items by the far side of their bounds (the instance spheres
from `ak_mesh_bounds`, cached per mesh), so a dome around the camera goes
last. Test `any_draw_order_with_list_indices_matches_list_order` draws five
items with coincident surfaces in all 120 orders and requires the list-order
colour and depth. A plain reversed order does differ from list order, so the
test scene really has ties.

The film already lists its items nearest first (terrain, rocks, trees,
towers, far, dome), so it gains less than the bench.

### 2. Half-res reflections after the scene, shading only what shows

With `mirror`, the half-res reflection used to render first, and the opaque
scene was then drawn over almost all of it. Now the opaque pass runs first.
`reflection_after` marks half-res pixels whose 4×4 upsampling footprint is
fully covered with depth −∞, so every test fails and they are never shaded.
`reflection_end` then writes the upsample only into pixels the scene left
uncovered, and keeps the depth. The upsample of a needed pixel reads the same
half-res values, and per-lane shading doesn't depend on queue grouping, so
the picture is the same. Test
`reflection_after_the_scene_matches_reflection_first` checks colour and depth
at 80×60, 81×63 and 33×18 against the old order. `resolution: 'full'` keeps
the old order.

### 3. The pipeline: preroll, jobs, the canvas round trip

- **Preroll.** Every worker drew 0.5 s (30 frames) unseen before its slice,
  so trails match. That cost the same as 30 output frames per job, ~7% of a
  three-worker render and far more for short slices. `--preroll <s>` sets it.
  For films whose `draw()` keeps no state (film.mjs: camera on a path, motion
  blur from `prevCamera`), `--preroll 0` gives the same pixels.
- **Balance.** Frame cost varies 5× along film.mjs (0.45 s plain frames,
  2.3–3.2 s in the three-look tease). One equal slice per worker leaves the
  others idle while one finishes the expensive stretch. `--chunk <s>` splits
  the range into jobs that free workers take in turn. Each job pays world
  setup (~2.5 s with the bake cache), and its preroll unless `--preroll 0`.
  The default is unchanged.
- **Canvas round trip.** A full-frame render went `putImageData` (13.8 ms at
  1080p in Skia) → `canvas.data()` (5.4 ms) → the frame buffer. The new
  `FrameInfo.pixels(px)` hands the frame to the grade directly. Transitions,
  impacts, BGRA read-back and `keepCanvas` (used by `--still` with `raw`)
  still go through the canvas. In the film profile, `read` dropped from
  7.2 to 1.7 ms, and `draw` lost the `putImageData`.

### Measured and left alone (fifth round)

| Item | Finding |
|---|---|
| `depth(cam)` linearisation in WebAssembly | Built and measured, not kept. Same f64 formula, bit-identical, but 5.18 → 4.52 and 10.86 → 10.09 ms (two interleaved pairs): division-bound in either place, plus an 8 MB copy to keep the JS-owned result. Not worth an export. |
| x264 preset (encoding) | 30 film frames at 1080p, `-crf 18`: medium 3.4 s user + 0.8 s sys (~140 ms CPU per frame), faster 2.6 + 0.2, veryfast 1.4 + 0.1. `-threads 1` cuts the sys time (0.8 → 0.1 s) but changes the bitstream (827,077 vs 827,361 bytes). ffmpeg runs beside each worker with 3 frames queued, so it overlaps rendering. The default worker count (cores − 1) leaves one core for the encoders. Kept medium; `--preset` already exists. |
| Raw RGBA vs I420 into ffmpeg | Already I420 from WebAssembly (1.5 bytes per pixel) with a 3-frame queue. `Buffer.from` is one 3 MB copy per frame. |
| Instance-buffer caching | Not done: setup is 2.6–5 ms per render, and `Instances.data` is writable in place. |
| Thin cone triangles | Not done: incremental edge stepping rounds differently, so it isn't bit-identical. |
| The film grade | `bench-post.mjs`: full grade 93 (base) vs 80 ms (new, unchanged code) in one pair, so ±15% noise. aa 17–20, bloom 14–15, rays 21–23, flares 19–21 ms. Not changed this round. |
| The film's own JS | `draw` minus the 3D passes is ~190 ms per frame on average in film.mjs, 270–290 ms in the comic look and ~500 ms in the tease. That is film code (the per-pixel `comic()` ink pass and the look `composite()` in JS), not the engine; it is the next largest cost after the 3D passes. |

## Fifth round (web features)

Goal: the web features that had never been benchmarked, against the obvious
competitor where one is installed. Starting point: `86b471c`. The only Rust
change is the GIF encoder (§5; output byte-identical; `anikit.wasm` 587.1 →
595.0 KB, gzip 218.1 → 220.5 KB, for the sorted search and its sort); the
core tween path (`animate`, timelines,
`props.ts`) was left alone, another round works on it.

**Method.** Same shared VM as the fourth round (four cores, other agents'
builds and renders running). Every before/after is interleaved with a copy of
the `86b471c` build (`DIST=` for the Node and dist-based scripts, `PKG=` for
`bench-scroll.mjs`, which bundles the source): base, new, base, new, base,
new, and given as the range over the three rounds. GSAP's own column in the
same runs is the noise gauge: it moved up to 1.8x between rounds.

New scripts (in `packages/anikit/scripts/`):

| Script | What it measures |
|---|---|
| `node --expose-gc scripts/bench-spring.mjs [N...]` | `spring()` creation (3 props), the frame of N running springs, a pointer follower (all N springs retargeted every frame, then a tick) with the heap kept per retarget, `inertia()` with bounds and snapping |
| `node scripts/bench-text.mjs` | Chromium: `splitText()` of P paragraphs into lines, words and chars, the re-split `autoSplit` does on a width change, and `flip()` of N boxes between two layouts; against GSAP's SplitText and Flip |
| `node scripts/bench-trigger.mjs` (`PIN=1`) | Chromium: creating N scrubbed `scrollTrigger`s and `refreshScrollTriggers()`, against GSAP's ScrollTrigger |
| `node scripts/bench-marquee.mjs` | Chromium: M `marquee()` rows down a long page (about 3 on screen), set-up and ms per frame including the style and layout work it causes |
| `node scripts/bench-gif.mjs` | `encodeGif()` (what `record()` runs on its frames): flat UI frames and a full-frame moving gradient, ms per frame and file size |
| `node scripts/bench-magnetic.mjs` | Chromium: M `magnetic()` buttons with the pointer sweeping across them, ms per move + frame and JS heap kept per move (after a GC, through the DevTools protocol) |
| `node scripts/bench-drag.mjs` (`FORCE=0`) | Chromium: `draggable()` against GSAP's Draggable, µs per synthetic pointer move and ms for the release (throw set-up) |
| `node scripts/bench-world-draw.mjs` | Chromium: `world().draw()` on a 2D canvas, N bodies, ms per frame including the raster |
| `cargo run --release -p anikit-core --example gif_prof` | native GIF encoder on gradient frames (a callgrind target) |
| `bench-scroll.mjs` | now takes `PKG=` (a copy of `packages/anikit`) to measure another version of the source, and has smoothed modes (`trigger-smooth`, `scroll-smooth`) |

### Results

`bench-spring.mjs` (Node, plain objects, ms; median of 3 runs per process,
three interleaved rounds):

| Case | N | `86b471c` | now |
|---|---:|---:|---:|
| **retarget all + frame** (pointer follower) | 1k | 2.45–4.67 | **0.61–0.68** |
| 〃 | 10k | 35.2–61.1 | **7.89–9.61** |
| **heap kept per retarget call while moving** | | **1090 bytes** (10k springs x 200 frames ran out of a 2 GB heap) | **0 bytes** |
| create (3 props) + first frame | 1k | 4.8–10.0 | 7.8–11.1 |
| 〃 | 10k | 62.7–120.2 | 53.3–64.3 |
| frame, N running | 1k | 0.18–0.20 | 0.19–0.25 |
| 〃 | 10k | 1.27–1.51 | 1.28–1.49 |
| `inertia()` create + 10 frames | 1k | 4.1–11.5 | 3.6–12.0 |
| 〃 | 10k | 61.2–98.0 | 48.8–98.3 |

Only the retarget rows changed beyond the noise.

The same fix on a DOM feature, `bench-magnetic.mjs` (Chromium, 600 pointer
moves each followed by a frame, median of 3 pages, three interleaved
rounds):

| M buttons | ms/move+frame `86b471c` | now | heap kept per move `86b471c` | now |
|---:|---:|---:|---:|---:|
| 20 | 0.094–0.098 | 0.066–0.085 | 7–8 B | 23–26 B |
| 200 | 0.694–1.003 | **0.547–0.674** | **811–818 B** | **26–29 B** |

The "now" heap column is a fixed ~15 KB over the run, not a per-move cost:
with 6000 moves instead of 600 (one run of 3 pages) it reads 3 B (20
buttons) and 4 B (200) per move, against 13 B and 36 B for `86b471c`, whose
retained closures are freed only when a button's spring comes to rest. GSAP has no spring, and its
InertiaPlugin does not register under Node (it reported "Missing plugin"),
so there is no competitor column here.

`bench-marquee.mjs` (Chromium, median of 3 pages, three interleaved rounds
with the final code and script; set-up = `marquee()` on every row plus a
forced layout):

| M rows | ms/frame `86b471c` | ms/frame now | set-up `86b471c` | set-up now |
|---:|---:|---:|---:|---:|
| 20 | 0.051–0.073 | **0.020–0.023** | 10.0–13.6 | 7.4–13.2 |
| 100 | 0.232–0.500 | **0.030–0.034** | 36.9–56.3 | 37.8–64.6 |

Set-up is within the noise; the saved re-clone happens in the resize
observer's first callback, after the timed region. (A first version compared
the container and track sizes after adding the clones, a second forced
layout per marquee that read 41.0–44.5 vs 34.9–41.1 ms at 100 rows; the
final one reads the container's box with the other sizes, before cloning.)

`bench-scroll.mjs` (Chromium, ms per scroll event in the scroll listeners,
three interleaved rounds, `SIZES=50,200`):

| Mode | N | `86b471c` | now |
|---|---:|---:|---:|
| scrubbed `scrollTrigger` | 50 | 0.071–0.102 | 0.075–0.088 |
| 〃 | 200 | 0.176–0.188 | **0.119–0.159** |

(The `scroll()` rows of the same rounds measured an experiment that was
reverted; see "Tried and not kept".)

Smoothed links retarget a spring per link on every scroll event, so they get
§1. New modes of `bench-scroll.mjs` (`MODES=trigger-smooth,scroll-smooth`:
`scrub: 0.5` and `scroll({ smooth: 0.3 })`, one engine frame per scroll step
outside the timed region), three interleaved rounds:

| Mode | N | `86b471c` | now |
|---|---:|---:|---:|
| `scrollTrigger`, `scrub: 0.5` | 50 | 0.094–0.158 | 0.069–0.102 |
| 〃 | 200 | 0.277–0.354 | **0.191–0.256** |
| `scroll({ smooth: 0.3 })` | 50 | 0.166–0.193 | 0.076–0.206 |
| 〃 | 200 | 0.485–0.545 | 0.348–0.498 |

Next target there: a smoothed link outside its range still calls `spring()`
on every scroll event with the same clamped target.

`bench-text.mjs` (Chromium, ms, median of 3 pages, three interleaved rounds
on a quieter machine; an earlier set of rounds ran next to other benchmarks
and is not used):

| Case | Size | `86b471c` | now | GSAP 3.15 (all rounds) |
|---|---:|---:|---:|---:|
| `splitText()` lines, words, chars | 20 paragraphs | 89.7–127.8 | 84.1–140.3 | 123.4–149.9 |
| 〃 | 100 paragraphs | 768–1076 | 477–654 | 763–1078 |
| re-split (`autoSplit` on a width change) | 20 | 122–176 | 104–131 | 155–236 |
| 〃 | 100 | 836–1336 | 588–969 | 741–1828 |

`bench-gif.mjs` (Node, 480x270, ms per frame, median of 3 runs, three
interleaved rounds; output sizes identical, files byte-identical):

| Scene | `86b471c` | now |
|---|---:|---:|
| flat UI, a moving card (69 KB for 60 frames) | 0.50–0.58 | 0.41–0.87 (noise: 0.42–0.52 vs 0.41–0.54 in three more rounds of 120 frames) |
| **moving gradient with a glow** (1.3 MB) | **33.4–38.7** | **11.2–11.9** |

Measured, code unchanged (one run each, median of 3 pages, against GSAP in
the same runs):

| Case | Size | Anikit | GSAP 3.15 | GSAP / Anikit |
|---|---:|---:|---:|---:|
| `scrollTrigger` create (scrubbed) | 100 | 13.1 | 30.2 | 2.3x |
| 〃 | 500 | 35.7 | 173.1 | 4.9x |
| `refreshScrollTriggers()` | 100 | 1.7 | 7.6 | 4.5x |
| 〃 | 500 | 11.0 | 59.9 | 5.4x |
| create, every 10th pinned (`PIN=1`) | 100 / 500 | 17.9 / 112.2 | 45.7 / 312.6 | 2.6x / 2.8x |
| refresh, every 10th pinned | 100 / 500 | 5.0 / 37.4 | 14.8 / 194.2 | 3.0x / 5.2x |
| `flip()` grid → list, start | 200 boxes | 14.8–16.5 | 136.7–143.4 | 8–10x |
| 〃 | 1000 boxes | 52.1–84.7 | 3058–4282 | 37–59x |

(`flip()` from `bench-text.mjs`, the time from capture to the started
animation plus a forced layout. Anikit's capture is one batch of reads and
its start writes transforms only; GSAP's side was not profiled.)

`bench-drag.mjs` (code unchanged, one run, median of 3 pages, 500 synthetic
moves): with a forced style + layout after every event, Anikit 10.6–11.2 µs
per move vs GSAP 5.4–6.2 µs, because Anikit writes the transform inside the
event (so `onDrag` sees the element where it is) and GSAP on its next tick;
without the forced layout (`FORCE=0`, handler cost only) 5.2 vs 5.6 µs. Real
pointer moves arrive once per frame in Chromium, where both pay one style
recalc per frame. The release with `inertia` (velocity, bounds, throw start)
takes 0.20–0.30 ms vs GSAP's 0.60–0.90 ms.

### 1. Retargeted springs kept everything alive (memory leak)

`spring()` is meant to be called on every pointer event (the docs' own
pointer follower, `magnetic()`, `scrollTrigger` with a numeric `scrub`,
`scroll({ smooth })`, `smoothScroll`). Each call made a `PhysicsAnimation`
with a promise, pushed a completion closure onto the running simulation,
took a snapshot and built a `Binding` it threw away. The closures lived until
the spring came to rest, so a follower that never rests grew the heap by
~1 KB per call. Now the promise and the completion hooks are made only when
someone awaits the controller (`then`/`finished`): each simulation counts
the times it came to rest or was disposed, and a controller compares that
with the count when it took the simulation over, so an await before, during
or after the motion still resolves at the same moment. Retargeting to a
plain number also skips reading the current value (a computed-style read on
CSS properties) and building the binding: the target is the number itself in
the property's default unit, exactly what the full path computed. Covered by
a new test (awaits before and after, a rested spring, 100 unawaited
retargets).

Not changed, but worth knowing: a `spring()` call whose options carry
`onUpdate`/`onComplete` adds those options to the simulation's listener list
unless it is the same options object, so a follower that passes a new
`{ onUpdate }` literal on every event calls more and more listeners per frame
until the spring rests. Reuse one options object (as `scrollTrigger` and
`scroll()` do).

### 2. Marquees off screen

`marquee()` wrote its track's transform every frame wherever it was. It now
keeps advancing its offset (so it is where it would have been) but skips the
write while an `IntersectionObserver` says it is off screen, with a
`rootMargin` of 50% so it is written again before it scrolls into view (the
callback arrives a frame late); it also skips the write when the offset did
not change (paused, reduced motion). The resize observer's first report,
which always arrives right after `observe()`, re-cloned all the content for
nothing: a report re-clones only when the container's box differs from the
one the last measure saw (`measure()` called directly still always
re-clones). `kill()` off screen writes the current offset once, so the track
is left where the marquee had got to.

### 3. Scrubbed triggers outside their range

A scrubbed `scrollTrigger` above or below the viewport reads progress 0 or 1
on every scroll event and set it again (`ak_set_progress` and a `flush()`).
It now seeks only when the progress changes, and always on the first update
and after a refresh.

### 4. `splitText()`: one layout read for all elements

With `type` including `'lines'`, each element was split (writes) and then
measured (`offsetTop` of every word) before the next one was split, so the
page was laid out again once per element. Now all elements are split first,
then every word of every element is measured in one pass, then the line and
mask wrappers are added. Pieces are cloned from a prepared span (class,
`display: inline-block`, `aria-hidden`) instead of being built attribute by
attribute, the grapheme segmenter is created once, and printable ASCII words
skip it. The output is the same markup. A Chromium CPU profile of the split
puts the layout (the `offsetTop` read and the forced layout after) at about
70%, the JS at about 15%: the split is layout-bound, so the gain is small.

### 5. GIF encoding: nearest colour without trying every entry

`record()` hands its frames to the core's GIF encoder. On a gradient (every
pixel changes, tens of thousands of distinct colours) a callgrind profile of
the new `gif_prof` example put most of a frame in `nearest()`: every
distinct colour tried all 256 palette entries. The palette is now kept
sorted by green, the heaviest term of the distance (`2·dr² + 4·dg² + 3·db²`),
and the search walks outwards from the pixel's green until `4·dg²` alone
exceeds the best distance so far, starting with the previous pixel's answer
as the bound (neighbouring pixels have similar colours, so the walk is
short). Entries with an equal distance are still visited and the smaller
index wins, as in the linear scan: the output is byte-identical (SHA-256 of
`encodeGif()` on flat UI, gradient and noise frames, before and after) and a
new test checks it against the linear search on random and tie-heavy
palettes. Median cut also rescanned every box's pixels to find its widest
channel at each of its 255 splits; that is now computed once per box (a
smaller gain: 13.6 → 13.1 ms per gradient frame on its own).

### Tried and not kept (fifth round)

| Idea | Result |
|---|---|
| `scroll()` links skip setting the same progress again (as scrubbed triggers now do) | no clear gain: 200 links 0.298–0.336 → 0.220–0.347 ms per scroll event over three interleaved rounds; and `scroll(fn)` would no longer call the function on every event once it is clamped. Reverted. |
| `world().draw()`: all circles in one path and one `fill()` for an opaque colour | slower at 5000 bodies (8.5–10.1 → 14.4–16.0 ms per frame including the raster): Skia rasterises one 5000-subpath path more slowly than 5000 ovals. Reverted. |
| `world().draw()`: set the default colour once instead of per body | within the noise (5000 bodies 8.5–10.5 vs 8.0–10.0 ms): the frame is raster-bound. Reverted. |
| GIF colour cache of 2¹⁸ entries instead of 2¹⁶ | gradient 13.1 → 11.4 ms per frame but flat UI frames slower (clearing the larger table every frame). Not kept. |
| Further `splitText()` JS work | a Chromium CPU profile puts ~70% in layout (the one `offsetTop` read and the forced layout after) and ~15% in the split itself (`cloneNode`, `appendChild`, walking). |

### Not measured in this round

`gesture()` (read: one `getBoundingClientRect` per press, one transform
write per move, no reads in the move path; ctrl-wheel pinch reads the box per
event), `observer()`, `smoothScroll()` (retargets a spring per wheel event,
so it gets §1), `parallax()` (a `scroll()` link
per element), `presence`/`exit`, `bindValues`, the `effects` presets (thin
wrappers over `animate()`, the other round's path).

`world()`, `pointCloud` + `glow`, `scene3d` and motion values were not
changed; for the record, one run each of their existing scripts on the final
build: `bench-physics.mjs` (`world()` vs matter.js, ms per step)
250 bodies 0.060 vs 4.82 (81x), 1000 0.361 vs 9.49 (26x), 2000 0.445 vs
12.6 (28x), 5000 1.08 vs 14.1 (13x); `bench-3d.mjs` (projection vs plain JS)
10k points 0.103 vs 2.06 ms (20x), 100k 1.23 vs 29.5 ms (24x), 500k 6.99 vs
200 ms (29x); `bench-cloud.mjs` (Chromium, step + render at 1080×675, ms
per frame) 10k particles 2.53, 50k 4.30, 150k 12.36, 300k 17.49 (100k sits
between 50k and 150k, roughly half a 60 fps frame); `bench-create.mjs`
`frame: motionValue -> 1000 derived + listeners` 0.098 ms (fourth round:
0.068–0.103).


## Fifth round: one evaluation per `animate()` call

Goal: the "8 keyframes" row of `scripts/bench.mjs` ahead of GSAP at 10k and
50k objects; it was 0.56–0.65x. Starting point: `165a22b` (base below:
`origin/claude/epic-ramanujan-qe7x4w` at `86b471c`, the same core plus the
unit formatter `32e3718`).

Where the time went (50k targets, `ak_tick` and the rest of `engine.tick`
timed separately, Node): tick ~0.87 ms, flush ~0.88 ms, against ~0.6–1.1
ms for GSAP's whole frame. Per target the cost doubled from 1k to 10k
(9 → 24 ns in the tick): memory traffic, not arithmetic. The tick read
each child's node (two to three cache lines on wasm32) and its own buffer;
the flush read the record and the Part before the target.

### 1. Dense slot tables in `flush()` (JS)

Each compact key set keeps dense arrays by node index: the handle of the
Part writing through its runner there, its target and its output offset.
Parts enter them on the compact path without `onUpdate` and leave on
`release` and `dispose`; the generated run loop checks one handle and
writes one target per record (the generation guards reused indices), so
it no longer loads the record or the Part. Flush, 8 keyframes, two
interleaved rounds: 10k 0.27–0.28 → 0.12–0.13 ms, 50k 1.02–1.07 → 0.71 ms.
Test: single-property runs match the generic loop through takeovers,
`onUpdate` Parts, kills and reused indices (dropping the slot clear in
`release` fails it).

### 2. Packed groups (core)

`Engine::group_pack` (`ak_group_pack`, called by `animate()` once the
children are added) copies the children of a group that share timing,
start, rate, flags, playback state and layout (all `simple` with the same
track count and easing, or all `kf` numbers with generated offsets, the
same count and easing) into dense arrays. While the first member stays in
the active iteration it was in, a frame renders it in full and computes the
others from the dense arrays (segment and eased progress from the memo it
just filled), writing only the pack's outputs, `first_outputs` and the
updated list: no member node or buffer is touched. Member times and `kf`
cursors are written back before anything reads them (`time`, `progress`,
any non-dense frame); outputs live in the pack (`values()` points there and
`animate()` re-reads each Part's offset). A member changed on its own
(track toggled by a takeover, timing, start, rate) is excluded and rendered
in full; a change to the child list stops the dense path. Staggered
children (different delays) and colours are not packed.
`ak_tick`, 8 keyframes: 10k 0.17 → 0.04 ms, 50k 0.87 → 0.26 ms. Tests:
`packed_groups_match_unpacked` (values, first outputs, updated lists,
events and public node state against an unpacked engine, and the member
nodes once written back, through seeks, reversal, rate changes, takeovers
of the leader and others, member rate/start changes, release and removal;
dropping the same-iteration check fails it) and the JS packed/unpacked
comparison (dropping the offset re-read fails it).

### Results

`bench.mjs`, three interleaved rounds (base / now), x GSAP (higher is
better):

| Case | N | base | now |
|---|---:|---:|---:|
| 3 numbers, `outCubic` | 1k | 0.28–2.90x | 3.34–4.90x |
| 〃 | 10k | 0.86–1.01x | 2.57–3.51x |
| 〃 | 50k | 0.81–1.05x | 2.87–3.18x |
| colour string | 1k | 1.59–3.04x | 1.66–2.21x |
| 〃 | 10k | 1.35–1.51x | 1.16–1.25x |
| 〃 | 50k | 1.96–2.11x | 1.33–1.69x |
| unit string | 1k | 1.08–1.85x | 0.97–1.34x |
| 〃 | 10k | 0.57–0.64x | 0.95–1.31x |
| 〃 | 50k | 0.64–0.67x | 1.07–1.36x |
| 8 keyframes, `inOutSine` | 1k | 0.89–1.19x | 1.67–1.88x |
| 〃 | 10k | 0.63–0.76x | 1.55–1.72x |
| 〃 | 50k | 0.57–0.79x | 1.35–1.78x |

Anikit ms in the same runs, 8 keyframes: 10k 0.31–0.41 → 0.14–0.16, 50k
1.52–1.81 → 0.74–0.89; 3 numbers 50k 2.68–3.17 → 0.76.

**Unresolved: colour at 50k is ~10–25% slower in the full `bench.mjs`
sequence.** Colours are not packed and their code path did not change, but
in three more interleaved full runs colour 50k went 2.53–2.75 → 2.97–3.42 ms
(still 1.5–1.9x GSAP). It comes with the packs made earlier in the process:
with the same build and packing turned off in JS (`packGroups = false`),
two rounds of "3 numbers then colour" at 1k/10k/50k gave base 2.70–2.71,
no packs 2.61–2.73, packs 3.08–3.17 ms. In a process that runs only the
50k sizes it does not show (base 3.08–3.16, now 2.79–3.20 ms), nor after
five 50k 3-number runs alone (3.17–3.53 → 2.47–2.56 ms). The suspect is
the wasm heap layout after the pack allocations are freed (the wasm code is
identical with and without packs); not resolved.

Staggered cases (added locally with `delay: stagger(0.001)`, three
interleaved rounds, Anikit ms): 3 numbers 50k 0.60–0.61 → 0.56–0.60, 10k
0.18–0.22 → 0.18–0.20; 8 keyframes 50k 0.53–0.67 → 0.57–0.74, 10k
0.18–0.20 → 0.14–0.16. Staggered children are not packed (different
delays); within noise.

`bench-create.mjs` (N = 10k, five interleaved rounds, ms, median and
range): one `animate()` of 3 numbers 61.5 (53.3–75.1) → 67.0 (52.7–78.6),
10k `animate()` calls 93.8 (51.9–115.0) → 91.0 (73.3–107.9), one
`animate()` of 8 keyframes 37.9 (22.8–60.3) → 29.5 (24.2–44.1); retained
heap per target unchanged (1.52 → 1.53 KB, 0.88 KB). The pack's copy and
the offset re-read are O(n) at creation; a CPU profile of the one-call case
is dominated by the garbage collector (~half), then `_build`, with every
wasm function together under 15%, so creating the children in one wasm call
was not attempted. Frames in the same script: timeline of 1000 steps x 10
targets 0.54–1.13 → 0.28–0.34 ms, 10k staggered targets 0.13–0.28 →
0.14–0.29 ms. `MANUAL=1 bench-dom.mjs`, two interleaved rounds, anikit ms
per frame: 500 0.43–0.56 / 0.46–0.48, 2000 2.13–2.16 / 1.61–2.09, 5000
5.11–6.10 / 5.06–5.84 (base / now; DOM writes are not on the compact path).

## Sixth round (creation)

Goal: animation **creation** cost, the one place GSAP still led
(`node --expose-gc scripts/bench-create.mjs`, N = 10k, first frame included).
Starting point: `583b4fb`. JavaScript only (`animate.ts`, `timeline.ts`,
`props.ts`, `wasm.ts`); no Rust change.

**Method.** Same shared 4-core VM, load 2–6 during the runs (other agents).
The `583b4fb` build and the final build were copied side by side and run
alternately: base, new, base, new, base, new. Single runs of the same build
moved by up to 2x (one animate(), 3 numbers: 40–63 ms on the base alone), so
only the retained-heap column and differences larger than the spread count.
A development CPU profile (`--cpu-prof`, 40 creations of 10k targets) was used
to pick the work; it is not the source of any number below.

**Where the time went** (profile of one `animate()` over 10k objects at the
start): garbage collector ~29%, `_build` ~17% self, all wasm functions
together ~15%, `_kill` + `unclaim` ~9%. Inside `_build`, once the closures
below were gone, the hottest lines were pushing the revert data into one
growing array (~18% of `_build`) and spreading the property `Map`
(`[...props]`, ~16% of `_build` for single-target calls).

### Changes (behaviour unchanged)

- **Per-target closures removed.** `Part.onEmpty` (one closure per Part) is a
  field holding the animation (`own`; not `ctl`, which is cleared when the
  controller detaches, while the closure kept the animation strongly). The
  only Part of a single-target animation forwards events through one shared
  function instead of a closure per animation. `addTrack` is created once per
  call, not per target.
- **`revert()` data for plain properties** (`Kind.Prop`) is stored flat
  (target, key, value) in one array per animation, sized once per call,
  instead of one closure per property per target. It is read at the same
  point as before (before any write, before `f64Buf`). Restores of other kinds
  stay closures; both are captured before any write and never cover the same
  target and key with different values, so their relative order does not
  matter.
- **Shared, read-only arrays and objects:** `Part.tm` (one array per call when
  the timing repeats, a frozen default), the default build context, a
  timeline's child build context, one track-offset array per call.
- **Overwrite claims** look each target's state up once per Part
  (`claimAll`/`unclaimAll`) instead of once per property; release order is
  unchanged.
- **No build closure** when the core is ready (`readyNow()`; `whenReady` is
  still used before it is), in `animate()` and `timeline.add()`.
- **Property map** read into two arrays with `forEach` instead of `[...props]`.
- **Exact-size binding arrays** per Part (the pushes left capacity for 17).
- **Timelines** keep their children for `kill()` directly instead of a closure
  per child.

Tests added (`animate: creation bookkeeping`): `revert()` of plain properties
with the same target twice, mixed with an element and the `text` special
property, and a second `revert()` restoring nothing; a takeover freeing a
multi-target animation, and a finished single-target animation's events.

### Results

`bench-create.mjs`, N = 10k, three interleaved rounds (ms; GSAP's range from
the same runs; retained = heap kept per target):

| Case | base `583b4fb` | now | GSAP | retained base → now (GSAP) |
|---|---|---|---|---|
| one `animate()`, 3 numbers | 52.0 · 53.1 · 63.5 | 30.3 · 48.1 · 55.8 | 4.3–5.7 | 1.53 → **0.86 KB** (0.47) |
| 10k `animate()` calls, 3 numbers | 60.6 · 66.3 · 66.8 | 58.5 · 65.6 · 89.2 | 13.2–30.2 | 2.48 → **1.66 KB** (1.14) |
| one `animate()`, stagger delay | 21.8 · 22.7 · 25.6 | 15.0 · 16.7 · 19.7 | 7.1–10.4 | 0.89 → **0.54 KB** (0.51) |
| one `animate()`, 8 keyframes | 24.0 · 24.4 · 27.0 | **17.0 · 18.0 · 20.0** | 33.8–37.2 | 0.88 → **0.45 KB** (0.53) |
| one `animate()`, colour string | 28.8 · 31.6 · 35.4 | 24.9 · 25.6 · 32.0 | 31.8–38.1 | 0.87 → **0.44 KB** (0.41) |
| timeline of 1000 steps | 6.0 · 6.3 · 6.5 | **4.5 · 4.7 · 5.5** | 1.1–1.8 | 2.67 → **1.83–1.87 KB** (1.09) |
| `animate()` taking over a running one | 46.9 · 47.9 · 48.5 | **38.4 · 39.6 · 44.1** | 347–376 | |
| `kill()`: timeline of 5000 steps | 1.03 · 1.07 · 1.10 | 0.79 · 0.82 · 0.92 | 0.02 | |

An earlier set of three rounds, before the binding-array change, read the
same way (one `animate()` 40.2–62.6 → 46.2–55.8 ms, retained 1.53 → 0.97 KB).

So: the heap kept per target dropped by 35–50% in every case, and the 8
keyframes, timeline, takeover and staggered rows are faster by more than the
spread. The two rows that matter most against GSAP, one `animate()` of 3
numbers and 10k separate calls, overlap with the base in these runs: the
gap to GSAP (about 10x and 3–4x) is **not closed**.

Per frame, `scripts/bench.mjs`, three interleaved rounds: Anikit still ahead
of GSAP in all 12 cases in every round, now 1.08–4.91x against 1.06–4.93x
for the base; Anikit ms per frame within the base's range in every case
(e.g. 3 numbers 50k 0.73–0.75 → 0.75–0.80, 8 keyframes 50k 0.70–0.75 →
0.68–0.71, colour 50k 3.05–3.24 → 3.00–3.18). The frame rows of
`bench-create.mjs` moved likewise within noise.

### Not done (sixth round)

- **One wasm call for all targets** of a call (one encoded record with
  per-target delays, straight into a pack): not attempted. The wasm functions
  together are ~15% of the one-call profile; GC and JS bookkeeping dominate.
- **Bindings per target.** Each Part still keeps one `Binding` per property
  (15 fields plus a boxed `ln` double): with the Part itself, the largest
  part of the 0.86 KB left. A struct-of-arrays form per call touches
  `prep`, `release`, `dispose`, `info` and `toCSS`.
- **Shared track encoding** when all targets have identical static values:
  with an implicit start value (`{ x: 500 }`) every target's track differs in
  its first value, so it needs an encoding with a per-target start slot.
- `numberTrack`'s per-target `TrackDef` and value arrays are short-lived and
  still allocated per property per target.
