# Taste: what makes a piece land

Every frame can be striking as a still, and every second worth hearing, and "incredible" comes in many forms: spectacle, wit, beauty, clarity, rhythm, surprise, craft. Nothing here is a rule or a house style. It is what tends to work, what tends to fail and why, and how to check. Depth and sources: `docs/video/taste/TASTE.md` (§1 type · §2 colour · §3 composition · §4 light · §5 motion · §6 editing · §7 sound · §8 concept · §9 tells · §11 ambition).

**Routes to a striking frame, and the tools that make each one cheap:**
- **Illustration and print:** components `brush`, `sketch`, `hatch`, `paper`, `cutout` (`docs components`); one-line style filters `{"shader": "halftone|risograph|linocut|watercolour|crosshatch|screenprint|print-offset|dither|film", "filter": true}`.
- **Graphic design and data:** paths and `repeat`/`script` for dense fields of real data, type set as the image.
- **Light and material:** `scene3d` with a motivated key; non-photoreal `"look": "toon|ink|hatch|clay|paper|cardboard|gouache"` (or `photo`) with `outline`; environments beyond the studio: `overcast`, `interior`, `city`, `underwater` (`docs 3d`).
- **Spectacle and texture:** your own GLSL worlds, particles and physics as imagery, grain and print texture.
- **References:** `anikit-video board refs/` turns 3–6 real-world images from outside tech (paintings, posters, prints, film stills, architecture) into palette, value, texture and line tokens (`docs board`).

**What tends to land:**
- **One idea only this brief could have.** Swap in a competitor's name: if it still works, it's probably generic. Good ideas tend to come late: 30+ one-liners, then turn the obvious one (reverse it, remove what's taken for granted, borrow a medium from another world), with references from outside the category (`docs/video/craft/art-direction.md`).
- **A shape chosen on purpose.** A question in the first second, one turn, a payoff held ~1.5–2.5 s (memory needs it), then a small button. There are 20 structures to pick from in `docs/video/craft/storytelling.md`, from chain reaction and reversal to a story told through an interface or a loop.
- **One material pushed to the limit.** Award winners put their craft into one thing (real smoke, gravel with weight, one tile rule) rather than five effects (`docs/video/craft/award-level.md`).
- **Image-led, with type in support.** In 61 of 79 world-class pieces type is absent or supporting; where it leads, it is one face set with restraint or made of the film's own material. The camera moves with intent in 50 of 79, and pace comes from change inside shots (the median shot is 2.5 s across the set: 1.0 s for sport, 1.5 s for idents, 3.5 s for titles and explainers).
- **Density with intent.** A paused frame reads as a designed still: a subject, a world, texture, a reason for each layer. Restraint can be rich, but empty frames tend to read as unfinished.
- **Life in the surface.** Texture locked to the object and following its form, boiled every 2–4 frames; parts that lag 1–3 frames behind what drives them; particles in 3+ depth layers lit by the key (`docs/video/craft/finish-and-life.md`). Uniform, screen-locked noise and rigid silhouettes read as procedural.
- **Legible words.** Brightness contrast of about 4.5:1 or more (colour-only contrast smears in MP4), ~36 px or larger at 1080p, kept sharp while read, held for max(0.83 s, chars/15) after they settle.
- **Hierarchy and placement.** A few sizes with clear steps; off-centre with room to move, or symmetry that commits; light you could point to.
- **Motion with weight and choreography.** A fast attack and long settle, one thing leading, transitions that carry an idea (a match, a morph, a continuous move).
- **Rhythm with contrast.** Varied shot lengths, a breath after the densest run, cuts on action or comprehension, and a surprise somewhere.
- **Sound that belongs.** Instruments chosen for this world (synths when the idea is electronic), a planned silence, hits on the frame, sfx from the materials, −14 to −16 LUFS.

**What tends to fail, and why:**
- Trend looks people recognise instantly: indigo gradients and glass, cream paper + editorial serif + terracotta, a UI screenshot beside copy, "UI on dark with one accent", chrome on a studio floor. They read as defaults, not choices.
- Constant tempo, one easing everywhere, fade-and-rise on every element, and wall-to-wall stock beds. Without contrast, the attention fades.

**What the blind judge keeps saying** (all 11 round-1 pieces failed; `docs/video/craft/judge-r1-review.md`): the idea was the brand name drawn or the feature diagrammed (11/11); one layout with the words swapped (11/11); glow instead of light (10/11); texture as a filter (10/11); stock presets (10/11); a tiny tracked-caps label over a generic headline (9/11).

**Check before shipping:** pause on 3 random frames at 100% and look at the `critique --pack` strip; listen once muted and once audio-only. Would a senior motion designer stop scrolling for it? If you can name the trend it resembles, consider another direction. `critique` measures the checkable parts and gives advice, not verdicts.
