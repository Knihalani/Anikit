# Reuse: write a pattern once (instead of a generator script)

```json
{ "defs": {
    "plate": { "shape": "rect", "width": 220, "height": 300, "radius": 6, "fill": "$tone", "y": 540 },
    "beat":  { "duration": 1.2, "background": "#101418", "layers": [ { "text": "$word", "size": "18vh", "x": "8%", "anchor": "left", "color": "#F2EDE4" } ] } },
  "scenes": [
    { "duration": 3, "background": "#101418", "layers": [ { "use": "plate", "repeat": 6, "with": { "tone": "#C9B79C" },
        "x": "=180 + $i * 260", "enter": { "preset": "fadeUp", "delay": "=$i * 0.08" } } ] },
    { "use": "beat", "with": { "word": "Harvested" } },
    { "use": "beat", "with": { "word": "Dried" }, "background": "#1B2026" } ] }
```

- `defs` holds layers or scenes; `{ "use": "name" }` places one, and any other keys beside `use` override it (objects merge key by key).
- `"with": { "tone": … }` fills `"$tone"` inside the def; a string that is exactly `"$tone"` keeps the value's type.
- `"repeat": N` on a layer, scene or `use` makes N copies with `$i` (0-based) and `$n`, bound in that item's own keys and `with` (not inside the def body: pass values in with `with`). `"=…"` strings are computed after substitution.
- A `{ "scenes": [ … ], "repeat": 3 }` item in `scenes` loops a sequence of scenes.
- For real logic (conditions, data, randomness) use a `script` layer: `anikit-video docs script`.
- To change one value later: `anikit-video set s.json scenes.1.layers.0.text "New"`.
