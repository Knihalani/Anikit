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
- Helpers: `rand(seed)`, `rand(a, b)`, `noise(x)`, `lerp`, `clamp`, `remap`, `hsl(h, s, l)`, `fmt(x, digits)`. Maths: `sin cos sqrt abs floor PI() x.to_radians()`.
- A top-level `"script"` gets the whole spec as `spec`, and can change it or return a new one.
- Sandboxed and deterministic: no files, no network, no clock. For per-frame motion, generate expressions: `x: "=100 + 20 * sin(t + " + k + ")"`.
