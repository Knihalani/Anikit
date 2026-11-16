# Anikit Video: architecture

Goal: the fastest, most capable and easiest way for **agents** to make great videos in code.

## Why this design (research summary)

Existing tools draw each frame in a browser.

| Tool | How it renders | Cost |
|---|---|---|
| Remotion | Lays out a React DOM per frame in headless Chrome, then screenshots it | The browser's layout, paint and screenshot dominate |
| HyperFrames | Renders HTML plus GSAP in headless Chrome and seeks every frame | Same as Remotion; one review timed a 30 s 1080p render at about 3 minutes |
| Motion Canvas / Revideo | Draw on a canvas, but still inside a browser | Browser overhead remains |
| Manim | Cairo/OpenGL from Python | Slow, and no agent tooling |

Newer native Rust engines exist (wavyte, EffectCraft/FilmCraft), but none is agent-first.

**Our edge:**
- No browser.
- A native Rust renderer, SIMD and multithreaded.
- Frames are pure functions of `(spec, t)`, so they render in parallel across every core.
- Rendering writes YUV directly, with a chunked parallel encode.
- A declarative JSON spec that an agent can write in one shot, with schema validation and clear errors.

**Language choice:**
- **Rust core** for raw speed and SIMD, with no GC pauses. It reuses `anikit-core` for easings, springs, colour, paths and morph, and runs natively, in wasm (browser preview) and from Python.
- **The authoring surface is data**, the JSON spec, plus thin builders:
  - TypeScript: `anikit/video`, which produces the spec;
  - Python;
  - an MCP server.

Agents are best at writing structured data and TypeScript, and per-frame user code is the main thing that makes other tools slow. An escape hatch for custom per-frame logic exists, but it's never required.

The environment has `ffmpeg` with libx264, libx265, libsvtav1, libvpx-vp9, aac and opus. Machines have 4 cores.


## Foundation decisions (measured on this 4-core machine, 1080p)

| Choice | Measured | Decision |
|---|---|---|
| **Rasteriser** | Typical motion-graphics frame (gradient, 30 cards, 400 glyph-sized shapes, 10 strokes): tiny-skia 53 fps on one thread / 200 fps frame-parallel; **vello_cpu 250 fps on one thread / 935 fps frame-parallel** (sparse strips, SIMD) | **vello_cpu** 0.4 (Linebender) behind our display list. We may vendor and patch it. |
| **Encoder** | x264 on our content: ultrafast 330 fps (46 MB), veryfast 141 fps (6 MB), medium 71 fps; OpenH264 105–122 fps (11 MB, 43 dB). The full pipeline through ffmpeg (RGBA pipe + swscale + x264) is only 45–77 fps | **The encoder is the wall** (rendering is 10× faster), so we build our own: **`ak264`**, a renderer-aware H.264 encoder (see below). x264 via ffmpeg stays as the "max compression" option and the reference. |
| **Colour conversion** | ffmpeg swscale RGBA → YUV is single-threaded and costs about 35% of the pipeline | Fused SIMD RGBA → YUV420 inside the render workers |
| **Text** | n/a | **parley** (layout, Linebender) + **skrifa** outlines + **harfrust** shaping, drawn as vello_cpu glyph runs; bundled Inter, Inter Display and JetBrains Mono |
| **Layout** | n/a | **taffy** (flexbox and grid), so agents can lay out with CSS-like boxes they already know |
| **Images, audio, mux** | n/a | `image` / zune for images; symphonia for audio decode plus our own mixer; ffmpeg only for final muxing and AAC/Opus (cheap), with an optional pure-Rust MP4 muxer |

### `ak264`: why it can be orders of magnitude faster

General encoders spend their time discovering what changed (motion search, mode decisions) across the whole frame. Our renderer already knows:
- which 16×16 macroblocks changed since the previous frame (an exact diff of the rendered frames, which is cheap, or the display-list bounding boxes);
- often *how* they moved (layer transforms give exact motion vectors).

**Bitstream:** H.264 High or Main profile, Annex-B, decodable everywhere.

**Per macroblock:**
- **Unchanged:** P_Skip, at zero cost. Mind the predicted-MV rule: force MV 0 by choosing neighbours, or code P_L0_16x16 with MV 0 and no residual.
- **Moved by a known integer translation:** P_L0_16x16 with that MV plus a small residual.
- **Otherwise:** a fast intra mode decision (I16x16 / I4x4, DC/V/H/plane) with CAVLC residual.

**Constant QP:** quality is set by QP (CRF-like), with deblocking on or off (measured).

**Parallelism:** fully static frames become a single all-skip P slice, in nanoseconds. The video is split into GOP chunks, each starting with an IDR, encoded on all cores, and the Annex-B streams are concatenated.

**Target:** at least 10× x264 veryfast on typical motion graphics, at matched PSNR (≥ 40 dB). Verify every output decodes with ffmpeg, and report PSNR.

## Crate and package layout

| Path | Module | Owner (session) |
|---|---|---|
| `crates/anikit-video/src/dl.rs` | **Display list**: the contract between eval and render (already written) | parent; changes by agreement |
| `crates/ak264/` | **Renderer-aware H.264 encoder**, chunk-parallel, plus the Annex-B to MP4 path | ENCODER |
| `src/render/` | 2D rasteriser (**vello_cpu**): paths (analytic AA), strokes (joins, caps, dashes, trim), gradients, images, blend modes, layers, clips, filters (blur, shadow, glow, colour matrix), layer cache, tile-parallel, direct-to-YUV output | RENDER |
| `src/text/` | Fonts (TTF/OTF, bundled Inter / Inter Display / JetBrains Mono, system), shaping (rustybuzz), layout (wrap, align, rich spans), glyph outline cache and atlas → `Op::Glyphs`; kinetic text helpers (split by char/word/line) | TEXT |
| `src/spec/`, `src/eval/` | JSON spec types (serde), schema generation, validation with "did you mean" errors, timeline (scenes, transitions, keyframes, easings, springs, stagger, expressions), evaluation `(spec, t)` → `DisplayList` | SPEC |
| `src/pipeline/`, `src/bin/anikit-video.rs`, `src/audio/` | Frame scheduling (rayon), dedupe of identical frames, RGBA → YUV, encoders (x264, x265, AV1, VP9, ProRes, GIF, PNG sequence) via ffmpeg pipes, with chunked parallel encode and concat; audio mixing (music, SFX, ducking, fades), beat detection for sync, captions (SRT, word timing → burn-in); CLI commands: `render` / `still` / `sheet` / `check` | PIPELINE |
| `src/components/` | High-level blocks that expand to display-list ops: code blocks (syntax highlighting, typing, diff), charts (bar, line, pie, number counters), device mockups (phone, laptop, browser window), particles, 3D models via `anikit-core` raster, lower thirds, progress bars, logos, backgrounds (gradients, noise, grids) | COMPONENTS |
| `packages/anikit/src/video/`, `crates/anikit-py` video builders, `src/templates/`, MCP server, `docs/video/*.md`, `prompts/` | Agent layer: TS and Python builders, templates (launch, product demo, feature announcement, changelog, social teaser 9:16, data story, testimonial, tutorial), brand kits, a prompt library ("launch video generator" and more), an MCP server (`anikit-video mcp`), `AGENTS.md` and skills | AGENTS |

## Contracts

- **Display list:** `dl.rs`. Add variants only by agreement, announced to the other sessions.
- **`render::render(&DisplayList, &mut Frame)`:** `Frame` holds RGBA8 or planar YUV420 at the list's size. Rendering is thread-safe, and each worker owns its scratch memory.
- **`text::Fonts`:** loaded once and shared read-only. `layout(&TextSpec) -> Vec<Op>`.
- **`eval::Evaluator::new(spec)`** is preprocessing done once (fonts, images, text layout cache). Then `eval(t) -> DisplayList`, which must be pure and must not allocate on a steady state if possible.
- **Spec:** JSON in `docs/video/spec.md`, with a schema in `docs/video/spec.schema.json`. A spec can be authored by hand, by the TS or Python builders, or from a template plus parameters.

## Token efficiency (a user requirement)

An agent should spend as few tokens as possible to get a great video.

- **Shorthand spec:** `"animate": "rise"`, string durations, colour tokens, size presets. A good scene is 5–10 lines of JSON.
- **Templates:** filled from a few essential fields.
- **Compact tool output:** one line per issue; file paths rather than base64.
- **Short docs:** a 1-page cheat sheet first.

Track the cost of "make a launch video" end to end (prompt + spec + tool I/O) as a metric.

## Performance targets

Measured on this 4-core machine, honestly, against Remotion or HyperFrames installed locally where possible.

- 1080p60 motion-graphics video (text, shapes, images, transitions): render faster than real time, at 60 fps or more end to end including encode.
- First frame preview (`still`) in under 100 ms.
- 10× or more faster than a browser-based renderer on the same composition.

## Phases (UTC)

| Phase | Time |
|---|---|
| Build | 21:45 → 02:30 |
| Speed | 02:30 → 05:15 |
| Three flagship videos (new ideas) | 05:15 → 07:15 |
| Integration checkpoints | about every 90 minutes; the parent merges with the full gates |
