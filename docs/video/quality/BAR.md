# The quality bar (read before showing anyone a frame)

> How to read this: these are failure modes the user has rejected, with the reasons. They are advice for judging your own work (what tends to read as generic, illegible or slow, and why), not a style rule. Any look, medium or genre can work when it is chosen for the idea and executed well; use this list to notice when a choice is a default rather than a decision.

The positive canon behind these rules, with numbers and sources: [`../taste/TASTE.md`](../taste/TASTE.md) (run its §10 taste pass before any final render).

The user has rejected, in their own words, everything in `rejected/`:

- "all look terrible, like AI slop and are boring";
- "the ones that say no browser are REALLY BAD";
- "extremely basic", "no 3d", "don't make sense", "glitchy".

Those frames scored 100/100 on `critique --render` and up to 8/10 from an LLM grader. **Automated scores are not evidence of quality.** Only this checklist plus a look at full-size, in-motion frames counts.

## Common ways a frame fails (and why)

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

### Pace: run `python3 docs/video/quality/pace.py clip.mp4 --strip strip.png`, or `anikit-video critique spec.json --render`

These limits are for a **fast** intent (the user's taste for showcase work). `critique --render` judges against the spec's `"intent"`:
- frantic: 1.2 s longest shot, 0.7 s median;
- fast: the limits below;
- steady: 6 s and 3.5 s;
- calm: long holds are allowed, but a calm piece is still flagged if it is lifeless (dead air) or defective.

These thresholds are for **fast intents** (hype, launch, music, social), which is what this user wants for the flagship. Calm intents (brand films, explainers) may hold longer, but every hold must stay *alive*: motion, light change or parallax, with no dead air. `critique` judges pace against the spec's declared `intent`.

It FAILS if any of these is true:
- any shot holds longer than 2.5 s without a major change;
- the median shot is longer than 2.0 s (benchmark: 1.0 s sport, 1.5 s idents, 2.0 s launch, 2.5 s overall; dead air matters more than cut count);
- more than 20% of seconds look like the second before.

The target is cuts or major changes on the beat every 0.5–1.5 s (120–140 BPM), with energetic camera work: whips, snaps, speed ramps, impacts and match cuts.

### "Beyond other tools"

Every shot must show something that a browser-based tool (Remotion or HyperFrames with WebGL or three.js) can't realistically do. See `docs/video/quality/ONLY_ANIKIT.md`; BENCH maintains it, verified against those tools. A pretty shader or a 3D logo alone does not count: competitors can embed those too.

## Third rejection: Sift (forced 3D, low legibility, synth-heavy score)

The user's verdict on the Sift launch example (`docs/video/skills/launch-examples/sift/`): "the text is hard to read and looks low resolution, I'm hearing too much synthesizer in the music, and the lighting is off … Don't force 3D where it doesn't fit and don't use the same 3D visuals every time." On the reverse-engineer examples: "not good."

So, in addition to the checks above, watch for these (each has been rejected):
- **3D is forced.** The medium must come from the concept. Well-set 2D typography, illustration, UI, photography-style compositing and shader work are first-class; "make every word 3D" is a template, not a fix for flat design.
- **It repeats a look.** Same environment, cyc floor, chrome/lacquer type, falling-tiles or domino motif as a previous piece. Vary the medium, light, lens, palette and materials every time.
- **Text is not crisp and instantly readable.** Every word at full resolution (no reduced-resolution tracing, no DOF, motion blur or bloom on words while they are meant to be read), ≥ 4.5:1 contrast, set in a deliberate typeface at a deliberate size, held long enough to read.
- **The light is unmotivated.** Blown highlights, muddy blacks, flat ambient fill, or light that doesn't match the world. Check stills at 100% for exposure and a clear key light.
- **The score is generic.** Instrumentation must suit the brand and concept (acoustic, orchestral, band, piano, percussion, found sound…); synthesizers only when the concept calls for them.

## Fourth rejection: Margin ("complete AI slop")

The user's verdict on `docs/video/skills/launch-examples/margin/`: "margin also looks like complete AI slop." Swapping 3D clichés for restraint did not fix it; it swapped one generic look for another. The **tasteful-AI look** has been rejected; it tends to appear when restraint is used as a default rather than a choice:
- cream/off-white "paper" ground + an editorial serif (Instrument Serif and kin) + a single red or terracotta accent;
- aphorism copy ("Your first draft is a mess", "Less noise. More signal."), small-caps tracked labels over a serif line;
- sparse, near-static frames: a few lines of type in a mostly empty frame, slow drift as the only motion;
- any combination that could be a Notion/Linear/Arc-alike landing page turned into a video.

What the user is asking for instead is **amazing and specific**: density of craft (many considered layers per frame, real imagery made with the engine: illustration, simulation, shader worlds, photographic-grade compositing), motion with choreography and weight, an idea only this product could have, and a look a designer would not recognise as a trend. Restraint is a choice that must still be rich (cf. title sequences), never emptiness.
