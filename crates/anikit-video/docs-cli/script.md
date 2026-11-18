# Script: loops and logic that generate layers or scenes

```json
{ "scenes": [ { "duration": 4, "layers": [ { "script": [
  "let out = [];",
  "for k in 0..40 {",
  "  let a = k * 9.0;",
  "  out.push(#{ shape: \"rect\", width: 14, height: 120 + 200 * rand(k), radius: 7, fill: hsl(a, 75, 60),",
  "              x: 120 + k * 42, y: h / 2, enter: #{ preset: \"popIn\", delay: k * 0.02 } });",
  "}",
  "out" ] } ] } ] }
```

- `{ "script": "…" }` (a string or a list of lines) in any `scenes`, `layers` or `children` list is replaced by the list it returns. Objects are `#{ … }`, lists `[ … ]`.
- It runs once at load. Variables: `w`, `h`, `i`/`n` (inside `repeat`), and `with` values.
- Anywhere: a `{ "script": … }` item in ANY array (scene3d `objects`, `draw` items, emitters, keys, notes) splices the list it returns, or is replaced by the object.
- Helpers: `rand(seed)` `rand(a, b)` `noise(x[, y])` `noise2` `noise3` `fbm(x, y[, octaves])` `lerp` `clamp` `remap` `smoothstep(a, b, x)` `ease("outBack", x)` (`linear in out inOut inOutSine inExpo outExpo outBack outElastic outBounce`) `hsl(h, s, l[, a])` `rgb(r, g, b[, a])` `oklch(L, C, h[, a])` (colours as `"#rrggbb"`, same ranges as expressions) `palette("viridis", x)` (`viridis magma sunset ocean neon mono rainbow`) `fmt(x, digits)`.
- Geometry: `path_circle(cx, cy, r)`, `path_poly(cx, cy, r, sides)`, `path_star(cx, cy, r, inner, points)` → SVG `d`; vectors `[x, y]`: `vadd vsub vscale vlen vdist vnorm vrot(v, deg) vlerp`. Maths: `sin cos sqrt abs floor PI() x.to_radians()`.
- A top-level `"script"` gets the whole spec as `spec`, and can change it or return a new one.
- Sandboxed and deterministic: no files, no network, no clock. For per-frame motion, generate expressions: `x: "=100 + 20 * sin(t + " + k + ")"`.

## Per frame: `update`

```json
{ "scenes": [ { "duration": 4, "layers": [ { "shape": "circle", "width": 80,
  "update": "let r = 300 + 80 * noise(t * 2.0); #{ x: 960 + r * cos(t), y: 540 + r * sin(t), fill: palette(\"neon\", fract(t / 4.0)) }" } ] } ] }
```

- `update` runs every frame and returns props to set: `x y width height scale rotation opacity blur radius fill stroke …`. It sees `t T gt p i n w h fps frame beat bar level bpm`.
- It's compiled once and costs about 4 µs per layer per frame. Unknown props and errors are reported at `check`, with "did you mean".
