# Scoreboard: DOM and interaction

Anikit against GSAP 3 and Motion 12.43 (vanilla `motion` package) in
headless Chromium (Playwright), on a shared four-core VM with other builds
running. Lower is better everywhere. Every number comes from a script in
`packages/anikit/scripts/`; rerun them before quoting. The core and engine
scoreboard lives in `docs/scoreboard.md` (core session).

Motion's vanilla package has no layout animation, drag or text splitting
(those are in its React package or Motion+), so those rows are against GSAP's
Flip, Draggable and SplitText only.

## Real frames (`node scripts/bench-motion.mjs`)

Main-thread ms per frame (script + style + layout from Chrome's Performance
metrics) over 1.5 s of real `requestAnimationFrame` frames, and set-up ms;
median of 3 fresh pages.

| Case | N | Anikit set-up | Anikit ms/frame | GSAP set-up | GSAP ms/frame | Motion set-up | Motion ms/frame |
|---|---|---|---|---|---|---|---|
| Staggered entrance (opacity + y) | 1000 | 36.8 | **2.25** | 461.6 | 3.18 | **23.3** | 5.92 |
| Spring followers, retargeted every 100 ms | 200 | 4.3 | **0.75** | **2.0** | 0.96 | 8.7 | 1.30 |
| | 1000 | **12.3** | **2.75** | 12.5 | 5.05 | 25.3 | 5.28 |
| Scroll-linked sections, page scrolled every frame | 50 | 7.3 | **0.17** | 17.0 | 0.19 | **4.0** | 1.28 |
| | 200 | 13.1 | **0.21** | 56.7 | 0.23 | **10.1** | 3.49 |

- GSAP has no spring physics: its spring row is `elastic.out` with `overwrite`.
- Scroll-linked: `anikit-compositor` (`scroll(compositor(…))`, a native
  `ScrollTimeline`) measured 8.9 / 32.5 ms set-up and 0.17 / 0.33 ms per frame
  for 50 / 200; the JS path is as fast here.
- Motion defers its keyframe resolution to the first frame, so part of its
  work lands in the frames rather than the set-up.

## A realistic landing page (`node scripts/bench-page.mjs`)

The same page built with each library (`bench/page/`): a split-word hero
reveal, six sections of eight cards with a staggered entrance on scroll, a
pinned scrubbed section, three magnetic buttons and a marquee. One minified
bundle per library, Chromium with the CPU throttled 4× (a mid-range phone),
median of 5 fresh pages. `anikit` embeds the WebAssembly core in the bundle;
`anikit-lean` is the lean build (`anikit/lean`), which streams `anikit.wasm`
so the browser compiles it off the main thread. Motion has no pinning (a
sticky wrapper scrubbed with `scroll()` stands in) and neither GSAP's free
build nor Motion splits text (words split by hand, same markup).

| | gzip KB | ready ms | TBT ms | long tasks | heap MB | p95 / max frame ms | main-thread ms per frame while scrolling |
|---|---|---|---|---|---|---|---|
| Anikit (embedded core) | 392.6 | 174 | 58 | 1 | 2.81 | 16.8 / 16.8 | 0.82 |
| Anikit lean | 297.9 (JS 42 + wasm 256) | 216 | 53 | 2 | 1.93 | 16.8 / 16.8 | **0.80** |
| GSAP | 45.8 | 153 | 58 | 1 | 1.96 | 16.7 / 16.8 | 1.05 |
| Motion | **26.3** | **76** | **0** | **0** | **1.80** | 16.7 / 16.8 | 1.30 |

- Scrolling, every library holds 60 fps at 4× throttling (p95 = one frame);
  Anikit does the least main-thread work per frame (0.80 vs 1.05 / 1.30 ms).
- **Loading, Motion wins** and Anikit loses: the WebAssembly core is 256 KB
  gzipped, and its compile plus the trigger set-up (layout reads for six
  triggers and a pin, synchronous so `start`/`end` are known at once) make
  one or two long tasks; Motion defers its work and has no pinning.
- Found while building it: the lean build re-bundled into a plain script
  (no `import.meta`) threw "Invalid URL" at load before the page could call
  `init(url)`. Fixed (it now waits for `init()`); browser test `lean › …`.

## Scripted (`bench-text.mjs`, `bench-drag.mjs`, `bench-scroll.mjs`)

| Case | Size | Anikit | GSAP | |
|---|---|---|---|---|
| `flip()` vs Flip, ms | 200 boxes | **9.4** | 189.2 | 20× |
| | 1000 boxes | **42.0** | 3128.8 | 74× |
| `splitText()` vs SplitText (lines, words, chars), ms | 20 paragraphs | **95.0** | 138.2 | 1.45× |
| | 100 paragraphs | **469.3** | 782.3 | 1.67× |
| Re-split after a width change, ms | 20 / 100 | **107.2 / 507.5** | 134.3 / 818.8 | 1.25× / 1.61× |
| `draggable()` vs Draggable, µs per move, handler only (`FORCE=0`) | 0 siblings | 6.0 | **5.0** | GSAP renders on its ticker, not on the move |
| | 2000 siblings | **6.4** | 7.6 | |
| Drag release (throw set-up), ms | | **0.2–0.3** | 0.5–0.7 | |
| Scrubbed `scrollTrigger` vs ScrollTrigger, ms per scroll event | 200 | 0.010–0.024 | 0.015–0.029 | level |
| `scrollTrigger` create / refresh, ms | 500 | **29–33 / 8–9** | 140–201 / 53–57 | |

## Where we lose, and why

- **Staggered entrance set-up vs Motion** (36.8 vs 23.3 ms for 1000): this is
  `animate()` creation (core); Motion resolves keyframes in its first frame
  instead. Per frame Anikit costs 2.25 ms against Motion's 5.92, so over the
  animation Anikit does far less work.
- **Spring set-up for 200 vs GSAP** (4.3 vs 2.0 ms): a `spring()` is a real
  simulation with a revert snapshot; GSAP's row is a plain tween. Per frame
  Anikit is ahead.
- **Scroll-linked set-up vs Motion** (13.1 vs 10.1 ms for 200; it was 23.5
  before this round's batched first apply).
- **Drag handler with no siblings** (6.0 vs 5.0 µs): within this VM's noise
  between runs; Anikit also writes the transform on the move, GSAP does not.
