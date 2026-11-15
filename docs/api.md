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
