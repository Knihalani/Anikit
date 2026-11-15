# Anikit TypeScript API

Times are in **seconds**. Every function accepts targets as a CSS selector, an element, a list/NodeList, a plain object, or an array mixing these.

```ts
import { animate, timeline, stagger, spring, inertia, scroll, inView, splitText, flip,
         compositor, batch, set, from, fromTo, engine, ready } from 'anikit';
// or: import ... from 'anikit/lean'  (streams anikit.wasm instead of embedding it)
```

The WebAssembly core starts compiling on import. Calls made before it's ready are queued and run in order, so you rarely need it, but `await ready` is available.

---

## `animate(targets, keyframes, options?) → Animation`

```ts
animate('.box', { x: 200, rotate: 45, backgroundColor: '#0af' }, { duration: 0.6, ease: 'outBack' });
animate(el, { opacity: [0, 1], y: [20, 0] });                     // from → to
animate(el, { x: [0, 100, 50] }, { times: [0, 0.3, 1] });          // keyframes
animate(el, [{ x: 0 }, { x: 100, offset: 0.2, ease: 'inQuad' }, { x: 0 }]); // WAAPI-style list
animate(el, { x: { to: 100, ease: 'outElastic' }, y: '+=50' });  // per-property easing, relative
animate(els, { x: (el, i, n) => i * 20 }, { delay: stagger(0.05, { from: 'center' }) });
animate(obj, { value: 1 }, { onUpdate: () => render(obj.value) }); // plain objects
```

**Values:** numbers (with the default unit: `px` for lengths, `deg` for angles), strings with units, colours in any CSS syntax, complex strings (`'0 4px 12px rgba(0,0,0,.2)'`), discrete strings, `null` for "the current value", relative values (`'+=10'`, `'-=10'`, `'*=2'`) and `(target, index, count) => value` functions. Options may also be written inside the keyframes object (GSAP style).

**Special properties**

| Property | Meaning |
|---|---|
| `x y z translateX/Y/Z rotate rotateX/Y/Z scale scaleX/Y/Z skew skewX/Y perspective` | Transform components, composed into one `transform` per element per frame. |
| `'--name'` | CSS custom property. |
| `attr: { r: 20, cx: 50 }` | Attributes. Non‑CSS attributes of SVG elements are detected automatically. |
| `d: pathData \| element \| selector` | Shape morph between any two paths. |
| `path: pathData \| { path, start?, end?, rotate? }` | Motion along a path. `rotate: true` (or an angle offset) follows the tangent. |
| `draw: [0, 1]` or `['0 0', '0.2 0.8']` | Visible stroke segment as fractions of the length. |

**Options** (`AnimateOptions`)

| Option | Default | |
|---|---|---|
| `duration` | `0.5` | Seconds. Spring easings default to their natural settle time. Accepts a function per target. |
| `delay` | `0` | Seconds, a function, or `stagger()`. |
| `ease` | `'outQuad'` | An easing (see below), or an array with one easing per keyframe segment. |
| `repeat`, `repeatDelay`, `yoyo`, `direction` | | `repeat: Infinity` (or `-1`) loops. |
| `speed` | `1` | Playback rate. Negative plays backwards. |
| `times` | | Keyframe offsets for array values. |
| `colorSpace` | `'oklab'` | `'srgb' \| 'linear' \| 'oklab' \| 'oklch' \| 'hsl'`. |
| `fillMode` | `'both'` | `'forwards' \| 'backwards' \| 'none'` (named `fillMode` so `fill` stays the SVG/CSS property). |
| `overwrite` | `true` | A new animation takes over the properties it animates. |
| `immediateRender` | auto | Render the first keyframe right away. Defaults to on for delayed animations with explicit start values. |
| `autoplay` | `true` | |
| `onStart`, `onUpdate`, `onComplete`, `onRepeat`, `onReverseComplete` | | `onUpdate` fires once per frame, after DOM writes. |

`from(targets, values, options)`, `fromTo(targets, fromValues, toValues, options)` and `set(targets, values)` (instant) are shorthands.

### Controller (`Animation`, `Timeline`, `Batch`)

`play()`, `pause()`, `reverse()`, `restart()`, `seek(seconds)`, `complete()`, `cancel()`, `kill()`/`stop()`. Properties: `progress` (get/set, 0–1), `time` (get/set), `duration`, `speed` (get/set), `state` (`'idle' | 'running' | 'paused' | 'finished'`) and `iteration`. Controllers are thenables: `await animate(...)`, or use `.finished`.

---

## Easings

Accepted everywhere an `ease` is:

- **CSS:** `'linear'`, `'ease'`, `'ease-in-out'`, `'cubic-bezier(.2,.8,.2,1)'`, `'steps(4, jump-start)'`, `'step-end'`, `'linear(0, 0.25 75%, 1)'`.
- **Penner/Motion names:** `'inQuad'`, `'easeOutCubic'`, `'inOutQuart'`, `'outQuint'`, `'inSine'`, `'outExpo'`, `'circIn'`, `'outBack'`, `'inOutElastic'`, `'outBounce'`, plus `outIn*` variants.
- **GSAP names:** `'power1'`–`'power4'`, `'sine'`, `'expo'`, `'circ'`, `'back(1.7)'`, `'elastic(1, 0.3)'`, `'bounce'` with `.in`, `.out` or `.inOut`; `'pow(3).in'`.
- **Springs:** `'spring'`, `'spring(stiffness, damping, mass, velocity)'`, `'spring(bounce: 0.3, duration: 0.6)'`, or an object `{ stiffness, damping, mass }` / `{ bounce, duration }`.
- **Extras:** `'wiggle(5)'`, `'shake(8)'`, `'slowmo(0.7, 0.7)'`, `'anticipate'`.
- **Other forms:** a `[x1, y1, x2, y2]` bezier array, or any `(t) => number` function (sampled once into a 1024‑point table).

Helpers: `ease(e, t)`, `easing(e) → (t) => number`, and `cssEasing(e)` (native CSS string; springs become `linear()`).

---

## `timeline(options?) → Timeline`

```ts
const tl = timeline({ defaults: { duration: 0.4, ease: 'outCubic' }, repeat: 1, yoyo: true })
  .add('.title', { y: [30, 0], opacity: [0, 1] })
  .add('.item', { x: [-20, 0], opacity: [0, 1] }, { delay: stagger(0.05) }, '-=0.2')
  .label('spin')
  .add(logo, { rotate: 360 }, {}, 'spin+=0.1')
  .add(otherTimelineOrAnimation, '<')
  .call(() => console.log('halfway'), '>')
  .set(el, { display: 'none' });
tl.seek('spin'); tl.playFrom(0.5);
```

**Positions:**
- a number of seconds;
- `'<'` / `'>'`: start or end of the previous child, with an optional offset (`'<0.2'`, `'>-0.1'`);
- `'+=0.5'` / `'-=0.5'`: relative to the timeline end;
- a label, optionally with an offset (`'label'`, `'label+=0.3'`).

With no position, a child goes at the end of the timeline. Options: `delay`, `repeat`, `repeatDelay`, `yoyo`, `speed`, `ease` (applied to the overall progress), `autoplay` and `defaults`. Sequential tweens of the same property chain their start values.

---

## Physics

```ts
const s = spring(el, { x: 200, y: 50 }, { stiffness: 200, damping: 20 });   // or { bounce: .3, duration: .5 }
spring(el, { x: 0 });               // retarget: position and velocity are preserved
s.velocity;                          // { x: …, y: … } in units/s
await s;                             // resolves at rest

inertia(card, { x: { velocity: vx, min: 0, max: 600, snap: 100 } }, { power: 0.8, timeConstant: 0.35 });
```

---

## Scroll & viewport

```ts
const stop = scroll(animate('.bar', { scaleX: [0, 1] }, { ease: 'linear' }));       // page progress
scroll(animate(img, { y: [-80, 80] }), { target: img, offset: ['start end', 'end start'], smooth: 0.1 });
scroll((p) => (counter.textContent = Math.round(p * 100) + '%'), { container: panel });
inView('.reveal', (el) => { animate(el, { opacity: [0, 1] }); return () => animate(el, { opacity: 0 }); }, { amount: 0.3 });
```

---

## SVG

`morph(a, b) → (t) => d`, `morphPlan(a, b)`, `pathLength(p)`, `pathBounds(p)`, `normalizePath(p)`, `shapeToPath(el)` and `new MotionPath(p).at(t) → { x, y, angle }`. Shape builders: `circle`, `ellipse`, `rect`, `polygon` and `star`.

---

## Text, layout, compositor, batches

```ts
const { chars, words, lines, revert } = splitText('h1', { type: 'lines,words,chars' });
animate(chars, { y: ['100%', 0], opacity: [0, 1] }, { delay: stagger(0.02) });

flip('.item', () => grid.classList.toggle('dense'), { duration: 0.6, ease: 'spring(bounce: .2)' });

compositor('.toast', { y: [20, 0], opacity: [0, 1] }, { ease: 'spring(bounce: .4)' }); // compositor thread

const b = batch(50_000, { x: (i) => [0, i % 500], y: [0, 300], a: [0, 1] },
                { duration: 2, delay: stagger(0.00005), onUpdate: () => draw(b.data, b.stride) });
// b.data: Float32Array (count * stride), e.g. gl.bufferSubData(gl.ARRAY_BUFFER, 0, b.data)
```

---

## Engine

`engine.manual(true)` with `engine.tick(ms)` drives the engine yourself (tests, video export). Also available:

- `engine.speed = 0.5`: global time scale (also scales physics worlds, point clouds and `onFrame` `dt`);
- `engine.lagSmoothing(threshold, adjusted)`;
- `engine.fps = 30`: frame‑rate cap;
- `engine.onFrame(cb)`: per‑frame callback;
- `engine.stats()`: active and allocated nodes.

`flush()` applies pending results synchronously.

Utilities: `stagger(each, { from, grid, axis, ease, total, start, seed })`, `staggerValues(count, each, opts)`, `mixColor(a, b, t, space)`, `parseColor(s)`, `clamp`, `lerp`, `mapRange`, `wrap`, `snap` and `random(min, max, seed?)`.

---

## More easings

- **Drawn curves:** `'custom(M0,0 C0.2,0 0.3,1.2 0.6,1 0.8,0.9 1,1)'` turns an SVG path into an easing ("CustomEase"). Paths in SVG space (y‑down, any size) are normalised by their start and end points.
- **Modifiers:** `'reverse(inQuad)'` turns an in‑ease into an out‑ease, `'mirror(outBack)'` goes there and back in one iteration (good for pulses), and `'blend(inQuad, outElastic, 0.3)'` mixes two easings.
- **Spring presets:** `'spring.gentle'`, `'wobbly'`, `'stiff'`, `'slow'`, `'molasses'`, `'snappy'`, `'bouncy'`.
- **Others:** `'rough(points, strength, seed, template)'` (seeded jitter) and `'expoScale(1, 5)'` (compensates perceived speed when zooming).

## More values

```ts
animate(el, { x: 'random(-100, 100, 10)', rotate: 'random([0, 90, 180])' });   // per target
animate(obj, { x: 1000 }, { snap: { x: 50 }, round: ['y'], modifiers: { x: (v) => v % 500 } });
animate('#price', { textContent: 12499 }, { format: { style: 'currency', currency: 'USD' }, locale: 'en-US' });
animate(label, { text: 'Hello world' });                                       // typewriter
animate(label, { text: { value: 'DECRYPTED', type: 'scramble', chars: 'upper' } });
animate(el, { xPercent: -50, yPercent: -50 });                                 // translate(-50%, -50%)
animate(path, { d: [null, circle(50, 50, 40), star(50, 50, 6, 45, 20)] });     // morph through N shapes
```

## Control, cleanup and accessibility

- `anim.revert()` kills the animation and restores every property it touched to its original inline value. `timeline.revert()` does the same for every child.
- `context(fn, scope?)` collects everything created inside `fn` (animations, timelines, scroll triggers, draggables, observers…), scopes selector lookups to `scope`, and offers `ctx.revert()` / `ctx.kill()`. `fn` may return a cleanup function. It's the natural fit for component lifecycles in React, Vue or Svelte.
- `matchMedia().add('(min-width: 800px)', setup)` runs `setup` while the query matches and reverts it automatically when it stops matching. `add({ wide: '…', reduce: '(prefers-reduced-motion: reduce)' }, ({ wide, reduce }) => …)` takes several named conditions.
- `engine.reducedMotion = 'user' | 'always' | 'never'`: when motion is reduced, animations jump straight to their end state, while callbacks and promises still fire.
- `quickSetter(el, 'x')` returns a fast setter; `quickTo(el, 'x', opts)` returns a function that re‑targets a tween on each call.
- `delayedCall(seconds, fn)` runs `fn` on engine time, so it respects the global speed and pausing.

## Timeline extras

`tl.tweenTo(labelOrTime, opts)` and `tl.tweenFromTo(a, b, opts)` animate the playhead. There's also `tl.remove(child)`, `tl.clear()`, `tl.getChildren()`, `tl.currentLabel`, `tl.nextLabel()` and `tl.previousLabel()`.

```ts
sequence([
  ['.a', { x: 100 }],
  ['.b', { opacity: [0, 1] }, { at: '<' }],
  'middle',
  [() => console.log('middle reached')],
  ['.c', { scale: 2 }, { at: 'middle+=0.2', duration: 1 }],
], { defaults: { ease: 'outCubic' } });
```

## Ballistic physics

```ts
physics(ball, { x: { velocity: 300 }, y: { velocity: -600, acceleration: 1500, max: floorY } }, { restitution: 0.6, friction: 0.2 });
```

`physics()` simulates gravity, wind, drag and bouncing bounds. It's solved exactly per frame, with collisions resolved at the moment of impact, so it comes to rest the same way at any frame rate. It returns the same controller as `spring()`/`inertia()`.

## Bake to CSS

`anim.toCSS({ name, selector, samples })` produces `@keyframes` plus an `animation:` rule, sampled from the engine. Any easing, spring, colour space or keyframe set works. Use it for SSR, HTML emails or zero‑JS pages.

## Effects

Over 35 presets. Every one accepts the usual animation options plus `stagger`, `distance`, `from`, `intensity` and `loop`:

- **Entrances and exits:** `fadeIn`/`fadeOut`, `slideIn`/`slideOut` (`from: 'up' | 'down' | 'left' | 'right'`), `scaleIn`/`scaleOut`, `zoomIn`/`zoomOut`, `pop`, `bounceIn`/`bounceOut`, `dropIn`, `flipIn`/`flipOut`, `rotateIn`, `blurIn`/`blurOut`, `revealClip`, `cascade`.
- **Attention:** `shake`, `headShake`, `wobble`, `jello`, `rubberBand`, `swing`, `tada`, `pulse`, `heartbeat`, `flash`, `highlight`.
- **Loops:** `float`, `breathe`, `spin`, `wiggleLoop`.
- **Text:** `typewriter`, `scramble`, `countUp`.
- **SVG:** `drawSVG`, `undrawSVG`.

```ts
effects.slideIn('.menu li', { from: 'left', stagger: 0.04 });
registerEffect('glow', (t, o) => animate(t, { filter: ['drop-shadow(0 0 0 gold)', `drop-shadow(0 0 ${o.size}px gold)`] }, o), { size: 12 });
effects.glow('.star');
```

## Scroll

```ts
scrollTrigger({
  trigger: '.section', start: 'top 80%', end: '+=600',   // "<trigger edge> <viewport edge>", px, %, or '+=' relative
  animation: tl, scrub: 0.5,                              // true, or smoothing lag in seconds
  pin: true, pinSpacing: true,
  snap: { to: 'labels', duration: 0.4 },                  // step, list, 'labels' or function
  toggleActions: 'play pause resume reverse',             // onEnter onLeave onEnterBack onLeaveBack
  toggleClass: 'active', once: false, markers: true,
  onEnter, onLeave, onEnterBack, onLeaveBack, onToggle, onUpdate,
});
scrollTriggerBatch('.card', { start: 'top 85%', animation: (el, i) => animate(el, { y: [40, 0] }, { autoplay: false }) });
scrollTo('#pricing', { duration: 1, offset: 80 });
const smooth = smoothScroll({ smooth: 0.6 });
```

One passive listener per scroller drives every trigger, and layout is measured only on refresh (load and resize). Call `refreshScrollTriggers()` after you change the layout yourself. Pinning uses `position: fixed` for the page and a counter‑translate inside scrolling elements.

`observer({ target, type: 'wheel,touch,pointer,scroll', tolerance, onUp, onDown, onLeft, onRight, onChange, onDrag, onPress, onRelease, onStop })` normalises input intents. Use it for full‑page section transitions and carousels.

## Interaction

```ts
const [d] = draggable('.card', { bounds: '.board', inertia: true, snap: { x: 100, y: 100 }, edgeResistance: 0.35, lockAxis: true });
draggable('.knob', { type: 'rotate', bounds: { minRotation: -135, maxRotation: 135 }, onDrag: (d) => setVolume(d.rotation) });
hover('.btn', (el) => { animate(el, { scale: 1.05 }); return () => animate(el, { scale: 1 }); });
press('.btn', (el) => { animate(el, { scale: 0.95 }); return () => animate(el, { scale: 1 }); });
```

`Draggable` exposes `x`, `y`, `rotation`, `isDragging`, `enable()`, `disable()`, `setPosition()`, `kill()` and `revert()`.

## Dev tools

`inspector(animationOrTimeline)` opens a floating panel to play, pause, scrub, restart, loop, change speed and jump to labels. Space toggles playback.

## More utilities

| Function | Purpose |
|---|---|
| `interpolate([0, 100, 200], [0, 1, 0], { ease, clamp })` | Map through stops. Outputs can be numbers, colours or values with units. |
| `distribute({ base, amount, from, grid, ease })` | Spread any value across targets. |
| `noise2(x, y, seed)`, `noise3`, `fbm(x, y, z?, { octaves })`, `noiseMany(xyz)`, `wander({ speed, amplitude })` | Seeded simplex noise. |
| `trimPath(d, 0.2, 0.8)`, `pathThrough(points, { curviness, closed })`, `simplifyPath(d, tolerance, smooth)`, `pathArea(d)`, `flattenPath(d, tol)` | Path operations. |
| `MotionPath.closest(x, y)`, `.atLength(px)`, `.points(n)` | Path queries. |
| `VelocityTracker`, `shuffle`, `pipe`, `splitColor`, `getUnit`, `roundTo`, `normalize`, `toArray` | Helpers. |

---

## Physics worlds

`world(options?)` is a 2D rigid-circle physics world solved in WebAssembly: position-based dynamics with sub-steps, a sorted uniform grid for broad phase, restitution and friction, distance constraints (ropes, cloth, soft bodies, tearing), static boxes, attractors and picking. It steps on the engine clock, so `engine.speed` slows it too.

```ts
const w = world({ container: '#stage', gravity: [0, 980], substeps: 4, restitution: 0.2, friction: 0.05 });
w.addMany(Array.from({ length: 2000 }, () => ({ x: Math.random() * 800, y: Math.random() * 400, r: 3 + Math.random() * 4 })));
w.bind('.letter', { rotate: true });        // DOM elements become bodies; their transforms follow
w.obstacles('.wall');                       // element boxes become static colliders
w.rope([[100, 40], [400, 40]], { segments: 24, pinStart: true });
w.cloth(200, 60, 30, 20, 12, { pinTop: 3, tear: 2.5 });
w.pointer({ mode: 'repel', radius: 140 });  // or 'attract'
w.dragging();                               // grab and throw bodies
w.draw(ctx, { color: (i) => palette[i % 4], links: '#fff6' });
```

| Member | Purpose |
|---|---|
| `add(x, y, r, mass?)`, `addMany(bodies)` | Add bodies (mass ≤ 0 makes them static). Returns the first index. |
| `bind(targets, { radius, mass, rotate })` | Turn elements into bodies. |
| `box(x0, y0, x1, y1)`, `obstacles(targets)` | Static colliders. |
| `link(a, b, { length, stiffness, tear })`, `rope(points, opts)`, `cloth(x, y, cols, rows, spacing, opts)` | Constraints. `linksAlive()` reports torn links. |
| `attract(x, y, strength, radius)`, `setAttractor(i, …)`, `clearAttractors()`, `pointer(opts)` | Force fields. |
| `impulse(x, y, radius, strength)`, `setPosition`, `setVelocity`, `velocity(i)`, `pin(i)`, `remove(i)`, `setRadius` | Direct control. |
| `pick(x, y)`, `x(i)`, `y(i)`, `angle(i)`, `radius(i)`, `data` | Queries. `data` is a zero-copy `Float32Array` of `[x, y, angle]` per body. |
| `energy`, `pairTests`, `count` | Diagnostics. |
| `onStep(cb)`, `step(dt)`, `pause()`, `play()`, `set(options)`, `refreshBounds()`, `destroy()` / `revert()` | Lifecycle. |

## 3D

```ts
// Transform lists interpolate like browsers do (decompose → slerp → recompose):
animate(card, { transform: 'rotateY(180deg) translateZ(40px)' }, { ease: 'spring(bounce: .3)' });
tilt('.card', { max: 12, scale: 1.03 });              // pointer tilt on a spring

const scene = scene3d({ canvas: '#c', fov: 50, camera: { z: 800 } });
scene.orbit({ autoRotate: 8 });                       // drag to rotate, with inertia
scene.draw(batchOrXYZ, { size: 2, fog: 0.6, color: (i) => '#9af' });
const { data, order, count } = scene.project(points); // [sx, sy, depth, scale] * n, far→near order
```

- The `transform` property accepts any CSS transform list. Lists are decomposed (CSS Transforms 2), rotations slerped as quaternions, and recomposed to `matrix3d()` in the core.
- `parseTransform(css)`, `toMatrix3d(m)` and `mixTransforms(a, b, t)` expose the same maths.
- `scene.project()` transforms and depth-sorts (LSD radix sort) entirely in WebAssembly. Sources: a `batch()` with `x`/`y`/`z` channels, a `pointCloud()`, or a flat `[x, y, z, …]` array.

## Point clouds and glow

`pointCloud(count, options)` keeps particles in WebAssembly. Each one spring-follows a blend between target shapes with its own timing, plus a turbulence field, so morphs ripple through the cloud and can be scrubbed, interrupted or blasted apart. `glow(canvas, options)` renders projected points as additive light straight into pixels: sub-pixel bilinear splats, soft bokeh for near points, depth fog, a pointer lens, motion trails and tone mapping. JavaScript does one `putImageData` per frame.

```ts
const cloud = pointCloud(150_000, {
  shapes: [textPoints('anikit', 150_000, { width: 800 }), (i, n) => sphere(i, n)],
  stiffness: 34, damping: 7.5, spread: 0.75, swirl: 70,
});
const scene = scene3d({ fov: 40, camera: { z: 1000 } });
const g = glow('#bg', { resolution: 0.75, trail: 0.5, background: '#07080b', budget: 6 });
g.resize(innerWidth, innerHeight);
g.colors(cloud.count, (i) => [0.4, 0.45, 1]);
engine.onFrame(() => g.render(scene, cloud, { size: 1.2, intensity: 0.3, fog: [700, 2600], lens: { x, y } }));

cloud.morph(1, { duration: 2.5, ease: 'inOutCubic' }); // tweened
cloud.morph(2); cloud.mix = scrollProgress;          // or drive the blend yourself
cloud.explode(0, 0, 0, 1200);                        // shockwave; particles find their way home
```

| Member | Purpose |
|---|---|
| `shape(k, xyzOrFn)`, `snap(k)` | Define / jump to shapes. |
| `from`, `to`, `mix`, `morph(k, { duration, ease, from })` | Blend state. |
| `set({ stiffness, damping, spread, noise, swirl, frequency })` | Tune the motion. |
| `step(dt)` (automatic unless `autoStep: false`), `explode(x, y, z, strength)`, `positions`, `speed` | Simulation. |
| `textPoints(text, count, { font, weight, width, depth })` | Sample points from rendered text. |
| `glow.render(scene, source, { size, intensity, fog, lens, exposure, maxRadius })` | Draw a frame. |
| `glow.colors(count, rgbOrFn)`, `glow.resize(w, h)`, `glow.options.trail`, `glow.lastMs`, `glow.avgMs` | Colours, size, trails, timing. |

`budget` (ms, default 7) enables adaptive quality: if rendering stays over budget, the pixel resolution steps down (to `minResolution`) so slower machines keep their frame rate.

## Capture: screenshots and recordings

```ts
const png = await screenshot('#card');                          // Blob (canvas, element or page)
const rec = await record('#hero', { duration: 2, fps: 30 });     // frame-exact GIF by default
rec.download('hero.gif');
await record(canvas, { duration: 3, format: 'webm' });           // or 'frames' (PNG blobs)
await record('#stage', { animation: tl });                       // seek a timeline per frame
```

Recordings are deterministic: engine time is switched to manual and advanced exactly `1 / fps` per frame, however slow the machine. GIFs are encoded by the core (median-cut palettes with exact slots for frequent colours, changed-rectangle delta frames, LZW). `GifWriter` and `encodeGif(frames, w, h, fps)` work in Node too.

Headless, from the command line (Playwright + Chromium):

```bash
npx anikit-record ./index.html --out hero.gif --duration 3 --fps 30 --selector '#hero'
```

```bash
npx anikit-record https://example.com --out clip.mp4 --size 1280x720
```

```bash
npx anikit-record ./index.html --screenshot shot.png --at 1.5
```

Output can be `.gif`, `.mp4` or `.webm` (ffmpeg), or a directory of PNG frames. Local files are served over a fake origin, so ES modules work without a server.

## Testing and diagnostics

```ts
import { testing, engine } from 'anikit';

testing.useFakeTime();              // no rAF: time only moves when you say
animate(card, { x: 100 }, { duration: 1, ease: 'linear' });
testing.advance(0.5);               // card.style.transform === 'translate(50px, 0px)'
testing.finishAll();                // run until idle
const warnings = testing.captureWarnings();
engine.inspect();                   // [{ id, state, time, duration, progress, type, target, props }]
testing.reset();
```

- Warnings start with `anikit:` and include "did you mean" suggestions for misspelled easings, properties and effect names. `setWarnings(false)` silences them; `onWarning(fn)` routes them.
- `engine.inspect()` returns a JSON-serialisable snapshot of everything running; it's also reachable as `globalThis.__ANIKIT__` for agents driving a headless browser.
- `engine.speed` scales every animation, world, point cloud and `onFrame` `dt` together (bullet time).
