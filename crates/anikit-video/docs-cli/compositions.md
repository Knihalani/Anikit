# Compositions: nested timelines

```json
{ "compositions": {
    "ticker": { "size": [800, 200], "duration": 2, "background": "#000",
      "layers": [ { "text": "north sea · salt · stone · tide · ", "size": 90, "color": "#C8FF00", "x": "=400 - (t*600 % 800)" } ] } },
  "scenes": [ { "duration": 4, "layers": [
    { "composition": "ticker", "loop": true, "width": 800, "height": 200, "rotation": -8 },
    { "composition": "ticker", "loop": true, "speed": 1.5, "trimStart": 0.5, "width": 800, "height": 200, "y": "75%", "rotation": 6 } ] } ] }
```

- A composition is a mini-video (`size`, `duration`, `background`, `layers` or `scenes`) with its own clock, clipped to its box.
- Use it as a layer (`composition`, `fit`, `trimStart`, `speed`, `loop`), as a shader channel (`"channels": ["#ticker"]`), or as a texture on 3D objects (`"texture": { "composition": "ticker", "frame": "=i" }`, thousands of instances).
- `defs` + `{ "use": "name", "with": {...} }` reuse a layer or scene; `"repeat": n` copies with `$i`.
