---
name: anikit-launch-video
description: Make a product launch, announcement or teaser video with anikit-video from a short brief. Principles, process and capabilities, not a template - every film gets a structure, tempo and look designed from the product's own story - with a render-and-look QA loop. Use when asked for a launch, product, announcement, teaser or feature video.
---

# Launch video

**First:** read `skills/anikit-video/SKILL.md` (setup, the loop, primitives, costs), then `anikit-video docs launch`.
Open `docs 3d`, `docs particles`, `docs physics`, `docs shader` and `docs music` only for what you use.
Companion files: `concepts.md` (how to find the film's idea) and `beats.md` (how to time picture to music).

**There is no template.** No fixed act structure, tempo, length, palette or shot list. If the film you are
planning would fit any product, rethink it: design it from this product's own story.

## Principles (what makes launch films work)

1. **Earn attention in the first second.** Frame 1 moves and raises a question: an image, a sound, a contradiction.
   It is not a logo, a fade from black or a title card.
2. **The product, or its effect, is the hero.** Show the real thing working (its UI, its output, what changes in the
   world because of it). Use real copy from the brief, never placeholder UI.
3. **One idea, carried through.** One visual motif or device runs through the whole film and pays off at the end.
4. **Show, don't claim.** Replace adjectives with demonstrations. Every word on screen must earn its place; fewer
   is stronger.
5. **The ending resolves the idea.** The motif lands, and the name and the call to action follow from it. The logo
   owns the frame when it arrives.
6. **Sound and picture move together.** Cuts, hits and holds follow the music you chose for this film (which may be
   calm), with sfx where things happen.
7. **Honesty.** No invented statistics, customers or features. Proof is a fact from the brief or a demonstration.

## Process

1. **Read the brief; ask nothing you can infer.** You need the product and what it does. Infer the rest from the
   brief, URL, README or screenshots: features, audience, brand colours and fonts, length, aspect (9:16 for
   Reels/TikTok/Shorts, 1:1 for feed) and mood. Default to 15–30 s at 16:9 only when nothing says otherwise.
2. **Find the idea** (`concepts.md`): the product's nouns and verbs, its before → after, one motif. Write it down
   in `concept.txt` next to the spec. Look at what previous videos in the folder used, and go somewhere new.
3. **Design the film's structure from that idea.** Decide what the viewer must understand, in what order, and where
   the turn is. Shapes that have worked, **as inspiration only** (don't pick one; invent the one this product needs):
   - one continuous, impossible camera move through the product's world;
   - a reverse reveal: start at the result, then rewind to how it happened;
   - a single object transforming for the whole film;
   - a demo-only film with no words until the name;
   - a typographic manifesto where the words *are* the set;
   - a loop that ends exactly where it started, now changed;
   - a day-in-the-life time-lapse with the product as the constant;
   - a scale journey from macro detail to cosmic (or back).
4. **Choose tempo, pace and length from the brief and the idea.** Declare `"intent": {"pace": …, "genre": "launch"}`.
   A calm, held film is as valid as a frantic one; pick music to match (`docs music`) and time to it (`beats.md`).
5. **Build with the capabilities that serve the idea** (below). Iterate with `still` and draft renders.
6. **Run the quality checks** (below) and fix the worst frame first. Two or three iterations is normal.

## Capabilities (use where they serve the story, not as a checklist)

- **Product as hero:** the UI as a composition, used as a texture (`"texture": "#ui"`) on a `pane`, a device body
  (an `aluminium` `box` behind a pane) or thousands of instances; or `product-shot` (a phone on a plinth in blinds light).
- **Ray-traced worlds:** `scene3d` with PBR materials, DOF, fog and light shafts, `heightfield`/`extrude`/`lathe`/
  `tube`/`surface` geometry, and instancing with per-instance expressions.
- **Simulation:** `physics` (piles, dominoes, impulses, `timeScale` speed ramps); `particles` (emit from text, images
  or compositions; flow and attractors; millions of points).
- **Shader worlds:** your own GLSL or the library (`anikit-video recipes`) as a set, filter or transition.
- **Typography:** 2D type with real design (`split`, masks, grids, kinetic motion); 3D `text`, type as a texture or
  particle type when the idea is physical.
- **Music and sound:** a generated score in any style and tempo (`"music": {...}`, `hits`), `@beatN`/`@barN`
  timing, `beat`/`bar`/`level` expressions, and sfx (`whoosh`, `impact`, `riser`, `click`, `chime` …).
- **Camera:** keyed 3D cameras, whips, snap zooms, shake, rack focus and real motion blur.

## Craft guidance: what works, what to avoid, and why

The depth lives in `docs/video/taste/TASTE.md`: Type §1, Colour §2, Composition §3, Light §4, Motion §5, Editing §6,
Sound §7, Concept §8, tells §9, the taste pass §10. These notes are the launch-specific lessons. They are advice
with reasons, not bans: break one when the concept truly needs it, and say why in `concept.txt`.

- **Medium.** *Works:* choosing the medium the idea lives in. A physical product or material idea earns ray-traced
  3D; an idea about words earns typography; an idea about data earns simulation or instancing. *Avoid:* reaching for
  3D (or for flat 2D) by default. *Why:* Sift forced every word into 3D and the user said "don't force 3D where it
  doesn't fit"; a flat headline on a fill reads as a slide (BAR #1). Design solves both.
- **Freshness.** *Works:* listing what previous films used (medium, world, motif, materials, light, palette, music;
  see `docs/video/skills/launch-examples/`), then deliberately going somewhere else. *Avoid:* landing on looks the
  user has rejected: tile storms and dominoes on a cyc, chrome or lacquer type (Sift); fogged terrain (Fernway);
  cream paper with an editorial serif, a red accent and aphorism copy (Margin, "complete AI slop"). *Why:* each one
  read as a template the second time.
- **Trends.** *Works:* naming the trend the piece resembles ("tasteful AI editorial", "Apple-style product orbit",
  "SaaS gradient", "Y2K chrome" …). If you can name it, rethink the piece until the idea, not the trend, sets the
  look. *Why:* a recognisable trend is what makes a film read as generic (TASTE §9, BAR's last section).
- **Density.** *Works:* frames with a clear subject plus layered detail (texture, secondary motion, typographic
  hierarchy, light) and something to look at the whole time they are on screen. *Avoid:* sparse, static frames
  presented as minimalism. *Why:* Margin's empty restraint read as unfinished.
- **Copy.** *Works:* the brief's real nouns, numbers and actions: what it does, for whom, in what situation.
  *Avoid:* aphorisms that could sit on any brand ("Your first draft is a mess", "Built to last"). *Why:* specific
  copy is proof; generic copy is a tell.
- **Legibility.** *Works:* every word rendered at full resolution (2D layers, or `scene3d` at `"resolution": 1` and
  not `draft`), in focus with no motion blur, bloom or grain while it is read, about 4.5:1 contrast or more, at
  sizes that hold on a phone (roughly ≥ 4.5 vh for body and ≥ 9 vh for hero words at 16:9; ≥ 5 and ≥ 11 vw at 9:16),
  and held for about 0.3 s + 0.06 s per character. *Why:* the user found Sift's type "hard to read and low
  resolution" (cause in Gotchas). Type sets the perceived quality of the whole film.
- **Light.** *Works:* a motivated key, fill and rim that belong to the world, with the key's direction readable;
  exposure checked on stills at 100%. *Avoid:* clipped whites on type or product, crushed blacks, flat ambient fill.
  *Why:* the user said Sift's "lighting is off" (TASTE §4).
- **Music.** *Works:* instrumentation chosen from the brand and the idea. Acoustic and organic palettes (the
  `piano`, `acoustic`, `orchestral`, `cinematic`, `jazz`, `rock` and `pop` styles, or `tracks` with piano, strings,
  guitars, harp, marimba, brass or GM drums) suit most products; synth styles (`house`, `synthwave`, `tech`, `dnb`,
  `trap`) suit electronic ideas. *Avoid:* a synth bed by default. *Why:* Sift used `house` and the user heard "too
  much synthesizer". `anikit-video music instruments` lists everything (TASTE §7).
- **Ending.** *Works:* the logo owning the frame as the idea resolves. *Avoid:* a small wordmark in a corner.

## Checks (process)

- **The loop:** `anikit-video check s.json` → `still --at 0.3` (is frame 1 intriguing with no context?) → stills of
  every word at 100% crop → `render --quality draft --scale 0.5` → `pace.py` for fast intents →
  `critique s.json --render --pack rv/` (look at every image) → final `render` at full quality, then look at frames
  from the MP4 itself at 100%.
- **The taste pass** (TASTE §10) and BAR.md before showing anyone.
- **Ask yourself:** can you name the idea from the strip alone? Would this film fit a different product (if yes,
  rethink it)? Which trend does it resemble? Would the brand's team ship it?

## Gotchas

- A `path` layer sits at its bounding box: set `x`/`y` to the path's top-left.
- In `scene3d`, an expression `scale` must be a vector, `"=[s, s, s]"`; `0` hides an instance.
- `card` is a reflector, invisible to the camera; `phone` is life-size (16 cm).
- Keep a camera above any `heightfield` (inside it the frame turns flat white). A dark set under `night` needs a
  low key light (`intensity` 10–20).
- `pace.py` can miss a cut between two shots of similar brightness.
- 3D and particles cost 0.3–1.5 s a frame: iterate with `"quality": "draft"` and `"resolution": 0.5`, but **the final
  must use `"resolution": 1` and no `draft`.**
- **Why Sift's type looked low-resolution:** every `scene3d` shot kept the iteration settings in the final
  (`"resolution": 0.5`, which traces at half size and upscales, plus `"quality": "draft"`). On top of that the words
  sat inside shallow DOF (`fstop` 2–3), under bloom and `post` grain, and were then re-encoded at CRF 22. Fix: words
  in 2D layers, or 3D at full resolution with the word in the focal plane; no grain on type; `render --crf 18` or
  lower for type-heavy finals.
- `render --crf N` sets the file size (higher N = smaller file). `"text": "=expr"` evaluates expressions.
