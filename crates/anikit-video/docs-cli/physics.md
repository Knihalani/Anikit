# Physics (inside `scene3d`)

```json
{ "scenes": [ { "duration": 5, "layers": [ { "component": "scene3d", "props": {
  "env": "cyc", "camera": { "keys": [ { "t": 0, "pos": [0, 3, 9], "target": [0, 0.5, 0], "focal": 35 } ] },
  "physics": { "timeScale": "=1 - 0.75*smoothstep(1.4, 1.6, t)",
    "bodies": [ { "type": "box", "count": 400, "layout": "pile", "size": [0.25, 0.25, 0.25], "material": "plastic" } ],
    "impulses": [ { "t": 1.5, "at": [0, 0.2, 0], "strength": 9, "radius": 3 } ] } } } ] } ] }
```

- `bodies`: any object type (or `instance` of a named object), `count` up to 200 k, `layout: dominoes|grid|wall|pile|rain|expr`, `density friction restitution velocity angular ccd`.
- `impulses` (radial pushes at times), `gravity`, `ground`, `start`, and `timeScale` as an expression of `t` = **speed ramps / slow motion**.
- Bodies can carry live composition textures (10,000 video panes falling as dominoes).
- **Joints:** `"chain": "ball"|"fixed"|"hinge"|"spring"|"rope"` links consecutive bodies (ropes of beads, bridges, banners); `"pin": "first"|"last"|"ends"|[i…]` fixes bodies to the world; `physics.joints: [{ "type": "hinge", "a": [group, i], "b": "world"|[group, i], "anchor": [x,y,z], "axis": [0,0,1], "length", "stiffness", "damping" }]`.
- **Any shape:** a body can be any object, e.g. `{ "type": "model", "src": "part.glb" }` (glTF, convex hull) or `{ "text": "A" }`.
- **Pushers and obstacles:** any object with `"collide": true` is a fixed collider, or a kinematic pusher if it's animated (`keys` or expressions of `t`).
- Deterministic (rapier, enhanced determinism). Cached on disk per spec (`ANIKIT_CACHE`), so a later still or render of the same physics reads the frames instead of simulating. Reference: docs/video/simulation.md.
- **Cloth:** `{ "type": "cloth", "hang": "horizontal"|"vertical", "size": [w, h], "res": 48, "center": [x, y, z], "pin": "top"|"corners"|"left"|"none", "colliders": [{ "pos": [x, y, z], "radius": r }], "wind": [x, y, z], "gust": 0.5, "bend": 0.15, "stiffness": 1, "damping": 0.01, "friction": 0.5, "thickness": 0.01, "floor": true|y, "iterations": 8 }` is a draping sheet, curtain or flag. It's two-sided, matte by default, takes any `material`, and lives in world space (place it with `center`, not `pos`). The demo is examples/video/sim/cloth.json: silk dropped over a sphere.
