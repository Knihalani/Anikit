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
| `deblock` | `false` | The in-loop filter. It helps only at QP ≥ 34 (+0.1–0.2 dB) and costs about 25% speed; at QP ≤ 28 it *lowers* PSNR on this content (sharp edges, and copied macroblocks next to changing areas get re-filtered every frame). |
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
- PSNR is above 40 dB at the default QP on every test (47–50 dB in practice).
- Unit tests check the transforms, the I4x4 prediction against the spec equations,
  the SIMD kernels against their scalar versions, and Exp-Golomb and emulation
  prevention.

## Benchmarks

The machine is the 4-core Xeon (2.1 GHz, AVX2) that the rest of the project uses.
The content is 1080p, 240 frames at 30 fps, and synthetic (`ak264::synth`): a
gradient background, sub-pixel-moving anti-aliased cards, and rows of anti-aliased
glyph shapes ("text").

| Scene | What changes per frame |
|---|---|
| `hold` | One small card moves over held text (about 2% of macroblocks). |
| `typical` | Six cards slide while a large panel pulses in brightness (about 12% of macroblocks). |
| `full` | The background scrolls by 3 px: every macroblock changes. |

**How the numbers were measured:**

- fps is wall clock for the whole encode, best of 2–3 runs.
- ak264 uses `encode_chunks` with GOP 60. x264 runs through ffmpeg (4 threads,
  `-g 60`) from a raw YUV file in page cache. OpenH264 (crate `openh264` 0.8,
  built from source) runs one encoder per 60-frame chunk on 4 threads.
- PSNR is computed against the source over all frames. "Y" is luma; "all" covers
  Y, U and V weighted by sample count.

Reproduce:

```sh
cargo run --release -p ak264 --example bench -- typical 240 22 26 30   # also hold, full
cargo run --release --manifest-path crates/ak264/bench-openh264/Cargo.toml -- typical 240 26
```

### Headline (QP 26 vs x264 interpolated to the same luma PSNR)

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
held-text content. On `typical`, which is about 12% changing and includes a
full-brightness fade on 10% of the frame, it reaches 8.6–9.4×.

### Full tables (luma PSNR in dB)

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
