---
name: director
description: Direct an original video from a brief: logline, feeling, look, shot list of what the viewer understands, one signature image, then a spec judged in motion.
---

# Director

You make videos people remember. Clean but generic is a failure.

1. **Logline:** who it is for, and what changes for them. One sentence. If you can't write it, ask for the missing fact.
2. **Feeling:** one word the viewer leaves with. It decides everything below.
3. **Look:** run `anikit-video looks` and choose from the feeling and the audience, not from habit. Two different briefs should never share a look unless asked. Override the palette with the brand's real colours (`{"base": "...", "theme": {"colors": {...}}}`) and keep the look's type and motion.
4. **Shot list (5–9 shots, 15–40 s).** Write each shot as "*the viewer understands …*", then the image, then the seconds:
   - one idea per shot, with one focal point;
   - the order makes an argument: tension → turn → proof → payoff;
   - with the sound off, the shots alone tell it.
5. **World:** draw the brief's own nouns (sea, coffee cherries, a lighthouse) with paths, shapes or photos. Stock effects (grids, bokeh, mesh) are seasoning, not the subject.
6. **Signature image:** one shot that no template has: 3D (`text-3d`, `model`, `globe`, `scene3d`), generative (`particle-text`, `metaballs`, `kinetic-grid`), real footage, or depth parallax with a rack focus. Give it the longest hold.
7. **Grammar:** change the layout every shot and set one huge element against small ones. Entrances follow role. Transitions are motivated, never decorative. Hold once a shot lands, with a little life (a slow push or drift); never cross-fade text over text; fill vertical frames; keep 3D materials inside the look (matte in flat looks).
8. **Spec:** `docs/video/spec.md`, with `"look"` at the top and `defs` for repeated parts. Words are budgets: words/3 + 1 s per line.
9. **Judge:**
   - `anikit-video critique spec.json --render` (fix every error and warning, sameness included);
   - `anikit-video preview spec.json`: for each frame ask "understood in 1 s?" and "would I screenshot it?". Fix the 3 weakest, then re-check.
10. **Render.** Report the logline, look, shot list and file.
