# Graphic design: grids, shapes, patterns, print treatments

```json
{ "background": "#ece8df",
  "grid": { "columns": 12, "rows": 6, "margin": [60, 72], "gutter": 24, "baseline": 12,
            "areas": { "head": "1-7/1-3", "art": "8-12/1-6", "body": "1-3/5-6" } },
  "scenes": [ { "duration": 4, "layers": [
    { "cell": "art", "pattern": { "kind": "truchet", "spacing": 64, "mark": 0.18, "seed": 4 }, "fill": "#e03a1e" },
    { "cell": "head", "text": "Konkrete\nKunst", "size": 190, "weight": 800, "lineHeight": 0.9, "color": "#111", "enter": "rise" },
    { "cell": "body", "text": "Zwanzig Positionen, 1936 bis heute.", "size": 26, "color": "#111" },
    { "cell": "9-12/4-6", "shape": "squircle", "exponent": 5, "fill": "#111", "halftone": { "cell": 9, "ink": "#111", "paper": "#ece8df" } } ] } ] }
```

- **Grid** `grid` on the video, a scene (replaces) or a `group` (for its children): `columns` (12), `rows` (8), `margin`
  (length, `[v, h]` or `[t, r, b, l]`; default 5% of the short side), `gutter` (length or `[col, row]`; default 2%),
  `baseline` px, `areas` name → range, `show: true` tints fields and baselines over the scene to check a layout;
  `show: "under"` draws the field edges as hairlines beneath the layers (a guidelines look; `color` sets them).
- **Cell** `"cell": "head"` | `"1-6/2-4"` (columns 1–6, rows 2–4) | `"7/1"` | `"9-"` (column 9 to the end, all rows) |
  `[col, row, cols, rows]`. Sets x, y, width, and height (not for text). `anchor` picks the point of the cell (default
  `topLeft`; `bottomLeft` sits a caption on the field's foot) and text alignment follows it. With `baseline`, a text's
  leading rounds to whole baseline units. Explicit `x`/`y`/`width`/`height` still win. Animation applies on top.
- **Shapes** `squircle` / `superellipse` (`exponent`: 2 ellipse, 4–5 squircle, 2.5 Piet Hein, < 1 pinched star),
  `blob` (`sides` lobes, `wobble` 0..1, `seed`; animate with `wigglePath`), `arc` (`startAngle`, `endAngle` deg, 0 = 12
  o'clock clockwise; `innerRadius` 0..1 makes a ring segment; 0 = pie wedge).
- **Patterns** `"pattern": "dots 32"` or `{kind, spacing, mark, angle, seed, jitter, ramp}` fill the layer's box (clipped):
  `dots grid lines stripes checker triangles diamonds crosses rings waves zigzag truchet arcs scales bricks`.
  `mark` = mark size / spacing; `ramp: [0.05, 1]` grows marks across the box (a vector halftone ramp); `jitter` 0..1.
  One path per layer: thousands of marks cost one fill. Keep `spacing` ≥ 6 px (finer turns to moiré after H.264).
- **Treatments** on any layer (image, video, shader, text, group, composition), applied before blur/shadow:
  `duotone: [shadows, highlights]`, `gradientMap: [c1, c2, c3…]` (or `[offset, colour]` pairs; interpolated in OKLab),
  `posterize: N` (luma steps with a map, RGB steps alone), `toneRange: [black, white]` (stretch a flat source first),
  `toneAmount` 0..1, `halftone: 10` or `{cell, angle, shape: dot|line|square, ink, paper, gain}` (ink `"source"` keeps
  each dot's colour; paper `"none"` leaves only dots; the screen is fixed to the frame, as on press).
- **Ink texture** `"inkTexture": 0.35` or `{amount, scale, seed}`: uneven coverage (blotches + pinholes) fixed to the frame,
  so it reads as ink on paper rather than a grain overlay. Use it on the ink layers only; leave type you must read lighter (≤ 0.2).
- **Stepped time** `"step": "twos"` (or `"threes"`, or fps like `8`): the layer and its children animate held, like
  stop-motion or hand-pulled print, while the rest stays smooth. **Boil** `"boil": 2` or `{amount, rotate, fps, seed}`:
  a seeded jitter re-drawn `fps` (12) times a second; on a `multiply` ink layer it reads as plates out of register.
- **Overprint**: two ink layers with `"blend": "multiply"` mix like spot inks; offset one 3–6 px for misregistration.
  Whole-frame print looks (risograph, screenprint, linocut, print-offset…) are shader filters: `docs shader`.

Taste, numbers and recipes (Swiss, riso, identity, editorial data, Memphis, Polish poster, op-art): `docs/video/craft/graphic-design.md`.
