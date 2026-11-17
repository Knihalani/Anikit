# Fluid: ink in water, smoke, fire (`fluid` component)

```json
{ "scenes": [ { "duration": 6, "layers": [ { "component": "fluid", "props": {
  "look": "ink", "background": "#ece6dc",
  "text": { "text": "INK", "size": 0.34, "pos": [0.5, 0.48], "color": "#0c1630", "weight": 900 },
  "sources": [ { "pos": [0.08, 0.78], "to": [0.5, 0.62], "angle": -20, "speed": 0.9, "radius": 0.035, "color": "#8a1238", "amount": 2.2, "start": 0.6, "end": 2.6 } ],
  "vorticity": 0.5, "stir": 0.05 } } ] } ] }
```

A real 2D fluid simulation (stable fluids, a pressure projection, vorticity
confinement), deterministic and cached. It's full frame or any box. The text
holds for about a second, then the flow takes it apart.

- `look`: `ink` (absorbing dye on a light ground; wisps pale, the core deep), `smoke` (self-shadowed, lit by `light` degrees, on dark), `fire` (heat glows like a blackbody, with soot).
- `text`: `{ text, font, weight, pos, size, color, amount, heat, at }`. Set `heat` > 0 for smoke type that rises.
- `sources`: jets, `{ pos, to, angle (deg, -90 = up), speed (box heights/s), radius, color, amount, heat, start, end }`.
- Motion: `vorticity` (swirl kept alive: 0.3–0.6 ink, 0.7–1 smoke), `stir` (curl-noise stirring, 0.03–0.1) at `stirScale`, `buoyancy` (heat rises), `weight` (ink sinks), `fade` and `cool` (fraction kept per second), `speed` (time scale).
- Detail: `resolution` (velocity cells along the long side, 192) × `detail` (dye grid multiple, 4). About 12 ms per simulated step at the defaults; a 1080p frame renders in tens of ms.
- Craft notes: docs/video/craft/simulation.md. One readable force, voids around a dense core, and an event (lay down, release, settle).
