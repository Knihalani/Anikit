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

Same VM throughout: 4 vCPU (Intel Xeon @ 2.1 GHz), 16 GB, Linux 6.18. Wall time is the median of 3 runs. In the fps column, entries marked "(timed)" time the whole spec; the others time the clip.

### Now (`qbench`, latest merged trunk + render + components + 3D + shader)

| spec | frames (timed) | wall s | fps | CPU ms/frame: eval · render · copy · encode · idle | startup · 1st frame ms | render SSIM min | PSNR-Y min | ΔE p99.9 / max | flicker | delivered SSIM min / PSNR-Y | result |
|---|---:|---:|---:|---|---|---:|---:|---:|---:|---|---|
| ui | 120 (600) | 0.33 | 1840 | 0.02 · 0.37 · 0.03 · 0.91 · 0.81 | 2 · 13 | 1.00000 | 99.0 | 0.00 / 0.1 | 0.0 | 0.9992 / 53.1 | PASS (bit-identical to reference) |
| text | 120 (600) | 0.76 | 786 | 0.01 · 1.48 · 0.06 · 2.64 · 0.42 | 1 · 10 | 1.00000 | 89.7 | 0.00 / 0.3 | 0.0 | 0.9996 / 55.4 | PASS |
| explainer | 120 (600) | 0.34 | 1785 | 0.01 · 0.54 · 0.04 · 1.26 · 0.77 | 2 · 9 | 1.00000 | 85.7 | 0.00 / 0.5 | 0.0 | 0.9988 / 52.7 | PASS |
| backdrop | 105 | 1.51 | 70 | 10.86 · 24.39 · 0.44 · 16.98 · 5.19 | 1 · 65 | 0.98919 | 47.5 | 3.29 / 12.3 | 2.7 | 0.9813 / 47.0 | KNOWN: components' low-res mesh field |
| particles | 120 | 2.21 | 54 | 1.61 · 17.97 · 0.38 · 22.57 · 30.52 | 143 · 9 | 0.98844 | 34.4 | 35.3 / 61.4 | 65.0 | 0.9857 / 42.4 | KNOWN: fxkit shortcuts (visible) |
| launch | 120 (1005) | 7.73 | 130 | 0.73 · 26.02 · 0.16 · 4.91 · 5.05 | 3 · 79 | 0.99337 | 47.8 | 2.51 / 3.2 | 1.9 | 0.9821 / 48.1 | KNOWN: mesh field |
| scene3d | 9 | 12.31 | 0.7 | 4096 · 12 · 0.6 · 11 · 1349 | 1658 · 1523 | 1.00000 | 93.4 | 1.28 / 2.3 | 0.0 | 0.9948 / 50.7 | PASS (3D has no reference knobs yet) |
| shader | 60 | 51.06 | 1.2 | 0.02 · 3350 · 0.49 · 38.6 · 13.9 | 30 · 861 | 1.00000 | 92.0 | 1.02 / 1.8 | 0.0 | 0.9319 / 39.8 | PASS |
| post | 90 | 7.56 | 12 | 0.03 · 307 · 0.52 · 20.4 · 7.8 | 1 · 138 | 0.99866 | 55.2 | 0.96 / **25.2** | 1.6 | 0.9504 / 42.7 | PASS; ⚑ a visible strip at frame 0 (motion-blur boxes, reported to RENDER) |
| audio | 120 | 1.20 | 100 | 0.05 · 19.57 · 0.27 · 12.01 · 5.25 | 590 · 35 | 0.99957 | 62.9 | 1.51 / 1.8 | 0.9 | 0.9942 / 47.6 | PASS |

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
- `anikit-video render … --profile`: stage profile of a real render.
- `cargo test -p anikit-video --lib upscale_timing -- --ignored --nocapture` and `blur_timing`: kernel timings.
