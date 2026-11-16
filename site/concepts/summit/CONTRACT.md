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

## Layering (`main.js`; bottom to top, all `position: fixed`, full screen)

1. `<canvas id="mountain">`: WebGL2. Stops rendering once the portal covers the screen.
2. `<canvas id="hall">`: WebGL2.
   - Visible only inside the portal, through CSS `clip-path: circle(Rpx at Xpx Ypx)`.
   - `main.js` sets the clip-path from `hands.frame()`'s return value.
   - Hidden (`visibility: hidden`, not rendered) while R = 0.
3. `<canvas id="hands">`: WebGL2.
   - Transparent: `premultipliedAlpha: true`, clears to 0, `pointer-events: none`.
4. DOM overlay: minimal text and the hall HUD.

## Interfaces

All times are in seconds. All sizes are in CSS px unless stated. Each module creates its own WebGL2 context on the canvas it is given.

```js
// mountain.js
export function createMountain(canvas) → {
  resize(cssW, cssH, dpr),
  // p: scroll progress 0..1 for the whole intro
  // look: smoothed pointer -1..1
  // push: 0..1 forward dolly as you step through
  frame(t, dt, p, look, push),
}

// hands.js
export function createHands(canvas) → {
  resize(cssW, cssH, dpr),
  // g: gesture progress 0..1
  //   0      arms out of view
  //   0.25   arms raised
  //   0.4    ring hand forward
  //   0.55   other hand circles
  //   0.75   portal fully open
  //   1      arms lowered or out of view as you step through
  // returns the same object every call (no allocation);
  // portal.r = 0 while closed; r grows to cover the screen at g = 1
  frame(t, dt, g, look) → { portal: { x, y, r } },
}

// hall.js
export function createHall(canvas, overlayEl) → {
  resize(cssW, cssH, dpr),
  // the hall seen through the portal from just outside the doorway;
  // k: 0..1 approach, 1 = standing inside at the spawn point
  preview(t, dt, k),
  enter(),        // take control: input listeners, HUD in overlayEl, pointer lock on click
  exit(),         // release control (back to scroll)
  frame(t, dt),   // while active
}
```

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
