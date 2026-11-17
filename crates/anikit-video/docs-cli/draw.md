# Draw: raw paths and points, each with its own style

```json
{ "scenes": [ { "duration": 3, "layers": [ { "enter": "draw", "draw": [
  { "d": "M200 800 C600 200 1300 1200 1720 300", "stroke": "$accent", "width": 6, "cap": "round" },
  { "points": [[300, 900], [700, 760], [1100, 880], [1500, 700]], "colors": ["#22d3ee", "#a78bfa", "#f472b6", "#facc15"], "width": 8 },
  { "points": [[860, 400], [1060, 400], [960, 560]], "closed": true, "smooth": true, "fill": "#ffffff22", "stroke": "#fff" },
  { "circle": [960, 540, 40], "fill": "$primary" },
  { "rect": [100, 100, 300, 160], "radius": 24, "fill": "$surface" },
  { "line": [0, 1000, 1920, 1000], "stroke": "$muted", "width": 1 } ] } ] } ] }
```

- Coordinates are frame pixels: the layer covers its parent. (A `shape: "path"` layer with `d` is different: it is placed by its bounding box like any shape, so `x`/`y` move the path's centre. For raw coordinates, use `draw`.) Each item takes one of `d`, `points` (`closed`, `smooth`), `circle [x, y, r]`, `rect [x, y, w, h]` (`radius`) or `line [x1, y1, x2, y2]`.
- Style per item: `fill`, `stroke`, `width`, `cap` (`butt round square`), `join`, `opacity`. `colors` gives one colour per point (gradient segments).
- The layer animates like any layer (transforms, presets, `trimEnd`; `"enter": "draw"` draws every stroke on). Generate large drawings with a `script`.

- Thousands of points (maps, scatter plots): a `dots` layer, `{ "dots": { "data": "cities.csv", "size": 3, "color": "$accent", "appear": "=clamp(t * 2000 - i, 0, 1)" } }`. `data` is `[[x, y], …]` (optionally `[x, y, "#colour"]`) or a CSV of `x,y[,colour]`; it draws one path per colour, so 6,300 dots render at about 220 fps.
