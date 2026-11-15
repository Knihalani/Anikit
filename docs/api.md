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

- `engine.speed = 0.5`: global time scale;
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
