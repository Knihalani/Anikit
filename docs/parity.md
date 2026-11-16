# Feature parity: anikit vs GSAP, Motion and anime.js

How anikit's feature set compares with the main web animation libraries, area by area. The goal is to be accurate, not to flatter anikit: where another library does something better or anikit lacks it, the table says so, and the honest gaps are listed at the end.

**What was compared**

- **anikit:** the exports of `packages/anikit/src/index.ts` and the behaviour documented in [api.md](api.md).
- **GSAP 3.15** (core plus every plugin, all free since 3.13): checked against `gsap/dist/*.js` and `gsap/types/*.d.ts`.
- **Motion 12.43** (`motion` / `framer-motion` / `motion-dom`): checked against the `.d.ts` files. Many Motion features are React-only, and the notes say which. Paid "Motion+" components (`AnimateNumber`, `Ticker`, `splitText`) are not counted because they aren't in the npm package.
- **anime.js v4:** not installed here. That column comes from the v4 API (`animate`, `createTimeline`, `createTimer`, `createAnimatable`, `createDraggable`, `createScope`, `onScroll`, `stagger`, `svg.morphTo` / `createDrawable` / `createMotionPath`, `splitText`, `createSpring`, `utils`, `waapi`) as known up to v4.2 and **was not checked against source**. Newer v4 releases may have added features.

Legend: ✅ supported · ⚠️ partial, or only with extra work · ❌ not available

## Core tweening

| Feature | anikit | GSAP | Motion | anime.js |
|---|---|---|---|---|
| Tween DOM, SVG, CSS vars, plain objects | ✅ `animate` | ✅ `gsap.to` | ✅ `animate` | ✅ `animate` |
| from / fromTo / set | ✅ `from`, `fromTo`, `set` | ✅ | ⚠️ keyframe arrays; no `set` | ✅ `[from, to]`, `utils.set` |
| Keyframes (arrays, offsets, per-segment ease) | ✅ `times`, WAAPI-style lists | ✅ `keyframes` | ✅ `times` | ✅ |
| Relative values (`+=`, `-=`, `*=`) | ✅ | ✅ (`+=` / `-=`) | ❌ | ✅ |
| Function-based values per target | ✅ `(el, i, n) => v` | ✅ | ⚠️ via `stagger()` and delay functions | ✅ |
| `random()` strings in values | ✅ `'random(-100, 100, 10)'` | ✅ | ❌ | ⚠️ `utils.random` in a function |
| Complex strings (shadows, filters, gradients) | ✅ reconciles mismatched lists | ✅ | ✅ | ⚠️ matching structure needed |
| Colour interpolation space | ✅ OKLab default; sRGB, linear, OKLCH, HSL | ⚠️ RGB / HSL | ⚠️ linear-light RGB | ⚠️ RGB |
| `height: 'auto'` | ✅ stays responsive afterwards | ✅ | ✅ | ⚠️ manual measuring |
| Full CSS `transform` lists (decompose + slerp) | ✅ | ⚠️ separate components | ⚠️ separate components | ⚠️ separate components |
| Overwrite / auto-kill conflicting tweens | ✅ `overwrite` | ✅ `overwrite: 'auto'` | ✅ | ✅ `composition: 'replace'` |
| Additive composition | ❌ | ❌ | ❌ | ✅ `composition: 'add'` |
| Modifiers / snap / round | ✅ `modifiers`, `snap`, `round` | ✅ `modifiers`, `snap` | ⚠️ motion-value transforms | ✅ `modifier` |
| Compositor-thread (WAAPI) playback | ✅ `compositor()`, springs as `linear()` | ❌ | ✅ hybrid engine | ✅ `waapi.animate` |
| Bake an animation to CSS `@keyframes` | ✅ `anim.toCSS()` | ❌ | ❌ | ❌ |
| Bulk numeric tweens into a `Float32Array` | ✅ `batch()` (50k+ items) | ❌ | ❌ | ❌ |
| Quick re-targeting setters | ✅ `quickTo`, `quickSetter` | ✅ `quickTo`, `quickSetter` | ✅ motion values | ✅ `createAnimatable` |
| Promise / thenable controllers | ✅ | ✅ `.then()` | ✅ | ✅ `.then()` |

## Timelines

| Feature | anikit | GSAP | Motion | anime.js |
|---|---|---|---|---|
| Timeline with positions (`<`, `>`, `+=`, labels) | ✅ `timeline()` | ✅ | ⚠️ `animate(sequence)` with `at` | ✅ `createTimeline` |
| Array-based sequence shorthand | ✅ `sequence()` | ❌ | ✅ | ❌ |
| Nesting timelines and animations | ✅ | ✅ | ❌ | ✅ `sync()` |
| Callbacks / set at a position | ✅ `call`, `set` | ✅ | ⚠️ function segments | ✅ `call`, `set` |
| Label navigation, `tweenTo`, `tweenFromTo` | ✅ | ✅ | ❌ | ⚠️ seek to labels |
| `addPause`, `getById` | ❌ | ✅ | ❌ | ❌ |
| Delayed calls / timers on engine time | ✅ `delayedCall` | ✅ `delayedCall` | ✅ `delay` | ✅ `createTimer` |
| Repeat, yoyo, speed, reverse, seek, revert | ✅ | ✅ | ⚠️ no revert | ✅ |

## Easing

| Feature | anikit | GSAP | Motion | anime.js |
|---|---|---|---|---|
| Penner set (quad…expo, back, elastic, bounce) | ✅ Penner, Motion and GSAP names | ✅ | ⚠️ no elastic or bounce | ✅ |
| CSS strings (`cubic-bezier`, `steps`, `linear()`) | ✅ | ⚠️ `steps(n)`; bezier needs CustomEase | ✅ arrays, `steps` | ✅ |
| Spring as an easing | ✅ `'spring(bounce: .3)'`, presets | ❌ | ✅ | ✅ `createSpring` |
| Draw an ease from an SVG path (CustomEase) | ✅ `'custom(M0,0 C…)'` | ✅ CustomEase | ❌ | ❌ |
| CustomBounce (with squash) | ✅ `customBounce({ strength, squash })` → `{ ease, squash }` | ✅ CustomBounce | ❌ | ❌ |
| CustomWiggle | ✅ `customWiggle(n, { type })` (easeOut, easeInOut, anticipate, uniform, random), `'wiggle(n)'` | ✅ CustomWiggle | ❌ | ❌ |
| EasePack (SlowMo, RoughEase, ExpoScaleEase) | ✅ `slowmo`, `rough`, `expoScale` | ✅ EasePack | ❌ | ⚠️ `irregular` |
| Ease modifiers (reverse, mirror, blend) | ✅ `reverse()`, `mirror()`, `blend()` | ⚠️ in / out / inOut variants | ✅ `reverseEasing`, `mirrorEasing` | ⚠️ in / out variants |
| Typo suggestions for ease names | ✅ throws "did you mean" | ❌ | ❌ | ❌ |

## Physics and springs

| Feature | anikit | GSAP | Motion | anime.js |
|---|---|---|---|---|
| Interruptible spring that keeps velocity | ✅ `spring()` retargets | ❌ | ✅ `spring`, `useSpring`, `springValue` | ⚠️ spring ease only |
| Inertia / momentum with bounds and snap | ✅ `inertia()` | ✅ InertiaPlugin | ✅ `inertia` | ⚠️ draggable release only |
| Physics2D (velocity, gravity, friction) | ✅ `physics()` per axis | ✅ Physics2DPlugin (velocity + angle) | ❌ | ❌ |
| PhysicsProps (any property) | ✅ `physics()` per property | ✅ PhysicsPropsPlugin | ❌ | ❌ |
| Bouncing off bounds, frame-rate independent | ✅ `restitution`, exact solve | ❌ | ❌ | ❌ |
| Rigid-body world (bodies, ropes, cloth) | ✅ `world()` in Wasm | ❌ | ❌ | ❌ |

## Scroll

| Feature | anikit | GSAP | Motion | anime.js |
|---|---|---|---|---|
| Scroll-linked progress / scrub | ✅ `scroll()`, `scrub` | ✅ ScrollTrigger `scrub` | ✅ `scroll`, `useScroll` | ✅ `onScroll({ sync })` |
| Native `ScrollTimeline` (no JS while scrolling) | ✅ with `compositor()` | ❌ | ✅ | ❌ |
| Enter / leave triggers, `toggleActions` | ✅ `scrollTrigger()` | ✅ | ⚠️ `inView`, `whileInView` | ✅ `onScroll` callbacks |
| Pinning | ✅ `pin`, `pinSpacing` | ✅ | ❌ (CSS `sticky`) | ❌ |
| Snapping (steps, labels, function) | ✅ | ✅ | ❌ | ❌ |
| Debug markers | ✅ `markers` | ✅ | ❌ | ✅ `debug` |
| Batch triggers | ✅ `scrollTriggerBatch` | ✅ `ScrollTrigger.batch` | ⚠️ `inView` on many elements | ❌ |
| Horizontal sections (`containerAnimation`) | ❌ | ✅ | ❌ | ❌ |
| `scrollerProxy` / `normalizeScroll` | ❌ | ✅ | ❌ | ❌ |
| Animated scroll-to | ✅ `scrollTo` | ✅ ScrollToPlugin | ❌ | ⚠️ animate `scrollTop` |
| Smooth scrolling | ⚠️ `smoothScroll` (Lenis-style), no `data-speed` / `data-lag` | ✅ ScrollSmoother | ❌ | ❌ |
| Parallax helper | ✅ `parallax()` | ⚠️ ScrollSmoother `data-speed` | ⚠️ `useScroll` + `useTransform` | ⚠️ manual |
| Scroll progress as a reactive value (with velocity) | ✅ `scrollProgress()` | ⚠️ `getVelocity()` on a trigger | ✅ `useScroll`, `useVelocity` | ❌ |

## Gestures and interaction

| Feature | anikit | GSAP | Motion | anime.js |
|---|---|---|---|---|
| Draggable with bounds, snap, edge resistance | ✅ `draggable()` | ✅ Draggable | ✅ `drag` prop (React only) | ✅ `createDraggable` |
| Throw / momentum on release | ✅ `inertia: true` | ✅ with InertiaPlugin | ✅ `dragMomentum` | ✅ spring release |
| Rotation dragging | ✅ `type: 'rotate'` | ✅ | ❌ | ❌ |
| Auto-scroll the container while dragging | ❌ | ✅ `autoScroll` | ❌ | ✅ |
| `hitTest`, `top,left` drag type | ❌ | ✅ | ❌ | ❌ |
| Drag-to-reorder list | ❌ | ❌ | ✅ `Reorder` (React) | ❌ |
| Hover / press helpers (touch-safe) | ✅ `hover`, `press` | ❌ | ✅ `hover`, `press`, `whileHover`, `whileTap` | ❌ |
| Pan gesture | ✅ `gesture` | ⚠️ Observer `onDrag` | ✅ `onPan` (React) | ❌ |
| Pinch zoom and two-finger rotate | ✅ `gesture()` | ❌ | ❌ | ❌ |
| Wheel / touch / pointer intent observer | ✅ `observer()` | ✅ Observer | ❌ | ❌ |
| Magnetic buttons, 3D tilt, marquee | ✅ `magnetic`, `tilt`, `marquee` | ❌ | ❌ | ❌ |

## Layout and presence

| Feature | anikit | GSAP | Motion | anime.js |
|---|---|---|---|---|
| FLIP layout transitions | ✅ `flip()`, `flip.capture` / `from` | ✅ Flip | ✅ `layout` prop, vanilla layout builder | ❌ |
| Shared-element transitions | ✅ `data-flip-id` | ✅ `data-flip-id` | ✅ `layoutId`, `LayoutGroup` | ❌ |
| Scale distortion correction | ⚠️ `correctRadius` (opt-in), no shadows | ⚠️ `scale: false` | ✅ radius + box-shadow, automatic | ❌ |
| `Flip.fit()` (move one element onto another's box) | ❌ | ✅ | ❌ | ❌ |
| Exit animations before removal | ✅ `exit`, `enter`, `presence()`, React `<Presence>` | ⚠️ Flip `onLeave` | ✅ `AnimatePresence` (React) | ❌ |
| Presence modes (wait / sync / pop layout) | ⚠️ `replace(..., { mode })` wait / sync | ❌ | ✅ | ❌ |
| View Transitions API wrapper | ✅ `viewTransition()` with fallback | ❌ | ✅ `animateView` | ❌ |
| Declarative variants / orchestration | ✅ `variants()` with `delayChildren`, `staggerChildren`, `when` | ❌ | ✅ `variants`, `staggerChildren` | ❌ |
| Reactive values (map, derive, bind) | ✅ `motionValue`, `derived`, `bindValues` | ⚠️ `quickTo` | ✅ `useMotionValue`, `useTransform`, `useSpring` | ⚠️ `createAnimatable` |

## SVG

| Feature | anikit | GSAP | Motion | anime.js |
|---|---|---|---|---|
| Shape morphing between arbitrary paths | ✅ `d:`, `morph()`, `morphSequence` | ✅ MorphSVG | ❌ (third-party mixer) | ⚠️ `svg.morphTo`, point-based |
| Manual point mapping (`shapeIndex`) | ❌ (automatic matching only) | ✅ | ❌ | ❌ |
| Stroke drawing | ✅ `draw: [0, 1]` | ✅ DrawSVG | ✅ `pathLength` | ✅ `svg.createDrawable` |
| Motion along a path with auto-rotate and align | ✅ `path: { rotate, align }` | ✅ MotionPath | ⚠️ CSS `offset-path`, `arc()` | ✅ `svg.createMotionPath` |
| Smooth curve through points | ✅ `pathThrough` | ✅ `curviness` | ❌ | ❌ |
| Visual path editor | ❌ | ✅ MotionPathHelper | ❌ | ❌ |
| Path utilities (length, bounds, trim, simplify, transform) | ✅ | ⚠️ MotionPath utilities | ❌ | ❌ |
| Shape builders (circle, star, polygon…) | ✅ | ❌ | ❌ | ❌ |

## Text

| Feature | anikit | GSAP | Motion | anime.js |
|---|---|---|---|---|
| Split into chars / words / lines | ✅ `splitText`, grapheme-safe, bidi-aware | ✅ SplitText | ❌ (Motion+ only) | ✅ `splitText` |
| Line masks, auto re-split on resize | ✅ `mask`, `autoSplit`, `onSplit` | ✅ | ❌ | ⚠️ |
| Typewriter text | ✅ `text: '…'` | ✅ TextPlugin | ❌ | ❌ |
| Scramble text | ✅ `type: 'scramble'` | ✅ ScrambleText | ❌ | ❌ |
| Formatted number counters (`Intl`) | ✅ `format`, `locale` | ⚠️ `snap` + `onUpdate` | ⚠️ motion value + render | ⚠️ `modifier` |
| Text as particles / kinetic type on canvas | ✅ `textPoints`, `typeFx` | ❌ | ❌ | ❌ |

## Utilities

| Feature | anikit | GSAP | Motion | anime.js |
|---|---|---|---|---|
| clamp, mapRange, wrap, snap, interpolate, pipe | ✅ | ✅ `gsap.utils` | ⚠️ `clamp`, `wrap`, `interpolate`, `mix` | ✅ `utils` |
| `wrapYoyo`, `unitize`, `selector` | ❌ (`context` scopes selectors) | ✅ | ❌ | ❌ |
| Seeded random and shuffle | ✅ | ⚠️ unseeded | ❌ | ✅ |
| Stagger with grid, `from`, ease | ✅ `stagger`, `distribute` | ✅ | ⚠️ no grid | ✅ |
| Kill / query tweens of a target | ❌ (per controller or `context().revert()`) | ✅ `killTweensOf`, `getTweensOf`, `isTweening` | ❌ | ✅ `utils.remove` |
| Cleanup scope + media queries | ✅ `context`, `matchMedia` | ✅ `gsap.context`, `gsap.matchMedia` | ⚠️ React lifecycle, `useAnimate` scope | ✅ `createScope` |
| Reduced-motion handling | ✅ `engine.reducedMotion`, `'fade'` mode | ⚠️ via `matchMedia` | ✅ `MotionConfig`, `useReducedMotion` | ⚠️ via `createScope` |
| Registered effects / presets | ✅ 35+ presets, `registerEffect` | ⚠️ `registerEffect`, no presets | ❌ | ❌ |
| Global speed, lag smoothing, fps cap | ✅ `engine` | ✅ `globalTimeline`, `ticker` | ⚠️ frame loop | ✅ `engine` |
| Simplex noise, velocity tracker | ✅ `noise2/3`, `fbm`, `VelocityTracker` | ⚠️ velocity tracking (Inertia) | ⚠️ motion-value velocity | ❌ |

## 3D, particles and video

| Feature | anikit | GSAP | Motion | anime.js |
|---|---|---|---|---|
| 3D projection and depth sort in Wasm | ✅ `scene3d` | ❌ | ❌ | ❌ |
| Point clouds with shape morphing + glow renderer | ✅ `pointCloud`, `glow` | ❌ | ❌ | ❌ |
| Particle emitters | ✅ `emitter` | ❌ | ❌ | ❌ |
| Software 3D renderer (meshes, shadows, DoF) | ✅ `renderer3d`, `mesh`, `shatter`, `flightPath` | ❌ | ❌ | ❌ |
| Canvas library plugins (Pixi, Easel) | ❌ (tween plain objects instead) | ✅ PixiPlugin, EaselPlugin | ❌ | ❌ |
| Audio analysis (tempo, beats, onsets) | ✅ `analyzeAudio` | ❌ | ❌ | ❌ |
| Frame-exact capture to GIF / WebM / PNG | ✅ `record`, `screenshot`, `GifWriter` | ❌ | ❌ | ❌ |
| Code-defined films rendered offline (browser / Node) | ✅ `composition`, `anikit-render` | ❌ | ❌ | ❌ |

## Tooling and DX

| Feature | anikit | GSAP | Motion | anime.js |
|---|---|---|---|---|
| TypeScript types | ✅ | ✅ | ✅ | ✅ |
| React / Vue bindings | ⚠️ `anikit/react`: `useAnikit`, `useAnimate`, `useRecipe`, `useSpring`, `useScroll`, `useTransform`, `useMotionValue`, `<Presence>`; `anikit/vue` directives (`v-recipe`, `v-anikit`); `anikit/svelte` actions (`use:recipe`, `use:anikit`) | ✅ `@gsap/react` (separate package) | ✅ React; Vue via `motion-v` | ❌ |
| Unknown-option and unit warnings with "did you mean" | ✅ `anikit:` warnings | ⚠️ invalid-property warnings | ⚠️ some dev warnings | ❌ |
| Deterministic fake time for tests | ✅ `testing.useFakeTime`, `advance` | ⚠️ seek the global timeline | ⚠️ `MotionGlobalConfig` skip / instant | ⚠️ manual engine loop |
| Visual playback inspector | ⚠️ `inspector()` (play, scrub, speed, labels) | ✅ GSDevTools | ❌ | ❌ |
| JSON snapshot of running animations (for agents) | ✅ `engine.inspect()` | ❌ | ❌ | ❌ |
| `llms.txt` in the package | ✅ | ❌ | ❌ | ❌ |
| Headless recording CLI | ✅ `npx anikit-record` | ❌ | ❌ | ❌ |
| Python bindings (same core) | ✅ `crates/anikit-py` | ❌ | ❌ | ❌ |
| One-call recipes | ✅ `anikit.recipe(name, el, options)` | ❌ | ❌ | ❌ |
| Project scaffold | ✅ `npx anikit new` | ❌ | ❌ | ❌ |

## Lottie

Lottie (`lottie-web`) is in a different category from the others. It's a **player** for After Effects animations exported as JSON with Bodymovin, not a library for animating your own DOM. It was not installed here, so this section is based on knowledge of lottie-web rather than its source.

| Lottie capability | anikit equivalent |
|---|---|
| Plays designer-authored After Effects JSON (shapes, masks, mattes, precomps, text, partial expressions) | ❌ No Lottie / AE import. Motion has to be written in code. |
| SVG, canvas and HTML renderers | ⚠️ anikit animates the real DOM / SVG, and draws to canvas with its own renderers (`glow`, `renderer3d`). |
| `play`, `pause`, `setSpeed`, `setDirection`, `goToAndStop`, `playSegments` | ✅ The same controls on any `Timeline` / `Animation` (`seek`, `speed`, `reverse`, labels). |
| Scroll or hover sync (lottie-interactivity) | ✅ Any timeline can go to `scroll()`, `scrollTrigger({ scrub })` or `hover()`. |
| `.lottie` / dotLottie packaging, theming and state machines (separate players) | ❌ |
| Export to video / GIF | ✅ `record()`, `composition` + `anikit-render` (Lottie needs external tools). |

To ship a designer's After Effects animation, Lottie is still the right tool. anikit can sit next to it, driving `lottie.goToAndStop(frame, true)` from a `scroll()` or a timeline `onUpdate`.

## Gaps

The honest remaining gaps in anikit, most important first:

1. **Thin framework bindings, no declarative component API.** `anikit/react` has hooks and `<Presence>`, and Vue/Svelte get directives and actions, but there are no `whileHover` / `whileInView` props on components. This is Motion's core audience.
2. **No Lottie / After Effects import.** Designer-authored motion can't be played: everything is code.
3. **Layout animation is less complete than Motion's.** Box-shadow distortion isn't corrected, radius correction is opt-in, and there's no `Reorder`, no `popLayout` presence mode and no `Flip.fit()`.
4. **Missing scroll features.** There's no `containerAnimation` for horizontal-scroll sections and no `scrollerProxy` / `normalizeScroll` (mobile address-bar jumps, third-party scrollers). `smoothScroll` has none of ScrollSmoother's `data-speed` / `data-lag` effects (`parallax()` partly covers them).
5. **No global tween management.** There's no `killTweensOf` / `getTweensOf` / `isTweening` / `getProperty`: you need the controller or a `context()`. There's also no additive composition (anime.js `composition: 'add'`).
6. **Missing Draggable features.** There's no `autoScroll` near container edges, no `hitTest` and no `top,left` positioning mode.
7. **Lighter dev tools.** `inspector()` is lighter than GSDevTools (no timeline picker, in/out points or persistence), and there's no visual motion-path editor like MotionPathHelper.
8. **Less control over morphs.** Point matching is automatic only: there's no `shapeIndex` or manual mapping when the automatic match twists.
9. **Timeline extras.** There's no `addPause()` and no `getById()`.
10. **Niche integrations.** There's no PixiPlugin / EaselPlugin (plain-object tweening covers most uses), no CSSRulePlugin, and no `wrapYoyo` / `unitize` utilities.
