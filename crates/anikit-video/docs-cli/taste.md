# Taste: what makes a piece land

Every frame can be striking as a still, and every second worth hearing, and "incredible" comes in many forms: spectacle, wit, beauty, clarity, rhythm, surprise, craft. Nothing here is a rule or a house style. It is what tends to work, what tends to fail and why, and how to check. Depth and sources: `docs/video/taste/TASTE.md` (§1 type · §2 colour · §3 composition · §4 light · §5 motion · §6 editing · §7 sound · §8 concept · §9 tells · §11 ambition).

**Routes to a striking frame, and the tools that make each one cheap:**
- **Illustration and print:** components `brush`, `sketch`, `hatch`, `paper`, `cutout` (`docs component brush`, …); one-line style filters `{"shader": "halftone|risograph|linocut|watercolour|crosshatch|screenprint|print-offset|dither|film", "filter": true}`.
- **Graphic design and data:** paths and `repeat`/`script` for dense fields of real data, type set as the image.
- **Light and material:** `scene3d` with a motivated key; non-photoreal `"look": "toon|ink|hatch|clay|paper|cardboard|gouache"` (or `photo`) with `outline`; environments beyond the studio: `overcast`, `interior`, `city`, `underwater` (`docs 3d`).
- **Spectacle and texture:** your own GLSL worlds, particles and physics as imagery, grain and print texture.
- **References:** `anikit-video board refs/` turns 3–6 real-world images from outside tech (paintings, posters, prints, film stills, architecture) into palette, value, texture and line tokens (`docs board`).

**What tends to land:**
- **One idea only this brief could have.** Swap in a competitor's name: if it still works, it's probably generic. Ideas often come from references outside the category; writing 3 directions that differ on every axis and keeping the most specific helps.
- **Density with intent.** A paused frame reads as a designed still: a subject, a world, texture, a reason for each layer. Restraint can be rich, but empty frames tend to read as unfinished.
- **Legible words.** Brightness contrast of about 4.5:1 or more (colour-only contrast smears in MP4), ~36 px or larger at 1080p, kept sharp while read, held for max(0.83 s, chars/15) after they settle.
- **Hierarchy and placement.** A few sizes with clear steps; off-centre with room to move, or symmetry that commits; light you could point to.
- **Motion with weight and choreography.** A fast attack and long settle, one thing leading, transitions that carry an idea (a match, a morph, a continuous move).
- **Rhythm with contrast.** Varied shot lengths, a breath after the densest run, cuts on action or comprehension, and a surprise somewhere.
- **Sound that belongs.** Instruments chosen for this world (synths when the idea is electronic), a planned silence, hits on the frame, sfx from the materials, −14 to −16 LUFS.

**What tends to fail, and why:**
- Trend looks people recognise instantly: indigo gradients and glass, cream paper + editorial serif + terracotta, a UI screenshot beside copy, "UI on dark with one accent", chrome on a studio floor. They read as defaults, not choices.
- Constant tempo, one easing everywhere, fade-and-rise on every element, and wall-to-wall stock beds. Without contrast, the attention fades.

**Check before shipping:** pause on 3 random frames at 100% and look at the `critique --pack` strip; listen once muted and once audio-only. Would a senior motion designer stop scrolling for it? If you can name the trend it resembles, consider another direction. `critique` measures the checkable parts and gives advice, not verdicts.
