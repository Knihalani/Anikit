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
