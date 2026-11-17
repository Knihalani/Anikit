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
   - components in reference mode (`components::reference()`, for components with their own speed knobs).

   Every frame is rendered from scratch and converted to YUV in double precision. Same post look and motion-blur decisions as the default, so only *approximations* are measured, not intent.
3. **Render fidelity.** The default pipeline's frames are tapped exactly as they go into the encoder (`pipeline::render_ak264_tap`) and compared with the reference. This isolates what the renderer's fast paths cost.
4. **Delivered fidelity.** The decoded MP4 is compared with the reference, encoder loss included, against `qbench/baseline.json`.
5. **Determinism.** A second default render of the clip must produce the same frames.

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

4 vCPU (Intel Xeon @ 2.1 GHz), 16 GB, Linux 6.18. "before" is the merged trunk at the start of this work, with today's metrics. "after" is the current branch. Wall time is the median of 3 runs.

<!-- qbench:start -->
(filled in by the latest run, see below)
<!-- qbench:end -->

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

Open items, with their owners:
- **Post stack** (`render/post.rs`, RENDER): ~385 ms/frame on one thread at 1080p for the cinematic preset (lens 85, bloom 150, grade/encode 145). It's scalar AoS code; planar SIMD with the static maps cached (lens table, vignette, dither) should get it under 30 ms. Keep the current code as the reference path.
- **scene3d** (3D): ~3.2 s CPU per 1080p frame for the chrome lookdev. It doesn't honour `components::reference()` yet, so its own approximations aren't measured.
- **Shader layer** (SHADER): see the shader row.
- **Encoder**: once rendering is fast, ak264 is the largest share of CPU on busy content (backdrop, particles: 17–20 ms/frame). The next step is intra/inter decision pruning, gated by the delivered columns.

## Tools

- `examples/post_cost.rs`: one frame's cost by stage (render, post, motion blur). `OPS=1` gives the cost of each top-level op (or layer subtree) by cumulative prefixes. `FIELD=1` gives the share of the gradient field.
- `anikit-video render … --profile`: stage profile of a real render.
- `cargo test -p anikit-video --lib upscale_timing -- --ignored --nocapture` and `blur_timing`: kernel timings.
