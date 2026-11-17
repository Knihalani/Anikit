# Launch video (the full skill: skills/anikit-launch-video/SKILL.md)

**No template.** Design each film's structure, tempo, length and look from the product's own story. If the plan
would fit any product, it is wrong.

Principles: earn attention in the first second (frame 1 moves and raises a question) · the product, or its effect,
is the hero (real UI and real copy) · one idea or motif carried through · show, don't claim · the ending resolves
the idea and the logo owns the frame · sound and picture move together (calm is fine) · honesty: no fake stats.

Process: read the brief and ask nothing you can infer → find the idea (nouns and verbs, before → after, one motif;
write `concept.txt`; never reuse the last one) → invent the film's shape (e.g. one impossible camera move, a reverse
reveal, one object transforming, a wordless demo, a typographic manifesto, a loop, a time-lapse or a scale journey:
inspiration, not options) → choose tempo from the idea → build → check.

Two deliberately different fragments (not starting points):

```json
{ "size": "16:9", "intent": { "pace": "calm", "genre": "launch" }, "music": { "style": "ambient", "bpm": 72, "key": "D" },
  "scenes": [ { "duration": 6, "layers": [ { "component": "scene3d", "props": { "env": "cyc", "quality": "draft", "resolution": 0.5,
    "camera": { "keys": [ { "t": 0, "pos": [-2.5, 0.8, 4], "target": [0, 0.5, 0], "focal": 50, "fstop": 2 },
                          { "t": 6, "pos": [2.5, 0.6, 3.6], "target": [0, 0.5, 0], "focal": 55, "fstop": 2 } ] },
    "objects": [ { "type": "lathe", "points": [[0, 0], [0.3, 0.05], [0.36, 0.5], [0.2, 0.9], [0, 0.92]], "material": "ceramic" },
                 { "text": "Kiln", "height": 0.3, "pos": [0, 0, 1.1], "material": { "base": "#1A1714", "roughness": 0.4 } } ] } } ] } ] }
```

```json
{ "size": "9:16", "intent": { "pace": "frantic", "genre": "launch" }, "music": { "style": "dnb", "bpm": 172, "hits": ["@bar3"] },
  "scenes": [ { "duration": 1.4, "camera": { "shake": { "amount": 8, "at": 0, "duration": 0.15 } }, "layers": [
    { "component": "scene3d", "props": { "env": "night", "quality": "draft", "resolution": 0.5, "camera": "static",
      "objects": [ { "type": "particles", "count": 300000, "emit": { "text": "SHIP", "height": 0.8, "pos": [0, 0.6, 0] },
        "flow": 0.6, "life": 2, "size": 0.004, "color": ["#C8FF00", "#00E5FF"], "colorBy": "age", "emissive": 3, "blend": "add" } ] } } ] } ] }
```

Quality checks: no flat type (every word is 3D `text`, a texture, particles, or masked by the motif; 2D text only
for captions, legal lines and the URL) · hero words ≥ 3:1 against the set and unobstructed ≥ 0.5 s · logo ≥ 40%
of the frame width at the end · BAR.md's nine fail rules · `check` → `still --at 0.3` → draft render → `pace.py`
for fast intents → `critique --render --pack rv/` (look at every image) → final `render` (`--crf N` sets size).
Worked examples (two possible films, not the format): docs/video/skills/launch-examples/.
