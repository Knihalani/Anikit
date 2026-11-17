# What only Anikit Video can do (verified)

This page answers "what can Anikit Video do that browser renderers can't?" with measurements, not claims. Every
number was measured on the benchmark machine: 4 vCPU Xeon @ 2.1 GHz, 16 GB RAM, **no GPU**. The engine was main
`0c72f62`; the other versions were HyperFrames 0.9.59, Remotion 4.0.544 and chrome-headless-shell 153. Scripts:
`bench/video/floor/measure.sh` and `bench/video/run.mjs`, with results in `docs/video/BENCHMARKS.md`. Where a browser
tool matches or beats us, this page says so.

## The browser floor: what a competitor *can* make

`bench/video/floor/hyperframes/index.html` is the most impressive 15 s we could build in HyperFrames in an hour. It
runs in three parts, each with GSAP kinetic type:
1. three.js `InstancedMesh` of **30,000 PBR cubes** in a spiral galaxy, with an environment map and bloom;
2. a full-screen **raymarched GLSL** scene (smooth-union metaballs, soft shadows, AO, a reflection bounce);
3. **500,000 GPU particles** in a flow field computed in the vertex shader, blended additively.

Stills are in `bench/video/floor/stills/`.

It works, and it looks like real motion design. **That is the honest floor:** WebGL gives browser tools 3D, PBR,
shaders and hundreds of thousands of particles.

| Render of the 15 s floor (450 frames) | Wall time | Output |
|---|---:|---|
| HyperFrames defaults | **13 min 28 s** (0.56 fps) | 74 MB, H.264 1080p |
| HyperFrames `--workers 4 --quality draft` | **10 min 55 s** (0.69 fps) | |

**Limits we hit:**
- No GPU here, so Chrome falls back to SwiftShader (software WebGL): "browserGpuMode probe → software". With a GPU,
  this piece would render far faster. That is the main caveat on everything below.
- `hyperframes lint` reports one error (a GSAP `letterSpacing` tween snaps to whole pixels) and one shader
  warning (an unbounded `pow` base). It renders regardless; both are style issues, not blockers.
- Every frame must be a pure function of the timeline time, so there is no stateful simulation.
- The CDN imports used in HyperFrames' own examples had to be vendored. Our import map points at `node_modules/three`.

## Verified: where Anikit Video is clearly ahead

### 1. End-to-end speed on motion graphics: 19–42× (measured)
On the same 2D compositions (kinetic type, product UI, explainer, and a 60 fps 60 s version), Anikit Video's
default output is a finished H.264 MP4 in 0.31–0.73 s per 20 s video, and 3.37 s for 60 s at 60 fps. That is 19–42×
faster than the fastest HyperFrames or Remotion setting and 37–88× faster than their defaults, at equal or higher
PSNR. The tables are in `docs/video/BENCHMARKS.md`.

**Why it holds:** there is no browser layout, paint or screenshot, and the encoder (`ak264`) skips unchanged
macroblocks. **With a GPU, browser tools would still pay for layout, screenshot capture and encoding,** which
dominate on 2D content: their fastest settings barely moved when the encoder got cheaper (BENCHMARKS.md, "settings
tried").

### 2. Many independent animated objects (2D): 5–8× (measured)
5,000 dots, each on its own orbit, for 5 s (150 frames):
- Anikit Video: a spec using `"repeat": 1000` five times, with expressions per dot, renders in **4.61 s**.
- HyperFrames: 5,000 DOM divs moved by a GSAP `onUpdate` each frame takes **38.1 s** at defaults and **22.7 s** at
  `--workers 4 --quality draft`.

Sources: `bench/video/floor/scale/`.

**Honest limit:** a browser tool could draw these 5,000 dots with WebGL points instead of the DOM. The floor
draws 500,000 that way. Anikit's advantage here is over *DOM-based* authoring, which is how most HyperFrames and
Remotion videos are written.

### 3. Cold start and preview: ~100× (measured)
One still frame from a fresh process takes **19 ms** with `anikit-video still`, against 1.5 s for HyperFrames
`snapshot` and 4.0 s for Remotion `still` (BENCHMARKS.md). An agent can iterate on frames in a tight loop.

### 4. Memory and footprint (measured)
Anikit Video peaks at about 265 MiB PSS while rendering, against 0.9–1.6 GiB for the browser tools (BENCHMARKS.md).
It is one static binary with no Chrome, no Node and no GPU driver.

### 5. Long renders scale linearly (measured)
60 s at 60 fps (3,600 frames) took **3.37 s**, against 69–152 s for the browser tools. The ratio stayed the same as
for 20 s.

## Verified: where browser tools match or beat Anikit Video today

| Claim one might make | What we measured or found in the code | Verdict |
|---|---|---|
| "Shaders/raymarching are faster natively" | The **same raymarch GLSL**, 1080p, 150 frames: Anikit's CPU JIT shader layer (`floor/anikit/raymarch.json`) took **435.7 s**; HyperFrames on software WebGL took **415.9 s**. Anikit's own docs give 371 ms/frame for its built-in `sdf-scene` (`docs/video/shader.md`). | **No advantage.** Even without a GPU, the browser is as fast. With a GPU it would be orders of magnitude faster. |
| "Massive particle counts" | Anikit's `particle-text` is capped at 50,000 (`components/particle_text.rs:255`); 50,000 particles over 150 frames render in 8.9 s (18 fps). three.js drew 500,000 GPU points in the floor. Spec `repeat` is capped at 1,000 per layer (`spec/reuse.rs:25`). | **Browser wins on raw count** (with WebGL). |
| "Real 3D" | Anikit has `model` (primitives or OBJ, CPU rasteriser, environment reflections, vector contact shadow) and `text-3d`. Shadow maps and planar reflections exist in `anikit-core::raster` but are **not used by video**. three.js has PBR, shadow maps, glTF and post-processing. | **Browser is more capable** for general 3D today. |
| "CPU ray tracing / path tracing" | There is no BVH, ray tracer or path tracer in `anikit-video` or `anikit-core::raster`. `anikit-core/src/blackhole.rs` (a geodesic renderer) is only exposed to wasm. | **Not available in Anikit Video.** |
| "Physics simulation, point clouds, Gaussian splats, SDF meshing" | `anikit-core` has `world` (PBD physics), `cloud`, `splat`, `emitter`, `gsplat` (a sort/pack for WebGL, not a CPU rasteriser) and `sdf` (surface nets), but **none is imported by `anikit-video`**, so no spec can use them. Video motion is closed-form in t. | **Not available in Anikit Video today.** Exposing `world` (pre-simulated, then sampled by frame) would be a genuine differentiator, because a seekable browser timeline cannot step a simulation cheaply. |
| "Deterministic, frame-exact renders" | Rendering `bench/video/anikit/kinetic.json` twice with identical settings gave **different MP4s**: 150 of 600 frames differ, with PSNR 75.9 dB between runs. 1 vs 4 threads gave 58.0 dB. HyperFrames' kinetic render was **byte-identical** across two runs and across 1 vs 4 workers. | **Browser wins.** The differences are invisible, but Anikit is not bit-exact across runs or thread counts on this engine, contrary to `docs/video/render.md`. This needs a fix: likely per-worker incremental and encoder state at chunk boundaries. |

## Bottom line

- **Anikit's real, verified edge is throughput and footprint on the content most videos are made of:** 2D motion
  graphics, type, UI and charts. It is 19–42× faster end to end, 100× faster to first frame, a quarter of the
  memory, and needs no browser or GPU.
- **On spectacle per frame, a browser with WebGL is at least as capable today:** shaders, 3D, particle counts.
  Without a GPU it is just as slow as our CPU shaders, and with one it is far faster.
- **What we could make uniquely ours, but haven't yet:** expose `world` physics, `cloud`/`emitter` particles and
  shadow-mapped 3D to the spec, and make renders bit-exact. Until then, "only Anikit can do X" is true for speed,
  not for visuals.
