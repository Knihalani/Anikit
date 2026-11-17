---
name: anikit-launch-video
description: Make a product launch, announcement or teaser video with anikit-video from a short brief. Principles, process and capabilities, not a template - every film gets a structure, tempo and look designed from the product's own story - with a render-and-look QA loop. Use when asked for a launch, product, announcement, teaser or feature video.
---

# Launch video

**First:** read `skills/anikit-video/SKILL.md` (setup, the loop, primitives, costs), then `anikit-video docs launch`.
Open `docs 3d`, `docs particles`, `docs physics`, `docs shader` and `docs music` only for what you use.
Companion files: `concepts.md` (how to find the film's idea) and `beats.md` (how to time picture to music).

**There is no template.** No fixed act structure, tempo, length, palette or shot list. If the film you are
planning would fit any product, it is wrong: design it from this product's own story.

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
   in `concept.txt` next to the spec. Never reuse the concept or look of a previous video in the folder.
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
- **Type inside the world:** extruded 3D `text`, type as a texture, particle type, or type masked by the motif.
- **Music and sound:** a generated score in any style and tempo (`"music": {...}`, `hits`), `@beatN`/`@barN`
  timing, `beat`/`bar`/`level` expressions, and sfx (`whoosh`, `impact`, `riser`, `click`, `chime` …).
- **Camera:** keyed 3D cameras, whips, snap zooms, shake, rack focus and real motion blur.

## Quality checks (must pass before the final render)

- **No flat type.** Every word is 3D `text` in the set, part of a texture on an object, particles or emissive type,
  or masked by the motif. Top-level 2D `text` is only for captions, legal lines and the end URL. No headline on a
  solid or gradient fill, no word in a coloured pill (BAR #1, #3). Self-test (lists every 2D text layer):
  `python3 -c "import json,sys; f=lambda L: [x for l in L for x in ([l.get('text')] if 'text' in l else []) + f(l.get('layers', []))]; print([w for sc in json.load(open(sys.argv[1]))['scenes'] for w in f(sc.get('layers', []))])" s.json`
- **Hero words read at once:** ≥ 3:1 contrast against the set, and nothing passes in front of them for ≥ 0.5 s.
- **The end logo owns the frame:** about 40% or more of the frame width.
- **Every frame passes BAR** (`docs/video/quality/BAR.md`, all nine fail rules), judged by looking at full-size frames.
- **Loop:** `anikit-video check s.json` (errors and the real length) → `still --at 0.3` (is frame 1 intriguing with
  no context?) → `render --quality draft --scale 0.5` → for fast intents, `python3 docs/video/quality/pace.py draft.mp4`
  → `critique s.json --render --pack rv/` (open `rv/review.md` and look at every image) → final `render`.
- Then ask: can you name the idea and the motif from the strip alone? Would this film fit a different product? (If
  yes, redesign it.) Would the brand's team ship it?

## Gotchas

- A `path` layer sits at its bounding box: set `x`/`y` to the path's top-left.
- In `scene3d`, an expression `scale` must be a vector, `"=[s, s, s]"`; `0` hides an instance.
- `card` is a reflector, invisible to the camera; `phone` is life-size (16 cm).
- Keep a camera above any `heightfield` (inside it the frame turns flat white). A dark set under `night` needs a
  low key light (`intensity` 10–20).
- `pace.py` can miss a cut between two shots of similar brightness.
- 3D and particles cost 0.3–1.5 s a frame: iterate with `"quality": "draft"`, `"resolution": 0.5`.
- `render --crf N` sets the file size (higher N = smaller file). `"text": "=expr"` evaluates expressions.
