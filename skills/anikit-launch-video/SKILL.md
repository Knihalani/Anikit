---
name: anikit-launch-video
description: Make a launch, announcement, teaser or product video with anikit-video that stands next to studio work. A gated process (promise → ideas → one world → style frames → beat sheet → pass 1 → outside critique → pass 2) built from blind tests against professional launch films. It is a method, not a template; every film gets its own idea, world and structure. Use for any launch, product, feature, announcement or teaser video.
---

# Launch video

Read `skills/anikit-video/SKILL.md` first for the spec, the docs, previews and the render. This skill adds the launch method. Each step writes a file in your work folder (`launch/`), and the next step reads it. **Don't skip a step, and don't start the next one until its file exists.** The steps exist because each one fixes a failure we measured in blind tests against professional films (`docs/video/craft/launch-evidence.md`, `launch-research.md`).

## Why films fail (what the blind jury said about ours)

- **Literal idea:** the product's name or mechanism drawn as a picture, or one beat per feature with a caption under each ("effect demos").
- **No world:** each shot has a different look, so the film reads as "three templates stitched together". Or the opposite: the same layout in every frame with the words changed.
- **Slide type:** a centred headline, or a tiny tracked-caps label over a gradient, sitting on the image instead of living in it.
- **Flat light:** glows and gradients instead of a key light with falloff on real material.

Professional launch films do the reverse. They hold **one material world, one light logic and one palette family** for the whole film. They change **the subject, scale and composition** in every shot. And they show **one promise** through a device built from the subject.

## The steps

1. **`launch/1-promise.md`: the one promise** (see `idea.md`).
   - Write the single benefit the audience should believe afterwards, as ≤ 15 words without the product's name.
   - Choose the mode: **proof** (the real product or UI visible by 5 s) or **teaser** (withheld until the end).
   - List the brief's other facts under "not in this film". A film carries one promise; the rest goes on the end card at most.
1b. **`launch/refs/`: research and references** (see `research.md`).
   - **Subject:** research the subject's world: users, rituals, materials and insider details (`refs/world.md`).
   - **References:** collect 6–10 professional films in this category and neighbouring ones, plus 2–3 from outside motion. For each, write the moves worth stealing (`refs/refs.md`).
   - **Assets:** gather real assets (HDRIs, materials, models, fonts, the brand's own material) and, if you have images, run `board`.
   - **Gate:** every idea in step 2 must cite one insider detail and one reference move.
2. **`launch/2-ideas.md`: four ideas, then pick one.**
   - Write ≥ 4 ideas, each a different metaphor or device, one deliberately out there.
   - Run the **literal test** and the **swap test** on each (`idea.md`), then pick one and say why.
   - The winner states the device in one sentence: what visibly changes in the world because the product exists.
3. **`launch/3-world.md`: one world** (see `world.md`). Fix these once for the whole film:
   - the material and surface;
   - the key light (direction, quality, colour);
   - the palette family (an arc is fine, a swap is not);
   - the type voice (a face chosen for this world, and how type lives *in* it);
   - the sound world (instruments and foley taken from the subject);
   - the Anikit means (scene3d, shader, illo, graphic print, footage-like grade…).

   Then write what will **change** shot to shot: subject, scale, camera.
3b. **`launch/tests/`: play** (see `research.md`).
   - Before locking the world, make 4–8 quick technique tests (2–4 s clips or stills) that could carry the idea.
   - Use at least three different engine capabilities (3D, shaders, simulation, type as image, print, illustration, motion, sound-driven picture), including one you haven't used.
   - Compare them on one sheet and write in `tests/notes.md` what surprised you, what looked stock, and what can't be made convincing.
   - Build the world around the strongest test.
4. **`launch/frames/`: three style frames, compared.**
   - Make three `still`s of the same key moment, varying one decision (material, light, or type treatment), each a finished poster.
   - Put them side by side, pick one, and write in `3-world.md` why.
   - If none is a poster you'd hang, go back to step 3.
5. **`launch/4-beats.md`: the beat sheet on the music** (see `shape.md`).
   - Generate or choose the music first.
   - Write 5–9 shots against bars: what we see, what changes, where the device escalates, the one hard stop before the payoff, and the end.
   - Frame 0 is the hook.
6. **Pass 1.** Build the full film, preview it, then render it and its contact sheet: `render --sheet launch/pass1.png` (6+ frames, evenly spaced).
7. **`launch/5-crit.md`: compare against professional films** (see `review.md`).
   - Run the blind comparison against real professional launch films: `judge.py run … --practice`, about 3 min.
   - Paste every gap, and sort each one into IDEA, WORLD or CRAFT.
   - **Any IDEA gap means a new idea from a different family:** go back to step 2. Don't polish.
   - Cut the weakest third.
8. **Pass 2.** Rebuild, render (`ANIKIT_RENDER_CACHE=0`, foreground), and run crit 2 the same way.
   - Ship the higher-scoring pass; a difference under 0.75 is a tie. At most 3 passes.
   - Copy the shipped pass to `final.mp4`.

## Ground rules

- **Honesty:** use only the brief's real facts, names and UI. No invented numbers, customers, testimonials or interface.
- **Vary across films, not within one:** if this film looks like your last launch film (palette, type, layout family), change the world.
- **Looks already rejected as generic or AI-made:**
  - warm dark ground with a glowing serif;
  - cream paper with an editorial serif and aphorisms;
  - dark ground with one accent and a mono label;
  - tiles or dominoes on a cyc with chrome type;
  - fogged terrain;
  - landing-page furniture (pills, numbered eyebrows, LIVE dots).

  If your world resembles one of these, it needs a reason from the subject.
- **The ending performs:** the name lands through an action of the film's world in the last 5–10% of the film. A specific call to action holds 1.5–3 s. Never "Learn more".
- **Cost:** keep the final render under ~5 min (check `preview`'s estimate). Full-frame shaders and ray-traced 3D are the expensive parts; spend them on the shots that carry the device.
