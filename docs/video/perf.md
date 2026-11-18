# Anikit Video: quality-locked performance (`qbench`)

Anikit Video has to be very fast, and it must never pay for speed with visible quality. Earlier speed-ups that stepped animations or rendered backgrounds at low resolution looked glitchy. `qbench` exists so that can't happen again. Every fast path is measured against a ground-truth render, frame by frame, and a change that makes anything visibly worse fails.

```bash
cargo run --release -p anikit-video --example qbench                     # whole corpus, table on stdout
cargo run --release -p anikit-video --example qbench -- --gate           # exit 1 on any failure: run before pushing
cargo run --release -p anikit-video --example qbench -- --only ui,post --runs 1 --dump 12   # one spec, frame 12's images
```

**Before you push** a change to `render/`, `pipeline/`, `eval/`, `components/`, `scene3d/`, the shader layer or `ak264`, run `qbench -- --gate`. It takes about 10 minutes on the 4-vCPU VM; `--only` runs a subset. Paste the table into your commit or PR if numbers moved.

## What it does, for each corpus entry

1. **Default render, timed.** It does exactly what `anikit-video render spec.json -o out.mp4` does: ak264, standard quality, full resolution, audio muxed. A fresh evaluator is built per run and the median of `--runs` (default 3) is reported. Entries with `time_full` time the whole spec, because sub-second clips time too noisily. The stage profile comes from `Summary.profile` (`pipeline::StageProfile`): eval, render, copy and encode CPU ms per frame summed over threads, and idle (cores waiting). It also reports startup (parse + compile) and the first frame (eval + full render), which is what an agent waits for.
2. **Reference render.** `render::Quality::Reference` turns off every speed-only approximation:
   - glyph runs and scaled images at their exact sub-pixel position, with no caches;
   - card shadows through the general blur (no sprites);
   - blurs at full resolution (no decimation);
   - no layer cache, no fused backdrop → YUV path, no incremental regions, no held or deduplicated frames;
   - motion blur in ~0.5 px steps (8–96 samples) with whole-frame samples;
   - shader layers supersampled 2×2 at every pixel, except filter shaders, which are never supersampled (the look, not an approximation);
   - components in reference mode (`components::reference()`, for components with their own speed knobs).

   Every frame is rendered from scratch and converted to YUV in double precision. Same post look and motion-blur decisions as the default, so only *approximations* are measured, not intent.
3. **Render fidelity.** The default pipeline's frames are tapped exactly as they go into the encoder (`pipeline::render_ak264_tap`) and compared with the reference. This isolates what the renderer's fast paths cost.
4. **Delivered fidelity.** The decoded MP4 is compared with the reference, encoder loss included, against `qbench/baseline.json`.
5. **Determinism.** A second default render of the clip must produce the same frames.
6. **Render cache.** Timed runs always render everything: qbench sets `ANIKIT_RENDER_CACHE=0` itself. Then, for every entry without a `runs` override, it renders the clip twice more with the cache on, into a scratch cache directory: once cold (filling it) and once warm (splicing cached chunks). Both MP4s must equal the uncached MP4 byte for byte, or the entry FAILs ("render cache … differs"). `--no-cache-check` skips this.
7. **Thread count.** For every entry without a `runs` override whose timed render takes at most 40 CPU-seconds, the clip is rendered again on a one-thread rayon pool. The MP4 must equal the default (all cores) MP4 byte for byte, or the entry FAILs: work stealing must never change output. `--no-thread-check` skips this. The `comps` entry (nested compositions, a shader filter, post) is there for this check.
8. **Skipped frames.** The timed render's held and unchanged frames (`StageProfile.unchanged`) are ratcheted in `baseline.json` (`skipped`). Fewer means static spans re-render, a silent speed regression, and the entry FAILs.
9. **Memory.** The `RSS MiB: start · peak` column is the process's resident memory when the timed render started and its peak while frames rendered and encoded (`StageProfile.mem_*`, from `memstat`). qbench holds earlier entries' results, so for a clean number per spec use `examples/mem_probe.rs` (below).

## Metrics and thresholds

| Metric | How | Fails when (any frame) |
|---|---|---|
| SSIM | luma, 8×8 windows, stride 4 (as x264 measures it) | < 0.995 |
| PSNR-Y | luma | < 42 dB |
| ΔE p99.9 | CIE76 on S-CIELAB-style pooled planes: Gaussian σ = 1 px on luma, σ = 1 chroma px on Cb/Cr (less pooling than S-CIELAB at 1080p from 3 picture heights, so stricter). 99.9th percentile over the frame. | > 2.3 (one JND) |
| flicker p99.9 | frame-to-frame change of the luma error (default − reference), pooled σ = 1 px, in 8-bit levels. A stable approximation has a still error; stepped motion, popping and flicker make it move. | > 4 |
| delivered | SSIM min and mean PSNR-Y of the decoded MP4 vs the reference | below baseline by > 0.002 SSIM or > 0.3 dB |
| determinism | two default renders, frame for frame | any difference |

Reported, not gated:
- raw per-pixel ΔE76 (it counts ±1 LSB dither as "visible");
- pooled CIEDE2000 p99.9;
- ⚑ flags: max pooled ΔE > 5, pixels over one JND in the worst frame, delivered SSIM < 0.97 (film grain is expensive for any lossy encoder at a fixed QP).

Why pooled: per-pixel ΔE flags differences no one can see, such as dither noise and sub-LSB rounding. Spatial pooling models how the eye integrates small detail, and it keeps anything with spatial extent: edge shifts, banding, haloes, wrong colours. Flicker catches what a per-frame metric can't.

## Output

`target/qbench/`:
- `results.json` (every number);
- `<name>.frames.json` (per-frame stats);
- `<name>.heat.png`: the max pooled ΔE over the clip over a dimmed reference frame (blue < 1 < green < 2.3 < yellow < 5 < red);
- `<name>.mp4`;
- with `--dump k`: `<name>-f<k>.{test,ref,err,flicker}.png` (error ×16 around grey).

Other flags:
- `--dump-nondet` writes the first frame that differs between two renders;
- `--update-baseline` rewrites `qbench/baseline.json`. Do it only when delivered quality legitimately changed, for example a new encoder setting that was checked by eye, and say so in the commit.

## Corpus (`crates/anikit-video/qbench/corpus.json`)

| Entry | What it covers |
|---|---|
| `ui` | product UI: cards, counters, a bar chart, a toast, a cursor (`bench/video/anikit/product.json`, 6–10 s) |
| `text` | kinetic type with a camera push (`kinetic.json`, 0–4 s) |
| `explainer` | held slides and crossfades (`explainer.json`, 4–8 s) |
| `backdrop` | mesh-gradient and aurora fields, a light leak, a moving radial glow, glow and shadowed text |
| `particles` | starfield, confetti, sparks, bokeh, dust, particle text |
| `launch` | the launch template expanded with `examples/video/launch/params.json`: HUD, mesh backdrop, blur transition, CTA confetti (2–6 s) |
| `scene3d` | the ray-traced chrome lookdev (0–0.3 s; one timed run) |
| `shader` | a built-in GLSL shader layer with a title over it |
| `post` | the cinematic post stack plus motion blur on fast-moving shapes |
| `audio` | `examples/video/pulse` (9:16, music, sfx, beat sync), 0–4 s |

Add an entry when you add a feature with its own fast paths. If a component has speed knobs, make it honour `components::reference()`.

## Results

Same VM throughout: 4 vCPU (Intel Xeon @ 2.1 GHz), 16 GB, Linux 6.18. Wall time is the median of 3 runs. In the fps column, entries marked "(timed)" time the whole spec; the others time the clip.

### Phase 2, 16:30 (main 8cf24ac merged; `--runs 3` for the content-heavy rows, `--runs 1` for the rest)

| spec | fps | CPU ms/frame: eval · render · encode | render SSIM min | ΔE p99.9 | flicker | result |
|---|---:|---|---:|---:|---:|---|
| ui | 1450–1875 | 0.05 · 0.57 · 1.24 | 1.00000 | 0.00 | 0.0 | PASS (bit-identical) |
| text | 695–747 | 0.01 · 2.09 · 2.59 | 1.00000 | 0.00 | 0.0 | PASS |
| explainer | 1386–1709 | 0.01 · 0.61 · 1.45 | 1.00000 | 0.00 | 0.0 | PASS |
| backdrop | 39 | 50.2 · 20.5 · 16.8 | 0.99795 | 1.21 | 1.1 | **PASS** (the components fix; was KNOWN) |
| particles | 35 | 17.4 · 36.7 · 23.9 | 0.99911 | 1.67 | 1.4 | **PASS** (the components fix; was KNOWN at ΔE 36) |
| launch | 115 | 6.5 · 14.6 · 4.7 | 0.99694 | 2.38 | 1.6 | KNOWN (1 frame) |
| scene3d | 2 | 515 · 12 · 11 | 0.99893 | 5.24 | 7.8 | KNOWN (3D's reference mode now wired; its default differs visibly) |
| shader-hero | 9–14 | – · 133 · 6 | 0.99955 | 1.17 | 2.8 | PASS |
| shader (aurora) | 1 | – · 1711 · 40 | 0.95376 | 2.71 | 1.6 | KNOWN (SHADER: aliasing) |
| post | 12 | – · 216 · 18 | 0.99864 | 1.03 | 1.5 | PASS (motion-blur strip fixed by RENDER) |
| audio | 94–107 | 0.05 · 17.5 · 11.2 | 0.99957 | 1.51 | 0.9 | PASS |

The components' quality fixes cost speed: backdrop eval went 10.5 → 50–73 ms per frame, and backdrop 73 → 39 fps. Their speed pass is in progress.

**Time to first still** (CLI → PNG, 1080p): product.json 21 ms. The music spec (pulse) went 750 → ~200 ms on repeat runs with the new on-disk analysis cache (`diskcache`), with byte-identical output. Fixed along the way: a visualizer's relative `src` wasn't resolved against the spec's folder, so from any other folder it silently showed a synthetic groove instead of the music.

### Earlier: after merging main 43f56da (`qbench --gate`, after merging main 43f56da; one run each)

| spec | frames (timed) | wall s | fps | CPU ms/frame: eval · render · copy · encode · idle* | startup · 1st frame ms | render SSIM min | ΔE p99.9 / max | flicker | delivered SSIM min / PSNR-Y | result |
|---|---:|---:|---:|---|---|---:|---:|---:|---|---|
| ui | 120 (600) | 0.35 | 1729 | 0.05 · 0.57 · 0.04 · 1.24 · 0.41 | 4 · 17 | 1.00000 | 0.00 / 0.1 | 0.0 | 0.9993 / 53.2 | PASS |
| text | 120 (600) | 0.84 | 712 | 0.01 · 2.09 · 0.07 · 2.59 · 0.84 | 2 · 11 | 1.00000 | 0.00 / 0.0 | 0.0 | 0.9996 / 55.4 | PASS |
| explainer | 120 (600) | 0.35 | 1709 | 0.01 · 0.61 · 0.05 · 1.45 · 0.20 | 2 · 9 | 1.00000 | 0.00 / 0.5 | 0.0 | 0.9989 / 53.1 | PASS |
| backdrop | 105 | 1.44 | 73 | 10.5 · 23.7 · 0.48 · 15.6 · 4.7 | 2 · 52 | 0.98919 | 3.29 / 12.3 | 2.7 | 0.9813 / 47.0 | KNOWN (COMPONENTS) |
| particles | 120 | 2.38 | 51 | 1.5 · 19.2 · 0.41 · 23.1 · 34.2 | 133 · 11 | 0.98832 | 36.1 / 61.4 | 64.9 | 0.9855 / 42.2 | KNOWN (COMPONENTS) |
| launch | 120 (1005) | 6.29 | 160 | 0.78 · 15.5 · 0.16 · 4.9 · 3.7 | 4 · 72 | 0.99337 | 2.51 / 3.2 | 1.9 | 0.9821 / 48.1 | KNOWN (COMPONENTS) |
| scene3d | 9 | 13.39 | 0.7 | 1464 · 11 · 0.5 · 10.6 · 4465* | 1502 · 1525 | 1.00000 | 1.27 / 2.3 | 0.0 | 0.9948 / 50.5 | PASS (no 3D reference knobs yet) |
| shader-hero | 60 | 4.28 | 14 | 0.02 · 133 · 0.52 · 6.1 · 146* | 66 · 77 | 0.99955 | 1.17 / 9.2 | 2.8 | 0.9964 / 50.6 | PASS |
| shader (aurora) | 60 | 53.5 | 1.1 | 0.03 · 1711 · 0.51 · 40.3 · 1817* | 30 · 921 | 0.95376 | 2.71 / 10.9 | 1.6 | 0.9410 / 40.3 | KNOWN (SHADER: aliasing vs 2×2 supersampled reference) |
| post | 90 | 7.28 | 12 | 0.03 · 216 · 0.48 · 18.0 · 88.6* | 1 · 129 | 0.99864 | 0.97 / 8.8 | 1.6 | 0.9504 / 42.7 | PASS |
| audio | 120 | 1.13 | 106 | 0.05 · 17.5 · 0.30 · 11.2 · 6.0 | 595 · 33 | 0.99957 | 1.51 / 1.8 | 0.9 | 0.9942 / 47.6 | PASS |

\* idle is wall × threads minus the per-frame stage times, so for stages with nested parallelism (shaders, 3D, post) it includes their work on other threads. Rendering frames of short clips in parallel was tried and gained nothing; the cores are already busy.

Output is byte-identical between 1 and 4 threads (checked: launch, product, backdrop, shader-hero, post).

KNOWN means the entry is over the thresholds today, recorded in `qbench/baseline.json` (`known`, with an owner). It passes the gate only while it gets no worse. Run `qbench --exact-components` to see what removing those shortcuts costs: particles pass at 43 fps instead of 54; the mesh field drops to 4 fps, so it needs a better fast path, not the exact one.

### Before, at the start of this work (same harness and metrics, original fast paths)

| spec | wall s | fps | render SSIM min | ΔE p99.9 / max | flicker | result |
|---|---:|---:|---:|---:|---:|---|
| ui | 0.07 (clip) | 1678 | 0.99988 | 2.66 / 6.8 | 3.4 | FAIL: ¼-px glyph phases |
| text | 0.14 (clip) | 871 | 1.00000 | 0.00 / 0.3 | 0.0 | PASS |
| explainer | 0.18 (clip) | 672 | 0.99987 | 2.96 / 6.5 | 7.4 | FAIL: ¼-px stepping on a slow rise |
| backdrop | 1.40 | 75 | 0.99624 | 2.90 / 8.4 | 3.7 | FAIL: YUV-space overlay compositing |
| particles | 1.52 | 79 | 1.00000 | 0.29 / 1.2 | 0.2 | PASS (the components' shortcuts weren't measured yet) |
| launch | 1.65 (clip) | 73 | 0.99696 | 3.86 / 6.7 | 7.6 | FAIL: blur-σ steps, YUV compositing |
| scene3d | 35.6 | 0.25 | 1.0 | 1.28 / 2.1 | 0.0 | PASS |
| post | 11.2 | 8 | 0.99980 | 2.38 / 6.5 | 4.4 | FAIL |
| audio | 1.37 | 88 | 0.99874 | 2.48 / 4.3 | 4.9 | FAIL |

Not deterministic: `launch.json` rendered differently run to run.

### Speed, like for like

The content changed during this work (the components' premium pass made backdrop, particles and launch heavier). So each speed change is measured with binaries on the same content: `anikit-video render`, best of 2, whole spec.

| Change | Spec | Before | After |
|---|---|---:|---:|
| Deterministic chunk-owned renderers, cubic upscaler rows vectorised, RGBA compositing | launch (whole, 1005 frames) | 163 fps | 285 fps (content then) |
| same | backdrop | 67 fps | 103 fps (content then) |
| Post: lens 4×, final pass 1.3× (byte-identical) | post | 15.3 s (6 fps) | 7.45 s (12 fps) |
| Upscaler (RENDER's fixed-point rewrite) vectorised (byte-identical) + post | launch (current content) | 8.18 s | 7.63 s |
| same | backdrop (current content) | 2.16 s | 1.98 s |
| Audio decode / beat caches | pulse, 4 s with audio | 2.07 s | 1.95 s |

For reference, trunk before the in-flight render and components branches rendered launch at 411 fps. Their quality work (cubic fields, full-quality components) costs most of that gap; the fast paths here took back what could be taken back without losing quality.

## What the harness found, and what changed

| Finding | Visible? | Fix | Effect |
|---|---|---|---|
| The glyph-bitmap cache drew runs at ¼-px phases. Slow eases (a rising title) advanced in ¼ px steps. | yes: micro-stepping on slow text moves (explainer flicker 7.4) | exact-phase cache key; static text still hits | ui and explainer now **bit-identical** to the reference; same speed |
| Scaled images were drawn with vello bilinear on first sight and tent-resampled from the second frame on (quarter-pixel snapped). | yes: a one-frame pop; stepping on slow Ken Burns moves; output depended on cache history | always area-resample at the exact placement | deterministic; no pop |
| The renderer was rebuilt whenever any thread interned a font, and a thread waiting in nested parallel work rendered stolen GOP chunks with throwaway renderers. | yes: the same spec rendered differently run to run (launch) | renderer state per GOP chunk, not per thread; fonts swapped into the renderer in place | byte-identical output run to run; no cold renderers |
| Decimated blurs used integer box radii (σ quantised to `f` px) and ignored the variance the down/up-sampling adds. | yes: blur transitions advanced in jumps (launch flicker 7.2, ΔE 2.7) | extended boxes (exact, continuous variance) with the resampling variance compensated | launch passes: flicker 1.4, ΔE 1.7 |
| Overlays on a fused backdrop were composited in YUV (alpha and chroma per 2×2 block). | marginal: up to 2.5 ΔE at glow and text edges | RGBA compositing in the overlay boxes; YUV compositing is draft-only | backdrop ΔE 2.55 → 1.28 |
| The cubic field upscaler used a scalar float loop; Rust's saturating `f32 as u8` stops vectorisation. | no (speed) | vectorised rows; `render::sat_u8` | full-frame upscale 17 → 7 ms (one thread). RENDER has since rewritten it in fixed point. |
| Motion blur rendered every sample as a whole frame and called `powf` 6M times a frame. | no (speed) | samples only where things move; LUT encode | 448 → ~400 ms CPU/frame on `post`. The post stack dominates, see below. |

Open items, with their owners (all messaged with numbers):
- **Components' visible shortcuts** (COMPONENTS): fxkit light and particles (source-over instead of Add, sprite defocus, wide god-ray slices; ΔE p99.9 35, flicker 65; exact costs only ~15%), and the mesh-gradient field resolution (ΔE 3.3; exact is 12× slower, so it needs a better fast path). These are recorded as KNOWN in the baseline.
- **Motion-blur box artefact** (RENDER): a hard-edged strip at the edge of a card starting to move (qbench `post`, frame 0; pooled ΔE max 25).
- **Post stack** (`render/post.rs`, RENDER): now ~290 ms per 1080p frame on one thread for the cinematic preset (decode 5, lens 95, bloom 35, grade/encode 155), down from 640 ms. The rest needs an explicit AVX2 pass (planar, gathers for the LUTs); keep the current code as the reference path.
- **Shader layer** (SHADER): 3.35 s CPU per 1080p frame for `aurora`. It's exact (no approximations to measure), just slow.
- **scene3d** (3D): 4.1 s CPU per 1080p frame for the chrome lookdev (3× faster than at the start). It doesn't honour `components::reference()` yet, so its own approximations (spp, adaptive termination, DOF, bloom) aren't measured.
- **Startup with music** (SPEC/COMPONENTS): pulse takes 0.6 s to compile, from beat analysis (225 ms), the visualizers' own decode and analysis at their sample rate (~170 ms) and particle-text's glyph sampling (160 ms). These run one after another; component builds could run in parallel.
- **Encoder** (ak264): it's now the largest CPU share on busy content (backdrop, particles: 17–23 ms/frame). Stage split, from timing counters: I4×4 trial plus coding is about half on particle-heavy content, and the rest is spread across motion search, sub-pel, intra16, residual coding, deblock (1.7 ms) and half-pel planes (1.2 ms). Mode pruning would change bitstreams; gate it with the delivered columns and file sizes.
- **Load balance on short clips**: one GOP chunk per core leaves cores idle when chunks differ in cost (particles: 30 ms/frame idle on a 4 s clip). Long videos (≥ 12 chunks) are fine.

## Tools

- `examples/post_cost.rs`: one frame's cost by stage (render, post, motion blur). `OPS=1` gives the cost of each top-level op (or layer subtree) by cumulative prefixes. `FIELD=1` gives the share of the gradient field.
- `anikit-video render … --profile`: stage profile of a real render, with a memory line: resident MiB at start, peak while rendering and encoding, the encoded stream's size, and peak while muxing.
- `cargo run --release -p anikit-shader --example file_cost -- shader.glsl [time] [frames] [name=value …]`: one GLSL file at 1080p with one sample, adaptive AA (the default) and full 2×2. Uniforms default to their `// = v` comment.
- `examples/mem_probe.rs spec.json [runs] [from] [to]`: renders a spec repeatedly in one process and prints resident memory after build, after render and after dropping the evaluator, plus the process peak. Use it to catch memory that a long-lived process (MCP server, preview) keeps between renders.
- `cargo test -p anikit-video --lib upscale_timing -- --ignored --nocapture` and `blur_timing`: kernel timings.

## Incremental re-render (render cache)

`anikit-video render` caches each encoded GOP chunk on disk (`diskcache`: `$ANIKIT_CACHE_DIR`, `$XDG_CACHE_HOME/anikit` or `~/.cache/anikit`). A chunk's key covers:
- the encoder config;
- the chunk's index and length;
- this binary (size and mtime);
- for every frame in it, the evaluated display list, with image ids replaced by content identities (shader-job key, per-frame image key, pixel hash for stored images), plus the post look and motion-blur flag.

Font ids in a display list are handed out in the order threads first use a face, so they differ between runs. Keys replace each with a hash of the face (font bytes, index, variation coordinates, synthetic bold and skew), as they replace image ids with content keys. Before this, launch reused only 6–8 of its 16 chunks on an unchanged re-render; now all 16.

Every chunk key also covers `Evaluator::inputs_key`: the contents of every file the spec names, such as fonts, LUTs, images, shader sources, HDRIs, models and nested composition specs (and the files those name). A changed file at the same path is never served stale. Before this, a font or LUT changed in place could hit.

A render in which any renderer asked for an image and didn't get it (an evicted per-frame image, a failed component or shader image) stores nothing, and says so. A frame drawn without a layer is never cached and spliced into later renders: CEILING-2 saw three flat-background frames stick that way. `render --no-cache` (or `ANIKIT_RENDER_CACHE=0`) renders everything and stores nothing, for a clean final.

A re-render splices unchanged chunks back in byte for byte and renders only the rest. An audio-only change reuses every video chunk; only the mux runs again. A frame with an image whose content can't be identified without rendering makes its chunk uncacheable, so it is simply rendered. `ANIKIT_RENDER_CACHE=0` turns the cache off. Progress output prints `render cache: reused N/M chunks`.

Measured on the Tern launch spec (20 s, 1080p30, 4 vCPU):

| render | time | md5 |
|---|---:|---|
| full, cache off | 148.3 s | 15becf74… |
| cold cache (fills it) | 154.5 s | 15becf74… |
| unchanged spec again | **1.7 s** (16/16 chunks reused) | 15becf74… |
| one font size changed in the last scene | **32.8 s** (13/16 reused) | 2b3ea57d… |
| the same edit, full render, cache off | 148.2 s | 2b3ea57d… |

## Memory

Measured with `mem_probe` (2 s of each spec, 4 renders in one process, 4 vCPU, 1080p). The peak while rendering is about the same before and after; what changed is what a process keeps afterwards.

| spec | peak rendering | kept after render 4, before | kept after render 4, after |
|---|---:|---:|---:|
| particles | 211–215 MiB | 157 MiB (growing) | 45 MiB |
| post | 270 MiB | 347 MiB (growing) | 23 MiB |
| backdrop | 438–450 MiB | 621 MiB (526 after dropping the evaluator) | 69 MiB |

Two causes:
- **glibc arenas.** Each rayon thread frees its frame buffers into its own malloc arena, and glibc keeps them. The pipeline now calls `memstat::trim()` (`malloc_trim(0)`) once, when the frames are done. That took a few milliseconds, too little to show in timing. mimalloc was tried as the global allocator: it was ~3% faster but kept more (peak +15–20%), so it was not adopted.
- **The per-frame image ring.** Component fields, rays and particle text live in a 96-slot ring of full-frame pixmaps (`SharedAssets` volatile ring), which stayed alive with the evaluator: about 250 MiB for backdrop. `SharedAssets::release_frame_images()` drops it when a render finishes. It resets the sequence too, so a later render uses the same ids as a fresh evaluator. Output is byte-identical: every qbench MP4 matched main's.
- **The CLI's pre-render estimate.** Before a heavy render, `anikit-video render` samples frames to predict the time, through thread-local workers (renderer, frame buffers, layer caches) on every rayon thread. The video pipeline renders with fresh per-chunk workers, so those stayed alive, unused, for the whole render. `pipeline::release_workers()` (a `rayon::broadcast`) plus a heap trim now runs after the estimate and when a render finishes. On offset2027 (div-g, 15 s, shader-heavy), resident memory before the first frame went from 534 to 309 MiB and the peak from 1247 to 945 MiB, with byte-identical output.
- **scene3d's frame-buffer pool** (`scene3d::kernel::pool`) keeps up to 2 GiB of frame planes warm between frames, to avoid first-touch page faults. Nothing emptied it, so a process that had rendered a scene3d or particle scene kept it all. `pool::release()` now runs when a render's frames are done. On the swell deliverable's 80k-particle scene (7.8–8.3 s), memory kept after the render went from 1481 MiB to 277 MiB, with the same time (7.5 s vs 7.8 s) and byte-identical output.

## Post: planar, vectorised passes (bit-exact)

`examples/post_stages.rs [threads] [preset]` times the post look on a 1080p test card, then with one stage off at a time (the difference is that stage's cost). `cinematic`, one thread:

| stage | before | after |
|---|---:|---:|
| whole look | 290 ms | 80 ms |
| decode + final pass (grade, tone map, vignette, grain, dither, encode) | 167 ms | 28 ms |
| lens (distortion + chromatic aberration) | ~85 ms | ~30 ms |
| bloom + halation | ~53 ms | ~25 ms |
| whole look, 4 threads | – | 23 ms |

The per-pixel loops did their table lookups, clamps, branches and bounds checks inline, so nothing vectorised. Each row now runs in chunks of 64 pixels, one stage at a time, over planar arrays:
- coordinates, table indices and weights in vector loops;
- table lookups and bilinear taps gathered in a scalar loop;
- blends and the rest of the arithmetic in vector loops, under AVX2 when the CPU has it.

The arithmetic is the same, in the same order. Rust never fuses multiply-adds, so the output is bit-identical: `render::post::tests::final_pass_matches_scalar` checks every preset, ACES and no-tone-map variants, with and without glow, at odd sizes, against the original per-pixel loops (kept as the path for grade LUTs, and forced in tests). The bloom's upsample and halation lookups compute their bilinear taps once per column and per row instead of per pixel.

## Startup: time to the first still

`examples/still_cost.rs spec.json [t]` splits a still's startup into parse, font store, evaluator build (assets, music, beat analysis, component preparation) and the first frame.

| spec | build before | build after |
|---|---:|---:|
| `examples/video/pulse` (warm disk cache) | 165 ms | 33 ms |

The cost was `particle-text` placing its target dots: it tests a jittered grid of candidate points (3 passes of about count points per word) for being inside the glyph outlines, and `kurbo`'s winding number re-splits every outline segment at its extrema for every point. A segment whose control points' y-range (padded for rounding) excludes the point contributes exactly 0, so the index (`particle_text::Winding`) tests only the segments that can matter, each as a one-segment path through the same kurbo code. The result is identical: `winding_index_matches_kurbo` checks 120k points, including points on segment ends, and qbench's audio MP4 (pulse) is byte-identical.
