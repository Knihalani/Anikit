# Changelog

## Afternoon session (2026-11-16): three looks, faster everything

Benchmark numbers come from `packages/anikit/scripts/` and `docs/perf-notes.md` ("Fourth round (a)" and "(b)"); the machine was shared between two optimisation passes and renders, so figures are ranges over interleaved runs.

### The film

`film/film.mjs` + `film/world.mjs`: **ONE WORLD, THREE LOOKS**, 21.4 s at 1080p60. One continuous FPV flight through a procedural valley (a mirror lake, a saddle, a small city, a river canyon, the last ridge and a long lake under snowy peaks) that keeps switching how it is drawn: photographic golden hour (baked sun, shadows and occlusion, long tree shadows, mirror water, a painted cloud sky), neon (contour lines, lit windows, light trails, a synthwave sky) and comic (cel palette, ink from the depth buffer, halftone, speed lines). Six different transitions (a front from the horizon, a scan glitch, an ink burst, halftone dots, a slash of light, a reveal from the lens); the hook and the ending split the frame into all three looks. No captions, no music. The world bakes once (about a minute, cached in `film/.cache`); the film renders in about 7 minutes on 3 workers.

### Video creation

- `Renderer3D.depth(scene?)`: the depth buffer as distances (or NDC z) for depth-driven post effects.
- Material `fog` (0–1): sky domes and painted backdrops can skip the distance fog.
- Material `textureFilter: 'aniso'`: anisotropic filtering keeps ground sharp at grazing angles.
- `flightPath({ bankWindow })` and a soft bank limit: banking eases towards `maxBank` instead of clipping, so S-bends no longer flip the camera in a frame (peak roll rate in the film 511 → 56 °/s).
- Python: `Material(fog=, texture_filter=)`, `flight_path(bank_window=)`.
- Faster renderer (`scripts/bench-terrain.mjs`, new): a 1080p terrain frame 648 → ~480 ms, a two-look transition frame ~945 → ~705 ms, a sky dome 128 → 67 ms, a textured scene 138 → 114 ms, all bit-identical (a faster bilinear sampler, an unlit fast path, skipping triangles that cover no pixel centre). Untextured scenes measured 3–5% slower; not yet bisected (see the perf notes).

### Web animation

`scripts/bench-create.mjs`, `bench-dom.mjs`, `bench.mjs` (new cases), `bench-scroll.mjs` (new):

- Taking over a running multi-target animation (10k targets) 1.25–1.8 s → 41 ms (was O(n²)); killing a 5,000-step timeline 46 → ~1 ms.
- Creation: 10k separate `animate()` calls 66–72 → 43–50 ms; retained heap per target 5.5 → 2.4 KB; startup (import → ready) ~18 → ~15 ms.
- DOM frames: 500 boxes 0.58–0.60 → 0.37–0.50 ms, 5,000 boxes 5.7–8.7 → 4.3–5.3 ms (cheaper transform strings, cached style declarations).
- Colour strings on objects 2× faster (10k: 1.22 → 0.55 ms); unit strings ~1.7× faster.
- Fixed: killed multi-target animations leaked through the record table. `motionValue` no longer allocates per update.
- Still behind GSAP: 8-keyframe tracks at 50k (0.56–0.65×), unit strings on objects (~0.5×), creation cost.

## Overnight session (2026-11-15 → 16)

Everything below is on `claude/epic-ramanujan-qe7x4w`. Benchmark numbers come from the scripts named next to them (`packages/anikit/scripts/`, `crates/anikit-core/examples/`); `docs/perf-notes.md` has the before/after detail.

### Video creation

- **Renderer (`renderer3d`)**: point lights (up to 8), key-light shadow maps, a sky with a sun, ground fog, depth of field, depth-based camera motion blur (with `prevCamera` so previews match the final render), mirror floors, mipmapped textures, procedural glowing patterns (hex, dots, circuit), open tubes along paths (light trails), solid 3D text, per-instance frustum culling.
- **Film grade (`post`)**: god rays, FXAA (`aa`), half-resolution zoom blur, **lens flares** (`flares`: ghosts of the hottest lights mirrored through the centre, plus a halo), all animatable.
- **Impacts**: `shockwave` sends a refracting, chromatic ring out from the hit (`origin`, `shockSpeed`).
- **Choreography**: every scene has a **clock** (`draw({ clock })`, slowed by `timeScale`); `film.timeAt(scene, clock)` / `film.clockAt(scene, t)` land impacts on story beats in bullet time. `flightPath().apply(cam, s, roll, { shake })` adds a deterministic handheld wobble. `shatter()` breaks any mesh into shards.
- **CLI (`anikit-render`)**:
  - `--review` — contact sheet + pacing report;
  - `--profile` — where each frame's time goes, down to every 3D pass;
  - `--check film.mp4` — frame-to-frame pops, seams and freezes in a rendered video;
  - `--still t --watch` — re-render on save; `--still` now saves the *graded* frame;
  - `--doctor` — checks Node, canvas, ffmpeg, a fresh build;
  - `--init --template journey`, live progress with ETA across workers.
- **Guards**: a warning when `draw()` keeps creating meshes or instance buffers; a warning when `dist/` or the generated wasm is older than the sources.
- **Python**: the same renderer, grade and flight paths on numpy frames (point lights, shadows, sky, DOF, motion blur, tubes, patterns, half-res mirrors, `flares`, `shock`, `shake`), and `video.VideoWriter` to encode MP4s through ffmpeg.
- **Templates**: `journey` (3D fly-through) and a `film` starter that follows the "colour in light" playbook.

### Web animation

- **Reactive values**: `motionValue()` (get/set/on, `to()`, `spring()` keeping momentum, `map()`), `derived()`, `bindValues()` to styles, `scrollProgress()`.
- **`marquee()`**: seamless endless rows/columns, pause on hover, scroll boost, eased speed changes.
- **`magnetic()`** buttons and **`parallax()`** layers.
- Earlier in the session: presence/exit animations, FLIP enter/leave, View Transitions, native scroll timelines, text masks, gestures (pinch/rotate/pan), OKLab colour mixing, reduced-motion "fade" mode, aligned motion paths.
- `examples/interactions.html` shows the new pieces; the landing page has a feature ticker (marquee) and magnetic buttons.

### Performance

- **Plain-object tweens vs GSAP** (`scripts/bench.mjs`, 100 warm-up frames): 3 numbers 1.3–2.9× faster, colour strings 1.1–1.6× faster; 8-keyframe tracks went from 0.5–0.8× to 0.8–1.1× (still slightly slower above 1k objects). Generated per-shape writers and flush loops, memoised easing and colour conversions, a timing "lane" for sibling tweens, fast paths for simple from/to and evenly spaced keyframed tweens, and a contiguous first-output array the writers read sequentially. Colour strings at 10k objects run ~5% slower than before the last round (still faster than GSAP); see `docs/perf-notes.md`.
- **3D renderer** (wasm, 1080p): core rasterisation ~20% faster with bit-identical output (fully covered 4-pixel runs shaded out of line, SIMD depth test: 900 towers 76 → 64 ms, 6000 cubes 218 → 159 ms); camera motion blur 143 → ~55 ms (four taps per SIMD step, per-tile setup, SIMD reprojection, plus a smoothing pass that removes grain in long smears), depth of field −20%, half-resolution mirror reflections ~47 → ~20 ms, sky 23 → 10 ms (half-res grid), point lights culled per instance and per pixel group (~35 → ~20 ms), 3D text 500k → 182k triangles (flat runs merged, watertight-tested).
- **Film grade** (wasm, 1080p, the full grade of `film/film.mjs`): 147 → ~84 ms (SIMD pixel unpack and 16-byte group loads, fixed-point sampling, FXAA from two saved rows instead of a frame copy, span copies and slice walks in zoom blur, row-wise box blurs, a grain tile).
- New benches: `scripts/bench-post.mjs` (grade per effect), a film-like all-passes frame with a per-pass split in `scripts/bench-raster.mjs`, `examples/post3d_bench.rs`, `examples/post_prof.rs`.

### Developer and agent experience

- `docs/recipes.md` "A 3D fly-through film" with the pitfalls this film hit (bullet-time beats, roll unwinding, path ends, metal vs horizon, pulse wash).
- Every export documented with a type-checked example (`npm run typecheck`), JSDoc on every interface member, option-key tables with "did you mean" warnings, `llms.txt` / `llms-full.txt` regenerated.
- CI runs the doc gates, the native examples build and a render smoke test of both templates.

### The film

`film/film.mjs` — **VELOCITY**, 20.4 s at 1080p60 (1,227 frames), one continuous 3D flight: ignition in a dark ring tunnel whose circuits switch on with a flash, a neon city on a mirror floor with streams of light and a barrel roll through a gap, chrome gates, a black-chrome knot that shatters in bullet time with sparks and a shockwave, a vortex of light cubes, and the wordmark in chrome under the sun with a travelling sheen. No captions, no music. Pacing is a story-time warp over the choreography; every frame-to-frame spike in `anikit-render --check` is a placed flash, impact or fly-through. Renders in about 4 minutes on 3 workers.
