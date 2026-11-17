# The quality bar (read before showing anyone a frame)

The user has rejected, in their own words, everything in `rejected/`:

- "all look terrible, like AI slop and are boring";
- "the ones that say no browser are REALLY BAD";
- "extremely basic", "no 3d", "don't make sense", "glitchy".

Those frames scored 100/100 on `critique --render` and up to 8/10 from an LLM grader. **Automated scores are not evidence of quality.** Only this checklist plus a look at full-size, in-motion frames counts.

## A frame FAILS if any of these is true

1. **Centred headline on a gradient.** A bold sans headline (or any big centred text) sits over a gradient, blob or mesh backdrop.
2. **Soft blob backdrop.** The background is a blurred colour-blob, mesh-gradient or aurora wash doing the work of "design".
3. **Placeholder UI.** Rounded cards, pills, progress bars or placeholder lines stand in for UI or content (grey bars instead of real text or data).
4. **Small content in an empty frame.** The content occupies a small part of a mostly empty 1920×1080 frame.
5. **Flat 2D only.** There is no depth, material or light: no real 3D, no shading, no lens behaviour (DOF, bloom from real highlights), no parallax.
6. **Slide-deck structure.** Scenes are a sequence of title cards or slides rather than one designed world with motion that has weight.
7. **Stock-demo look.** It resembles a stock template or tutorial: checker floors, primitive spheres and cubes and tori, rainbow palettes, default Inter-on-dark, generic "SaaS dashboard".
8. **Effect without subject.** An effect is shown for its own sake (a lens flare, particles or glow over nothing).
9. **Any visible defect.** Stepping or juddering motion, banding, aliasing or shimmer, clipped text, or a pop at a cut.

## A frame PASSES only if

- it reads as a **designed image** a top studio would post: a clear subject, deliberate composition, real light and materials, and a considered palette and type;
- it holds up at **100% crop** and as **consecutive frames** taken from the MP4.

## Process

- Judge every shot against this list *before* sending it to the parent. A failing frame never reaches the user.
- Show work as a fast-cut test of at most 15 s that passes the pace check below, not as single slow shots or as galleries of isolated parts.

## Second rejection: pace and "beyond other tools"

The user's verdict on Meridian, Lighthouse Keeper, Tidewater and Northern Silver (stills in `rejected/*-slow.jpg`): "super boring, look generic, they don't show anything incredible/beyond what other products can do, super slow paced, aren't visually impressive."

Every one of them was a single 6–7 s slow dolly, with the same composition in all four quarters of the shot. A still checklist can't catch that, so clips now also face the two tests below.

### Pace: run `python3 docs/video/quality/pace.py clip.mp4 --strip strip.png`

These thresholds are for **fast intents** (hype, launch, music, social), which is what this user wants for the flagship. Calm intents (brand films, explainers) may hold longer, but every hold must stay *alive*: motion, light change or parallax, with no dead air. `critique` judges pace against the spec's declared `intent`.

It FAILS if any of these is true:
- any shot holds longer than 2.5 s without a major change;
- the median shot is longer than 1.5 s;
- more than 20% of seconds look like the second before.

The target is cuts or major changes on the beat every 0.5–1.5 s (120–140 BPM), with energetic camera work: whips, snaps, speed ramps, impacts and match cuts.

### "Beyond other tools"

Every shot must show something that a browser-based tool (Remotion or HyperFrames with WebGL or three.js) can't realistically do. See `docs/video/quality/ONLY_ANIKIT.md`; BENCH maintains it, verified against those tools. A pretty shader or a 3D logo alone does not count: competitors can embed those too.
