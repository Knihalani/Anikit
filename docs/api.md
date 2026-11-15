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

With no position, a child goes at the end of the timeline. Options: `delay`, `repeat`, `repeatDelay`, `yoyo`, `speed`, `ease` (applied to the overall progress), `autoplay`, `defaults` and the callbacks (`onStart`, `onUpdate`, `onComplete`, `onRepeat`, `onReverseComplete`). `onUpdate` fires once per frame while the playhead moves (playing or seeking), after that frame's DOM writes. Sequential tweens of the same property chain their start values.

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
scroll(compositor('.hero', { scale: [1, 1.15], opacity: [1, 0] }, { ease: 'linear' }), { target: hero, offset: ['start start', 'end start'] });
```

Give `scroll()` a `compositor()` animation and it's attached to a native `ScrollTimeline` where the browser has one: the browser scrubs it in step with the scroll, and no JavaScript runs while scrolling. The `offset` range is measured from layout (and again on resize), so it means the same as on the JavaScript path. Without `ScrollTimeline`, or with `smooth`, it's scrubbed from the scroll event instead (`native: false` forces that). In Chromium with 300 scroll-linked elements, script time while scrolling went from about 0.47 ms to 0.05 ms per frame; style and layout time stayed about the same.

`compositor()` animations honour `engine.reducedMotion` (they jump to the end), and `revert()` cancels them and puts back the inline styles from before.

---

## SVG

`morph(a, b) → (t) => d`, `morphPlan(a, b)`, `pathLength(p)`, `pathBounds(p)`, `normalizePath(p)`, `shapeToPath(el)` and `new MotionPath(p).at(t) → { x, y, angle }`. Shape builders: `circle`, `ellipse`, `rect`, `polygon` and `star`.

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

`splitText` options: `mask: 'lines' | 'words' | 'chars'` wraps each unit in an `overflow: clip` element (returned in `masks`; class via `maskClass`). `autoSplit: true` splits again when an element's width changes and once web fonts have loaded, so lines match the layout. `onSplit(result)` runs after every split; return the animation that uses the pieces, and on a re-split it's reverted and the new one is moved to the same time. `result.split()` re-splits on demand, and `revert()` restores the markup (a `context()` does this too).

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
- `scale: false` animates position only. With scale on, `border-radius` is counter-scaled every frame so corners don't stretch (`correctRadius: false` to skip it).
- Layout is read once for all elements (transforms off, measure, transforms back) rather than once per element.

## Presence: enter and exit

The DOM has no "about to be removed" phase. These helpers animate first and remove afterwards, and cope with an element coming back halfway through its exit.

```ts
exit('.toast', { opacity: 0, y: 20 }, { duration: 0.3 });  // animate out, then remove (after: 'hide' sets `hidden` instead)
enter('.panel', { opacity: [0, 1], y: [12, 0] });            // un-hide and animate in; turns a running exit() around

const list = presence('#todos', { enter: { opacity: [0, 1], y: [-8, 0] }, exit: { opacity: 0, scale: 0.9 }, duration: 0.3 });
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
animate(panel, { height: open ? 'auto' : 0 }, { ease: 'outCubic' });            // accordions
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
- `context(fn, scope?)` collects everything created inside `fn` (animations, timelines, scroll triggers, draggables, observers…), scopes selector lookups to `scope`, and offers `ctx.revert()` / `ctx.kill()`. `fn` may return a cleanup function. It's the natural fit for component lifecycles in React, Vue or Svelte.
- `matchMedia().add('(min-width: 800px)', setup)` runs `setup` while the query matches and reverts it automatically when it stops matching. `add({ wide: '…', reduce: '(prefers-reduced-motion: reduce)' }, ({ wide, reduce }) => …)` takes several named conditions.
- `engine.reducedMotion = 'user' | 'always' | 'never'`: when motion is reduced, animations jump straight to their end state, while callbacks and promises still fire.
- `engine.reducedMotionMode = 'fade'` takes out only the movement: transforms, motion paths, shape morphs and layout properties (sizes, offsets, margins, SVG geometry attributes) hold their end values, while opacity, colours and filters animate with their normal timing. With it, `flip()` and `presence()` still cross-fade, but nothing slides. The default, `'instant'`, jumps everything. `compositor()` animations always jump.
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

### Pinch, rotate and pan: `gesture`

```ts
const [g] = gesture('#photo', { minScale: 1, maxScale: 6, bounds: { minX: -400, maxX: 400, minY: -300, maxY: 300 } });
gesture('.sticker', { type: 'pinch,rotate', onChange: (g) => save(g.scale, g.rotation) });
gesture(canvas, { apply: false, onChange: ({ x, y, scale }) => view.set(x, y, scale) });   // drive anything
```

Two fingers zoom and rotate about their midpoint, and the point under the fingers stays there. One finger pans, and lifting a finger mid-gesture carries on smoothly with the other. Ctrl/⌘ + wheel (trackpad pinch) zooms about the pointer. A plain wheel still scrolls.

- **Limits:** `minScale`, `maxScale` and `bounds` are rubber-banded while held (`edgeResistance`, default 0.35; scale resists in log space). On release the scale springs back about the last finger position, or the element glides with momentum (`inertia: false` to stop dead).
- **Options:** `type` is any of `'pan,pinch,rotate'`. `apply: false` only reports through callbacks: `onStart`, `onChange`, `onEnd` (fingers up) and `onRest` (settled).
- **The `Gesture` object** has `x`, `y`, `scale`, `rotation`, `pointers`, `isActive`, `set({ x, y, scale, rotation })`, `enable()`, `disable()`, `kill()` and `revert()`.

## Dev tools

`inspector(animationOrTimeline)` opens a floating panel to play, pause, scrub, restart, loop, change speed and jump to labels. Space toggles playback.

## More utilities

| Function | Purpose |
|---|---|
| `interpolate([0, 100, 200], [0, 1, 0], { ease, clamp })` | Map through stops. Outputs can be numbers, colours or values with units. |
| `distribute({ base, amount, from, grid, ease })` | Spread any value across targets. |
| `noise2(x, y, seed)`, `noise3`, `fbm(x, y, z?, { octaves })`, `noiseMany(xyz)`, `wander({ speed, amplitude })` | Seeded simplex noise. |
| `trimPath(d, 0.2, 0.8)`, `pathThrough(points, { curviness, closed })`, `simplifyPath(d, tolerance, smooth)`, `pathArea(d)`, `flattenPath(d, tol)`, `transformPath(d, [a, b, c, d, e, f])` | Path operations. |
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

`mode: 'ink'` renders particles as pigment absorbing a light background (`bg / (1 + c)` per channel) instead of adding light; point colours are then absorption per channel (`[1, 1, 1]` = black ink, `[0.05, 0.78, 0.95]` = vermilion). Both `glow.options.mode` and `glow.options.background` can change every frame, and `render({ tint: [r, g, b] })` recolours every point at no cost.

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
```

Each worker builds the same composition (randomness is seeded), simulates to its slice without drawing, pre-rolls half a second so trails match, renders and encodes; slices join without re-encoding and the audio is muxed in. Needs the optional peer dependency `@napi-rs/canvas` and ffmpeg. `loadAnikit()`, `renderToFile()` and `renderPreview()` are the programmatic equivalents.

### Solid 3D: `renderer3d`, `mesh`, `instances`

A z-buffered, lit, instanced triangle rasterizer in WebAssembly — no WebGL. Per-vertex Blinn–Phong with key and fill lights, a fresnel rim and depth fog; instances are drawn front-to-back.

```ts
const r3 = renderer3d({ width: 1920, height: 1080 });     // or a canvas (drawn into automatically)
const cubes = instances(3000, (i) => ({ x, y, z, rx, ry, rz, scale: 14, color: i % 9 ? '#ece6da' : '#ff3b14' }));
const knot = mesh.torusKnot({ p: 2, q: 5, radius: 250, tube: 46 });
const px = r3.render(scene, [
  { mesh: mesh.cube(), instances: cubes, material: { specular: 0.5, rim: 0.4 } },
  { mesh: knot, material: { specular: 1.1, shininess: 70, rim: 0.9 } },
], { background: '#06070a', fog: [1400, 5200], keyColor: [1, 0.95, 0.9], rimColor: [1, 0.4, 0.25] });
```

`mesh.custom(positions, normals, indices)` takes your own geometry. `r3.triangles` and `r3.lastMs` report the last frame. `scene.viewProjection()` gives the matrix and model-space eye for your own renderers.

### Film grade, impacts and speed ramps

```ts
const film = composition({ …, motionBlur: { samples: 2 }, post: { bloom: 0.55, bloomThreshold: 0.72, aberration: 0.5, vignette: 0.4, grain: 5 } });
film.impact(12.75, { shake: 40, flash: '#fff', flashOpacity: 0.7, aberration: 9 });  // shake + flash + aberration kick
film.scene('flight', 16, 24, { timeScale: (t) => 1 - 0.88 * Math.exp(-((t - 18.7) / 0.55) ** 2), step: ({ dt }) => sim.step(dt), draw });
```

- `post` runs in WebAssembly once per output frame (after motion blur); grain is seeded by frame index, so parallel slices match.
- `impact()` shakes and flashes the finished frame (after transitions); `impactAt(t)` is its pure function of time.
- `timeScale(local)` scales engine time for that scene — tweens, particles and physics all slow together; `step()` receives the scaled `dt`.
- Per-scene `motionBlur` (samples) overrides the composition: use 1 for static shots and more for fast ones.
- Mistakes throw readable errors (overlapping scenes without a transition, missing `draw`, scenes outside the duration).
