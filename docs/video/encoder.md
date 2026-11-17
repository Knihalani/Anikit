# ak264: the renderer-aware H.264 encoder

`crates/ak264` is a pure-Rust H.264 (AVC) encoder written for Anikit Video. It
exists because encoding, not rendering, was the bottleneck: our renderer draws
1080p at roughly 935 fps on 4 cores, while x264 manages 70–140 fps on the same
content. ak264 is fast because it is told, or can cheaply find out, what changed
between frames, and because it only implements the tools that pay off on motion
graphics.

## Using it

```rust
use ak264::{encode_parallel, mp4, EncoderConfig, FrameHints};

let cfg = EncoderConfig { width: 1920, height: 1080, fps: 30.0, qp: 26, gop: 60, ..Default::default() };
// Render and encode in one pass: GOP chunks run in parallel, one encoder each.
let h264 = encode_parallel(&cfg, frames, |t, yuv, hints| {
    render_frame(t, yuv);                 // fill yuv.y / yuv.u / yuv.v (4:2:0)
    hints.changed = Some(dirty_mbs(t));   // optional: macroblocks that MAY have changed
});
std::fs::write("out.mp4", mp4::mux(&h264, 1920, 1080, 30.0))?;
```

Other entry points:

- `Encoder::new(cfg)` and `encode(&YuvFrame, &FrameHints) -> &[u8]`: streaming, one
  frame at a time. With `threads > 1`, each frame is split into that many slices,
  encoded in parallel.
- `encode_chunks(&cfg, &frames, &hints)`: frames already in memory, split into GOP
  chunks that are encoded in parallel.
- `Encoder::with_idr_base(cfg, chunk_index)`: one encoder per chunk, for your own
  scheduling. Chunks are concatenated in order.
- `mp4::mux(annexb, w, h, fps)` (pure Rust, fast start) or
  `mp4::mux_with_ffmpeg(path_in, path_out, fps)` (`-c copy`, e.g. to add audio).

### Configuration

| Field | Default | Notes |
|---|---|---|
| `qp` | 22 | Constant quantiser (0–51). 20–26 is visually lossless for motion graphics; ≥ 40 dB PSNR up to about QP 30 on our content. |
| `gop` | 250 | IDR interval, which is also the chunk length for parallel encoding. |
| `cabac` | `true` | Main profile with CABAC. `false` gives Constrained Baseline with CAVLC: 35–60% larger files, slightly faster. |
| `deblock` | `false` | The in-loop filter. It helps only at QP ≥ 34 (+0.1–0.2 dB) and costs about 25% speed; at QP ≤ 28 it *lowers* PSNR on this content (sharp edges, and copied macroblocks next to changing areas get re-filtered every frame). | With `threads > 1`, each slice deblocks its own rows in parallel (`disable_deblocking_filter_idc` 2).
| `subpel` | `false` | Quarter-pel motion refinement (exact 6-tap interpolation). On our scenes it saves 3–4% size for 20–40% more time. Turn it on for slow sub-pixel pans. |
| `threads` | 1 | Slices per frame for `encode()`. Chunk-parallel modes ignore it. |
| `full_range` | `false` | Signals the colour range (BT.709 primaries/matrix are always signalled). |

Width and height must be even. Any other size is fine: the picture is padded to a
multiple of 16 internally and cropped in the SPS.

### Hints

`FrameHints.changed` is a per-macroblock (16×16) *superset* of what changed, for
example the union of the display-list bounding boxes that moved. Unflagged
macroblocks are trusted to be identical to the previous frame and are never read.
Flagged ones are still compared exactly, so a loose box costs only a compare.
Without hints, every macroblock is compared (fused with the copy into the encoder's
buffer), which costs about 10% of a mostly static frame.

`FrameHints.motions` lists known translations, e.g. layer `(x, y, w, h)` moved by
`(dx, dy)`. They seed motion search for the macroblocks they cover.

## How it works

**Per macroblock:**

- **Unchanged:** the encoder emits `P_Skip` when the H.264 skip-MV predictor is
  zero (or when it predicts identical pixels), otherwise `P_L0_16x16` with MV 0 and
  no residual. Either way the reconstruction is a copy, and is not even copied when
  it is already in place. A frame with no changes is a single all-skip slice. For
  CABAC, its payload is cached.
- **Changed:**
  1. Early inter: if the predicted MV (or zero, or the hint) already matches with a
     tiny SAD, the macroblock is coded as inter immediately.
  2. Otherwise, Intra 16×16 (V/H/DC/Plane by SATD), then a small integer motion
     search: candidates from the predictors, neighbours and hints, then a diamond.
     Quarter-pel refinement is optional.
  3. I4x4 (all 9 modes) is tried when the best cost is still high.
  4. Residuals that provably quantise to zero (SAD below the quantiser's dead zone)
     skip the transform altogether.
  5. Inter macroblocks use x264-style coefficient decimation.

**Coding:**

- 4×4 integer transform, Hadamard luma DC (I16) and 2×2 chroma DC.
- CABAC (the byte-oriented arithmetic coder, with all context modelling) or CAVLC.
- Constant QP.
- The deblocking filter is bit-exact and optional.

**Parallelism:** chunks of `gop` frames, each starting with an IDR (with distinct
`idr_pic_id`s), are encoded independently on all cores and concatenated into one
valid stream. Within a frame, slices can also run in parallel.

**SIMD:**

- AVX2 or SSE4.1, detected at run time, with scalar fallbacks: SAD, SATD
  (16×16, 8×8, 4×4), and the frame compare.
- Transform, quantisation and IDCT run in SoA layout (one lane per 4×4 block,
  16 blocks at once), with SSSE3 byte transposes.
- Also vectorised: deblocking (16 lines at a time), sub-pixel interpolation and
  chroma MC.

## Validation

`cargo test -p ak264` runs these checks, using ffmpeg when it is installed:

- Every stream decodes with `ffmpeg -v error` with no messages.
- ffmpeg's decoded frames are **byte-identical to our reconstruction** for every
  test stream. This covers I and P frames, skips, CABAC and CAVLC, deblocking on and
  off, quarter-pel on and off, multi-slice, every QP from 0 to 51, odd sizes
  (2×2, 18×18, 100×58, 202×34), chunk concatenation, change-map and motion hints,
  and the MP4 container (checked with ffprobe).
- A second, independent decoder (Cisco OpenH264) decodes CABAC/Main and
  CAVLC/Baseline streams to exactly the same frames as ffmpeg:
  `cargo run --release --manifest-path crates/ak264/bench-openh264/Cargo.toml -- crosscheck`.
- PSNR is above 40 dB at the default QP on every test (47–50 dB in practice).
- Unit tests check the transforms, the I4x4 prediction against the spec equations,
  the SIMD kernels against their scalar versions, and Exp-Golomb and emulation
  prevention.

## Benchmarks

The machine is the 4-core Xeon (2.1 GHz, AVX2) that the rest of the project uses.
All clips are 1080p30, 300 frames (10 s).

| Clip | Source | What changes |
|---|---|---|
| `launch` | The real renderer: `anikit-video new launch`, frames 1–300 | An animated full-frame background (slowly drifting smooth blobs plus a grid), with titles. About 80% of macroblocks change by small amounts every frame, so skip rarely helps. |
| `tutorial` | The real renderer: `anikit-video new tutorial`, frames 1–300 | Text-heavy: titles and steps appear and hold. |
| `full` | Synthetic (`ak264::synth`) | The background scrolls 3 px per frame and six AA cards move at sub-pixel speeds, so every macroblock changes. |

Real-clip frames were rendered with `--codec png` and converted to yuv420p
(BT.709, limited range).

**How the numbers were measured:**

- fps is wall clock for the whole encode, best of 2–3 runs. Timings on this VM vary
  by about ±10%.
- ak264 uses `encode_chunks` with GOP 60. On 4 cores, 300 frames become 8 chunks
  of 37–38 frames each, each starting with an IDR (`chunk_ranges`).
- x264 runs through ffmpeg (4 threads, `-g 60`) from a raw YUV file in page cache.
- PSNR is computed against the source over all frames. "Y" is luma; "all" covers
  Y, U and V weighted by sample count.
- x264 sizes and speeds "at the same PSNR" are linear interpolations between the
  measured CRF points.

### Headline: QP 26 vs x264 at the same luma PSNR

| Clip | ak264 | vs veryfast | vs ultrafast | vs medium |
|---|---|---|---|---|
| launch | 360 fps, 2420 KB, 47.5 dB | **3.2× faster**, 1.14× size (≈ 113 fps, 2130 KB) | **1.1× faster, 0.74× size** (≈ 330 fps, 3270 KB) | 7.2× faster, 1.33× size (≈ 50 fps, 1816 KB) |
| tutorial | 836 fps, 499 KB, 51.1 dB | **4.5× faster**, 1.10× size (≈ 184 fps, 455 KB) | **2.3× faster, 0.93× size** (≈ 360 fps, ≈ 537 KB*) | 8× faster, 1.33× size (≈ 105 fps, 376 KB) |
| full | 468 fps, 903 KB, 48.1 dB | **3.0× faster**, 1.05× size (≈ 154 fps, 856 KB) | **1.3× faster, 0.57× size** (≈ 370 fps, 1584 KB) | 5.4× faster, 1.34× size (≈ 87 fps, 673 KB) |

\* x264 ultrafast's lowest measured point on `tutorial` is already 52.7 dB, so
this figure is extrapolated.

**Reading it:**

- ak264 is faster than x264 ultrafast everywhere, with files 7–43% smaller.
- Against veryfast it is 3–4.5× faster, with files 5–14% larger at the same
  quality. The remaining gap is mostly B-frames and smaller partitions, which ak264
  doesn't have.
- On content that is mostly static between frames, the margin grows a lot. On the
  synthetic `hold` scene (held text), ak264 is 13× faster than veryfast and 21%
  smaller. With renderer change maps it is 24× faster.

End to end in the pipeline, rendering now dominates on busy clips: SPEC measured
the launch template at 118–167 fps including rendering.

### Full tables (luma PSNR in dB)

`launch` (real renderer):

| Encoder | Setting | fps | KB | PSNR-Y | PSNR-all |
|---|---|---|---|---|---|
| ak264 chunks | qp 22 / 26 / 30 | 350 / 360 / 445 | 3660 / 2420 / 1630 | 50.21 / 47.52 / 44.59 | 50.72 / 47.92 / 45.35 |
| ak264 1 thread | qp 22 / 26 / 30 | 96 / 99 / 105 | 3622 / 2385 / 1606 | 50.13 / 47.43 / 44.46 | |
| ak264 4 slices (`threads: 4`) | qp 22 / 26 / 30 | 223 / 237 / 254 | 3682 / 2439 / 1650 | 50.10 / 47.41 / 44.44 | |
| x264 ultrafast | crf 22 / 26 / 30 | 270 / 313 / 356 | 6700 / 4350 / 2806 | 52.69 / 49.73 / 46.57 | 53.01 / 50.42 / 47.38 |
| x264 veryfast | crf 22 / 26 / 30 | 113 / 130 / 140 | 1998 / 1016 / 590 | 47.11 / 44.05 / 41.16 | 48.07 / 45.21 / 42.45 |
| x264 medium | crf 22 / 26 / 30 | 46 / 55 / 64 | 2470 / 1424 / 843 | 49.22 / 46.50 / 44.00 | 50.17 / 47.61 / 45.20 |

`tutorial` (real renderer):

| Encoder | Setting | fps | KB | PSNR-Y | PSNR-all |
|---|---|---|---|---|---|
| ak264 chunks | qp 22 / 26 / 30 | ≈ 770 / 836 / 865 | 714 / 499 / 377 | 53.97 / 51.13 / 47.91 | 54.66 / 50.96 / 48.49 |
| ak264 1 thread | qp 22 / 26 / 30 | 254 / 258 / 260 | 672 / 470 / 352 | 53.63 / 50.79 / 47.55 | |
| x264 ultrafast | crf 22 / 26 / 30 | 342 / 374 / 360 | 1451 / 1008 / 675 | 59.42 / 56.39 / 52.67 | |
| x264 veryfast | crf 22 / 26 / 30 | 181 / 186 / 188 | 479 / 295 / 200 | 51.57 / 48.22 / 44.95 | |
| x264 medium | crf 22 / 26 / 30 | 98 / 104 / 118 | 586 / 393 / 275 | 54.77 / 51.62 / 48.29 | |

`full` (synthetic):

| Encoder | Setting | fps | KB | PSNR-Y |
|---|---|---|---|---|
| ak264 chunks | qp 22 / 26 / 30 | 483 / 468 / 456 | 1167 / 903 / 638 | 51.29 / 48.12 / 44.17 |
| x264 ultrafast | crf 22 / 26 / 30 | 329 / 357 / 395 | 2507 / 2022 / 1371 | 53.95 / 50.38 / 47.02 |
| x264 veryfast | crf 22 / 26 / 30 | 152 / 154 / 159 | 960 / 708 / 506 | 49.39 / 46.31 / 43.14 |
| x264 medium | crf 22 / 26 / 30 | 86 / 86 / 93 | 904 / 698 / 531 | 51.74 / 48.58 / 45.50 |

Reproduce:

```sh
anikit-video new launch && anikit-video render launch/spec.json -o launch/png/f.png --codec png --no-audio
ffmpeg -framerate 30 -i launch/png/f_%05d.png -frames 300 \
  -vf "scale=out_color_matrix=bt709:out_range=tv,format=yuv420p" -f rawvideo launch.yuv
cargo run --release -p ak264 --example bench -- file:launch.yuv 300 22 26 30
cargo run --release -p ak264 --example bench -- full 300 22 26 30   # also typical, hold
cargo run --release --manifest-path crates/ak264/bench-openh264/Cargo.toml -- typical 240 26
```

### Earlier synthetic results

These use 240 frames: `hold` is held text, and `typical` is sliding cards plus a pulsing panel. They were measured before the later speed work, so the ak264 numbers are conservative.

| Scene | ak264 (4 cores) | x264 veryfast at the same PSNR | Speed | Size |
|---|---|---|---|---|
| hold | 2892 fps, 93 KB, 50.1 dB; **5367 fps** with renderer hints | ≈ 223 fps, 118 KB (50.0 dB) | **13×** (24× with hints) | **0.79×** |
| typical | 1268 fps, 453 KB, 49.8 dB; **1389 fps / 426 KB** with hints | ≈ 147 fps, ≈ 383 KB | **8.6×** (9.4× with hints) | 1.18× (1.11×) |
| full | 342 fps, 727 KB, 48.0 dB | ≈ 147 fps, ≈ 715 KB | 2.3× | 1.02× |

On `full`, x264 **ultrafast** runs at ≈ 305 fps and needs ≈ 1266 KB for 48.0 dB.
So ak264 is a little faster than ultrafast with files 43% smaller.

On `typical`, **OpenH264** (camera mode, `Complexity::Low`) runs at 551–606 fps
for 795 KB at 48.6 dB. ak264 is 2.1× faster with files 43% smaller, at 1.2 dB
better quality. Its constant-QP setting did not take effect through the crate, so
this is one quality point.

The target was 10× x264 veryfast on typical motion graphics. That is met on
held-text content. On `typical` (about 12% of macroblocks changing, including a
full-brightness fade on 10% of the frame), the table shows 8.6–9.4×. After the
later optimisations, `typical` runs at about 1500 fps without hints (≈ 10×) and
about 1760 fps with them (≈ 12×), at 458 KB / 49.8 dB and 438 KB respectively
(best of 3 runs).

On real renderer output with a fully animated background (`launch`), the margin
is 3.2× veryfast. Every macroblock really changes there, so ak264 has to do the
work of a normal encoder.

`typical`:

| Encoder | Setting | fps | KB | PSNR-Y | PSNR-all |
|---|---|---|---|---|---|
| ak264 1 thread | qp 22 / 26 / 30 | 350 / 347 / 354 | 568 / 453 / 375 | 53.36 / 49.77 / 46.52 | 53.83 / 49.96 / 47.21 |
| ak264 4 slices (`threads: 4`) | qp 22 / 26 / 30 | 475 / 445 / 400 | 622 / 494 / 409 | 53.37 / 49.74 / 46.52 | |
| ak264 chunks | qp 22 / 26 / 30 | 1170 / 1268 / 1252 | 568 / 453 / 375 | 53.36 / 49.77 / 46.52 | |
| ak264 chunks + hints | qp 22 / 26 / 30 | 1338 / 1389 / 1326 | 543 / 426 / 348 | 53.32 / 49.74 / 46.49 | |
| x264 ultrafast | crf 22 / 26 / 30 | 330 / 340 / 370 | 1703 / 1321 / 1023 | 57.72 / 54.51 / 52.91 | |
| x264 veryfast | crf 22 / 26 / 30 | 144 / 156 / 140 | 449 / 351 / 270 | 51.96 / 48.71 / 45.52 | |
| x264 medium | crf 22 / 26 / 30 | 97 / 107 / 104 | 491 / 387 / 295 | 54.45 / 51.32 / 48.09 | |
| OpenH264 Low / Medium | default QP | 551–606 / 433–449 | 795 / 788 | 48.59 / 48.21 | |

`full`:

| Encoder | Setting | fps | KB | PSNR-Y |
|---|---|---|---|---|
| ak264 1 thread | qp 22 / 26 / 30 | 128 / 96 / 92 | 930 / 727 / 495 | 51.42 / 48.01 / 44.13 |
| ak264 4 slices | qp 22 / 26 / 30 | 230 / 199 / 193 | 1033 / 770 / 516 | 51.12 / 47.89 / 43.98 |
| ak264 chunks | qp 22 / 26 / 30 | 451 / 342 / 303 | 930 / 727 / 495 | 51.42 / 48.01 / 44.13 |
| x264 ultrafast | crf 22 / 26 / 30 | 315 / 294 / 305 | 2002 / 1621 / 1105 | 53.96 / 50.34 / 46.94 |
| x264 veryfast | crf 22 / 26 / 30 | 143 / 150 / 138 | 794 / 584 / 411 | 49.17 / 46.08 / 42.84 |
| x264 medium | crf 22 / 26 / 30 | 80 / 83 / 88 | 720 / 551 / 426 | 51.47 / 48.35 / 45.25 |

`hold`:

| Encoder | Setting | fps | KB | PSNR-Y |
|---|---|---|---|---|
| ak264 chunks | qp 22 / 26 / 30 | 3059 / 2892 / 3530 | 111 / 93 / 80 | 53.71 / 50.09 / 46.91 |
| ak264 chunks + hints | qp 22 / 26 / 30 | 6747 / 5367 / 6677 | 111 / 93 / 79 | 53.69 / 50.08 / 46.91 |
| x264 ultrafast | crf 22 / 26 / 30 | 331 / 358 / 358 | 838 / 393 / 264 | 60.81 / 58.03 / 54.23 |
| x264 veryfast | crf 22 / 26 / 30 | 219 / 223 / 209 | 152 / 118 / 100 | 53.66 / 50.04 / 46.64 |
| x264 medium | crf 22 / 26 / 30 | 154 / 147 / 160 | 154 / 144 / 115 | 57.16 / 53.13 / 49.80 |

**Notes:**

- Timings on this VM vary by about ±10% between runs.
- `4 slices` scales poorly on mostly static content: the per-frame compare and
  copy are serial. Use chunks for throughput.
- "Hints" are the scene's own bounding boxes plus rounded card translations, which
  is what a renderer knows.

### Option trade-offs

Measured with `cargo run --release -p ak264 --example quality -- <scene> 60 <qp>`
(one thread, 60 frames):

| Scene, QP | default (CABAC) | + deblock | + subpel | CAVLC |
|---|---|---|---|---|
| typical, 26 | 320 fps, 111 KB, 50.12 dB | 247 fps, 111 KB, 48.80 dB | 302 fps, 108 KB, 50.09 dB | 336 fps, 174 KB, 50.12 dB |
| full, 26 | 93 fps, 179 KB, 48.68 dB | 79 fps, 197 KB, 48.25 dB | 65 fps, 197 KB, 49.03 dB | 96 fps, 247 KB, 48.68 dB |
| typical, 34 | 337 fps, 70 KB, 43.86 dB | 306 fps, 70 KB, 44.01 dB | 284 fps, 67 KB, 43.91 dB | 412 fps, 123 KB, 43.86 dB |

## Limitations and next steps

- P frames only: no B-frames, one reference, 16×16 partitions only. B-frames and
  smaller partitions would cut size on busy content, at a cost in speed.
- There is no rate control, only constant QP (CRF-like). A `target_bitrate` mode
  would need a per-frame QP loop.
- Brightness fades are coded as intra. Weighted prediction (as x264 uses) would
  make them nearly free.
- No 8×8 transform or High profile.
