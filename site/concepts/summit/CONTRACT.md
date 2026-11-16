# Summit — the landing page

An original scene. It is not a copy of any film.

## Story

1. **Summit.** You stand on a snowy mountain summit at golden hour in first person. "anikit" is written in the clouds, big and legible.
2. **Arms.** As you scroll, your own arms and hands rise into view.
3. **Gesture.** One hand wears a ring and stretches out; the other hand circles. A ring of orange sparks appears, and a portal opens. Through it you can see the hall.
4. **Through.** You push through the portal. It grows to fill the screen.
5. **Hall.** You arrive in a hall of demos and take control: walk around (WASD/arrows plus mouse look, touch joystick on phones) and use every demo.

Everything that moves is animated by anikit (spring, timeline, stagger, keyframes, world physics, pointCloud, flightPath, smoothScroll and so on). Rendering is hand-written WebGL2: rasterised meshes, instancing and particles. No full-screen ray marching, no three.js.

## Hard requirements

- 60 fps on an integrated laptop GPU:
  - GPU budget of about 8 ms per frame or less;
  - device pixel ratio capped at 1.5, resolution locked once at load;
  - no allocation per frame;
  - no layout reads in scroll or frame code;
  - one requestAnimationFrame loop, owned by `main.js`.
- Bright, beautiful and readable. Not dark and murky.
- Nothing pops: every change is eased or spring-driven.
- Minimal text. No slogans.

## Files and owners (each session edits only its own files)

| File | Owner |
|---|---|
| `index.html`, `main.js`, `style.css`, `mountain.js` | session A (integrator) |
| `hands.js`, `hands-test.html` | session B |
| `hall.js`, `hall-test.html`, `hall-*.js` | session C |

Shared branch: `claude/epic-ramanujan-qe7x4w-summit`. Commit only your own files, then `git pull --rebase origin claude/epic-ramanujan-qe7x4w-summit` and push. Push every 10–15 minutes.

Import anikit from `'../../../packages/anikit/dist/anikit.lean.js'`. The build rewrites that path. Build it first with:

```
cd packages/anikit && npm install && node scripts/build-wasm.mjs && node scripts/build.mjs
```

## One canvas, one WebGL2 context (main.js owns it)

Three stacked full-screen canvases cost three full-screen compositor blends at DPR 2 every frame on an integrated GPU, before any shading. So there is **one** `<canvas id="gl">` and one context, created by `main.js`:

```js
gl = canvas.getContext('webgl2', { antialias: false, alpha: false, depth: true, stencil: false, premultipliedAlpha: false, powerPreference: 'high-performance', desynchronized: true });
```

Each module exports `export const SHARED_GL = true` once it supports this. Until every module does, `main.js` falls back to the old per-canvas layering, so the page never breaks mid-migration.

### Per frame (`main.js`)

`main.js` binds the default framebuffer, sets `viewport(0, 0, W, H)`, and resets state to the defaults below. It then calls, in order:

1. `mountain.frame(t, dt, p, look, push, portal)`. Draws the whole screen; it's the only module that clears colour. It's skipped once the portal covers more than about 70% of the screen (main then clears to the hall's fog colour if needed).
2. `hall.preview(t, dt, k, rect, portal)`. Draws only inside `rect` (device px, top-left origin: `{ x, y, w, h }`), using `gl.scissor` on every pass. Its final composite to the default framebuffer discards fragments outside the circle `portal` (`{ x, y, r }`, device px, top-left origin), so there's no CSS clip-path. Inside the hall, `hall.frame(t, dt)` draws the full screen.
3. `hands.frame(t, dt, g, look)`. Clears depth only, draws the silhouettes, then the sparks (blended) inside their bounds.

### State contract

Defaults on entry to every module: no blend, no depth test, depth mask true, no cull, no scissor, viewport = full canvas, default framebuffer bound, no VAO bound, `UNPACK_FLIP_Y` false. A module may change anything, but it must restore these defaults before returning. Offscreen targets (haze, bloom, mirror) belong to the module, are allocated in `resize()`, and are never reallocated per frame.

### Interfaces (shared mode)

```js
createMountain(gl)            → { resize(cssW, cssH, dpr), frame(t, dt, p, look, push, portal) }
createHall(gl, overlayEl)     → { resize, preview(t, dt, k, rect, portal), enter(), exit(), frame(t, dt) }
createHands(gl)               → { resize, frame(t, dt, g, look) → { portal: { x, y, r, ring, power, formed, step } } }
```

- Every module's `resize(cssW, cssH, dpr)` receives the canvas's real pixel ratio, `canvas.width / cssW`. `main.js` sizes the canvas; modules never set `canvas.width`.
- Modules may still accept a canvas (the legacy path); detect it with `x instanceof WebGL2RenderingContext`.
- Portal numbers from `hands` are in CSS px; `main.js` converts them to device px for the hall.

### Quality governor (`main.js`)

`main.js` measures the real frame time (p90 over 90 frames). Above 17.5 ms it steps down one tier, at most once every 3 s; it steps back up only after 10 s under 12 ms. Modules may expose `setQuality(level)` with level 0 (full) to 2 (lowest):

- level 1 drops optional passes (haze, bloom quality, glow resolution);
- level 2 lowers particle counts.

The last resort, a render scale of 0.85, is applied by `main.js`.

## Scroll timeline (`main.js`)

- The page is about 600vh tall. Scroll progress is smoothed with an anikit spring.

| p | What happens |
|---|---|
| 0–0.15 | Summit; clouds spell "anikit"; pointer parallax |
| 0.15–0.75 | `g` runs 0 → 0.75 (arms, gesture, portal opens); the hall is visible through the portal with `preview(k = 0)` |
| 0.75–0.92 | `push` and `g` run 0.75 → 1; `k` runs 0 → 1; the portal grows to fill the screen |
| ≥ 0.95 | `hall.enter()`; page scroll is locked; hint: "Click to walk · WASD" (on phones, the joystick). Esc or a small "↑ summit" button calls `exit()` and scrolls back |

- Reduced motion: skip straight to the hall with a cross-fade.

## Hall demos (session C, each a station you walk up to)

- Everything is anikit-driven.
- Proximity activates a station; click or E interacts.

Stations:
- physics: a wall or floor of balls you can throw, using anikit `world()`;
- particles: a 100k-point sculpture morphing between shapes (anikit `pointCloud` or GPU points driven by anikit tweens);
- springs: objects that follow you or the cursor with `spring()`;
- timeline and stagger: a kinetic sculpture of a few hundred instanced blocks doing staggered waves you can trigger;
- text: big 3D letters animated with keyframes;
- benchmarks: honest numbers from `README.md` and `docs/`.
