# Particles (inside `scene3d`)

```json
{ "scenes": [ { "duration": 4, "layers": [ { "component": "scene3d", "props": {
  "env": "night", "camera": "static",
  "objects": [ { "type": "particles", "count": 300000,
    "emit": { "text": "IGNITE", "height": 1.0, "pos": [0, 0.6, 0] },
    "flow": { "strength": 0.8, "scale": 0.8, "evolve": 0.2 }, "gravity": 0.15, "drag": 0.4, "life": 4,
    "size": 0.004, "color": ["#ffb35c", "#ff5a1f", "#7a2bff"], "colorBy": "age", "emissive": 3, "blend": "add" } ] } } ] } ] }
```

- Up to 20 M particles. `emit`: a shape (`point sphere ball box disc`), any object (text, torus…), an image or a composition (pixels → coloured points).
- Forces: `flow` (curl noise), `attractors` (`pos`, `strength`, `swirl`), `gravity`, `drag`; `target` assembles particles into a shape between `from`/`to`.
- Look: `blend: add` (glowing light) or `over` (dust, sand, smoke: lit, self-shadowed volumes); `colorBy: age|speed|height|source`; `stretch` streaks; `"trail": 0.6` (or `{ "length", "taper", "opacity" }`) draws light trails along each particle's real past path, best with 1–10 k `add` particles (examples/video/sim/trails.json).
- Depth and scale, the studio way: `"layers": [{ "share": 0.0003, "emit": {...near}, "size_dist": {...big} }, { "share": 0.25 }, { "share": 0.75, "emit": {...far}, "opacity": 0.35 }]` splits one emitter into foreground bokeh, a crisp mid layer and far haze. `"size_dist": { "min", "max", "power": 2.5 }` gives power-law sizes. Lit particles take `"scatter": 0.6` (backlit forward glow) and `"glint": 1.5` (facet sparkle). See examples/video/sim/snow.json.
- Shapes: `"sprite": "confetti"` (or `petal`, `paper`, `square`, `{ "shape", "aspect", "tumble", "translucency" }`) draws tumbling, lit flat pieces instead of dots. See examples/video/sim/confetti.json, which uses an `emit.vel` expression for a conical burst.
- Liquids: `"liquid": { "color": "#5a2e14", "opacity": 1 }` (chocolate, paint), `{ "color": "#ffcf6a", "metal": 1 }` (liquid gold) or `{ "color": "#4fb0ff" }` (clear, tinted; no refraction yet) draws the particles as one smooth surface. See examples/video/sim/liquid.json.
- **Make anything: expressions** of `x y z vx vy vz t age i` (+ `sin noise rand mix clamp`…):
  - `"force": "=[-z*2, 0.4, x*2]"` is a custom acceleration field (a vortex here);
  - `"emit": { "pos": "=[cos(rand(i)*6.28), 0, sin(rand(i)*6.28)]", "vel": "=[0, 1, 0]" }` is a custom spawn rule;
  - `"size": "=0.002 + 0.002*rand(i)"`, `"color": "=[age*2, 0.5, 1 - age]"` (linear RGB) and `"opacity": "=1 - age"` set per-particle looks.
- Collisions: the floor and every object with `"collide": true` (spheres exactly, other shapes as boxes; animated objects sweep particles). `bounce`, `friction`.
- `over` particles cast soft shadows on the floor and objects (`shadows: false` turns it off). Defocus gives real disc bokeh; motion gives streaks.
- `quality: draft` splats about 2× faster (softer in-focus particles). Cost at 1080p: 1M is about 0.1 s/frame, 10M about 1.3 s/frame (sim + splat).
- Deterministic and cached on disk (`ANIKIT_CACHE`): renders split and resume identically, and stills are instant after the first. Reference: docs/video/simulation.md#particles.
