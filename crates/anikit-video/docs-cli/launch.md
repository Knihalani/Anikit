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
{ "size": "16:9", "intent": { "pace": "calm", "genre": "launch" }, "music": { "style": "piano", "bpm": 72, "key": "D" },
  "scenes": [ { "duration": 6, "layers": [ { "component": "scene3d", "props": { "env": "cyc", "resolution": 1,
    "camera": { "keys": [ { "t": 0, "pos": [-2.5, 0.8, 4], "target": [0, 0.5, 0], "focal": 50, "fstop": 2 },
                          { "t": 6, "pos": [2.5, 0.6, 3.6], "target": [0, 0.5, 0], "focal": 55, "fstop": 2 } ] },
    "objects": [ { "type": "lathe", "points": [[0, 0], [0.3, 0.05], [0.36, 0.5], [0.2, 0.9], [0, 0.92]], "material": "ceramic" },
                 { "text": "Kiln", "height": 0.3, "pos": [0, 0, 1.1], "material": { "base": "#1A1714", "roughness": 0.4 } } ] } } ] } ] }
```

```json
{ "size": "9:16", "intent": { "pace": "fast", "genre": "launch" }, "music": { "style": "acoustic", "bpm": 112, "hits": ["@bar3"] },
  "background": "#F3EEE4", "scenes": [ { "duration": 2.2, "layers": [
    { "text": "Write it *once.*", "font": "Instrument Serif", "size": "17vw", "color": "#1B1A17", "x": "8%", "y": "38%",
      "anchor": "left", "lineHeight": 0.95, "letterSpacing": "-0.02em", "enter": "mask by line up" },
    { "shape": "rect", "width": "84%", "height": 3, "fill": "#C2410C", "x": "8%", "y": "52%", "anchor": "left",
      "animate": [ { "from": { "scaleX": 0 }, "duration": 0.6, "ease": "inOutExpo" } ] } ] } ] }
```

What works and why (advice, not bans; depth in docs/video/taste/TASTE.md): choose the medium the idea lives in
(3D for physical ideas, type for word ideas) · go somewhere previous films didn't · if you can name the trend it
resembles, rethink it · rich, layered frames over empty restraint · copy from the brief's real nouns, not aphorisms
· crisp type (full resolution, in focus, ~4.5:1, held ~0.3 s + 0.06 s/char) · motivated light checked at 100% ·
music chosen from the brand (acoustic for most, synths for electronic ideas) · the logo owns the end.
Checks: `check` → stills at 100% → draft → `pace.py` (fast intents) → `critique --render --pack rv/` → TASTE §10 → final.
Examples (rejected by the user, kept as cautionary examples): docs/video/skills/launch-examples/.
