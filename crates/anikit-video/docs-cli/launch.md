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

Taste first (`docs taste`): 3 references outside tech, 3 directions, no screenshot-beside-copy or "UI on dark with one accent", frames dense with made imagery. Quality checks (BAR.md): the medium comes from the concept; 2D typography is first-class, 3D only
where the idea is physical · never repeat a previous film's medium, world, motif, materials or music · every word
at full resolution (no scene3d `resolution` < 1 or `draft` in finals; no DOF, blur, bloom or grain on words being
read), ≥ 4.5:1 contrast, held ≥ 0.3 s + 0.06 s per character · motivated key, fill and rim, with exposure checked
at 100% · acoustic or organic music first, synths only for electronic concepts · the logo owns the end ·
`check` → stills at 100% → draft → `pace.py` (fast intents) → `critique --render --pack rv/` → final render.
Examples (rejected by the user, kept as cautionary examples): docs/video/skills/launch-examples/.
