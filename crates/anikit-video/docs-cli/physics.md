# Physics (inside `scene3d`)

```json
{ "scenes": [ { "duration": 5, "layers": [ { "component": "scene3d", "props": {
  "env": "cyc", "quality": "draft", "camera": { "keys": [ { "t": 0, "pos": [0, 3, 9], "target": [0, 0.5, 0], "focal": 35 } ] },
  "physics": { "timeScale": "=1 - 0.75*smoothstep(1.4, 1.6, t)",
    "bodies": [ { "type": "box", "count": 400, "layout": "pile", "size": [0.25, 0.25, 0.25], "material": "plastic" } ],
    "impulses": [ { "t": 1.5, "at": [0, 0.2, 0], "strength": 9, "radius": 3 } ] } } } ] } ] }
```

- `bodies`: any object type (or `instance` of a named object), `count` up to 200 k, `layout: dominoes|grid|wall|pile|rain|expr`, `density friction restitution velocity angular ccd`.
- `impulses` (radial pushes at times), `gravity`, `ground`, `start`, and `timeScale` as an expression of `t` = **speed ramps / slow motion**.
- Bodies can carry live composition textures (10,000 video panes falling as dominoes).
- Deterministic, cached per spec. Reference: docs/video/simulation.md.
