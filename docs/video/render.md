# Anikit Video: renderer (`crates/anikit-video/src/render/`)

The renderer turns a `dl::DisplayList` into pixels on **vello_cpu 0.4**. It then converts them to YUV 4:2:0 for the encoder, and works out which macroblocks changed.

## API

```rust
use anikit_video::render::{Assets, Renderer, YuvFrame, MbMap};

let mut assets = Assets::default();
assets.images.insert_encoded(ImageId(1), &png_bytes)?;   // PNG / JPEG / WebP, decoded once
let font = assets.fonts.add(Arc::new(ttf_bytes), 0);     // FontStore, filled by `text`
let assets = Arc::new(assets);                           // shared read-only by every worker

let mut r = Renderer::new(assets.clone());               // one per worker thread
let rgba = r.render(&list);                              // &Pixmap, premultiplied RGBA8
r.render_yuv(&list, &mut yuv);                           // + BT.709 limited-range 4:2:0
let update = r.render_yuv_incremental(&list, &mut yuv, &mut changed); // dirty regions only
```

### Op semantics

| Op | Notes |
|---|---|
| `Fill` / `Stroke` | Non-zero and even-odd fills. Strokes support caps, joins, miter limit, dashes (odd dash lists repeat, SVG-style) and `trim` (trimmed by arc length to `[start, end]`). |
| `RoundRect` | `radius` is clamped to half the shorter side; radius 0 uses vello's `fill_rect`. |
| `Image` | `dst` is `[x, y, w, h]` in local coordinates. 1:1 images at integer positions are blitted directly. Axis-aligned scaled images that repeat across consecutive frames are resampled **once** with a tent filter (true area filtering when shrinking), then blitted. Anything else uses vello's bilinear filter. |
| `Paint::Image` | `transform` maps local coordinates → image pixels. It **tiles** (repeat); `Op::Image` draws once. |
| `Glyphs` | Drawn with the face instance in `FontStore` (`insert_face(id, Face { data, coords, embolden, skew })`): variable-font `coords` (F2Dot14 bits), synthetic bold (2% of the em per side) and oblique skew (degrees). Outlines are cached per (font, glyph). Each run is drawn as **one** path. Solid, translation-only runs are cached as bitmaps, keyed by content and quarter-pixel phase, and blitted. Colour fonts (COLR/CBDT/sbix) go through vello's glyph runs. |
| `PushLayer` | A layer with no filter and no `cache_key` becomes a native vello layer, or nothing at all when opacity is 1, the blend is normal and there is no clip. A layer with filters or a `cache_key` is rendered offscreen, sized to its filter-expanded bounds ∩ view ∩ clip, filtered and composited. A `cache_key` keeps that bitmap while the key repeats. A filtered layer without one is cached automatically by a hash of its content, filters and transform (layers drawing images excepted), so a static glow or blur is filtered once while its opacity animates. |
| `Filter::Blur(σ)` | Gaussian σ, as in CSS. Three variance-matched box passes; every pass is a vectorised row operation (horizontal passes run on a transposed copy). Decimated 2/4/8× for σ ≥ 4/8/24. |
| `Filter::Shadow` | σ blur, offset, colour. **Fast path:** when the layer's alpha is a single axis-aligned `RoundRect` (a card, optionally with content inside an opaque card), the shadow is a blurred sprite cached per size, radius, σ and colour, and drawn at an integer offset. No offscreen pass is needed. |
| `Filter::Glow` | Blurred alpha × intensity, tinted, drawn behind. |
| `Filter::ColorMatrix` | 4×5 matrix on unpremultiplied colour. |

### YUV and macroblock hints (for `ak264`)

- **`YuvFrame`:**
  - planar BT.709, limited range;
  - padded to whole 16×16 macroblocks by edge replication, with strides that are multiples of 16.
- **Conversion:**
  - one fixed-point kernel for every path: an AVX2 build of it via `madd`/`hadd` intrinsics, an SSE4.1 build and a scalar build;
  - all three are **bit-identical** (tested);
  - error against the float reference is ≤ 0.5 LSB;
  - white maps to 235/128/128.
- **Macroblock diffs:**
  - `diff_mb(prev, cur)` is the exact changed-macroblock map (luma plus both chroma planes).
  - `diff::diff_mb_ops(prev_dl, cur_dl)` is a conservative map computed from the display lists alone.

### Dynamic images

`Renderer::set_image_source(Arc<dyn DynamicImages>)` supplies images that are created while rendering, for ids not found in `Assets::images` (for example component images at ids ≥ `0x4000_0000` through `Host::pixmap`). The trait is `fn get(&self, ImageId) -> Option<Arc<Pixmap>>`, and any `Fn(ImageId) -> Option<Arc<Pixmap>> + Send + Sync` implements it. Return the same `Arc` while the pixels are unchanged. `Renderer::set_frame_image(id, Arc<Pixmap>)` overrides `Assets::images` for the following renders. Use it for decoded video frames and other per-frame textures; `clear_frame_image(id)` removes the override. Incremental renders track the identity of each `Arc`, so only the macroblocks under the image are re-rendered when a new frame is set.

### Full-frame backdrops

When a top-level `Op::Image` is opaque, axis-aligned and covers the whole frame (mesh and aurora fields, full-bleed photos), everything before it is hidden, so it is skipped. The image is bilinearly upscaled straight into the frame by a separable kernel: each source row is interpolated horizontally once, then each output row is a vectorised vertical lerp. vello then draws only the ops above it, with `TargetInit::SrcOver`, so there is no clear pass and no vello image sampling. Incremental regions use the same kernel. The result is within 3 LSB of vello's own bilinear filter (tested).

### Smooth backdrops

A leading run of top-level ops with no visible edge inside the frame is rendered by vello at ¼ resolution, cached by content hash, and upscaled with the same kernel. The run may contain:
- full-frame gradient or solid fills;
- low-res images upscaled ≥ 4×;
- radial "glows" whose gradient reaches zero alpha before their path edge (checked geometrically);
- gradients whose stops are at least 24 px apart on screen.

This applies once the run paints a frame's worth of pixels with a gradient or image. The launch template's background (a full-frame radial plus two moving glows) went from 118 to about 195 fps on one thread at 1080p (8-bit intermediate rows; the vertical lerp runs in 16-bit lanes). The result is within 6 LSB of a full-resolution render (tested).

### Fused backdrop → YUV

When `render_yuv` has a backdrop and the ops drawn over it fit in a macroblock-aligned box covering at most 60% of the frame, the low-res backdrop is converted to YUV 4:4:4 once and its planes are upscaled straight into the YUV frame: 1.5 bytes per pixel, with no full-frame RGBA. Odd frame sizes get edge-corrected chroma. Only the box goes through vello. When every top-level layer over the backdrop blends normally (the usual case), the box is drawn on a transparent canvas and composited source-over directly in YUV space (`Y = (Y_bg − 16)(1 − α) + 16 + luma(src)`, chroma per 2×2 block, fully transparent spans skipped), so no RGBA backdrop is made at all, however large the box. Otherwise the box is seeded with the upscaled backdrop in RGBA, but only while it covers at most 60% of the frame. The full RGBA frame is never written, so `frame()` is stale afterwards; call `render()` when you need RGBA. The output is within 2 LSB (luma) and 4 LSB (chroma) of `render()` followed by RGBA→YUV conversion (tested). On the launch template (restarted, slower VM) render + YUV goes from 6.4 ms unfused to 4.9 ms.

### Incremental frames

**Op alignment (insertion-tolerant):** ops are matched by content hash, in order, with a 24-op look-ahead resync, so an inserted or removed op marks only its own bounds. An unmatched layer marker, for example a clip that moved by 1 px or a wipe that widened, marks its whole subtree's old and new bounds (leaf bounds already include enclosing filter margins) instead of forcing a full frame. Any pixel outside the marks is covered by the same ops in the same order. Full-frame incremental renders take the fused backdrop → YUV path when it applies. On the launch template: Full 421 → 312 of 868 frames and Partial 27 → 136. The remaining Full frames are ones where the animated background glows really change every pixel.


`render_yuv_incremental` is the fastest path for real videos. Each worker keeps its previous frame and does the following:

1. Fingerprint every op: a content hash, plus device bounds grown by the filters that enclose it.
2. Compare the fingerprints index by index with the previous frame.
3. Mark the old and new bounds of every op that differs.
4. Cluster the dirty macroblocks into at most 8 rectangles.
5. Re-render only those rectangles, skipping ops whose bounds miss each one.
6. Re-convert only those rectangles to YUV.
7. Compare their macroblocks with the previous YUV, which gives the **exact** changed-macroblock map.

Fallbacks and special cases:
- A change in layer structure, or dirty area above 60% of the frame, falls back to a full render (still with an exact map, through a double-buffered diff).
- An unchanged frame costs only the fingerprinting.
- A partial render matches a full render to within 1 LSB on rare pixels: floating-point rounding at a shifted tile origin.

## Performance (measured, this 4-core machine, 1080p)

### Full frames

Source: `cargo run --release -p anikit-video --example render_bench`, 240 frames.

| Scene | 1 thread render | 1 thread render + YUV | 4 threads render + YUV |
|---|---|---|---|
| Busy: gradient background, 30 cards, 400 moving dots, 10 trimming strokes, 240 glyphs; everything moves | 215 fps | 175–189 fps | 640–710 fps |
| Busy + 30 drop-shadowed cards | 137 fps | 120 fps | 445–480 fps |

### Incremental

Source: same benchmark, `render_yuv_incremental`. Frame rates include the exact macroblock map.

| Scene | Changed macroblocks | 1 thread | 4 threads |
|---|---|---|---|
| Busy (everything moves) | 30% | 168 fps | 560–610 fps |
| Busy + 30 shadowed cards | 31% | 115 fps | 405–425 fps |
| **Mostly static** (4 cards and 1 stroke move, rest static; typical UI or explainer shot) | 4.3% | **350 fps** (vs 131 full) | **~1000 fps** (vs ~460 full) |

Frame-parallel runs use one `Renderer` per worker over contiguous chunks of frames, which keeps caches warm and makes incremental frames possible.

### Cost per op kind

Source: `cargo run --release -p anikit-video --example parts_bench`. Single thread, 1080p, whole frame including clear and pack.

| Content | ms/frame |
|---|---|
| Empty frame (clear + pack) | 0.73 |
| Full-frame solid / linear gradient | 0.85 / 2.3 |
| 30 cards 280×170 / 400 small circles | 1.16 / 1.41 |
| Image 800×600 → 1200×900 (static, resampled once) / 1:1 | 1.40 / 1.22 |
| Text: 1500 glyphs @ 24 px / 360 glyphs @ 96 px | 1.36 / 1.77 |
| Blur layer 400×300, σ 8 / full-frame blur, σ 20 | 1.19 / 8.2 |
| YUV conversion alone (AVX2) | 0.75 |

### End to end (templates, after the speed phase)

`anikit-video render` at 1080p30 with the `ak264` encoder, on a restarted VM that measures about 1.5–2× slower than the one used for the tables above:

| Template | Frames | Wall time | Throughput |
|---|---|---|---|
| launch | 752 | 2.38 s | 315 fps (was 167 before the backdrop paths) |
| testimonial | 699 | 2.11 s | 332 fps |
| tutorial | 818 | 2.04 s | 400 fps |
| logo-reveal | 120 | 0.51 s | 235 fps |

Single-thread `render_yuv` on these templates now costs 4–5 ms per frame. Most of that is the 1080p plane writes, the overlay boxes and text; the background gradients are nearly free.

### What moved the numbers

Before → after, same benchmarks:

| Change | Before | After |
|---|---|---|
| AVX2 YUV kernel | 1.94 ms | 0.75 ms |
| Blur (row passes + transposes + decimation; filters compiled for AVX2) | 48 ms full-frame, 3.5 ms per 400×300 layer | 8.2 ms, 1.2 ms |
| Moving shadowed cards: cached shadow sprites | 25 fps | 137 fps |
| Text: run bitmaps | 4.9 ms / 1500 glyphs | 1.36 ms |
| Scaled static image: resample once | 5.7 ms | 1.4 ms |
| Mostly-static shot: incremental | 131 fps | 350 fps (1 thread) |

## Tests

- **`tests/render_golden.rs`:** golden images for every op (references in `tests/golden/`; regenerate with `AK_BLESS=1`), with a tolerant compare: mean ≤ 0.5, max ≤ 48. The same file covers:
  - layer-cache hits match uncached output;
  - the changed-macroblock map is correct, and the display-list map is a superset of it;
  - cached image resampling matches direct sampling;
  - incremental frames match full renders, with an exact changed-macroblock map, `Unchanged` on repeated frames and `Full` on a layer-structure change.
- **Unit tests:**
  - YUV accuracy against the float BT.709 reference, and agreement across kernels and regions;
  - blur profile against a true Gaussian;
  - trim;
  - resampling.

## End-to-end pipeline (spec → MP4), measured

`anikit-video render` with ak264 (the MP4 default) runs **fused**: frames are split into GOP chunks (`gop = clamp(n / (3 × threads), 30, 300)`), and each worker evaluates, renders incrementally (`render_yuv_incremental`), copies only changed macroblocks into its chunk buffer and encodes, with the exact change map as the encoder hint. Spans that `Evaluator::is_static` proves identical are **held**: no eval, no render, all-skip P frames. `--profile` prints the per-stage breakdown.

4-core container, 1080p, wall clock including spec compile and the MP4 mux (`render --json`, `render_fps`), 2026-11-17 02:35 UTC:

| Content | Frames | ak264 (fused) | x264 veryfast (ffmpeg pipe) | Frames skipped |
|---|---|---|---|---|
| `new launch` template (benchmark of record: mesh backdrop, devices, chart, confetti) | 881 @ 30 | **125 fps** (7.0 s, 7.1 MB) | 75 fps (11.8 s, 6.3 MB) | 0 |
| same, without the animated mesh backdrop | 881 @ 30 | **333 fps** (2.6 s) | 122 fps | 83 |
| `docs/video/examples/showcase.json` | 222 @ 30 | **573 fps** (0.39 s) | 135 fps | 23 |
| title + chart slides (mostly holds) | 570 @ 60 | **2197 fps** (0.26 s) | 214 fps | 392 |

Before the fused path (render batches → separate encoder thread), the benchmark of record ran at 109 fps.

**After the backdrop and settle fixes from RENDER, COMPONENTS and AGENTS (merged 02:40 UTC):** the current `new launch` template renders 901 frames in 2.55 s = **353 fps** with ak264. 404 frames are skipped as Unchanged (eval 0.5 / render 6.2 / encode 4.7 CPU ms per frame). The older launch spec above now runs at 268 fps.

`--profile` on the benchmark of record (CPU ms per frame, summed over 4 threads): eval 0.5, render 17–18 (all 881 frames Full), copy 0.4, encode 10–12. Two things keep it from being incremental:

1. The animated `mesh-gradient` backdrop changes every pixel of every frame (a keyframe blend), so no frame is Unchanged or Partial. Removing it gives 333 fps; `speed: 0` gives 230 fps.
2. Without the mesh, 770 / 881 frames are still Full, because any changed `PushLayer` (mostly clip layers inside device and toast components) forces a full frame (`examples/dl_churn.rs` counts them).

Correctness: fused outputs decode with ffmpeg and match the x264 path at 50–54 dB PSNR (`tests/pipeline_e2e.rs` checks pixels across held, partial and full frames and GOP boundaries).
