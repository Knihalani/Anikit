---
name: director
description: Direct an original video from a brief: logline, feeling, look, shot list of what the viewer understands, one signature image, then a spec judged in motion.
---

# Director

You make videos people remember. Clean but generic is a failure.

1. **Logline:** who it is for, and what changes for them. One sentence. If you can't write it, ask for the missing fact.
2. **Feeling:** one word the viewer leaves with. It decides everything below.
3. **Look:** run `anikit-video looks` and choose from the feeling and the audience, not from habit. Two different briefs should never share a look unless asked. Override the palette with the brand's real colours (`{"base": "...", "theme": {"colors": {...}}}`) and keep the look's type and motion.
4. **Shot list, cut to a 120–140 BPM track: 12–25 shots in a 15 s test.** No shot over 2.5 s; median 1.5 s or less. Write each shot as "*the viewer understands …*", then the image, then the seconds:
   - one idea per shot, with one focal point;
   - the order makes an argument: tension → turn → proof → payoff;
   - with the sound off, the shots alone tell it.
5. **World:** draw the brief's own nouns (sea, coffee cherries, a lighthouse) with paths, shapes or photos. Stock effects (grids, bokeh, mesh) are seasoning, not the subject.
6. **Signature image:** one shot that no template has: 3D (`text-3d`, `model`, `globe`, `scene3d`), generative (`particle-text`, `metaballs`, `kinetic-grid`), real footage, or depth parallax with a rack focus. It must be something a browser tool can't render (`docs/video/quality/ONLY_ANIKIT.md`).
7. **Grammar:** change the layout every shot and set one huge element against small ones. Entrances follow role. Transitions are motivated, never decorative. Energetic camera (whips, snaps, speed ramps, impacts, match cuts); never cross-fade text over text; fill vertical frames; keep 3D materials inside the look (matte in flat looks).
8. **Spec:** `docs/video/spec.md`, with `"look"` at the top and `defs` for repeated parts. Words are budgets: words/3 + 1 s per line.
9. **Judge:**
   - `anikit-video critique spec.json --render` (fix every error and warning, sameness included), and after rendering `python3 docs/video/quality/pace.py out.mp4` (must pass);
   - `anikit-video preview spec.json`: for each frame ask "understood in 1 s?" and "would I screenshot it?". Fix the 3 weakest, then re-check.
10. **Render.** Report the logline, look, shot list and file.
