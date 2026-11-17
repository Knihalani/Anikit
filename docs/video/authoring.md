# Authoring anything: expressions, scripts, raw drawing

Templates, components and looks are shortcuts. These three tools let you build what none of them has. All three are deterministic: the same spec always renders the same frames.

## 1. Expressions: any number or colour, computed per frame

Start a value with `=`. It works for any numeric property (`x y width height scale rotation opacity blur radius strokeWidth trimEnd value depth …`) and for colours (`fill`, `stroke`, text `color`, box `background`).

```json
{ "text": "Pulse", "scale": "=1 + 0.2 * level", "rotation": "=beat % 1 < 0.1 ? 4 : 0",
  "color": "=mix($primary, $accent, smoothstep(0, 1, p))" }
```

| | |
|---|---|
| Variables | `t` (layer seconds) · `T` (scene seconds) · `gt` (video seconds) · `p` (layer progress 0..1) · `frame` · `i` `n` (index and count among siblings) · `w` `h` (frame size) · `fps` · `beat` `bar` (beats and bars elapsed, fractional; music required) · `level` (music loudness 0..1) · `bpm` · `pi e tau` |
| Operators | `+ - * / % ^` · `< <= > >= == !=` · `&& || !` · `c ? a : b` |
| Functions | `sin cos tan asin acos atan atan2 sinh cosh tanh` · `abs sign floor ceil round fract mod min max clamp sqrt pow exp log log2 log10 hypot` · `mix lerp remap(x, a, b, c, d) smoothstep step pingpong if(c, a, b)` · `ease easeIn easeOut spring` (of 0..1) · `tri saw square` (period 1) · `noise(x) noise(x, y) rand(x) rand2(x, y)` |
| Units | `50%` (of the parent) · `10vw` `8vh` `5vmin` · `500ms` · `30deg` |
| Colours | `=rgb(r, g, b[, a])` (0–255, alpha 0–1) · `=hsl(h, s, l[, a])` (degrees, 0–100) · `=oklch(L, C, h[, a])` · `=mix(colourA, colourB, x)` (in OKLab) · `=cond ? colourA : colourB`; every argument is an expression and colours may be tokens (`$accent`) |

Place a layer relative to an earlier sibling with its `id`: `"y": "=title.bottom + 24"`, `"x": "=card.right + 40"` (`left right top bottom cx cy width height`; the box at rest, before animation).

`beat`, `bar`, `level` and `bpm` read the first `audio` track with a `src` (the music, not a voice-over). Shake on every beat: `"x": "=960 + 12 * (1 - fract(beat)) * sin(t * 90)"`.

## 2. Scripts: loops and logic inside the spec

`{ "script": … }` in any `scenes`, `layers` or `children` list is replaced by the items it returns. The language is [Rhai](https://rhai.rs/book/): JavaScript-like, with no files, no network and no clock, and bounded work. The script can be a string or a list of lines.

```json
{ "script": [
  "let out = [];",
  "for k in 0..24 {",
  "  let a = k * 15.0;",
  "  out.push(#{ shape: \"circle\", width: 30, x: w / 2 + 360 * cos(a.to_radians()), y: h / 2 + 360 * sin(a.to_radians()),",
  "              fill: hsl(a, 80, 60), enter: #{ preset: \"popIn\", delay: k * 0.03 } });",
  "}",
  "out" ] }
```

- Objects are `#{ key: value }` and lists are `[a, b]`. Return a list of layers or scenes, or just one. Strings work too: a string layer is text, a string scene is a title card.
- Variables: `w`, `h`, plus any bound `$params` (`i`, `n` inside `repeat`, and `with` values).
- Helpers: `rand(seed)`, `rand(a, b)` (0..1, deterministic), `noise(x)`, `lerp`, `clamp`, `remap`, `hsl(h, s, l)` → `"#rrggbb"`, `fmt(x, digits)`. Rhai's maths: `sin cos sqrt abs floor PI() …`, and `x.to_radians()`.
- A top-level `"script"` runs first with the whole spec in `spec`. Change it (`spec.scenes.push(…)`) or return a new one.
- Generated items are ordinary spec, so `use`, `repeat`, presets and expressions all work inside them. `anikit-video check` reports script errors with the line.

- `{ "script": … }` works in any array of the spec (scene3d `objects`, `draw` items, emitters, keys, notes). A list splices, and an object replaces the item.
- More helpers: `noise2 noise3 fbm smoothstep ease(name, x) palette(name, x)`, plus `path_circle path_poly path_star` (SVG `d`) and vector helpers `vadd vsub vscale vlen vdist vnorm vrot vlerp`. Run `anikit-video docs script` for the list.

Scripts run once, when the spec is loaded. For values that change every frame, use an expression (`"x": "=…"`), or a layer's `"update"` script: it runs every frame with `t T gt p i n w h fps frame beat bar level bpm` and returns the props to set, e.g. `"update": "#{ x: 960 + 300 * sin(t), fill: palette(\"neon\", fract(t)) }"`. It costs about 4 µs per layer per frame.

## 3. Raw drawing: paths and points with their own styles

A `draw` layer takes a list of primitives in frame pixels. The layer covers its parent's box, so `[960, 540]` is the centre of a 1080p frame.

```json
{ "draw": [
  { "d": "M200 800 C600 200 1300 1200 1720 300", "stroke": "$accent", "width": 6, "cap": "round" },
  { "points": [[300, 900], [700, 760], [1100, 880], [1500, 700]], "colors": ["#22d3ee", "#a78bfa", "#f472b6", "#facc15"], "width": 8 },
  { "circle": [960, 540, 120], "fill": "#ffffff22", "stroke": "#fff", "width": 2 },
  { "rect": [100, 100, 300, 160], "radius": 24, "fill": "$surface" },
  { "line": [0, 1000, 1920, 1000], "stroke": "$muted", "width": 1 } ],
  "enter": "draw" }
```

- Each item takes exactly one of `d` (SVG path data), `points` (with `closed`, `smooth` for a curve through them), `circle [cx, cy, r]`, `rect [x, y, w, h]` (with `radius`) or `line [x1, y1, x2, y2]`.
- Style per item: `fill`, `stroke`, `width` (default 2), `cap` (`butt round square`), `join` (`miter round bevel`), `opacity`. `colors` on `points` gives one colour per vertex, and each segment is a straight gradient line (even with `smooth`).
- The layer animates like any other (`x y scale rotation opacity blur`, presets). `trimStart`/`trimEnd` and the `draw` preset draw every stroke on.
- To generate thousands of points, use a script that returns a `draw` layer.
