# Anikit recipes

Task-oriented, copy-paste-ready examples. Every TypeScript block here is type-checked against the real API, and the ones with an HTML fixture also run in a test (`packages/anikit/test/recipes.test.ts`); the Python blocks run under pytest. Reference: [api.md](api.md).

Three rules cover most surprises:

1. **Times are seconds** (`duration: 0.6`, `stagger(0.05)`), except `engine.onFrame((timeMs, dtMs) => …)`.
2. **Use transforms** (`x`, `y`, `scale`, `rotate`) for motion, not `left`/`top`.
3. **Give a start value when it matters** (`opacity: [0, 1]`), and create animations for scroll triggers with `autoplay: false`.

Contents: [Hero entrance](#hero-entrance) · [Staggered list](#staggered-list) · [Reveal on scroll](#reveal-on-scroll) · [Scroll-scrubbed pinned section](#scroll-scrubbed-pinned-section) · [Page transition](#page-transition) · [Draggable carousel](#draggable-carousel) · [Physics toy](#physics-toy) · [SVG morph and line drawing](#svg-morph-and-line-drawing) · [Number counter](#number-counter) · [Hover and press](#hover-and-press) · [Pointer follower](#pointer-follower) · [Layout change (FLIP)](#layout-change-flip) · [Components and reduced motion](#components-and-reduced-motion) · [Testing an animation](#testing-an-animation) · [Recording a GIF or MP4](#recording-a-gif-or-mp4) · [Rendering a video in Node](#rendering-a-video-in-node) · [Python](#python)

---

## Hero entrance

Split the headline into characters, rise them in with a stagger, then bring in the rest with overlaps.

```html
<section class="hero">
  <h1 class="hero-title">Motion, compiled.</h1>
  <p class="hero-sub">A Rust/WebAssembly animation engine.</p>
  <a class="hero-cta" href="#start">Get started</a>
</section>
```

```ts
import { timeline, splitText, stagger } from 'anikit';

const { chars } = splitText('.hero-title', { type: 'words,chars' });   // words keep line breaks natural

timeline({ defaults: { ease: 'outCubic', duration: 0.6 } })
  .add(chars, { y: ['110%', 0], opacity: [0, 1] }, { delay: stagger(0.025) })
  .add('.hero-sub', { y: [16, 0], opacity: [0, 1] }, '-=0.35')          // overlap the previous step by 0.35 s
  .add('.hero-cta', { scale: [0.8, 1], opacity: [0, 1] }, { ease: 'spring(bounce: 0.4)' }, '-=0.2');
```

- `'-=0.35'` places a step relative to the end of the timeline; `'<'` starts with the previous step, `'label+=0.2'` after a label.
- Hide the hero with CSS (`opacity: 0`) only if you also give explicit start values like above, so there is no flash before the script runs.

## Staggered list

Items enter one after another; a removed item animates out before it leaves the DOM.

```html
<ul class="todo">
  <li>Write the docs</li>
  <li>Test the recipes</li>
  <li>Ship it</li>
</ul>
```

```ts
import { animate, stagger } from 'anikit';

animate('.todo li', { x: [-24, 0], opacity: [0, 1] }, { duration: 0.5, ease: 'outCubic', delay: stagger(0.06) });

export async function removeItem(li: HTMLElement) {
  await animate(li, { x: 40, opacity: 0, height: 0 }, { duration: 0.3, ease: 'inQuad' });   // await = finished
  li.remove();
}

export function addItem(list: HTMLElement, text: string) {
  const li = document.createElement('li');
  li.textContent = text;
  list.append(li);
  animate(li, { y: [12, 0], opacity: [0, 1] }, { duration: 0.4, ease: 'outBack' });
}
```

- `stagger(0.06, { from: 'center' })` ripples from the middle; `{ grid: [cols, rows] }` for grids.
- A new animation on the same property takes it over, so `removeItem` during the entrance is safe.

## Reveal on scroll

Each card plays when it enters the viewport and reverses when you scroll back above it.

```html
<div class="card reveal">One</div>
<div class="card reveal">Two</div>
<div class="card reveal">Three</div>
```

```ts
import { animate, scrollTriggerBatch } from 'anikit';

scrollTriggerBatch('.reveal', {
  start: 'top 85%',                              // when the card's top passes 85% down the viewport
  toggleActions: 'play none none reverse',       // onEnter onLeave onEnterBack onLeaveBack
  animation: (el) => animate(el, { y: [40, 0], opacity: [0, 1] }, { duration: 0.6, ease: 'outCubic', autoplay: false }),
});
```

Simpler, one-shot, without scroll math:

```ts
import { animate, inView } from 'anikit';

inView('.reveal', (el) => {
  animate(el, { y: [40, 0], opacity: [0, 1] }, { duration: 0.6 });
}, { amount: 0.3, once: true });
```

## Scroll-scrubbed pinned section

The section pins while its timeline follows the scrollbar, then releases. A progress bar tracks the whole page.

```html
<div class="progress"></div>
<section class="story">
  <div class="panel a">A</div>
  <div class="panel b">B</div>
</section>
<section class="after">Next</section>
```

```ts
import { scrollTrigger, timeline, scroll, animate } from 'anikit';

const tl = timeline({ autoplay: false, defaults: { ease: 'none' } })
  .add('.story .a', { x: [0, -300], opacity: [1, 0] })
  .add('.story .b', { x: [300, 0], opacity: [0, 1] }, '<')     // '<' = same start as the previous step
  .label('done');

scrollTrigger({
  trigger: '.story',
  start: 'top top',        // section top meets viewport top
  end: '+=1200',           // 1200 px of scrolling
  pin: true,
  scrub: 0.4,              // follow the scrollbar with 0.4 s of smoothing
  snap: { to: 'labels', duration: 0.3 },
  animation: tl,
});

scroll(animate('.progress', { scaleX: [0, 1] }, { ease: 'linear', autoplay: false }));   // whole-page progress
```

- Positions are `"<trigger edge> <viewport edge>"` with `top`/`center`/`bottom`, `%` or `px`, e.g. `'top 80%'`, `'center center'`, `'bottom+=100 top'`.
- After you change layout yourself (images loaded, content injected), call `refreshScrollTriggers()`.
- Pinning and scrubbing need real layout: check them in a browser (Playwright works headless), not in happy-dom/jsdom.

## Page transition

Fade the current page out, swap, fade the next in. `await` works on every animation and timeline.

```html
<main>
  <section class="page active" id="home">Home <a href="#about" data-to="about">About</a></section>
  <section class="page" id="about" style="display: none">About <a href="#home" data-to="home">Home</a></section>
</main>
```

```ts
import { animate } from 'anikit';

let busy = false;
async function show(id: string) {
  const current = document.querySelector<HTMLElement>('.page.active');
  const next = document.getElementById(id);
  if (busy || !current || !next || current === next) return;
  busy = true;
  await animate(current, { opacity: [1, 0], y: [0, -16] }, { duration: 0.25, ease: 'inQuad' });
  current.classList.remove('active');
  current.style.display = 'none';
  next.style.display = '';
  next.classList.add('active');
  await animate(next, { opacity: [0, 1], y: [16, 0] }, { duration: 0.4, ease: 'outCubic' });
  busy = false;
}

document.addEventListener('click', (e) => {
  const link = (e.target as Element).closest<HTMLElement>('[data-to]');
  if (link) {
    e.preventDefault();
    void show(link.dataset.to!);
  }
});
```

For full-page "one section per swipe" navigation, `observer({ onDown, onUp, tolerance: 20 })` turns wheel, touch and pointer input into intents.

## Draggable carousel

Drag (or throw) the track; it settles on a slide. Buttons animate to the neighbouring slide.

```html
<div class="carousel" style="overflow: hidden; width: 320px">
  <div class="track" style="display: flex; width: 960px">
    <div class="slide" style="width: 320px">1</div>
    <div class="slide" style="width: 320px">2</div>
    <div class="slide" style="width: 320px">3</div>
  </div>
  <button class="prev">‹</button>
  <button class="next">›</button>
</div>
```

```ts
import { draggable, animate } from 'anikit';

const track = document.querySelector<HTMLElement>('.track')!;
const width = 320;
const count = track.children.length;
const minX = -(count - 1) * width;

const [drag] = draggable(track, {
  type: 'x',
  bounds: { minX, maxX: 0 },     // transform px; or an element: bounds: '.carousel'
  inertia: true,                 // throw with momentum…
  snap: { x: width },            // …and come to rest on a slide
  edgeResistance: 0.5,
});

const go = (dir: number) => {
  const x = Math.round(drag.x / width) * width - dir * width;
  animate(track, { x: Math.max(minX, Math.min(0, x)) }, { duration: 0.5, ease: 'outCubic' });
};
document.querySelector('.prev')!.addEventListener('click', () => go(-1));
document.querySelector('.next')!.addEventListener('click', () => go(1));
```

`draggable()` returns an array (one `Draggable` per element), hence `const [drag]`. It reads the element's current transform on every press, so animating `x` from buttons and dragging mix freely.

## Physics toy

DOM elements fall, collide, and can be grabbed and thrown; the pointer pushes them away.

```html
<div id="stage" style="position: relative; width: 600px; height: 400px">
  <div class="ball" style="position: absolute; width: 40px; height: 40px; border-radius: 50%"></div>
  <div class="ball" style="position: absolute; width: 60px; height: 60px; border-radius: 50%"></div>
  <div class="wall" style="position: absolute; left: 200px; top: 300px; width: 200px; height: 20px"></div>
</div>
```

```ts
import { world } from 'anikit';

const w = world({ container: '#stage', gravity: [0, 980], restitution: 0.4, friction: 0.05 });
w.bind('#stage .ball', { rotate: true });   // elements become circular bodies; transforms follow
w.obstacles('#stage .wall');                // element boxes become static colliders
w.dragging();                               // grab and throw
w.pointer({ mode: 'repel', radius: 120 });  // or 'attract'
```

For thousands of bodies, skip the DOM: `w.addMany([...])` and `w.draw(ctx)` on a canvas, or read `w.data` (`[x, y, angle]` per body). Units are px and seconds; `engine.speed = 0.2` gives slow motion.

## SVG morph and line drawing

```html
<svg viewBox="0 0 100 100" width="200" height="200">
  <path id="shape" d="M50,10 L90,90 L10,90 Z" fill="currentColor" />
  <path id="line" d="M10,50 C30,10 70,90 90,50" fill="none" stroke="currentColor" />
</svg>
```

```ts
import { animate, timeline, star, circle } from 'anikit';

// Morph through several shapes (any path data, element or selector; point counts may differ).
animate('#shape', { d: [null, star(50, 50, 5, 40, 18), circle(50, 50, 40)] }, { duration: 2.4, ease: 'inOutCubic', repeat: Infinity, yoyo: true });

// Draw the stroke in, then out from the other end.
timeline({ repeat: Infinity, repeatDelay: 0.5 })
  .add('#line', { draw: [0, 1] }, { duration: 1.2, ease: 'inOutCubic' })
  .add('#line', { draw: ['0 1', '1 1'] }, { duration: 0.8, ease: 'inCubic' });
```

Motion along a path: `animate('#dot', { path: { path: '#line', rotate: true } }, { duration: 3, ease: 'linear' })`.

## Number counter

Count up when the number scrolls into view, formatted as currency.

```html
<p>Revenue: <span id="revenue">$0</span></p>
```

```ts
import { animate, inView } from 'anikit';

inView('#revenue', (el) => {
  animate(el, { textContent: [0, 128400] }, {
    duration: 1.6,
    ease: 'outExpo',
    format: { style: 'currency', currency: 'USD', maximumFractionDigits: 0 },
    locale: 'en-US',
  });
}, { once: true });
```

The preset does the same: `effects.countUp('#revenue', { to: 128400, format: { style: 'currency', currency: 'USD' } })`. Without `format`, use `round: true` for whole numbers.

## Hover and press

```html
<button class="btn">Buy now</button>
```

```ts
import { hover, press, spring } from 'anikit';

hover('.btn', (el) => {
  spring(el, { scale: 1.06, y: -2 }, { stiffness: 400, damping: 22 });
  return () => spring(el, { scale: 1, y: 0 }, { stiffness: 400, damping: 22 });   // runs on leave
});
press('.btn', (el) => {
  spring(el, { scale: 0.94 }, { stiffness: 600, damping: 30 });
  return () => spring(el, { scale: 1.06 }, { stiffness: 400, damping: 15 });      // runs on release
});
```

`spring()` retargets the running spring and keeps its velocity, so fast hover in/out never jumps. `hover` ignores touch, so nothing sticks on phones.

## Pointer follower

```html
<div class="dot" style="position: fixed; left: 0; top: 0; width: 16px; height: 16px; border-radius: 50%"></div>
```

```ts
import { spring, quickTo } from 'anikit';

// Springy, with momentum:
addEventListener('pointermove', (e) => {
  spring('.dot', { x: e.clientX - 8, y: e.clientY - 8 }, { stiffness: 300, damping: 25 });
});

// Or a plain eased chase, one function per property (cheap to call every event):
const toX = quickTo('.dot', 'x', { duration: 0.4, ease: 'outCubic' });
const toY = quickTo('.dot', 'y', { duration: 0.4, ease: 'outCubic' });
addEventListener('pointermove', (e) => { toX(e.clientX - 8); toY(e.clientY - 8); });
```

## Layout change (FLIP)

Change the layout however you like; `flip()` animates elements from where they were.

```html
<div class="grid">
  <div class="item">1</div>
  <div class="item">2</div>
  <div class="item">3</div>
</div>
<button class="toggle">Toggle</button>
```

```ts
import { flip } from 'anikit';

const grid = document.querySelector<HTMLElement>('.grid')!;
document.querySelector('.toggle')!.addEventListener('click', () => {
  flip('.grid .item', () => grid.classList.toggle('dense'), { duration: 0.6, ease: 'spring(bounce: 0.2)' });
});
```

For changes that happen later (after a fetch, in a framework render), split it: `const state = flip.capture('.item')`, update, then `flip.from(state, options)`.

## Components and reduced motion

Collect everything a component creates, scope selectors to it, and undo it all on unmount. Honour the OS "reduce motion" setting once at startup.

```html
<div id="app"><div class="card">A</div><div class="card">B</div></div>
```

```ts
import { context, animate, stagger, scrollTrigger, engine, matchMedia } from 'anikit';

engine.reducedMotion = 'user';     // animations jump to their end state when the OS asks for less motion

export function mount(root: HTMLElement) {
  const ctx = context(() => {
    animate('.card', { y: [24, 0], opacity: [0, 1] }, { delay: stagger(0.05) });   // only .card inside root
    scrollTrigger({ trigger: root, start: 'top 80%', onEnter: () => root.classList.add('seen') });
  }, root);
  return () => ctx.revert();       // kills animations/triggers and restores inline styles
}

const unmount = mount(document.getElementById('app')!);

// Responsive: set up while a query matches, reverted automatically when it stops matching.
matchMedia().add('(min-width: 800px)', () => {
  animate('.card', { rotate: [-2, 0] });
});
```

React: call `mount(ref.current)` in `useLayoutEffect` and return the cleanup. Vue: `onMounted` / `onUnmounted`. Svelte: an action that returns `{ destroy }`.

## Testing an animation

Deterministic, no `requestAnimationFrame`, works in vitest/jest with happy-dom or jsdom.

```html
<body></body>
```

```ts
import { animate, testing, engine } from 'anikit';

await testing.ready;
testing.useFakeTime();                        // time moves only when you advance it
const warnings = testing.captureWarnings();   // collect "anikit:" warnings; call before the code under test

const el = document.createElement('div');
document.body.append(el);
animate(el, { x: 100, opacity: [0, 1] }, { duration: 1, ease: 'linear' });

testing.advance(0.5);                         // seconds, in 60 fps frames
console.assert(el.style.transform === 'translate(50px,0px)');
console.assert(engine.inspect()[0].progress === 0.5);
testing.finishAll();                          // run until idle
console.assert(el.style.opacity === '1');
console.assert(warnings.length === 0);
testing.reset();                              // between tests
```

In a real browser (Playwright), `window.__ANIKIT__` is the engine: `__ANIKIT__.manual(true)`, `__ANIKIT__.tick(16.67)`, `__ANIKIT__.inspect()`.

## Recording a GIF or MP4

In the page (frame-exact; time is advanced exactly `1 / fps` per frame):

```ts no-run
import { record, screenshot, timeline } from 'anikit';

const tl = timeline({ autoplay: false }).add('#stage .box', { x: [0, 300] }, { duration: 2 });
const gif = await record('#stage', { duration: 2, fps: 30, animation: tl });   // seek tl per frame
gif.download('stage.gif');

const webm = await record('#stage', { duration: 2, format: 'webm' });
const png: Blob = await screenshot('#stage');
console.log(webm.frames, png.size);
```

From the command line, headless (needs `npm i -D playwright && npx playwright install chromium`; MP4/WebM need ffmpeg):

```bash
npx anikit-record ./index.html --out hero.gif --duration 3 --fps 30 --selector '#hero'
npx anikit-record http://localhost:5173 --out clip.mp4 --size 1280x720
npx anikit-record ./index.html --screenshot shot.png --at 1.5
```

## Rendering a video in Node

For films, trailers and title sequences rendered without a browser (Skia + ffmpeg; `npm i @napi-rs/canvas`). Start from the template, look at stills, then render:

```bash
npx anikit-render --init film.mjs                     # a working starter
npx anikit-render film.mjs --still 1.5 --out f.png    # one frame, fast
npx anikit-render film.mjs --preview 15 --out pv/     # every 15th frame as JPEG
npx anikit-render film.mjs --out film.mp4 --workers 4
```

A minimal composition module:

```js
// title.mjs: default-export a function that returns a composition
export default async function film({ anikit }) {
  const { composition, typeFx } = anikit;
  const film = composition({ width: 1280, height: 720, fps: 30, duration: 3 });
  film.scene('title', 0, 3, {
    draw({ ctx, t, width, height }) {
      ctx.fillStyle = '#0b0c10';
      ctx.fillRect(0, 0, width, height);
      typeFx(ctx, 'Hello, anikit.', width / 2, height / 2, { t, start: 0.3, effect: 'rise', size: 96, color: '#f4efe6', align: 'center' });
    },
  });
  return film;
}
```

Keep films deterministic: start tweens in a scene's `enter()`, advance simulations in `step({ dt })`, and only read state in `draw()`. `Math.random` is seeded; don't use `Date.now()`. See [Video: compositions](api.md#video-compositions).

## Python

The Python package animates plain objects (dicts, objects, lists of them) with the same engine. Times are seconds; options are keyword arguments in `snake_case`.

```python
import anikit

box = {"x": 0, "opacity": 0, "color": "#ff0000"}
anikit.animate(box, {"x": 100, "opacity": [0, 1], "color": "royalblue"}, duration=0.6, ease="outBack")

for frame, t in anikit.render(fps=30):      # advances the engine and writes values until idle
    pass
assert box["x"] == 100 and box["opacity"] == 1
```

Timelines: in Python the **position is the third positional argument** and options are keywords (in TypeScript, options come third).

```python
import anikit

a, b = {"x": 0}, {"y": 0}
tl = anikit.timeline(defaults={"duration": 0.5, "ease": "outCubic"})
tl.add(a, {"x": 100})
tl.add(b, {"y": 50}, "-=0.25")             # overlap the previous step
tl.label("end").call(lambda: print("done"), "end")
anikit.run_until_idle()
assert (a["x"], b["y"]) == (100, 50)
```

Springs, staggers and export to CSS:

```python
import anikit

dots = [{"x": 0} for _ in range(5)]
anikit.animate(dots, {"x": 200}, duration=0.8, delay=anikit.stagger(0.05, from_="center"))   # from_: `from` is a keyword in Python
anikit.spring({"scale": 1}, {"scale": 1.5}, stiffness=300, damping=20)
anikit.run_until_idle()

anim = anikit.animate({"x": 0, "opacity": 0}, {"x": 100, "opacity": 1}, duration=0.6, ease="spring(bounce: 0.3)", autoplay=False)
css = anim.to_css_keyframes("enter", selector=".card")    # @keyframes + a rule
print(css.splitlines()[0])
```

Physics world (no DOM): step it yourself and read positions.

```python
import anikit

world = anikit.World(gravity=(0, 980), bounds=(0, 0, 800, 600), restitution=0.3)
balls = [world.add_body(100 + i * 30, 50, radius=10) for i in range(10)]
world.run(2.0)                              # 2 simulated seconds at 60 fps (or world.step(dt) per frame)
print(round(balls[0].x), round(balls[0].y))
```

`world.bind(body, sprite)` (or `body.bind(sprite)`) writes a body's `x`, `y` and `rotation` into a dict or object after every step, like the DOM binding in TypeScript.
