# Particles (inside `scene3d`)

```json
{ "scenes": [ { "duration": 4, "layers": [ { "component": "scene3d", "props": {
  "env": "night", "quality": "draft", "camera": "static",
  "objects": [ { "type": "particles", "count": 300000,
    "emit": { "text": "IGNITE", "height": 1.0, "pos": [0, 0.6, 0] },
    "flow": { "strength": 0.8, "scale": 0.8, "evolve": 0.2 }, "gravity": 0.15, "drag": 0.4, "life": 4,
    "size": 0.004, "color": ["#ffb35c", "#ff5a1f", "#7a2bff"], "colorBy": "age", "emissive": 3, "blend": "add" } ] } } ] } ] }
```

- Up to 20 M particles. `emit`: a shape (`point sphere ball box disc`), any object (text, torus…), an image or a composition (pixels → coloured points).
- Forces: `flow` (curl noise), `attractors` (`pos`, `strength`, `swirl`), `gravity`, `drag`; `target` assembles particles into a shape between `from`/`to`.
- Look: `blend: add` (glowing light) or `over` (dust, sand, smoke: lit, self-shadowed volumes); `colorBy: age|speed|height|source`; `stretch` streaks.
- Deterministic and cached: renders split and resume identically. Reference: docs/video/simulation.md#particles.
