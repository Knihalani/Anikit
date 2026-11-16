# Prototype C: the studio

Procedural headphones in a bright photo studio, built with anikit end to end:

- **Modelling** (`model.js`): `lathe()` for the housings and glass caps, `capsule()` for the sliders, `tube()` for the band and band cushion (squashed into a flat section), `torus()` for the ear cushions, and an SDF (`sdTorus` + `sdSphere` + `smoothUnion`) meshed with `sdfMesh()` for each yoke.
- **Rendering**: `scene()` with PBR materials, the studio environment, a PCF shadow map, bloom + FXAA, `raycast()` picking, and `instances()` for the 300 copies (one draw call per part).
- **Motion**: one `timeline()` (with `stagger()`) scrubbed by `scrollProgress()` through a spring-smoothed `motionValue()`; drag to orbit with `inertia()`, and the pitch settles with `spring()`; finishes change with `spring()` on the material; the finale camera rides a `flightPath()`; label text changes with the `text` scramble.
- **Labels**: real HTML rasterised by `htmlCanvas()` into WebGL textures and pinned to each part's 3D position. A scene layer supplies the frame's matrices, and the cards draw after the post chain, so their colours and text stay exact.

Open `site/proto-c/` from a static server at the repo root, after `node scripts/build.mjs` in `packages/anikit`.
`?perf` shows frame p50/p95/max. `?q=0..3` pins the quality level. Otherwise a governor steps down when p95 exceeds 22 ms: bloom goes first, then shadows, then pixel ratio.

## Library gaps found (worked around here)

- `scene()` ships only a dark `studio` environment and has no public API to recolour it. The site tints the private `envC` gradient.
- There are no public runtime toggles for bloom or the pixel ratio. The governor sets `bloomOn`, `shadows` and `dpr` and calls `resize()` directly.
- Fixed in `gl.ts`: switching `shadows` off at runtime left a 2D texture on the shadow sampler's unit (a GL error on every draw). The shadow map now stays bound whenever it exists.
- Layers draw into the HDR buffer before tone mapping and FXAA, which is wrong for UI. Labels draw after `render()` instead.
- `InstancedMesh.update()` resets per-instance colours, so they are rewritten in the update callback.

`main-standin.js`, `studio-standin.js` and `geometry-standin.js` are the hand-written WebGL stand-in used before `scene()` landed. They are kept for comparison and are not loaded by the page.

## Verification (headless Chromium, SwiftShader)

- No console errors or warnings across load, a full scroll down and back, clicks on parts, hover and drag.
- First contentful paint is about 130 ms: the page background matches the studio, and the canvas fades in once the first frames are pre-warmed (copies and labels included).
- Startup work yields between parts. The only long task left is the first one (module, WebAssembly and shader compile), about 200 ms under SwiftShader.
- Frame times under SwiftShader say nothing about a real GPU. Use `?perf` on the target laptop. Hero: 34 draws, about 50k triangles. Finale: 299 copies × 12 parts as instanced draws, about 1.2M triangles, with copies left out of the shadow pass.
- Library checks for the `gl.ts` fixes: `tsc --noEmit`, `vitest run test/scene-geometry.test.ts` (8/8), `node scripts/test-browser.mjs scene` (6/6).
