# Launch video (the full skill: skills/anikit-launch-video/SKILL.md)

Brief → 15/30/60 s launch film. Ask nothing you can infer: name + one-line value are required; features, colours,
fonts, aspect (16:9; 9:16 for Reels/TikTok; 1:1 for feed) and proof come from the brief, README or screenshots.

1. **Concept first** (skills/anikit-launch-video/concepts.md): write noun · verb · before→after, pick one of 12
   worlds (material, macro, typographic, data, physics, particles, shader, blueprint, light, collage, miniature,
   signal), one motif carried through every shot, and *the turn* on `@bar3`. Never reuse the last concept.
2. **Structure at 128 BPM** (beat 0.469 s, bar 1.875 s; 15 s = 8 bars): hook (2 beats, motion in frame 1, no logo)
   → tension (3 cuts × 2 beats, 1–3 words) → reveal on `@bar3` (hero full-frame, impact) → 3 features (2 shots ×
   2 beats each) → proof (a fact from the brief or a demonstration, never a fake stat) → logo + CTA (2 + 4 beats).
3. **Copy:** ≤ 20 words for 15 s, ≤ 4 per shot, product name on screen ≤ 2 times.
   **Type rule:** every word is 3D `text` in the set, a texture on an object, particles, or masked by the motif;
   top-level 2D text only for captions/legal/the end URL. Hero words ≥ 3:1 against the set, unobstructed ≥ 0.5 s;
   the end logo ≥ 40% of the frame width.
4. **Top end, where it serves the story:** UI as a composition texture on a `pane`/`box` in `scene3d`; `physics`
   piles/dominoes; `particles`; library shaders; `heightfield` worlds; sfx on hits.

```json
{ "size": "9:16", "intent": { "pace": "fast", "genre": "launch" },
  "music": { "style": "house", "bpm": 128, "key": "Fm", "hits": ["@bar3", "@bar7"], "end": "ring" },
  "audio": [ { "sfx": "riser", "at": 3.75 }, { "sfx": "impact", "at": 3.75 } ],
  "scenes": [ { "duration": 0.9375, "camera": { "shake": { "amount": 7, "at": 0, "duration": 0.15 } }, "layers": [
    { "component": "scene3d", "props": { "env": "studio", "quality": "draft", "resolution": 0.5,
      "camera": { "keys": [ { "t": 0, "pos": [0.4, 0.6, 5.2], "target": [0, 0.32, 0], "focal": 36, "fstop": 3 } ] },
      "objects": [
        { "text": "Too much", "height": 0.62, "depth": 0.22, "material": { "base": "#F2EFE8" },
          "keys": [ { "t": 0, "rot": [-30, -25, 0], "scale": 1.25, "ease": "outExpo" }, { "t": 0.22, "rot": [0, -8, 0], "scale": 1 } ] },
        { "type": "box", "size": [0.62, 0.4, 0.02], "count": 90, "material": { "base": "#F2EFE8" },
          "pos": "=[-3 + 6*rand(i), 3.2 - 3.4*fract(rand(i+5) + 0.2*t), -2.4 + 2.8*rand(i+9)]",
          "rot": "=[360*rand(i+1) + 40*t, 360*rand(i+2) + 30*t, 0]" } ] } } ] } ] }
```

Gotchas found while proving it: a path layer is placed by its bounding box (set `x`/`y` to the path's top-left);
`text` does not evaluate `=expr` (use a few timed layers for a counter); objects' `scale` expressions must be
`"=[s, s, s]"`; dark-to-dark cuts are invisible to `pace.py`, so alternate light and dark shots.

QA (must pass): `check` (exact length) → `still --at 0.3` (is the hook intriguing?) → `render --quality draft
--scale 0.5` → `python3 docs/video/quality/pace.py draft.mp4` → `critique --render --pack rv/` (look at every
image) → BAR.md's nine fail rules → final `render`. Worked examples: docs/video/skills/launch-examples/.
