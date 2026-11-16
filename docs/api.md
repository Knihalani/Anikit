# Anikit TypeScript API

Times are in **seconds**. Every function accepts targets as a CSS selector, an element, a list/NodeList, a plain object, or an array mixing these.

```ts
import { animate, timeline, stagger, spring, inertia, scroll, inView, splitText, flip,
         compositor, batch, set, from, fromTo, engine, ready } from 'anikit';
// or: import ... from 'anikit/lean'  (streams anikit.wasm instead of embedding it)
```

The WebAssembly core starts compiling on import. Calls made before it's ready are queued and run in order, so you rarely need it, but `await ready` is available. Functions that return a computed value right away (`ease()`, `morph()`, `pathLength()`, `staggerValues()`, `world()`, …) need the core loaded: with `anikit/lean`, `await init()` (or `await ready`) first.

**Units:** times are **seconds** everywhere (`duration`, `delay`, `stagger(0.05)`, `seek(1.2)`), except the low-level clock: `engine.tick(ms)`, `engine.time` and `engine.onFrame((timeMs, dtMs) => …)` are milliseconds. Numbers default to `px` for lengths, `deg` for angles; `scale` and `opacity` are unitless (opacity 0–1).

Task-oriented examples: [recipes.md](recipes.md). Mistakes are reported as `anikit:` warnings with fixes (see [Testing and diagnostics](#testing-and-diagnostics)).

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

**Values:** numbers (with the default unit: `px` for lengths, `deg` for angles), strings with units, colours in any CSS syntax, complex strings (`'0 4px 12px rgba(0,0,0,.2)'`), discrete strings, `null` for "the current value", relative values (`'+=10'`, `'-=10'`, `'*=2'`) and `(target, index, count) => value` functions (which may return keyframes: `y: (el, i) => [0, i * 10]`). Options may also be written inside the keyframes object (GSAP style).

**Special properties**

| Property | Meaning |
|---|---|
| `x y z translateX/Y/Z rotate rotateX/Y/Z scale scaleX/Y/Z skew skewX/Y perspective` | Transform components, composed into one `transform` per element per frame. |
| `'--name'` | CSS custom property. |
| `attr: { r: 20, cx: 50 }` | Attributes. Non‑CSS attributes of SVG elements are detected automatically. |
| `d: pathData \| element \| selector` | Shape morph between any two paths. |
| `path: pathData \| { path, start?, end?, rotate?, align?, alignOrigin? }` | Motion along a path. `rotate: true` (or an angle offset) follows the tangent. `align: true` puts the target exactly on the path as drawn on the page (SVG user space, viewBox and all, mapped to the target's coordinates), with `alignOrigin` (default `[0.5, 0.5]`) on the line; pass an SVG element as `align` when `path` is raw data in its coordinates. |
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
| `autoplay` | `true` | `false` for scroll triggers and manual control. |
| `stagger` | | Shorthand for `delay: stagger(n)`; GSAP's `{ each, from, grid, amount }` works too. |
| `loop` | | `true` repeats forever (`repeat: Infinity`). |
| `onStart`, `onUpdate`, `onComplete`, `onRepeat`, `onReverseComplete` | | `onUpdate` fires once per frame, after DOM writes. |

`from(targets, values, options)`, `fromTo(targets, fromValues, toValues, options)` and `set(targets, values)` (instant) are shorthands.

```ts
from('.card', { y: 40, opacity: 0 });                            // from these values to the current ones
fromTo('.bar', { scaleX: 0 }, { scaleX: 1 }, { duration: 0.8 });
set('.panel', { x: -20, opacity: 0 });                           // instantly, no animation
```

### Controller (`Animation`, `Timeline`, `Batch`)

`play()`, `pause()`, `reverse()`, `restart()`, `seek(seconds)`, `complete()` (jump to the end), `cancel()` (pause and return to the start), `kill()`/`stop()` (stop where it is), `revert()` (stop and restore the original inline styles). Properties: `progress` (get/set, 0–1), `time` (get/set, seconds), `duration`, `speed` (get/set), `state` (`'idle' | 'running' | 'paused' | 'finished'`) and `iteration`. Controllers are thenables: `await animate(...)`, or use `.finished`. All of these are `Playable`s.

```ts
const a = animate('.box', { x: 200 }, { duration: 1, autoplay: false });
a.progress = 0.5;            // scrub by hand
await a.play();              // resolves when finished
const css = a.toCSS({ name: 'slide', selector: '.box' });   // see "Bake to CSS"

// The classes, for types and instanceof checks:
const all: Array<Animation | Timeline | Batch> = [a, timeline(), batch(10, { x: [0, 1] })];
all.forEach((c) => c.pause());
const s: PhysicsAnimation = spring('.dot', { x: 100 });
const c: CompositorAnimation = compositor('.toast', { opacity: [0, 1] });
await Promise.all([s, c]);   // both are awaitable too
```

`spring()`, `inertia()` and `physics()` return a `PhysicsAnimation` (`velocity`, `finished`, `then`, `kill()`/`stop()`, `revert()`); `compositor()` returns a `CompositorAnimation` (`play`, `pause`, `reverse`, `seek`, `speed`, `progress`, `kill`, `finished`).

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

```ts
ease('outCubic', 0.5);                       // 0.875
const bounce = easing('spring(bounce: 0.3)');   // (t) => number, for your own maths
el.style.transition = `transform 0.6s ${cssEasing('spring(bounce: 0.3)')}`;   // 'linear(…)' ('ease-out' where linear() is unsupported)
```

Unknown names throw with a suggestion (`'outCubik'` → "Did you mean "outCubic"?"). In TypeScript, `EasingName` autocompletes every named easing while any string is still accepted.

---

## Stagger

`stagger(each, options?)` returns a function value: use it for `delay` (seconds between targets) or any numeric property.

```ts
import { animate, stagger } from 'anikit';
animate('.cell', { scale: [0, 1] }, { delay: stagger(0.04, { grid: [10, 6], from: 'center' }) });   // ripples out of the middle
animate('.bar', { y: [40, 0] }, { delay: stagger(0.05, { start: 0.3, ease: 'inQuad' }) });          // base delay + eased spread
animate('.dot', { x: stagger(20) });                                                                // 0, 20, 40 … px
```

Options: `from` (`'first'` default, `'last'`, `'center'`, `'edges'`, `'random'` or an index), `grid: [columns, rows]` with optional `axis: 'x' | 'y'`, `total` (spread over this many seconds instead of `each` per step), `start` (added to every value), `ease` (across the distribution), `seed` (for `'random'`). `staggerValues(count, each, options)` returns the numbers directly:

```ts
staggerValues(4, 0.1, { from: 'center' });    // [0.15, 0.05, 0.05, 0.15]
```

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

With no position, a child goes at the end of the timeline. The position may also go straight after the properties, GSAP style: `tl.add('.b', { x: 100 }, '-=0.2')`. Options: `delay`, `repeat`, `repeatDelay`, `yoyo`, `speed`, `ease` (applied to the overall progress), `autoplay`, `defaults` and the callbacks (`onStart`, `onUpdate`, `onComplete`, `onRepeat`, `onReverseComplete`). `onUpdate` fires once per frame while the playhead moves (playing or seeking), after that frame's DOM writes. Sequential tweens of the same property chain their start values.

Children: `add(targets, props, options?, position?)` (alias `to`), `from(targets, fromProps, options?, position?)`, `fromTo(targets, fromProps, toProps, options?, position?)`, `set(targets, props, position?)`, `call(fn, position?)`, `label(name, position?)`, and `add(animationOrTimeline, position?)` to nest. `tl.labels` is a `Map` of label → seconds.

---

## Physics

```ts
const s = spring(el, { x: 200, y: 50 }, { stiffness: 200, damping: 20 });   // or { bounce: .3, duration: .5 }
spring(el, { x: 0 });               // retarget: position and velocity are preserved
s.velocity;                          // { x: …, y: … } in units/s
await s;                             // resolves at rest

inertia(card, { x: { velocity: vx, min: 0, max: 600, snap: 100 } }, { power: 0.8, timeConstant: 0.35 });
```

`spring(targets, props, options?)`: props are end values (numbers or strings with units, colours too). Options: `stiffness` (default 100), `damping` (10), `mass` (1), or the perceptual pair `bounce` (0–1) + `duration` (seconds); `velocity` (units/s, a number or per-property map), `restDelta`, `restSpeed`, `onUpdate`, `onComplete`. There is no `delay` or `ease`: call it later, or use `animate()` with `ease: 'spring(…)'` for a timed spring. `inertia()` adds `power` (0.8), `timeConstant` (0.35 s), `bounceStiffness` and `bounceDamping`; each prop is `{ velocity, min?, max?, snap? }` or a bare velocity. All three return a `PhysicsAnimation`.

---

## Scroll & viewport

```ts
const stop = scroll(animate('.bar', { scaleX: [0, 1] }, { ease: 'linear' }));       // page progress
scroll(animate(img, { y: [-80, 80] }), { target: img, offset: ['start end', 'end start'], smooth: 0.1 });
scroll((p) => (counter.textContent = Math.round(p * 100) + '%'), { container: panel });
inView('.reveal', (el) => { animate(el, { opacity: [0, 1] }); return () => animate(el, { opacity: 0 }); }, { amount: 0.3 });
scroll(compositor('.hero', { scale: [1, 1.15], opacity: [1, 0] }, { ease: 'linear' }), { target: hero, offset: ['start start', 'end start'] });
```

Give `scroll()` a `compositor()` animation and it's attached to a native `ScrollTimeline` where the browser has one: the browser scrubs it in step with the scroll, and no JavaScript runs while scrolling. The `offset` range is measured from layout (and again on resize), so it means the same as on the JavaScript path. Without `ScrollTimeline`, or with `smooth`, it's scrubbed from the scroll event instead (`native: false` forces that).

`compositor()` animations honour `engine.reducedMotion` (they jump to the end), and `revert()` cancels them and puts back the inline styles from before.

---

## SVG

`morph(a, b) → (t) => d`, `morphPlan(a, b)`, `morphSequence([a, b, c]) → { template, frames }` (one shared template for N shapes), `pathData(src)` (path data of a string, element or selector), `pathLength(p)`, `pathBounds(p) → [minX, minY, maxX, maxY]`, `normalizePath(p)`, `shapeToPath(el)` and `new MotionPath(p).at(t) → { x, y, angle }`. Shape builders return path data: `circle(cx, cy, r)`, `ellipse(cx, cy, rx, ry?)`, `rect(x, y, w, h, rx?, ry?)`, `polygon(points, closed?)` and `star(cx, cy, points, outer, inner?, rotation?)`.

```ts
import { animate, morph, circle, star, MotionPath } from 'anikit';
animate('#blob', { d: star(50, 50, 5, 40, 18) }, { duration: 0.8, ease: 'inOutCubic' });  // morph a <path>
const at = morph(circle(50, 50, 40), star(50, 50, 6, 45, 20));   // at(0.5) → path data halfway
animate('#dot', { path: { path: '#track', rotate: true } }, { duration: 3, ease: 'linear' });
const { x, y, angle } = new MotionPath('#track').at(0.25);
animate('#line', { draw: [0, 1] }, { duration: 1.2 });            // stroke draws in
```

Path utilities (all return path data strings or numbers; a source is path data, an SVG element or a selector):

```ts
pathLength('M0,0 L100,0');                       // 100
pathBounds(circle(50, 50, 40));                  // [10, 10, 90, 90]
pathArea(rect(0, 0, 10, 20));                    // 200
pathData('#logo path');                          // the d of an element (shapes are converted)
shapeToPath(document.querySelector('circle')!);  // <circle>/<rect>/<ellipse>/<line>/<poly*> → path data
normalizePath('M0 0 H10 V10 Z');                 // absolute cubic segments: 'M0 0C3.333 0 …Z'
ellipse(50, 50, 40, 20); polygon([[0, 0], [10, 0], [5, 8]]);
trimPath('M0,0 L100,0', 0.25, 0.75);            // the middle half: 'M25 0C41.667 0 58.333 0 75 0'
transformPath('M0 0 L10 0', [2, 0, 0, 2, 5, 5]);   // apply a 2D matrix [a, b, c, d, e, f]
pathThrough([[0, 0], [50, 40], [100, 0]], { curviness: 1 });   // smooth curve through points
simplifyPath(d, 1);                              // fewer points within 1 unit
flattenPath('M0,0 L10,0 L10,10');                // [[[0, 0], [10, 0], [10, 10]]] polylines per subpath
const plan = morphPlan(circle(0, 0, 10), rect(0, 0, 10, 10));      // { template, from, to }
const seq = morphSequence([circle(0, 0, 10), rect(0, 0, 10, 10), star(0, 0, 5, 10)]);   // { template, frames }
```

---

## Text, layout, compositor, batches

```ts
const { chars, words, lines, revert } = splitText('h1', { type: 'lines,words,chars' });
animate(chars, { y: ['100%', 0], opacity: [0, 1] }, { delay: stagger(0.02) });

// Lines rising from behind a mask; re-split (and the animation handed over) when the width changes
splitText('.lede', { type: 'lines', mask: 'lines', autoSplit: true,
  onSplit: ({ lines }) => animate(lines, { yPercent: [100, 0] }, { delay: stagger(0.08), ease: 'outCubic' }) });

flip('.item', () => grid.classList.toggle('dense'), { duration: 0.6, ease: 'spring(bounce: .2)' }); // see "Layout transitions"

compositor('.toast', { y: [20, 0], opacity: [0, 1] }, { ease: 'spring(bounce: .4)' }); // compositor thread

const b = batch(50_000, { x: (i) => [0, i % 500], y: [0, 300], a: [0, 1] },
                { duration: 2, delay: stagger(0.00005), onUpdate: () => draw(b.data, b.stride) });
// b.data: Float32Array (count * stride), e.g. gl.bufferSubData(gl.ARRAY_BUFFER, 0, b.data)
```

- `splitText(targets, { type: 'chars' | 'words' | 'lines' (comma separated, default 'words,chars'), charClass, wordClass, lineClass, mask, maskClass, autoSplit, onSplit }) → { chars, words, lines, masks, split(), revert() }`. Characters are grapheme-safe (emoji stay whole).
- `splitText` options: `mask: 'lines' | 'words' | 'chars'` wraps each unit in an `overflow: clip` element (returned in `masks`; class via `maskClass`). `autoSplit: true` splits again when an element's width changes and once web fonts have loaded, so lines match the layout. `onSplit(result)` runs after every split; return the animation that uses the pieces, and on a re-split it's reverted and the new one is moved to the same time. `result.split()` re-splits on demand, and `revert()` restores the markup (a `context()` does this too).
- `flip(targets, mutate, options?)` measures, runs `mutate()` (your DOM/class change), then animates from the old layout; see [Layout transitions](#layout-transitions-flip) for enter/leave, shared elements and presence.
- `compositor(targets, props, options?)` runs on the browser's compositor thread (Web Animations) with anikit easings, springs included; keeps going when the main thread is busy.
- `batch(count, props, options?) → Batch`: `data` (Float32Array, `count * stride`), `stride`, `channels` (`{ x: 0, y: 1, … }`), `get(i, prop)`. A prop is a shared keyframe list or `(index, count) => keyframes`.


---

## Layout transitions: `flip`

`flip(targets, mutate, options?)` records where elements are, runs `mutate`, and animates every element from its old box to its new one using only transforms. It returns a `Timeline` holding every animation it started, so `await flip(...)` waits for all of them.

```ts
flip('.item', () => grid.classList.toggle('dense'), { duration: 0.6, ease: 'spring(bounce: .2)' });
flip('.card', () => applyFilter(), { fade: true });                    // removed cards fade out in place, new ones fade in
flip('.card', () => list.prepend(card), { enter: { scale: [0.6, 1], opacity: [0, 1] }, leave: { scale: 0.6, opacity: 0 } });
flip('[data-flip-id]', () => { thumb.hidden = true; modal.hidden = false; });  // shared element: modal grows out of the thumbnail
const state = flip.capture('.item'); reorder(); flip.from(state, { duration: 0.4 }); // manual capture
```

- **Re-queried targets:** when `targets` is a selector it's queried again after `mutate`, so elements added by the change are found. Pass `targets` in the options to name them explicitly.
- **Entering elements** (new, or hidden before) play `enter` keyframes, or `{ opacity: [0, 1] }` with `fade: true`, or whatever `onEnter(els)` returns.
- **Leaving elements** (removed from the DOM or hidden by the change) are put back at their last box with `position: absolute`, so their siblings reflow straight away. They play `leave` (or a fade, or `onLeave(els)`) and are then removed or hidden for real, with their inline styles restored. If a later change brings one back mid-exit, the exit is cancelled and it animates from where it is.
- **Shared elements:** two different elements with the same `data-flip-id` (or the same `id(el)`) are treated as one: the incoming one starts on the outgoing one's box. If both stay visible they cross-fade (`crossfade: false` to turn it off).
- **Interrupted flips** start from where the elements are on screen and end on their own transforms.
- `scale: false` animates position only. `correctRadius: true` counter-scales `border-radius` every frame so rounded corners don't stretch while an element scales. It's off by default because it adds a style recalc and a repaint per element per frame, while transforms alone stay on the compositor.
- Layout is read once for all elements (transforms off, measure, transforms back) rather than once per element.

## Presence: enter and exit

The DOM has no "about to be removed" phase. These helpers animate first and remove afterwards, and cope with an element coming back halfway through its exit.

```ts
exit('.toast', { opacity: 0, y: 20 }, { duration: 0.3 });  // animate out, then remove (after: 'hide' sets `hidden` instead)
enter('.panel', { opacity: [0, 1], y: [12, 0] });            // un-hide and animate in; turns a running exit() around

const list: Presence = presence('#todos', { enter: { opacity: [0, 1], y: [-8, 0] }, exit: { opacity: 0, scale: 0.9 }, duration: 0.3 });
list.add(item);                         // insert (at the end, or before a node) and animate in
list.remove(item);                      // animate out, then remove
list.toggle(item, false);               // same, but keep it in the DOM with `hidden`
await list.replace(oldView, newView, { mode: 'wait' });  // 'sync' (default) runs both at once
```

With `layout: true` (the default) siblings slide into their new places through `flip()`, and a leaving child leaves the flow at once, pinned where it was while it animates out. With `layout: false` a leaving child stays in the flow until it's removed. Adding a child that is still animating out turns it around from where it is. Reduced motion skips the animations but still inserts and removes.

## View Transitions

`viewTransition(update, options?)` runs `update` (which changes the DOM) through `document.startViewTransition`, so the browser cross-fades and morphs snapshots on the compositor. Anikit adds:

- **Timing:** `duration`, `delay` and `ease` apply to every transition group. Any Anikit easing works, and springs and custom curves become native `linear()`.
- **Temporary names:** `names: { hero: el }` gives an element a `view-transition-name` for this transition only. A pair `[before, after]` names one element in the old state and another in the new one, so the browser morphs one into the other.
- **Custom snapshots:** `old` / `new` take WAAPI keyframes for the outgoing and incoming snapshot of `scope` (default `'root'`).
- **Fallback:** without the API, the named elements are animated with `flip()` (`fallback: 'none'` just runs the update). With reduced motion the update runs straight away.

```ts
await viewTransition(() => openDetail(id), {
  duration: 0.5, ease: 'spring(bounce: 0.2, duration: 0.5)',
  names: { hero: [thumb, () => document.querySelector('.detail img')] },
});
viewTransition(() => route(next), { old: { opacity: [1, 0] }, new: { transform: ['translateY(24px)', 'none'], opacity: [0, 1] }, types: ['forward'] });
```

The handle mirrors `ViewTransition`: `updateCallbackDone`, `ready`, `finished`, `skipTransition()`, plus `native` (whether the browser API ran it). Awaiting it waits for `finished`.

## Engine

`engine.manual(true)` with `engine.tick(ms)` drives the engine yourself (tests, video export). Also available:

- `engine.speed = 0.5`: global time scale (also scales physics worlds, point clouds and `onFrame` `dt`);
- `engine.lagSmoothing(threshold = 500, adjusted = 33)` (ms): a frame gap above `threshold` (tab switch, debugger) advances only `adjusted` ms; `lagSmoothing(0)` turns it off;
- `engine.fps = 30`: frame‑rate cap;
- `engine.onFrame((timeMs, dtMs) => …)`: per‑frame callback after the frame's DOM writes, in **milliseconds**; returns an unsubscribe function;
- `engine.time`: elapsed engine time (ms);
- `engine.reducedMotion = 'user'`: honour the OS setting (default `'never'`);
- `engine.inspect()`: JSON snapshot of everything running (see [Testing and diagnostics](#testing-and-diagnostics));
- `engine.stats()`: `{ active, nodes, time }`.

```ts
const off = engine.onFrame((time, dt) => { ctx.clearRect(0, 0, w, h); drawParticles(dt / 1000); });   // dt in ms
off();
flush();   // apply pending values right now (e.g. before measuring layout)
```

`flush()` applies pending results synchronously. `ready` is a promise for the core; `init(urlOrBytes?)` loads it from a URL or bytes instead of the embedded copy (`anikit/lean` does this from `./anikit.wasm` automatically).

```ts
await ready;                              // the core is loaded (rarely needed: calls queue until then)
await init('/assets/anikit.wasm');        // anikit/lean: load the module from your own URL, before other calls
```

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
animate(panel, { height: open ? 'auto' : 0 }, { ease: 'outCubic' });            // accordions
resolveRandom('random(0, 100, 5)');                                            // the same parser, standalone
```

Complex values (shadows, filters, gradients, `clip-path`, anything with numbers and colours in it) interpolate number by number. Two refinements apply:

- **Colours inside them** blend like solid colours: in `colorSpace` (OKLab by default) with premultiplied alpha, so a shadow fading in from `transparent` doesn't go dark on the way.
- **Lists that don't line up as written are reconciled:**
  - Filter lists get the missing functions at their identity values (`'none'` → `'brightness(1.5) blur(4px)'` starts from `brightness(1) blur(0px)`, not black). Percentages become numbers, and bare zeros get units.
  - Shadow lists are rewritten as `[inset] x y blur spread color`, which matches the colour-first form that computed styles use, and shorter lists are padded with transparent shadows.

```ts
animate(card, { boxShadow: '0 12px 32px rgba(0,0,0,.25), inset 0 0 0 1px #fff3' });  // from the stylesheet's shadow
animate(img, { filter: 'blur(4px) saturate(2)' });                                    // from 'none' or a different list
animate(hero, { backgroundImage: 'linear-gradient(90deg, #f0c, #0cf)' });             // stops blend in OKLab
```

`'auto'` works for `height`, `width`, `maxHeight`, `maxWidth`, `blockSize` and `inlineSize`, as a target, as a start value, or as a keyframe. The size is measured when the animation is built, and the property is written back as `auto` (`none` for max sizes) whenever the value sits on that size, so the element stays responsive after the animation ends or is reversed. Add `overflow: hidden` (or `clip`) yourself if the content shouldn't spill while it grows.

## Control, cleanup and accessibility

- `anim.revert()` kills the animation and restores every property it touched to its original inline value. `timeline.revert()` does the same for every child.
- `context(fn, scope?)` collects everything created inside `fn` (animations, timelines, scroll triggers, draggables, observers…), scopes selector lookups to `scope`, and offers `ctx.revert()` / `ctx.kill()`. `fn` may return a cleanup function. It's the natural fit for component lifecycles in React, Vue or Svelte. The returned `Context` also has `ctx.add(fn)` to collect more later (e.g. in event handlers) and `ctx.items`.

```ts
// React
useLayoutEffect(() => {
  const ctx = context(() => {
    animate('.card', { y: [24, 0], opacity: [0, 1] }, { delay: stagger(0.05) });   // '.card' inside rootRef only
  }, rootRef.current);
  return () => ctx.revert();
}, []);
```
- `matchMedia().add('(min-width: 800px)', setup)` runs `setup` while the query matches and reverts it automatically when it stops matching. `add({ wide: '…', reduce: '(prefers-reduced-motion: reduce)' }, ({ wide, reduce }) => …)` takes several named conditions.
- `engine.reducedMotion = 'user' | 'always' | 'never'`: when motion is reduced, animations jump straight to their end state, while callbacks and promises still fire.
- `engine.reducedMotionMode = 'fade'` takes out only the movement: transforms, motion paths, shape morphs and layout properties (sizes, offsets, margins, SVG geometry attributes) hold their end values, while opacity, colours and filters animate with their normal timing. With it, `flip()` and `presence()` still cross-fade, but nothing slides. The default, `'instant'`, jumps everything. `compositor()` animations always jump. In both modes `spring()` arrives at once, and `inertia()`, `draggable` throws and `gesture` flings don't glide.
- `quickSetter(el, 'x')` returns a fast setter; `quickTo(el, 'x', opts)` returns a function that re‑targets a tween on each call.
- `delayedCall(seconds, fn)` runs `fn` on engine time, so it respects the global speed and pausing.

```ts
const call = delayedCall(1.5, () => effects.pulse('.badge'));   // a Timeline: call.kill() cancels it
const setX = quickSetter('.cursor', 'x');                       // fastest way to write one property often
addEventListener('pointermove', (e) => setX(e.clientX));
const ctx: Context = context(() => {}, document.querySelector('#app'));
ctx.add(() => animate('.late', { opacity: [0, 1] }));          // collect more later; ctx.revert() undoes all
```

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

Options for specific effects: `countUp` takes `to`/`start` (plus `format`), `typewriter`/`scramble` take `text` (and `charDelay`, `delimiter`, `chars`), `highlight` takes `color`, `flipIn`/`flipOut` take `axis`. `effect('name', targets, options)` runs one by name, and `effectNames()` lists every registered effect.

```ts
effects.slideIn('.menu li', { from: 'left', stagger: 0.04 });
effects.countUp('#total', { to: 12500, format: { style: 'currency', currency: 'USD' } });
effects.typewriter('#hello', { text: 'Hello, world' });
registerEffect('glow', (t, o) => animate(t, { filter: ['drop-shadow(0 0 0 gold)', `drop-shadow(0 0 ${o.size}px gold)`] }, o), { size: 12 });
effect('glow', '.star', { size: 20 });   // by name
effectNames();                           // ['fadeIn', 'fadeOut', …, 'glow']
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
const st: ScrollTrigger = scrollTrigger({ trigger: '.chart', start: 'top 70%', onEnter: () => effects.drawSVG('.chart path') });
st.progress; st.isActive;
refreshScrollTriggers();                                  // after you change the layout yourself
const swipe = observer({ type: 'wheel,touch', tolerance: 20, onDown: () => next(), onUp: () => prev() });
swipe.kill();
```

One passive listener per scroller drives every trigger, and layout is measured only on refresh (load and resize). Call `refreshScrollTriggers()` after you change the layout yourself. Pinning uses `position: fixed` for the page and a counter‑translate inside scrolling elements.

- `animation` takes one animation or timeline; it is paused for you (create it with `autoplay: false` to avoid a first-frame flash). For one trigger per element, use `scrollTriggerBatch(targets, { animation: (el, i) => … })`, which takes a factory.
- `toggleActions` is four words for onEnter / onLeave / onEnterBack / onLeaveBack, each `play pause resume reverse restart reset complete none`; it is ignored with `scrub`.
- The `ScrollTrigger` instance has `progress` (0–1), `direction` (1 / -1), `isActive`, `start`/`end` (px), `refresh()`, `enable()`, `disable()`, `kill(revert?)` and `revert()`.

`observer({ target, type: 'wheel,touch,pointer,scroll', tolerance, onUp, onDown, onLeft, onRight, onChange, onDrag, onPress, onRelease, onStop })` normalises input intents. Use it for full‑page section transitions and carousels.

## Interaction

```ts
const [d] = draggable('.card', { bounds: '.board', inertia: true, snap: { x: 100, y: 100 }, edgeResistance: 0.35, lockAxis: true });
draggable('.knob', { type: 'rotate', bounds: { minRotation: -135, maxRotation: 135 }, onDrag: (d) => setVolume(d.rotation) });
hover('.btn', (el) => { animate(el, { scale: 1.05 }); return () => animate(el, { scale: 1 }); });
press('.btn', (el) => { animate(el, { scale: 0.95 }); return () => animate(el, { scale: 1 }); });
const [knob]: Draggable[] = draggable('.knob', { type: 'rotate' });
knob.setPosition(0, 0, 45);
```

`draggable()` returns one `Draggable` per element. It exposes `x`, `y`, `rotation`, `deltaX`/`deltaY`, `isDragging`, `isPressed`, `enabled`, `enable()`, `disable()`, `setPosition(x, y, rotation?)`, `kill()` and `revert()`. Options: `type` (`'x,y'`, `'x'`, `'y'`, `'rotate'`), `bounds` (element, selector or `{ minX, maxX, minY, maxY, minRotation, maxRotation }`), `inertia`, `snap` (step, list or function; per axis with `{ x, y, rotation }`), `edgeResistance` (0.35), `lockAxis`, `handle`, `minimumMovement` (3 px), `cursor`, and callbacks `onPress`, `onDragStart`, `onDrag`, `onDragEnd`, `onRelease`, `onThrowUpdate`, `onThrowComplete` (each receives the `Draggable`).

`hover(targets, fn)` and `press(targets, fn)` call `fn(el, event)` on enter/press; a function returned from `fn` runs on leave/release. Both return `{ kill, revert }`. Hover ignores touch, so nothing gets stuck.

### Pinch, rotate and pan: `gesture`

```ts
const [g]: Gesture[] = gesture('#photo', { minScale: 1, maxScale: 6, bounds: { minX: -400, maxX: 400, minY: -300, maxY: 300 } });
gesture('.sticker', { type: 'pinch,rotate', onChange: (g) => save(g.scale, g.rotation) });
gesture(canvas, { apply: false, onChange: ({ x, y, scale }) => view.set(x, y, scale) });   // drive anything
```

Two fingers zoom and rotate about their midpoint, and the point under the fingers stays there. One finger pans, and lifting a finger mid-gesture carries on smoothly with the other. Ctrl/⌘ + wheel (trackpad pinch) zooms about the pointer. A plain wheel still scrolls.

- **Limits:** `minScale`, `maxScale` and `bounds` are rubber-banded while held (`edgeResistance`, default 0.35; scale resists in log space). On release the scale springs back about the last finger position, or the element glides with momentum (`inertia: false` to stop dead).
- **Options:** `type` is any of `'pan,pinch,rotate'`. `apply: false` only reports through callbacks: `onStart`, `onChange`, `onEnd` (fingers up) and `onRest` (settled).
- **The `Gesture` object** has `x`, `y`, `scale`, `rotation`, `pointers`, `isActive`, `set({ x, y, scale, rotation })`, `enable()`, `disable()`, `kill()` and `revert()`.

## Dev tools

`inspector(animationOrTimeline)` opens a floating panel to play, pause, scrub, restart, loop, change speed and jump to labels. Space toggles playback.

```ts
const panel = inspector(timeline().add('.a', { x: 100 }).label('end'), { title: 'Intro', paused: true });
panel.destroy();
```

## More utilities

| Function | Purpose |
|---|---|
| `interpolate([0, 100, 200], [0, 1, 0], { ease, clamp })` | Map through stops. Outputs can be numbers, colours or values with units. |
| `distribute({ base, amount, from, grid, ease })` | Spread any value across targets. |
| `noise2(x, y, seed)`, `noise3`, `fbm(x, y, z?, { octaves })`, `noiseMany(xyz)`, `wander({ speed, amplitude })` | Seeded simplex noise. |
| `trimPath(d, 0.2, 0.8)`, `pathThrough(points, { curviness, closed })`, `simplifyPath(d, tolerance, smooth)`, `pathArea(d)`, `flattenPath(d, tol)`, `transformPath(d, [a, b, c, d, e, f])` | Path operations. |
| `MotionPath.closest(x, y)`, `.atLength(px)`, `.points(n)` | Path queries. |
| `VelocityTracker`, `shuffle`, `pipe`, `splitColor`, `getUnit`, `roundTo`, `normalize`, `toArray` | Helpers. |

```ts
// Numbers
clamp(150, 0, 100);                 // 100
lerp(0, 10, 0.25);                  // 2.5
mapRange(5, 0, 10, 100, 200);       // 150
normalize(25, 0, 200);              // 0.125
wrap(370, 0, 360);                  // 10
snap(37, 10); snap(37, [0, 25, 50]);   // 40, 25
roundTo(3.14159, 2);                // 3.14
random(0, 10); random(0, 10, 42);   // seeded when given a seed
shuffle([1, 2, 3, 4], 1);           // seeded shuffle (copy)
pipe((v: number) => v * 2, (v: number) => clamp(v, 0, 10))(7);   // 10

// Colours and units
mixColor('#ff0000', '#0000ff', 0.5);          // 'rgb(140,83,162)' (OKLab); pass 'srgb' for 'rgb(128,0,128)'
parseColor('rebeccapurple');                  // [102, 51, 153, 1]
splitColor('#ff000080');                      // [255, 0, 0, 0.502]
getUnit('12rem');                             // 'rem'
interpolate([0, 100, 200], [0, 1, 0])(150);   // 0.5
interpolate([0, 100], ['#000', '#fff'])(50);  // 'rgb(99,99,99)'
interpolate([0, 1], ['0px', '100px'], { ease: 'inQuad', clamp: true })(0.5);

// Targets and distribution
toArray('.card');                                                   // resolved targets, as an array
animate('.bar', { scaleY: distribute({ base: 0.5, amount: 1, from: 'center' }) });   // spread values, not delays

// Noise (seeded simplex, −1…1) and velocity
const n2 = noise2(x * 0.01, y * 0.01);
const n3 = noise3(x * 0.01, y * 0.01, t, 7);
const f = fbm(x * 0.01, y * 0.01, undefined, { octaves: 4 });
const many: Float64Array = noiseMany([0.1, 0.2, 0.3, 1.1, 1.2, 1.3]);   // one value per xyz triple
const drift = wander({ speed: 1, amplitude: 20 });                       // drift(t) → smooth −20…20
animate('.leaf', { x: (_el: Element, i: number) => drift(i) });
const vt = new VelocityTracker();             // window: 100 ms
addEventListener('pointermove', (e) => vt.add(e.clientX));   // timestamps default to performance.now() (ms)
vt.velocity;                                  // units per second
```

---

## Physics worlds

`world(options?) → World` is a 2D rigid-circle physics world solved in WebAssembly: position-based dynamics with sub-steps, a sorted uniform grid for broad phase, restitution and friction, distance constraints (ropes, cloth, soft bodies, tearing), static boxes, attractors and picking. It steps on the engine clock, so `engine.speed` slows it too.

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
const typed: World = w;                     // the class, for types
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

`mode: 'ink'` renders particles as pigment absorbing a light background (`bg / (1 + c)` per channel) instead of adding light; point colours are then absorption per channel (`[1, 1, 1]` = black ink, `[0.05, 0.78, 0.95]` = vermilion). Both `glow.options.mode` and `glow.options.background` can change every frame, and `render({ tint: [r, g, b] })` recolours every point at no cost.

`budget` (ms, default 7) enables adaptive quality: if rendering stays over budget, the pixel resolution steps down (to `minResolution`) so slower machines keep their frame rate.

## Capture: screenshots and recordings

```ts
const png = await screenshot('#card');                          // Blob (canvas, element or page)
const rec = await record('#hero', { duration: 2, fps: 30 });     // frame-exact GIF by default
rec.download('hero.gif');
await record(canvas, { duration: 3, format: 'webm' });           // or 'frames' (PNG blobs)
await record('#stage', { animation: tl });                       // seek a timeline per frame
const pixels: HTMLCanvasElement = await rasterize('#card', { scale: 2, background: '#fff' });
const gif = new GifWriter(320, 240);                              // incremental, also in Node
gif.add(new Uint8Array(320 * 240 * 4), 0.1);                      // RGBA pixels, seconds on screen
const bytes: Uint8Array = gif.finish();
const same = encodeGif([new Uint8Array(320 * 240 * 4)], 320, 240, 10);   // frames, width, height, fps
```

`rasterize(target, { scale, background }) → Promise<HTMLCanvasElement>` is the step both use: it draws a canvas, an element or the page into a canvas. `record()` resolves to `{ blob, url, frames, width, height, images?, download(name) }`; options are `duration` (s), `fps`, `format: 'gif' | 'webm' | 'frames'`, `animation` (seek this per frame instead of running the engine), `scale`, `background`, `loop`, `realtime` and `onProgress`.

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

await testing.ready;                // the core (once per test file)
testing.useFakeTime();              // no rAF: time only moves when you say
const warnings = testing.captureWarnings();   // before the code under test
animate(card, { x: 100 }, { duration: 1, ease: 'linear' });
testing.advance(0.5);               // card.style.transform === 'translate(50px,0px)'
engine.inspect();                   // [{ id, state, time, duration, progress, type, target, props }]
testing.finishAll();                // run until idle; returns the seconds it took
expect(warnings).toEqual([]);
testing.reset();                    // between tests: forget animations and shown warnings
setWarnings(false);                 // silence (they are off with NODE_ENV=production)
onWarning((msg) => myLogger.warn(msg));   // route them; onWarning(null) restores console.warn
```

- `testing.advance(seconds, fps = 60)` ticks in frames of `1/fps`; `testing.finishAll(maxSeconds = 60)` runs until nothing is active; `testing.inspect()` is `engine.inspect()`.
- Warnings start with `anikit:`, are shown once per unique message, and say how to fix the problem. `setWarnings(false)` silences them (they are off when `NODE_ENV=production`); `onWarning(fn)` routes them (`onWarning(null)` restores `console.warn`). `testing.captureWarnings()` returns a live array and starts afresh each call.
- Unknown easings, unknown effects, invalid selectors and wrong argument types (e.g. a function passed to `scrollTrigger({ animation })`) **throw** with a suggestion.
- `engine.inspect()` returns a JSON-serialisable snapshot of everything running; it's also reachable as `globalThis.__ANIKIT__.inspect()` for agents driving a headless browser.
- `engine.speed` scales every animation, world, point cloud and `onFrame` `dt` together (bullet time).

What gets a warning:

| Mistake | Example | Message says |
|---|---|---|
| Unknown option (every main API) | `{ durration: 1 }`, `spring(el, p, { delay: 1 })`, `stagger(0.1, { form: 'center' })` | it is ignored; did you mean `duration` / `from` |
| Another library's option name | `easing`, `iterations`, `fill`, `paused`, `playbackRate`, `onEnd` | the anikit name (`ease`, `repeat`, `fillMode`, `autoplay: false`, `speed`, `onComplete`) |
| Option typo among the properties | `animate(el, { x: 1, delya: 0.2 })` | did you mean the option `delay` |
| Milliseconds instead of seconds | `duration: 600`, `delay: 250`, `stagger(50)` | did you mean `0.6` |
| Wrong unit or range | `rotate: '45px'`, `x: '10deg'`, `scale: '2px'`, `opacity: 50` | the expected unit; `0.5` |
| `left`/`top` on a `position: static` element | `animate(el, { left: 100 })` | it has no effect; use `x`/`y` |
| Not a number for a transform | `x: '10px 20px'` | it was skipped; use `transform` |
| Unknown property | `animate(el, { colour: 'red' })` | did you mean `color` |
| Empty targets | `animate(null, …)`, `animate('hero', …)` with `.hero` on the page, an empty list | a `querySelector` returned null; did you mean `.hero`; the document is still loading |
| Bad enumerations | `colorSpace: 'okl'`, `direction: 'alternative'`, `fillMode: 'forward'`, `stagger(…, { from: 'centre' })`, `splitText(el, { type: 'letters' })` | the valid values |
| Scroll positions | `start: 'top middle'`, `toggleActions: 'play none none reversed'`, `toggleActions` with `scrub`, `pin: true` without a trigger | the accepted grammar |
| Argument order habits | `animate(el, props, 0.5)`, `animate(el, 'fadeIn')`, `stagger({ each: 0.1 })` | the anikit form (the first and last still work) |
| Path sources | `animate(dot, { path: '#missing' })` | neither path data nor a matching selector |

## Particle emitters

`emitter(options)` spawns, simulates and ages particles in WebAssembly: continuous `rate` or `burst(n)`, from a `point`, `sphere`, `box` or `ring`, launched in a cone (`direction`, `spread`, `speed`), with `gravity`, `drag`, `turbulence` and an `attractor`; size and colour over life (`sizeStart`/`sizeEnd`, up to four `colors`).

```ts
const sparks = emitter({ rate: 0, life: 1.1, speed: 1500, spread: Math.PI, gravity: [0, -500, 0], drag: 1.6,
  colors: ['#fff', '#ffd9a0', '#ff6a2a', '#200800'] });
sparks.burst(9000);
engine.onFrame(() => g.render(scene, sparks, { size: 2 }));   // light or ink, coloured by age
sparks.set({ origin: [x, y, 0] });                              // live updates
sparks.draw(ctx);                                               // or plain 2D dots
```

`positions`, `colors` and `sizes` are zero-copy views; `scene3d().project(emitter)` projects them in the core.

## Audio analysis

```ts
const a = analyzeAudio(await decodeAudio('/track.mp3'));   // or analyzeAudio(monoFloat32, sampleRate)
a.bpm; a.beats; a.onsets;                                  // tempo, beat grid, attacks (seconds)
a.at(t);                                                   // loudness 0…1
a.beatAt(t);                                               // { index, phase }
a.onBeat(audioEl, (i) => effects.pulse('.logo'));          // live sync to <audio>/<video>
```

A radix-2 FFT, spectral-flux onsets, autocorrelation tempo and a least-squares beat-grid fit run in WebAssembly.

## Video: compositions

Compose a film in code and render it frame-exactly — in the browser (WebCodecs → WebM) or in Node (Skia + ffmpeg → MP4), at any resolution and frame rate, faster than real time.

```ts
const film = composition({ width: 1920, height: 1080, fps: 60, duration: 30, motionBlur: 3 });
film.scene('intro', 0, 4, {
  enter: () => cloud.morph(1, { duration: 2 }),      // start tweens/timelines/simulations
  step: () => cloud.step(1 / 60),                    // runs every frame, drawn or not
  draw: ({ ctx, t, local, width, height }) => {
    g.render(scene, cloud); ctx.drawImage(g.canvas, 0, 0);
    typeFx(ctx, 'Motion, compiled.', 80, height - 80, { size: 120, t, start: 1, effect: 'rise' });
  },
});
film.append('next', 6, { draw: … });                  // a scene after the last one
film.cue(12.5, () => sparks.burst(4000));             // a one-off at an exact time
film.scene('next', 4, 8, {                             // overlaps the previous scene by 0.4 s
  transition: { duration: 0.4, type: 'whip', direction: 'left' },   // fade · iris · push · whip · slice · zoom · flash · mask · fn
  draw: …,
});

const webm = await exportWebM(film, { bitrate: 12e6 }); // browser
```

- Rendering runs anikit in manual time and ticks it exactly `1/fps` per frame, so output never depends on machine speed. Timelines created in `enter` start on schedule.
- `motionBlur: n` (or `{ samples, shutter }`) renders `n` sub-frames across the shutter and averages them in WebAssembly.
- `FrameBuffer` converts frames to I420 (BT.709) in WebAssembly — encoders receive 1.5 bytes/pixel and do no colour conversion.
- Transitions: the incoming scene's `transition` keeps the previous scene running for `duration`; both draw to offscreen canvases (`createCanvas` render option; supplied automatically in Node) and are composited — `mask` reveals the next scene through a growing letterform (`text`), `whip` adds a directional smear, `slice` staggers bands.
- `typeFx(ctx, text, x, y, { effect: 'rise' | 'wave' | 'type' | 'scramble' | 'fade' | 'slam', t, start, stagger, ease })` draws kinetic type deterministically.
- `muxWebM(chunks, …)` is the built-in WebM muxer used by `exportWebM`.

### Rendering in Node: `anikit/node` and `anikit-render`

```js
// film.mjs
export default async ({ anikit, node }) => {
  node.registerFont('./Inter-Black.otf', 'Display');
  const film = anikit.composition({ width: 1920, height: 1080, fps: 60, duration: 60 });
  const g = anikit.glow(node.glowTarget(1920, 1080), { mode: 'ink' });   // glow() into a Skia canvas
  …
  return film;
};
```

```bash
npx anikit-render film.mjs --out film.mp4 --workers 4 --fade-in 0.3 --fade-out 0.8   # --audio track.wav to mux a soundtrack
```

```bash
npx anikit-render film.mjs --preview 30 --out frames/
npx anikit-render film.mjs --review review/   # contact sheet + pacing report (holds, flat frames, cuts)
npx anikit-render film.mjs --profile          # where render time goes: steps, draws, each 3D pass, the grade
```

`--profile` times one frame per second (`--every N` for more) and prints a table per frame plus the mean, the biggest costs and the estimated render time. The same numbers are on every delivered frame as `frame.timing`, a `FrameTiming` (`{ step, draw, read, post, passes }`, milliseconds; `passes` splits `renderer3d` work into `setup`, `mirror`, `opaque`, `sky`, `light`, `dof`, `motionBlur`, `output`), and `r3.stats` holds the last render's passes:

```ts
const r3 = renderer3d({ width: 640, height: 360 });
const film = composition({ width: 640, height: 360, duration: 2 });
film.scene('s', 0, 2, { draw: () => { r3.render(scene3d({ width: 640, height: 360 }), []); } });
await film.render({
  canvas: document.createElement('canvas'),
  onFrame: ({ t, timing }) => console.log(t, timing.draw, timing.post, timing.passes.opaque, r3.stats.output),
});
```

Each worker builds the same composition (randomness is seeded), simulates to its slice without drawing, pre-rolls half a second so trails match, renders and encodes; slices join without re-encoding and the audio is muxed in. Needs the optional peer dependency `@napi-rs/canvas` and ffmpeg. `loadAnikit()`, `renderToFile()` and `renderPreview()` are the programmatic equivalents.

### Solid 3D: `renderer3d`, `mesh`, `instances`

A z-buffered, instanced triangle renderer in WebAssembly — no WebGL. Triangles are clipped at the near plane (fly *through* geometry), shaded **per pixel** four pixels at a time in SIMD, and tone-mapped so hot colours roll off towards white instead of clipping.

```ts
const r3 = renderer3d({ width: 1920, height: 1080 });     // or a canvas (drawn into automatically)
const towers = instances(900, (i) => ({ x, y, z, sx: 80, sy: h, sz: 80, color: [0.04, 0.045, 0.06] }));
const knot = mesh.torusKnot({ p: 2, q: 3, radius: 300, tube: 58, segments: 600, sides: 28 });
const px = r3.render(scene, [
  // Black chrome with light pulses running along the tube:
  { mesh: knot, material: { metal: 0.9, iridescence: 0.25, specular: 1.4, shininess: 120, lines: 1.2, lineColor: [0.3, 0.8, 1.6], grid: [0, 6], pulse: 2.5, pulseCount: 3, pulsePhase: t } },
  // Dark towers with glowing edges and a wave of light sweeping along z:
  { mesh: mesh.cube(), instances: towers, material: { lines: 1.6, lineColor: [1.4, 0.35, 0.9], grid: [1, 1], lineWidth: 0.035, pulse: 1.5, pulseDir: [0, 0, 0.0015], pulsePhase: t } },
  // Light: stars that streak with camera motion and hide behind the towers:
  { points: stars, colors, size: 3, intensity: 2.5, streak: 1.5, fog: [8000, 20000] },
  // Additive geometry (beams, holograms): depth-tested, not occluding:
  { mesh: beam, instances: beams, blend: 'additive', material: { emissive: 1 } },
], { background: '#04050a', fog: [1800, 5200], sky: [0.05, 0.07, 0.16], horizon: [0.9, 0.5, 0.95], ground: [0.01, 0.01, 0.02], strips: [2.2, 2.3, 2.6] });
```

**Materials** (`material`, all optional):

| Option | Effect |
|---|---|
| `ambient`, `diffuse`, `specular`, `shininess`, `rim` | Blinn–Phong key + fill light and a fresnel rim |
| `emissive` | base colour × emissive added unlit — glows under the grade's bloom |
| `metal` (0–1) | mirror reflection of the environment (`sky`/`horizon`/`ground` + sharp studio `strips`) |
| `iridescence` (0–1), `hue` | thin-film rainbow tint on reflections, shifting with viewing angle |
| `lines`, `lineColor`, `grid: [u, v]`, `lineWidth` | antialiased glowing lines in UV space; `0` cells = none that way. Cube faces are 0–1 (edge outlines with `[1, 1]`); tubes/knots run `u` along and `v` around |
| `pattern: 'hex' \| 'dots' \| 'circuit'` | what `lines` draws instead of grid lines: glowing hexagon cells, a dot matrix, or circuit traces with pads (in `grid` cells, `lineWidth` thick) |
| `pulse`, `pulseCount`, `pulsePhase`, `pulseDir` | bright pulses travelling along `u` (animate `pulsePhase`); `pulseDir` offsets each instance's phase by its position, so a wave sweeps through a field |
| `texture`, `textureMode`, `textureRepeat`, `textureGain` | an image from `texture(imageData | canvas)` mapped by UV: `'albedo'` tints the lit colour, `'emissive'` glows unlit (screens, signs); mipmapped, so it doesn't shimmer when small |

Lighting options: `key`, `keyColor`, `fill`, `fillColor`, `rimColor`, `fog: [near, far]`, `fogColor`, environment `sky`, `horizon`, `ground`, `strips`. Items draw in order; per item `cull: false` shows back faces (tumbling shards), `blend: 'additive'` adds light.

**Sky and atmosphere**: `render(…, { skybox: { sun: 2.5 }, key: [0, 0.12, -1], keyColor, sky, horizon, ground, fog, fogColor, groundFog: { bottom, top, range } })` paints every uncovered pixel with the environment gradient (meeting the fog at the horizon) and a sun disc + halo towards `key`; `groundFog` pools fog near the floor. Pair the sun with the grade's `rays` aimed at it.

**Point lights**: `render(…, { lights: [{ position, color, radius, intensity }] })` — up to 8 coloured local lights (diffuse + specular, smooth falloff) for neon pools on glossy floors, glowing trails that light the walls, muzzle flashes.

**Shadows**: `render(…, { key, shadows: { extent, center, size, strength, bias } })` renders the opaque items from the key light into a shadow map (orthographic, covering `extent` around `center`) and dims the key light where it's blocked (2×2 PCF). `castShadow: false` opts an item out.

**Depth of field**: `render(…, { dof: { focus, aperture, max } })` blurs by distance from the focal plane (thin lens; `aperture` = blur in px at infinity). Animate `focus` for rack-focus shots.

**Camera motion blur**: `render(scene, items, { motionBlur: { shutter: 0.5, max } })` reprojects every pixel with the previous `render()`'s camera using the depth buffer and blurs along that motion (depth-aware, so near geometry smears over the background). Smooth, ghost-free speed from one render per frame — use it with composition `motionBlur: 1`, give points `streak: 0`–`0.5`, and call `r3.resetMotion()` after a cut. Pass `prevCamera` (the camera one frame earlier, `scene.viewProjection().matrix`) so previews and `--review`, which skip frames, blur exactly like the final render — see `templates/journey.mjs`.

**Mirror floors**: `render(scene, items, { mirror: { y, strength } })` renders the scene reflected about the plane `y`, then draws items marked `mirror: true` (the floor) glossily over that reflection (`strength` 0–1), then everything else. Neon on wet asphalt. The mirror item should cover the view below the horizon; `reflect: false` keeps an item out of the reflection. The reflection renders at half resolution (about a quarter of the cost of the main pass, and softer, like a real glossy floor); `mirror: { y, resolution: 'full' }` keeps it sharp.

**Meshes**: `mesh.cube()`, `sphere(rings, segments)`, `torus(radius, tube, segments, sides)`, `torusKnot({ p, q, radius, spread, tube, segments, sides })`, `plane(cells)` (XZ, faces +Y — floors that run to the horizon with `grid`), `cylinder(segments, caps)`, `heightfield(w, d, heights | (x, z) => h, { skip })`, `text(str, { font, depth, bevel, resolution })` (solid bevelled 3D text in any font; stand it up with `rx: 90` and use `sz: size * mesh.aspect`), `custom(positions, normals, indices, uvs?)`.

**Light points** (`{ points, colors?, size, intensity, streak, fog }`): `points` is a flat xyz array or a live `emitter()` / `pointCloud()`. `streak` stretches each point along its screen motion since the previous `render()` (1 = shutter-accurate, 3+ = warp).

**Shatter**: `shatter(mesh, { transform, force, spread, spin, gravity, drag, ripple, color })` turns every triangle into a flying shard; `shards.update(secondsSinceHit)` poses them; draw `{ mesh: shards.mesh, instances: shards.instances, cull: false }`. Deterministic, so parallel render workers agree.

**Camera flights**: `flightPath(points, { lookAhead, bank, maxBank, closed })` is a smooth (centripetal Catmull–Rom) path with an arc-length table. `path.apply(scene, s, extraRoll)` places the camera at distance `s`, looking `lookAhead` ahead and banking into turns; `path.timing([[sec, fraction], …])` returns a monotone, smoothly accelerating seconds→distance function (hold a fraction to stop). `scene.roll` rolls any camera about its view axis.

```ts
const path = flightPath([[0, 300, 2000], [0, 120, -2000], [180, 60, -8000], [0, 400, -26000]], { lookAhead: 900, bank: 1.4 });
const s = path.timing([[0, 0], [1.5, 0.2], [4, 0.9], [5, 1]]);
film.scene('fly', 0, 5, { draw: ({ t }) => { path.apply(cam, s(t), t > 3 ? (t - 3) * 360 : 0); /* barrel roll */ … } });
```

`r3.triangles` and `r3.lastMs` report the last frame. `scene.viewProjection()` gives the matrix and model-space eye for your own renderers. Benchmark: `node scripts/bench-raster.mjs`.

### Film grade, impacts and speed ramps

```ts
const film = composition({ …, motionBlur: { samples: 2 }, post: {
  bloom: 0.8, bloomThreshold: 0.55, bloomRadius: 40,          // glow around highlights
  streaks: 0.5, streakLength: 260, streakTint: [0.6, 0.75, 1], // anamorphic flares
  aberration: 0.6, vignette: 0.45, grain: 4,
  zoomBlur: (t) => (t > 6 && t < 7 ? 0.25 : 0),               // speed lines on the warp
  rays: 1, raysCenter: [0.5, 0.42], raysLength: 0.7,           // god rays from a light behind the scene
  aa: 1,                                                       // FXAA-style edge antialiasing for renderer3d geometry
  flares: 0.6,                                                 // lens flares: ghosts of bright lights mirrored through the centre, plus a halo
  contrast: 1.08, saturation: 1.1, shadows: [0, 0.015, 0.04], highlights: [0.06, 0.02, -0.02], // grade + split tone
} });
film.impact(12.75, { shake: 40, flash: '#fff', flashOpacity: 0.7, aberration: 9 });  // shake + flash + aberration kick
film.impact(14.2, { shake: 20, shockwave: 32, origin: [0.5, 0.45] });                 // + a refracting ring expanding from the hit
film.scene('flight', 16, 24, { timeScale: (t) => 1 - 0.88 * Math.exp(-((t - 18.7) / 0.55) ** 2), step: ({ dt }) => sim.step(dt), draw });
```

- `post` runs in WebAssembly once per output frame (after motion blur); grain is seeded by frame index, so parallel slices match. Every value may be a function of composition time, so the grade can follow the edit.
- `impact()` shakes and flashes the finished frame (after transitions); `impactAt(t)` is its pure function of time. `shockwave` (px of displacement, 20–40) adds a refracting ring that expands from `origin` at `shockSpeed` frame diagonals per second and fades as it goes; the grade's `post` pass draws it (Python: `post(shock=(x, y, radius, half_width, strength))`).
- `timeScale(local)` scales engine time for that scene — tweens, particles and physics all slow together; `step()` receives the scaled `dt`.
- Choreograph bullet time on the scene's own clock: `FrameInfo.clock` is the sum of those `dt`s (seconds since the scene started, slowed by `timeScale`). `film.clockAt(name, t)` gives it as a pure function of composition time, and `film.timeAt(name, clock)` maps it back, so impacts and cues land on story beats:

```ts
const HIT = 6; // the knot breaks when the slowed clock reads 6 s
film.scene('knot', 0, 10, {
  timeScale: (t) => 1 - 0.8 * Math.exp(-(((t - 6.4) / 0.4) ** 2)),
  draw: ({ clock }) => { path.apply(cam, s(clock)); /* … shards.update(clock - HIT) … */ },
});
film.impact(film.timeAt('knot', HIT), { shake: 30, flash: '#fff' });
```
- Per-scene `motionBlur` (samples) overrides the composition: use 1 for static shots and more for fast ones.
- Mistakes throw readable errors (overlapping scenes without a transition, missing `draw`, scenes outside the duration).

## Python: solid 3D, camera flights and film grade

The Python package (`crates/anikit-py`) exposes the same renderer and grade on numpy frames: `anikit.raster` (solid 3D, `shatter`), `anikit.video` (film grade, I420, motion blur) and `anikit.flight_path`. Both submodules need numpy (`pip install anikit[numpy]`) and load on first access. Names are snake_case versions of the TypeScript options; colours are CSS strings or `(r, g, b)` floats in 0–1 (above 1 glows).

| TypeScript | Python |
|---|---|
| `renderer3d({ width, height })` | `raster.Renderer3D(width, height)` |
| `r3.render(scene, items, { background, mirror, …light })` | `r3.render(camera, items, background=, transparent=, mirror=, lighting=, **light)` → `(h, w, 4)` uint8 array |
| `mesh.cube()`, `sphere`, `torus`, `torusKnot({…})`, `plane`, `cylinder`, `heightfield`, `custom` | `raster.cube()`, `sphere`, `torus`, `torus_knot(p=, q=, radius=, spread=, tube=, segments=, sides=)`, `plane`, `cylinder`, `heightfield(w, d, heights \| f(x, z), skip=)`, `custom(positions, normals, indices, uvs=None)` |
| `mesh.tube(points, { radius, sides, radii, taper })` | `raster.tube(points, radius=10, sides=8, radii=None, taper=None)` (`(n, 3)`, flat or a list of points; `(n - 1) * sides * 2` triangles) |
| `instances(n, (i) => ({ x, ry, scale, color }))`, `inst.set(i, {…})` | `raster.instances(n, x=xs, ry=angles, scale=12, color=…)` (vectorised) or `instances(n, lambda i: {...})`; `inst.set(index, **fields)`; `inst.data` is `(n, 15)` float32, `inst.positions` / `inst.colors` are writable views |
| `{ mesh, instances, material, cull, blend, mirror, reflect }` | `raster.Item(mesh, instances, material, cull=, blend="opaque" \| "additive" \| "gloss", mirror=, reflect=)` or a dict |
| `{ …, castShadow: false }` | `raster.Item(…, cast_shadow=False)` |
| `material: { pattern: 'hex' }` | `raster.Material(pattern="grid" \| "hex" \| "dots" \| "circuit")` (`raster.PATTERNS`) |
| `groundFog: { bottom, top, range }` | `ground_fog=(bottom, top, range)` or a dict (render keyword or `Lighting` field) |
| `r3.render(…, { skybox, shadows, dof, motionBlur })`, `r3.resetMotion()` | `r3.render(…, skybox=True \| sun, shadows=raster.Shadows(extent, center=, size=, strength=, bias=) \| extent \| dict, dof=focus \| dict(focus=, aperture=, max=), motion_blur=True \| shutter \| dict(shutter=, max=))`, `r3.reset_motion()` |
| `{ points, colors, size, intensity, streak, fog }` | `raster.Points(points, colors, size=, intensity=, streak=, fog=)` (`(n, 3)` arrays or a `PointCloud`) |
| `material: { metal, lineColor, pulsePhase, texture, textureMode, … }` | `raster.Material(metal=, line_color=, pulse_phase=, texture=, texture_mode=, …)` or a dict |
| `texture(imageData)` | `raster.texture(array)` (`(h, w, 4)` or `(h, w, 3)`, uint8 or 0–1 floats) |
| `shatter(mesh, { transform, force, … })`, `shards.update(t)` | `raster.shatter(mesh, transform=dict(…), force=, …)`, `shards.update(t)`, `shards.item()` |
| `scene.camera` / `scene.target` / `scene.roll` | `anikit.transform.Camera(position, target, fov=, near=, far=, roll=)` |
| `flightPath(points, { lookAhead, bank, maxBank, closed })` | `anikit.flight_path(points, look_ahead=, bank=, max_bank=, closed=)`: `.point(s)`, `.tangent(s)`, `.at(s)`, `.apply(camera, s, extra_roll)`, `.timing(keys)`, `.points(n, from_=0, to=None)` |
| `composition({ post: { bloom, bloomThreshold, … } })` | `video.post(frame, bloom=, bloom_threshold=, …, seed=)` or `video.PostFx(**options)(frame, t=, seed=)` |

```python
import anikit
from anikit import raster, video
from anikit.transform import Camera

r3 = raster.Renderer3D(1920, 1080)
knot = raster.torus_knot(p=2, q=3, radius=300, tube=58, segments=600, sides=28)
floor = raster.Item(raster.plane(), raster.instances(1, y=-420, scale=20000, color=(0.04, 0.04, 0.05)),
                    {"lines": 0.6, "grid": (60, 60), "line_color": (0.5, 0.3, 1.2)}, mirror=True)
path = anikit.flight_path([(0, 60, 2300), (1500, 0, 1000), (1500, 200, -1100)], look_ahead=900, bank=1.2)
s = path.timing([(0, 0), (5, 1)])
cam = Camera(fov=45, far=20000)
grade = video.PostFx(bloom=0.8, bloom_threshold=0.55, streaks=0.4, vignette=0.45, grain=3,
                     zoom_blur=lambda t: 0.25 if 3 < t < 4 else 0)
for i in range(150):
    t = i / 30
    path.apply(cam, s(t))                                   # position, look-ahead target, banking roll
    frame = r3.render(cam, [
        raster.Item(knot, material=raster.Material(metal=0.9, lines=1.2, line_color=(0.3, 0.8, 1.6),
                                                   grid=(0, 6), pulse=2.5, pulse_count=3, pulse_phase=t)),
        floor,
    ], background="#04050a", mirror=raster.Mirror(-420, 0.55), fog=(1500, 5000),
       sky=(0.05, 0.07, 0.16), horizon=(0.9, 0.5, 0.95), strips=(2.2, 2.3, 2.6))
    grade(frame, t=t, seed=i)                               # in place
    yuv = video.rgba_to_i420(frame)                         # for an encoder's stdin
```

- **Frames**: `render()` returns a new writable array backed by one copy of the colour buffer; `video.post` grades it in place without another copy (any other C-contiguous uint8 `(h, w, 4)` array is copied in and back). The native draw releases the GIL.
- **Mirror floors**: `mirror=y`, `(y, strength)`, a dict or `raster.Mirror(y, strength)` runs the same passes as TypeScript: the scene reflected about `y` (winding flipped; additive, mirror and `reflect=False` items skipped), a depth clear, the `mirror=True` items blended glossily over the reflection, then the scene.
- **Points** streak along their motion since the previous `render()` of the same renderer.
- **Pass order** matches TypeScript: shadow map (opaque, non-mirror items with `cast_shadow=True`; `center` defaults to `Camera.target` and is required for a `(matrix, eye)` camera), mirror passes, opaque items, `skybox`, then additive items and points in item order, then `dof`, then `motion_blur` (against the previous `render()`'s camera; none on the first frame or after `reset_motion()`).
- **Grade options**: `bloom`, `bloom_threshold`, `bloom_radius`, `aberration`, `vignette`, `grain`, `streaks`, `streak_length`, `streak_tint`, `zoom_blur`, `zoom_center`, `rays`, `rays_center`, `rays_length`, `aa`, `exposure`, `contrast`, `saturation`, `shadows`, `highlights` (`video.POST_OPTIONS`); each may be a function of `t`. Misspelt options raise with a "did you mean".
- `video.MotionBlur(w, h)`: `add(frame)` up to 257 sub-frames, `resolve()` the average.
- Not in Python: `mesh.text()` (needs a canvas to rasterise the font; build a `heightfield` from your own glyph bitmap instead).
- Point lights: `Lighting(lights=[{"position": (x, y, z), "color": …, "radius": 500, "intensity": 1}])` (or `render(..., lights=[...])`), up to 8.
- Example: `crates/anikit-py/examples/knot_mirror.py` writes PNG frames with numpy and the standard library.

---

## Export index

Every export of `anikit`, with where it is described. Types are listed by name; hover them in an editor for per-option docs (defaults, units, examples).

| Area | Functions and classes | Types |
|---|---|---|
| [Animate](#animatetargets-keyframes-options--animation) | `animate`, `from`, `fromTo`, `set`, `Animation`, `resolveRandom` | `AnimateOptions`, `Props`, `TransformProps`, `PropValue`, `Keyframe`, `MotionPathOptions`, `FnValue`, `Scalar`, `TextOptions` |
| [Timelines](#timelineoptions--timeline) | `timeline`, `Timeline`, `sequence`, `delayedCall` | `TimelineOptions`, `Position`, `SequenceStep` |
| [Controllers](#controller-animation-timeline-batch) | (methods on every controller) | `Playable`, `PlayState` |
| [Easings](#easings) | `ease`, `easing`, `cssEasing` | `EasingInput`, `EasingName`, `EasingFn`, `SpringOptions` |
| [Stagger](#stagger) | `stagger`, `staggerValues`, `distribute` | `StaggerOptions`, `StaggerFn` |
| [Physics](#physics) | `spring`, `inertia`, `physics`, `PhysicsAnimation` | `PhysicsOptions`, `InertiaOptions`, `InertiaProp`, `PhysicsProp`, `BallisticOptions` |
| [Scroll & viewport](#scroll--viewport) | `scroll`, `inView`, `scrollTrigger`, `scrollTriggerBatch`, `ScrollTrigger`, `scrollTo`, `smoothScroll`, `refreshScrollTriggers` | `ScrollOptions`, `InViewOptions`, `ScrollTriggerOptions`, `ScrollToOptions`, `SmoothScrollOptions` |
| [Text, layout, compositor, batches](#text-layout-compositor-batches) | `splitText`, `flip`, `compositor`, `CompositorAnimation`, `batch`, `Batch` | `SplitOptions`, `SplitResult`, `FlipOptions`, `FlipState`, `BatchOptions`, `BatchValue` |
| [SVG](#svg) | `morph`, `morphPlan`, `morphSequence`, `shapeToPath`, `pathData`, `pathLength`, `pathBounds`, `normalizePath`, `MotionPath`, `circle`, `ellipse`, `rect`, `polygon`, `star`, `trimPath`, `pathThrough`, `simplifyPath`, `pathArea`, `flattenPath`, `transformPath` | `PathSource` |
| [Utilities](#more-utilities) | `mixColor`, `parseColor`, `splitColor`, `clamp`, `lerp`, `mapRange`, `wrap`, `snap`, `random`, `interpolate`, `normalize`, `roundTo`, `pipe`, `shuffle`, `getUnit`, `toArray`, `VelocityTracker` | |
| [Noise](#more-utilities) | `noise2`, `noise3`, `fbm`, `noiseMany`, `wander` | |
| [Cleanup](#control-cleanup-and-accessibility) | `context`, `Context`, `matchMedia`, `quickTo`, `quickSetter` | `Revertible` |
| [Effects](#effects) | `effects`, `effect`, `registerEffect`, `effectNames` | `EffectOptions`, `EffectFn`, `EffectName` |
| [Layout transitions](#layout-transitions-flip), [presence](#presence-enter-and-exit) and [View Transitions](#view-transitions) | `flip`, `presence`, `Presence`, `enter`, `exit`, `viewTransition` | `FlipOptions`, `FlipState`, `PresenceOptions`, `ViewTransitionOptions`, `ViewTransitionHandle` |
| [Interaction](#interaction) | `draggable`, `Draggable`, `observer`, `hover`, `press`, `gesture`, `Gesture` | `DraggableOptions`, `Bounds`, `ObserverOptions`, `ObserverEvent`, `GestureOptions` |
| [Engine](#engine) | `engine`, `flush`, `ready`, `init` | |
| [Dev tools](#dev-tools) and [testing](#testing-and-diagnostics) | `inspector`, `testing`, `setWarnings`, `onWarning` | `InspectorOptions` |
| [Physics worlds](#physics-worlds) | `world`, `World` | `WorldOptions`, `BodyInit` |
| [3D](#3d) | `scene3d`, `Scene3D`, `tilt`, `parseTransform`, `toMatrix3d`, `mixTransforms` | `SceneOptions`, `Projection`, `DrawOptions`, `TiltOptions`, `Vec3`, `Matrix` |
| [Point clouds and glow](#point-clouds-and-glow) | `pointCloud`, `PointCloud`, `textPoints`, `glow`, `Glow` | `CloudOptions`, `ShapeInput`, `GlowOptions`, `GlowRenderOptions` |
| [Particle emitters](#particle-emitters) | `emitter`, `Emitter` | `EmitterOptions`, `SpawnShape` |
| [Solid 3D](#solid-3d-renderer3d-mesh-instances) | `renderer3d`, `Renderer3D`, `mesh`, `Mesh3D`, `instances`, `Instances`, `texture`, `Texture3D`, `shatter`, `Shards`, `flightPath`, `FlightPath` | `InstanceInit`, `MaterialOptions`, `LightOptions`, `DrawItem`, `PointsItem`, `ShatterOptions`, `FlightPathOptions`, `FlightPose`, `Point3` |
| [Audio](#audio-analysis) | `analyzeAudio`, `decodeAudio` | `AudioAnalysis` |
| [Video](#video-compositions) | `composition`, `Composition`, `FrameBuffer`, `typeFx`, `exportWebM`, `muxWebM` | `CompositionOptions`, `SceneHooks`, `SceneDef`, `FrameInfo`, `RenderOptions`, `RenderedFrame`, `TypeFxOptions`, `Transition`, `TransitionFn`, `PostOptions`, `PostValue`, `PostColor`, `ImpactOptions` |
| [Capture](#capture-screenshots-and-recordings) | `screenshot`, `record`, `rasterize`, `encodeGif`, `GifWriter` | `CaptureTarget`, `RasterOptions`, `RecordOptions`, `Recording` |
