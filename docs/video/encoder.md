# ak264: the renderer-aware H.264 encoder

`crates/ak264` is a pure-Rust H.264 (AVC) encoder written for Anikit Video. It
exists because encoding, not rendering, was the bottleneck: our renderer draws
1080p at roughly 935 fps on 4 cores, while x264 veryfast manages 70–190 fps on the same
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
| `subpel` | `true` | Quarter-pel motion refinement (exact 6-tap interpolation, half-pel planes kept up to date incrementally). It saves 16–46% on real renderer clips with sub-pixel motion, for 15–25% more time. |
| `aq_strength` | `-0.5` (auto) | Adaptive quantisation: smooth macroblocks (fades, lit floors) get a lower QP, −(4 + up to 7)·strength, with a wide AC dead zone; busy ones keep `qp`. ≥ 0 is used as given. Negative is automatic per frame: \|value\| on frames dominated by smooth textured content (3D renders, video), 0 on UI and motion graphics. |
| `ssim_floor` | `0.985` | Per-frame quality floor. After each frame the luma SSIM of the reconstruction is estimated; while it is below the floor the next frames use a lower slice QP (`slice_qp_delta`), and an IDR below the floor is coded a second time at the estimated QP. The QP comes back up towards `qp` when there is margin. It is deterministic: it depends only on frames already coded in the stream or chunk. 0 = constant QP. Content above the floor (all UI, text and motion-graphics clips tested), and frames already above 45 dB PSNR-Y, are bit-identical to constant QP. |
| `ssim_max_drop` | `8` | Lowest QP the floor may use, as an offset below `qp`. It bounds the size on content where SSIM rises slowly with bits, such as per-frame film grain. |
| `bframes` | `0` (anikit-video: 1) | Non-reference B pictures between reference pictures (CABAC, `threads == 1`). Output is delayed (an IDR also waits one frame, see "IDR lookahead"): call `Encoder::flush()` at the end of a stream (the chunk encoders do). anikit-video uses 1 for its CABAC qualities. See "B-frames" below. |
| `transform8` | `true` | 8×8 transform for inter macroblocks (High profile; CABAC only). |
| `partitions` | `false` | 16×8, 8×16 and 8×8 P partitions. About 1.5% smaller at equal SSIM, for about 1.5× the encode instructions, so off. |
| `skip_threshold` | `0` | Perceptual skip: code a changed macroblock as a copy when every sample is within this distance of the reconstructed reference. Off by default (see the measurements below). |
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

`FrameHints.grain` is the film grain the renderer added: the standard deviation of the luma grain at mid-grey, in 8-bit code values. For anikit-video's post grain this is `grain × 219 / √6`, about 3.1 for `cinematic`. The SSIM floor then treats that much missing source variance as free, so grain that is re-drawn every frame doesn't drive the QP down, while lost detail still does.

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
- Browser playback: headless Chromium (Playwright) plays the MP4.
  `canPlayType('avc1.4d4028')` returns "probably", playback advances, and a frame
  drawn to a canvas matches the content. Run
  `node crates/ak264/scripts/browser-check.js out.mp4`.
- The stream is Main profile at a level computed from size and frame rate (4.0
  for 1080p30), with an `avc1` sample entry and `avcC`. The VUI carries BT.709 and
  the range, `max_num_reorder_frames = 0` and `max_dec_frame_buffering = 1`, as
  strict players (Safari/QuickTime) expect for streams without B-frames.
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

- Test setup: a 4-vCPU VM (Intel Xeon @ 2.10 GHz, AVX2), rustc 1.97.0, release
  profile. x264 is libx264 0.164.3108 through ffmpeg 6.1.1 (Ubuntu packages).
- fps is wall clock for the whole encode, best of 2–3 runs. Timings on this VM vary
  by about ±10%.
- ak264 uses `encode_chunks` with GOP 60. On 4 cores, 300 frames become 8 chunks
  of 37–38 frames each, each starting with an IDR (`chunk_ranges`).
- x264 runs through ffmpeg (4 threads, `-g 60`) from a raw YUV file in page cache.
- PSNR is computed against the source over all frames. "Y" is luma; "all" covers
  Y, U and V weighted by sample count.
- x264 sizes and speeds "at the same PSNR" are interpolated between the two
  nearest measured CRF points (linear in PSNR, logarithmic in size). Values marked
  \* lie outside the measured range and are extrapolated.

### Headline: QP 26 (defaults, quarter-pel on) vs x264 at the same luma PSNR

| Clip | ak264 | vs veryfast | vs ultrafast | vs medium |
|---|---|---|---|---|
| launch | 344 fps, 2071 KB, 47.7 dB | **3.5× faster, 0.90× size*** (≈ 98 fps, 2291 KB) | 1.0× speed, **0.63× size** (≈ 329 fps, 3296 KB) | 7.2× faster, 1.13× size (≈ 48 fps, 1827 KB) |
| tutorial | 437 fps, 498 KB, 51.2 dB | **2.2× faster**, 1.10× size (≈ 199 fps, 454 KB) | **1.5× faster, 0.86× size*** (≈ 286 fps, 576 KB) | 4.1× faster, 1.33× size (≈ 106 fps, 376 KB) |
| full | 474 fps, 1074 KB, 48.9 dB | **3.2× faster**, 1.18× size (≈ 148 fps, 913 KB) | **1.3× faster, 0.63× size** (≈ 367 fps, 1700 KB) | 5.4× faster, 1.50× size (≈ 87 fps, 715 KB) |
| spec-to-screen | 190 fps, 8392 KB, 42.0 dB | **2.1× faster, 0.97× size** (≈ 89 fps, 8611 KB) | 0.6× speed, **0.48× size** (≈ 322 fps, 17561 KB) | 4.7× faster, 1.40× size (≈ 41 fps, 6004 KB) |

Measured on the final build (automatic AQ, smooth-ramp I4x4 rule, low-level
optimisations). ak264 fps is the median of its three QP runs, because single
runs on this VM varied by up to ±30%. Tutorial ran at 651 / 377 / 437 fps at
QP 22 / 26 / 30, and earlier sessions measured 750–900 fps at QP 26. x264 ran
in the same session, under the same conditions.

**Reading it:**

- Against veryfast, ak264 is 2.1–3.5× faster at about the same size (0.90–1.18×).
- Against ultrafast, files are 14–52% smaller. Speed ranges from 1.5× faster
  (text) to 0.5× (spec-to-screen, the hardest clip: nearly every macroblock
  changes with sub-pixel motion of smooth gradients). On that clip ultrafast is
  faster but needs twice the bytes for the same PSNR.
- Medium is still 11–50% smaller. The gap is B-frames, smaller partitions and
  psycho-visual tuning, which ak264 doesn't have.
- On content that is mostly static between frames, the margin grows a lot. On the
  synthetic `hold` scene (held text), ak264 is 13× faster than veryfast and 21%
  smaller. With renderer change maps it is 24× faster.

**Where ak264 loses:**

- x264 medium (and slower presets) makes 12–50% smaller files at the same PSNR.
  ak264 has no B-frames, no partitions below 16×16, no 8×8 transform, one reference
  frame and no psycho-visual tuning.
- On the hardest full-change content (spec-to-screen), x264 ultrafast is about 2×
  faster, though it needs about 2× the bytes for the same PSNR.
- veryfast is up to 18% smaller on synthetic full-frame motion (`full`).
- ak264 has no rate control, only constant QP, so it can't hit a target bitrate.
- All numbers are luma PSNR. No perceptual metric (SSIM, VMAF) was measured, and
  x264's psycho-visual tuning is invisible to PSNR, which favours ak264's
  comparisons.

**Quarter-pel is the default since it pays on real renderer output.** Integer-pel
(`subpel: false`) is 15–25% faster but much larger on smooth sub-pixel motion:

| Clip, QP 26 | integer-pel | quarter-pel (default) |
|---|---|---|
| launch | 395 fps, 2420 KB, 47.52 dB | 349 fps, 2061 KB (−15%), 47.75 dB |
| tutorial | ≈ 940 fps, 499 KB, 51.13 dB | 750–900 fps, 492 KB (−1%), 51.21 dB |
| full (synthetic) | 565 fps, 903 KB, 48.12 dB | 425 fps, 1075 KB (+19%), 48.90 dB |
| spec-to-screen | 160 fps, 12825 KB, 41.83 dB | 161 fps, 8332 KB (−35%), 42.03 dB |

(Integer-pel rows were measured before the I4x4 gate change below; quarter-pel rows
after it. The gate made spec-to-screen about 18% cheaper in instructions.)

On spec-to-screen the saving grows with QP: −24% at QP 22 and −46% at QP 30.

### Spec-to-screen, 10–20 s (the hardest real clip)

`examples/video/spec-to-screen/spec.json --from 10 --to 20`: 600 frames at
1920×1080, 60 fps, with the globe and aurora moving under the code block. About
70% of macroblocks change every frame.

| Encoder | Setting | fps | KB | PSNR-Y | PSNR-all |
|---|---|---|---|---|---|
| ak264 chunks | qp 22 / 26 / 30 | 151 / 195 / 190 | 17156 / 8392 / 4265 | 43.57 / 42.04 / 40.09 | 44.74 / 43.05 / 41.18 |
| ak264 chunks, integer-pel | qp 22 / 26 / 30 | 117 / 160 / 193 | 22160 / 12825 / 7816 | 43.45 / 41.83 / 39.83 | 44.63 / 42.85 / 40.94 |
| ak264 1 thread | qp 22 / 26 / 30 | 43 / 53 / 58 | 16794 / 8241 / 4149 | 43.54 / 42.02 / 40.06 | |
| ak264 4 slices | qp 22 / 26 / 30 | 88 / 113 / 131 | 17081 / 8493 / 4321 | 43.54 / 42.01 / 40.05 | |
| x264 ultrafast | crf 18 / 23 / 28 | 222 / 276 / 315 | 61502 / 24532 / 12158 | 45.83 / 43.14 / 40.83 | 46.99 / 44.34 / 41.99 |
| x264 veryfast | crf 18 / 23 / 28 | 73 / 105 / 109 | 21760 / 4296 / 1627 | 43.52 / 40.93 / 38.41 | 44.67 / 42.15 / 39.70 |
| x264 medium | crf 18 / 23 / 28 | 25 / 41 / 56 | 23138 / 6294 / 2474 | 43.96 / 42.13 / 40.35 | 45.28 / 43.43 / 41.62 |

One thread spends about 19 ms per frame here (53 fps at QP 26). In P frames,
I4x4 is tried only when both P16x16 and I16x16 cost more than λ·64 + 1024: before
that gate, I4x4 trials were a quarter of the instructions while winning about 30
macroblocks per frame. The profile below was taken before the gate. By inclusive instruction
count, a profile splits that roughly as follows:

| Work | Share |
|---|---|
| inter coding (transform, quant, reconstruction) | 17% |
| motion search | 13% |
| intra decisions (I16 and I4) | 19% |
| CABAC | 10% |
| half-pel plane updates | 5% |
| sub-pel prediction | 6% |

The rest is mode decision and copies.

Perceptual skip (`skip_threshold`) was measured and left off. At QP 26 it saved
only 0.6–4% (launch 2420 → 2328 KB at T = 2) while costing 0.4–1.3 dB, which is
worse than raising the QP. On `full`, T = 1 made the file 20% larger. Holding slow
backgrounds still in the renderer removes ±1 drift at the source instead.

For camera moves, `FrameHints::from_affine(w, h, [a, b, c, d, e, f])` turns the
camera transform into one motion hint per macroblock. In tests, zooms of 1.5%, 6%
and 12% per frame (192×128) produced byte-identical output with or without the
hints, because the motion search already finds that motion. The hints matter only
when motion goes beyond the search range, for example fast zooms at 1080p.

### Full tables (luma PSNR in dB)

These ak264 rows were measured with integer-pel motion (`subpel: false`, the
earlier default). For quarter-pel, see the comparison above.

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

These use 240 frames: `hold` is held text, and `typical` is sliding cards plus a pulsing panel. They were measured with the earlier defaults (integer-pel). Quarter-pel, now the default, costs about 20% speed on `typical` for a 3% smaller file, so treat these speeds as an upper bound for today's defaults.

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

### Low-level optimisation

Every change below leaves the bitstream bit-identical: each clip's size and PSNR
match exactly before and after. Wall-clock time on this shared VM varied by up to
±30% between runs of the same binary, so the work was steered by instruction
counts (callgrind, 20 frames, one thread). The pipeline's configuration is QP 23
with deblocking on:

| Clip | Before | After | Change |
|---|---|---|---|
| launch | 11.16 G | 8.93 G | −20% |
| tutorial | 4.93 G | 3.98 G | −19% |
| spec-to-screen | 13.30 G | 10.65 G | −20% |

The final builds include automatic AQ; its per-frame measurement costs about 1%.

What changed:
- **Sub-pel refinement:** AVX2 SAD two rows per step, one call per candidate
  pattern, and a phase table instead of a match.
- **I16 mode decision:** SADs of V, H, DC and plane come straight from the edges
  (a SIMD plane ramp), and only the winning prediction is built.
- **SATD:** the last butterfly stage uses |p + q| + |p − q| = 2·max(|p|, |q|),
  which is exact and uses fewer operations.
- **Chroma MC:** SSSE3 `pmaddubsw` bilinear interpolation.
- **Deblocking:** one bounds check per edge and an SSE2 8×4 transpose for chroma
  vertical edges. Cb and Cr edges are filtered together as one 16-lane pass.
  Internal-edge strengths come from a coefficient bitmask, and edges without
  bS 4 skip the strong filter at compile time.
- **Motion search:** the full search returns its seed when it ends on the seed's
  vector, with no second prediction copy and no second SATD.
- **CABAC:** decisions renormalise only when `range < 256`, with no bounds checks;
  empty 4×4 blocks are a single bin; the residual coder walks a nonzero bitmask.
- **Smaller items:**
  - Block copies take one bounds check per block.
  - An 8 KB table gives `se(v)` lengths for MV costs.
  - Half-pel plane updates use hoisted row pointers and merge runs across
    one-macroblock gaps.
  - Change detection zeroes only the accumulators it uses.

Two attempts made things worse and were dropped:
- A register-resident sliding window for the vertical half-pel filter spilled
  (12 YMM rows plus constants).
- Wide row loads in the deblocking filter beat the compiler's own code by less
  than the extra copies cost.

A 4-candidate half-pel diamond is 33% cheaper but makes spec-to-screen 1.9%
larger, so it was not kept.

### Smooth gradients: block structure and adaptive quantisation

At QP 23 a renderer's low-amplitude texture on smooth areas is below the
quantiser step. That covers ±1 dither on dark fades and denoiser noise on a lit
3D floor. Once it is gone, the gradient underneath shows its 1-level steps.

Two encoder effects made those steps look like blocks:

1. **4×4 staircases from I4x4 on fades.** Fixed, and on by default: smooth
   macroblocks (sum of |second differences| under 2 per sample) don't try I4x4
   (except CAVLC at QP < 2, where I16 DC levels would exceed CAVLC's escape
   range). I16 plane prediction and inter prediction keep fades smooth.
   - On COMPONENTS' dark aurora fade it is size-neutral (580 vs 583 KB) and
     slightly better in every metric.
   - Launch and tutorial grow 0.7–0.9% at the same PSNR.
2. **A patchwork of flat 16×16 tiles on gentle gradients.** On the
   `lookdev/cyc.json` floor (frame 60, QP 23):
   - The delivered error is small: 8×8 means p99 0.8, max 1.3 levels.
   - But 57% of the floor macroblocks come out perfectly flat, while their
     neighbours keep the source's noise and slope.
   - The result is 1-level steps on the macroblock grid. QP 19 and x264 at
     CRF 23 show the same.

**`aq_strength`** targets the second effect.
- **Which macroblocks:** smooth ones, bright or dark.
- **QP:** −(4 + up to 4 for smoothness + up to 3 for darkness) × strength,
  coded with `mb_qp_delta` (CABAC and CAVLC); deblocking uses each edge's
  averaged QP.
- **Dead zone:** those macroblocks get a wide AC dead zone (rounding 1/8), so
  the QP buys DC precision rather than coded noise.
- **Effect:** at 1.0 the cyc floor turns from a patchwork into clean bands that
  follow the gradient.

Applied everywhere, smooth-heavy UI content pays heavily (QP 23, 300 frames):

| Clip | AQ 0 | AQ 0.5 | AQ 1.0 |
|---|---|---|---|
| cyc floor (3D, 120 frames) | 2934 KB, 47.50 dB | 3043 KB (+4%), 47.65 dB | 3210 KB (+9%), 47.77 dB |
| launch | 2991 KB, 49.81 dB | 4063 KB (+36%), 50.62 dB | 6281 KB (+110%), 51.44 dB |
| tutorial | 658 KB, 53.52 dB | 863 KB (+31%), 54.32 dB | 1354 KB (+106%), 54.80 dB |
| spec-to-screen | 4080 KB, 44.39 dB | 4485 KB (+10%), 44.48 dB | 5972 KB (+46%), 44.60 dB |
| full (synthetic) | 1296 KB, 51.44 dB | 1333 KB (+3%), 52.09 dB | 1577 KB (+22%), 52.92 dB |

**The default is automatic (`-0.5`).**
- **Rule:** per frame, from that frame's own samples, so it is deterministic and
  independent of chunking and thread count. AQ 0.5 applies when at least 60% of
  a 2×2-subsampled macroblock grid is smooth but textured (second differences
  between 0.25 and 2 per sample), as in renders of 3D scenes and video. UI
  backgrounds are mostly exactly flat and stay at constant QP.
- **Effect at QP 23:**

  | Clip | Size change | Result |
  |---|---|---|
  | launch, tutorial, full | 0% (auto stays off) | unchanged |
  | spec-to-screen | +1.8% (some frames qualify) | |
  | cyc floor | +4% | patchwork gone |
  | COMPONENTS' dark aurora | +29% (580 → 747 KB) | smoothed PSNR 56.8 → 57.8 dB, the QP 19 level at fewer bytes than QP 19 (789 KB) |

- **Cost:** the measurement adds about 1% instructions.
- **Strength:** 0.5 already removes the cyc patchwork; 1.0 looks the same there
  at +9%.

On the PERF corpus (`qbench`), automatic AQ was compared with the earlier
encoder:
- **Fidelity:** delivered quality is equal or better on every spec. Backdrop goes
  46.96 → 47.42 dB, launch 48.12 → 48.47 dB, scene3d 50.72 → 51.08 dB (min
  PSNR-Y). Minimum SSIM rises with them, and particles moves by −0.0001.
- **Size:** where AQ fires, files grow: backdrop +35%, launch +23%, scene3d
  +39%. Everything else changes by 0–4%.

The x264 size gap on cyc: x264 at CRF 23 is 791 KB but 42.5 dB, five dB below
ak264 at QP 23. At matched PSNR ak264 needs about 1080 KB (1.36×), the usual gap
from B-frames and RD tools on busy content. A wide AC dead zone on smooth inter
macroblocks (coding less denoiser noise) changed the size by only 0.2%, so the
bits are not going into floor noise.

Variants that were measured and rejected:
- **x264-style symmetric AQ** (raising QP on busy macroblocks): launch doubled
  in size and lost 1.5 dB, because it moves bits from text to the background.
- **AQ on intra-coded macroblocks only:** still +49% on launch, which has about
  1,800 intra macroblocks per P frame.
- **A plane-mode bias for I16:** no visual change, slightly worse metrics.
- **An intra penalty for flat P-frame macroblocks:** 4× less flicker, but 3.3 dB
  of drift on the gradient.
- **Keeping DC-only corrections out of decimation:** no effect at QP 23.

No QP short of near-lossless keeps the ±0.5-level texture itself. The renderer's
dithers are already fixed per pixel, frame to frame (RENDER), so static areas
skip for free.

Reproduce with `cargo run --release -p ak264 --example dark`. For a rendered
clip: `FILE=clip.yuv AQ=1 FRAME=60 OUT=dir CROP=x,y LIFT=lo,gain` writes a
lifted crop, and `DUMP=frame.yuv` writes the reconstructed frame.

### Quality floor (SSIM)

At a constant QP 23, two renderer specs delivered a visibly lower SSIM than the others: `shader` (0.933 min, 39.8 dB) and `post` (0.950 min, 42.7 dB). Every other spec is ≥ 0.993. The error has two sources:

- **`shader`: high-frequency procedural detail that moves.** It is spread over the whole frame, with a mean |Laplacian| of 17. Constant QP 19 reaches 0.968 and QP 15 reaches 0.984.
- **`post`: film grain.** The frame is dark (mean luma 40) and covered in grain of about ±2 levels that is re-drawn every frame. 34% of the pixels change by more than 2 between frames where nothing moves. An inter frame can't predict new noise, so at QP 23 the grain is smoothed away, which costs SSIM everywhere. x264 behaves the same way: `medium` at CRF 18 delivers 0.950 at 5.5 MB, and `-tune grain` doesn't help.

`ssim_floor` (default 0.985) handles both with per-frame feedback. The results are for 90 frames at 1080p30 and `qp` 23; SSIM-Y and PSNR-Y are from ffmpeg on the decoded stream:

| Spec | Constant QP 23 | Floor 0.985, max drop 8 (default) | Floor, max drop 12 |
|---|---|---|---|
| shader | 0.933 min / 0.939 mean, 39.8 dB, 2.7 MB | 0.981 / 0.982, 45.4 dB, 10.5 MB | – |
| post | 0.950 / 0.951, 42.8 dB, 1.1 MB | 0.977 / 0.981, 47.0 dB, 21.6 MB | 0.977 / 0.988, 49.0 dB, 31.7 MB |
| particles, UI, text, … | ≥ 0.993 | unchanged (bit-identical) | – |

The floor removes the visible loss on `shader` for 3.8× the bytes. It stops lowering the QP once a frame's luma PSNR reaches 45 dB, because beyond that the error isn't visible however low SSIM reads on fine texture. So an explicit high quality such as `--crf 18` isn't pushed further: the 20 s Tern launch spec at `--crf 18` is 14.2 MB with or without the floor, against 25.0 MB without the cap. For reference, x264 `medium` without B-frames reaches about 0.97 at 3.0–3.4 MB, so a coding-efficiency gap of about 2× on this kind of content remains: x264 has RD mode decision, several references, 8×8 transforms and trellis quantisation. On `post`, without a hint, the bytes go to coding random grain, which is expensive with any encoder. With `FrameHints.grain` = 3.1 (the `cinematic` grain), `post` stays at QP 23: 1.1 MB, 1.0× the size.

- Its grain-compensated SSIM is 0.994 min. This is the floor's own measure: the source variance minus the grain's, per 8×8 block.
- Its 2×2-downsampled SSIM is 0.982. That measure still contains half the grain amplitude, so it would need QP 20, at 5× the size, to reach 0.985.

The same compensation applied to grain-free `shader` still reads 0.975 at QP 23, so real detail loss is still caught. Synthesising the grain at playback, or rendering it with a fixed pattern, would be far cheaper than encoding it.

### Motion search, B-frames and the 8×8 transform

Measured with `examples/bd.rs`: five 1080p clips (launch, tutorial, shader, post,
tern; 90–240 frames), QPs 19/23/27, BD-rate at equal SSIM-Y (negative = smaller
at the same quality), with the anikit-video settings (CABAC, deblocking,
quarter-pel; AQ and the SSIM floor off so that QP alone sets quality).

| Change | launch | tutorial | shader | post | tern | mean |
|---|---|---|---|---|---|---|
| Motion search: rings + hexagon | −3.4% | −2.9% | −26.4% | +1.8% | −32.1% | −12.6% |
| + one B picture (quality guard, adaptive QP offset) | −12.2% | −13.6% | −19.8% | +3.3% | +8.2% | −6.8% |
| + 8×8 transform | −0.3% | +0.2% | −5.6% | −0.2% | −1.5% | −1.5% |
| + B direct early exit | −0.3% | −1.7% | −0.1% | 0.0% | −0.9% | −0.5% |
| **All, vs the encoder before** | | | | | | **≈ −20%** |

- **Motion search.** Candidates (zero, skip and predicted vectors, renderer
  hints, neighbours), then rings at radius 4, 8, 12 and 16 around the best,
  hexagon steps, and an iterated square refine.
  - The old 1-pel diamond lost any motion that no predictor pointed to, and
    those macroblocks were coded intra. On tern with references three frames
    apart, P-frame bytes drop 69%.
  - Positions already evaluated are not evaluated again. Candidates are read
    through one bounds check per search.
- **B pictures.** One non-reference B picture between P pictures, coded with
  - 16×16 L0, L1 and bi-predicted macroblocks;
  - spatial direct and B_Skip (direct is used only over an unpartitioned
    co-located macroblock);
  - intra 16×16;
  - QP +4 (adaptive), and no deblocking in the encoder (B pictures are never
    referenced; the decoder deblocks them).

  Searches are seeded with the direct vectors and the co-located P vector
  scaled by temporal distance, without long-range rings. Macroblocks whose
  direct prediction is already good (SAD ≤ 64λ) are coded direct without a
  search.

  A **quality guard** recodes a B picture at the P QP when its SSIM estimate is
  more than 0.001 below the following P picture's. Without the guard,
  `particles` failed qbench's delivered-SSIM gate on B-frame minima. Each
  recode lowers the next B picture's starting offset, and B pictures that
  match their P picture raise it again.

  There are no B pictures on frames with a grain hint. The MP4 muxer writes
  `ctts` and an edit list from the slice POCs (POC type 0 when B-frames are
  on).
- **IDR lookahead.** With B-frames on, an IDR waits for the next frame. Its
  macroblocks that are unchanged there (mean absolute luma difference ≤ 0.5)
  are coded 6 QP lower. Static content is then copied onward by skips, so it
  pays to code it well once: a one-frame version of x264's macroblock tree.
  Content that changes keeps the frame QP, so clips that change everywhere
  (film grain, full-frame motion, `post`) are coded as before.
  - The IDR is first coded at the frame QP. It is coded again with the drop only
    if its luma PSNR is under 45 dB (the SSIM floor's cap): above that the
    error isn't visible.
  - Ungated, the near-lossless qbench specs grew for nothing: ui +23%, scene3d
    +25%, shader-hero +13%, at 51–60 dB. Gated, they are byte-identical.
  - A flat IDR QP offset was tried first: −3 helped octava by 12% but cost
    `post` 8%.

  Together with the B offset (QP +4 instead of +2; the guard's slack stays
  0.001: at 0.0015 `particles` fails qbench's delivered-PSNR gate),
  BD-rate (SSIM) at GOP 60 with B-frames:

  | launch | tutorial | shader | post | tern | octava (paper texture) | hill (grain) | mean |
  |---|---|---|---|---|---|---|---|
  | −0.2% | −2.3% | −7.7% | −0.6% | −3.5% | −20.2% | +0.1% | −4.9% |

  At the pipeline's settings (QP 23, SSIM floor, 20-frame chunks), octava
  goes from 31.1 MB / SSIM 0.988 to 24.4 MB / 0.982. x264 CRF 20 is
  8.4 MB / 0.986: it puts I frames at QP 14 and B frames at QP 26 (macroblock
  tree), and gains 14% from partitions and 10% from extra references.
- **SSIM floor bitrate ceiling.** The ceiling is a running average of P and B
  picture bytes against 25 Mbit/s at 1920×1080. It scales with frame area
  (never below a 640×360 frame's share) and with `fps`.
  - Above the ceiling, the floor stops lowering the QP and gives back what it
    took. Texture whose SSIM rises slowly with bits (re-drawn film grain,
    paper, halftone) could otherwise take the floor's full 8-QP drop.
  - With `ceiling_raise` (and `qp` ≥ 20), it then raises the QP up to that
    much above `qp` until the average falls under 80% of the ceiling.
    anikit-video sets 4 for films of 10 s or more.
  - Short clips keep their bursts. A chunk of 20–60 frames can't tell a burst
    from a film over budget, and raising always throttled the one-second
    35 Mbit/s burst in `particles`, failing qbench's delivered-PSNR gate.

  | clip (pipeline settings) | before tonight | now | x264 |
  |---|---|---|---|
  | octava, 13.5 s @ 24 fps | 31.1 MB / SSIM 0.988 | 23.8 MB / 0.982 | 8.4 MB / 0.986 (CRF 20) |
  | hill, 11.9 s @ 24 fps | 33.7 MB / 0.983 | 32.4 MB / 0.982 | 20.3 MB / 0.980 (CRF 21) |
  | e-data, 30 s @ 30 fps with `post.grain` | 149.3 MB | 97.6 MB | |
  | demo/data, 30 s (pack-ice shader) | 88.5 MB (main's `final.mp4`) | 80.5 MB | |

  Octava and hill run at 14–19 Mbit/s, under the ceiling, so their gains come
  from the IDR lookahead and the B offset. The ceiling only acts on films like
  e-data, where QP 23 alone costs ~26 Mbit/s of re-drawn grain; demo/data
  (21 Mbit/s) is under it. On octava, the
  static paper texture is unchanged: the IDR lookahead codes it once at the
  lower QP, and it keeps about 90% of the source's horizontal-gradient energy
  (x264: 100%).
- **8×8 transform.** For inter macroblocks, used when the residual's 8×8
  Hadamard cost is below its 4×4 SATD (x264's rule, SSE2). It includes CABAC
  category-5 residuals, `transform_size_8x8_flag` (coded as 0 for I4×4
  macroblocks), and no luma deblocking inside 8×8 blocks. Smooth macroblocks
  under AQ keep the 4×4 transform: its wide dead zone is what stops them
  coding noise. The SPS is profile 100 and the `avcC` carries the High-profile
  fields.

Everything above decodes bit-exactly in ffmpeg (tests in `tests/decode.rs` and
`tests/mp4.rs`, with and without B-frames, deblocking and partitions).

**qbench `--gate`** (the default render path, delivered quality checked
against the baseline) passes. File sizes against the encoder before these
changes, at equal or better delivered SSIM:

| spec | ui | text | explainer | backdrop | particles | launch | scene3d | shader-hero | shader | post | audio |
|---|---|---|---|---|---|---|---|---|---|---|---|
| size | −4.4% | −23.4% | −19.1% | −3.7% | −7.5% | −10.2% | −6.2% | −10.3% | −22.0% | −0.8% | −9.8% |

(The backdrop and shader figures are from before the smooth-AQ fix to the 8×8
transform, which makes both smaller.)

**Speed.** Encode instructions for 30 frames of launch at QP 23 (callgrind,
five repetitions) were 13.8G before these changes and are 15.7G now (+13%).
The stronger search costs more; B pictures and the direct early exit win part
of it back.

**Tried and rejected** (BD-rate at equal SSIM):
- An intra penalty in P frames: helps tern, costs 37% on post.
- An early skip trial: +12% mean.
- IDR at QP −3 for the whole frame (see the IDR lookahead instead).
- The same lookahead for P pictures (hold each P one frame): tern +3–4%.
- Temporal AQ (QP from the inter/intra cost ratio): no gain on octava or hill.
- x264-style variance AQ (lower QP on low-variance blocks). On hill it keeps
  more dark grain: high-frequency energy in dark grain blocks goes from 0.88
  to 0.94 of the source's. x264 CRF 21 keeps 1.00, and 0.88 with psy off.
  But BD-rate (SSIM) gets worse on everything except octava: post +15%,
  launch +5%. SSIM does not reward keeping grain, so this needs a perceptual
  metric before it can be a default.
- A narrower inter dead zone (1/4, 1/3) or no decimation: grain energy +1%.
- SATD instead of SAD for sub-pel candidates: 0%.
- A quick B search with no hexagon: +5.9%.
- A larger P quick-exit bound: +3.4%.
- A quarter-resolution lookahead and a measured-cost feedback loop for the
  B-or-P decision: neither separated tern, where B loses, from the clips where
  it wins.

**Known limits.**
- tern loses with B pictures (+8%): its shader backdrop evolves, so a P
  picture two frames from its reference costs much more.
- HEVC/AV1 are not implemented.

## Limitations and next steps

- One reference; B pictures are 16×16 (L0/L1/bi/direct) and never referenced;
  P partitions are opt-in.
- There is no rate control, only constant QP (CRF-like). A `target_bitrate` mode
  would need a per-frame QP loop.
- Brightness fades are coded as intra. Weighted prediction (as x264 uses) would
  make them nearly free.
- No intra 8×8 (the 8×8 transform is used for inter macroblocks only).
