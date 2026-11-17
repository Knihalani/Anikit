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
4. **Top end, where it serves the story:** UI as a composition texture on a `pane`/`box` in `scene3d`; `physics`
   piles/dominoes; `particles`; library shaders; `heightfield` worlds; sfx on hits.

```json
{ "size": "9:16", "intent": { "pace": "fast", "genre": "launch" },
  "music": { "style": "house", "bpm": 128, "key": "Fm", "hits": ["@bar3", "@bar7"], "end": "ring" },
  "audio": [ { "sfx": "riser", "at": 3.75 }, { "sfx": "impact", "at": 3.75 } ],
  "scenes": [ { "duration": 0.9375, "background": "#14161B", "layers": [
    { "text": "Too much", "font": "Inter Display", "size": "18vw", "weight": 900, "color": "#F2EFE8", "anchor": "center",
      "animate": [ { "from": { "scale": 1.9, "opacity": 0, "blur": 14 }, "duration": 0.12, "ease": "outExpo" } ] } ] } ] }
```

Gotchas found while proving it: a path layer is placed by its bounding box (set `x`/`y` to the path's top-left);
`text` does not evaluate `=expr` (use a few timed layers for a counter); objects' `scale` expressions must be
`"=[s, s, s]"`; dark-to-dark cuts are invisible to `pace.py`, so alternate light and dark shots.

QA (must pass): `check` (exact length) → `still --at 0.3` (is the hook intriguing?) → `render --quality draft
--scale 0.5` → `python3 docs/video/quality/pace.py draft.mp4` → `critique --render --pack rv/` (look at every
image) → BAR.md's nine fail rules → final `render`. Worked examples: docs/video/skills/launch-examples/.
