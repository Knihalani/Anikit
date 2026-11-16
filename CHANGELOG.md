# Changelog

## Gaussian splats

- `splats(canvas, set)`: real-time 3D Gaussian splatting on WebGL2. Loads `.ply` (3DGS training output or coloured point clouds) and `.splat`; procedural sources `splats.fromPoints`, `fromMesh`, `fromText`, `fromSDF`, `merge`. Splats are packed into one `RGBA32UI` texture (32 bytes each), projected to 2D covariances in the vertex shader and blended premultiplied back to front.
- Depth sort: a 16-bit counting sort in the Rust core (`gsplat::Sorter`), run in a Web Worker that instantiates the shared WebAssembly module (main-thread fallback), only when the view or morph moves past a threshold. About 1.5 ms for 100k splats in headless Chromium.
- GPU effects driven by anikit tweens: `morph(otherSet)` (staggered, any counts), `explode`, `swirl`, `dissolve`, `reveal`, `fx.size`/`fx.opacity`; `camera` and `transform` are plain animatable objects. `draw(view, proj)` renders inside another WebGL2 scene.
- `examples/splats/` (procedural, no downloads; open your own `.ply`/`.splat`), `scripts/bench-splats.mjs`.

## Eighth round (2026-11-16, to 18:05 UTC)

Details and interleaved before/after runs are in `docs/perf-notes.md` under "Eighth round (core)", "(DOM)" and "(video)".

### Core
- **Creation is about 2× cheaper.** The core can now create an animation from borrowed data with one exact allocation, and creates many in one call (`ak_anim_create_many`). Plain objects and numbers take a fast path, timeline steps included.
  - One `animate()` over 10k objects: 22.6–27.2 → 11.5–14.5 ms.
  - 10k separate calls: 31.5–37.0 → 24.7–30.3 ms.
  - 8 keyframes: ~13 → ~7 ms.
  - GSAP is still faster at creation (3.3–4.1 ms for one call over 10k objects).
- **Per frame**, `bench.mjs` stays ahead of GSAP in all 12 cases (1.08–4.21×).
- **Bugs fixed:**
  - handles of generation 2048 or more became negative int32 in JS, so those animations never rendered (hit after about 2k create/kill cycles on one slot);
  - `animate([o, o])` killed itself at build but kept running.
- **New tests:** a fuzz test that compares every fast path, frame for frame, against the slow path and a reference timing model.

### DOM and interaction
- **One scroll read per event per scroller.** Per scroll event:

  | Case | Before | After |
  |---|---|---|
  | 200 `scroll()` links | 0.37–0.39 ms | 0.017–0.020 ms |
  | 200 scrubbed triggers | 0.15–0.22 ms | 0.010–0.024 ms (GSAP ScrollTrigger: 0.015–0.029 ms) |
  | Smoothed modes | 0.24–0.63 ms | 0.012–0.019 ms |

- `spring()` keeps one `onUpdate` per simulation.
- No allocation on drag, gesture or world pointer moves.
- **Bugs fixed:**
  - pinning wiped the element's inline styles;
  - refreshing during a pin shifted triggers;
  - `scroll()` and `inView()` were not tracked by `context()`;
  - draggable multi-touch hijacking, and kill/revert during a drag;
  - nested and scrolled `flip`s, and overlapping `flip`s;
  - `splitText` with markup across lines, mixed bidi text and Arabic joining;
  - presence under rapid add/remove;
  - marquee under reduced motion;
  - `magnetic`, `tilt` and `bindValues` revert;
  - `world().dragging()` click handling.
- **New real-browser test runner:** `npm run test:browser`, 67 cases.

### Video and 3D
- **New APIs:**
  - `mesh.update()` and `tube(points, { into })` animate geometry in place (also in Python);
  - `free()` is idempotent. A double free used to free a slot that had been reused.
- **Speed**, with output bit-identical at even frame sizes:
  - the mirror pass skips fully covered rows: 82–87 → 65–68 ms;
  - FXAA: about 22 → 17.5 ms;
  - shadow-map fill four wide: 4.3 → 3.2 ms.
- **Other fixes:**
  - odd frame sizes now encode correctly;
  - `--check` no longer reports false spikes on short videos;
  - `--profile` honours `--from` and `--to`.

### Films
| Film | Length | Frames | Render time, 4 workers |
|---|---|---|---|
| ONE WORLD THREE LOOKS | 21.5 s | 1288 | 464 s |
| CALIBRE | 17.5 s | 1050 | 363 s |

- **CALIBRE:**
  - a pooled hairspring (no meshes built per draw);
  - a slimmer case with a polished bevel, and a domed crystal;
  - the 9 s highlight spike is gone.
- **ONE WORLD THREE LOOKS:**
  - inked comic water;
  - a swell on the photographic lake;
  - needle-textured conifers.

### Site
The landing page is now `site/concepts/summit/`:
- you stand on a mountain summit in first person, with "anikit" written in clouds;
- your hands open a spark portal;
- you walk into a hall of live demos: physics, a particle sculpture, springs, a stagger wave, keyframed letters and the benchmark board.

Everything that moves is animated by anikit, and rendering is hand-written WebGL2. `site/build.mjs` serves it at the root.

## Final round (2026-11-16, to 13:15 UTC): unfairly fast

Numbers from `packages/anikit/scripts/` (ms per frame, median of 5 runs × 60 frames after 100 warm-up frames; the machine was rendering a film at the same time), detail in `docs/perf-notes.md` "Fifth round".

### Web animation: faster than GSAP in every per-frame case of `scripts/bench.mjs`

| case | 1k | 10k | 50k |
|---|---|---|---|
| 3 numbers | 4.4× | 3.1× | 3.4× |
| colour strings | 2.6× | 1.1× | 2.1× |
| unit strings (`'100px'`) | 1.4× | 1.3× | 1.3× |
| 8 keyframes | 1.6× | 1.5× | 1.5× |

- **Group evaluation in the core**: the children of one `animate()` call that share timing are evaluated once per frame from dense arrays (`ak_group_pack`), and the flush loop writes through dense handle/target/offset tables. 8 keyframes at 50k 1.5–1.8 → 0.74–0.89 ms; 1000-step timelines 0.54–1.13 → 0.28–0.34 ms per frame. Tested against the ungrouped path through seeks, reversal, rate changes, takeovers, release and removal.
- Unit strings: one concatenation and the last string reused.
- `spring()` retargeting no longer keeps a completion hook alive per call (a leak: ~1 KB per call): 10k retargets + frame 35–61 → 8–10 ms; `magnetic`, `smoothScroll` and smoothed scroll links benefit.
- `marquee` skips writes off screen and when unchanged (100 rows 0.23–0.50 → 0.03 ms/frame); scrubbed `scrollTrigger` 0.18 → 0.12–0.16 ms per scroll event at 200 triggers; `splitText` measures in one layout pass (100 paragraphs 768–1076 → 477–654 ms, GSAP SplitText 763–1078 ms).
- GIF encoder: palette search sorted by green from the previous pixel's answer, byte-identical output, gradient frames ~3× faster.
- Measured against GSAP for the first time: `flip()` 15–17 ms vs 137–143 ms (200 boxes); 500 scroll triggers created in 36 ms vs 173 ms, refreshed in 11 ms vs 60 ms.
- **Still behind GSAP: creation.** One `animate()` over 10k objects costs ~60 ms (first frame included) against GSAP's 4–7 ms; about half of it is garbage collection. Deferring set-up to the first tick was tried and made 10k separate calls 2× slower, so it was not kept.

### Video rendering

- `render(…, { sort: 'near' })` draws opaque items nearest first with exact depth-tie handling, so the picture is bit-identical in any order: the terrain bench's opaque pass ~300 → ~190 ms.
- The half-resolution mirror pass renders after the scene and only where it shows.
- `FrameInfo.pixels(px)` hands a finished frame straight to the grade and encoder (skips `putImageData` + read-back, ~19 ms at 1080p).
- `anikit-render --preroll <s>` (0 skips the unseen warm-up frames each worker drew) and `--chunk <s>` (work-stealing jobs).
- Earlier today: opt-in anisotropic filtering, a faster bilinear sampler, an unlit fast path (sky domes 128 → 67 ms), skipping triangles that cover no pixel centre; material `fog`, `Renderer3D.depth()`, soft `flightPath` banking with `bankWindow`.

### Films and site

- `film/film.mjs`: ONE WORLD, THREE LOOKS (golden-hour photographic, neon, printed comic) — one flight, six depth- and mask-driven transitions, the frame split three ways at the start and the end.
- `film/film2.mjs`: EVENT HORIZON — a hyperspace tunnel, a monolith shattering in bullet time, a black hole with a Doppler-beamed disk, a supernova.
- The landing page is rebuilt around one chrome knot (anikit's own CPU 3D renderer) that breaks into seven pieces, each beside a live demo, and re-forms above the install command; the lean bundle streams the wasm (hero 3D visible ~1.3 s → ~0.2 s unthrottled, 524 → 359 KB). No video on the site.

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
