# Primitives: text, shapes, paths, images, groups

Every layer: `x y` (default centre; px, `"50%"`, `"10vw"`) · `anchor` · `width height` · `scale rotation opacity blur blend` · `shadow glow clip mask depth` · `start end` · `enter exit animate`.

```json
{ "size": "16:9", "background": "#0B0B0E", "scenes": [ { "duration": 3, "layers": [
  { "shape": "path", "d": "M0 200 C 300 0, 600 400, 900 120", "stroke": "#FF5A1F", "strokeWidth": 6, "fill": "none",
    "x": "10%", "y": "30%", "anchor": "topLeft", "animate": [ { "from": { "trimEnd": 0 }, "to": { "trimEnd": 1 }, "duration": 1.2, "ease": "inOutExpo" } ] },
  { "shape": "circle", "width": 180, "height": 180, "fill": "radial-gradient(#FFD27A, #FF5A1F)", "x": "78%", "y": "40%", "glow": "#FF5A1F" },
  { "text": "Make **anything**.", "font": "Instrument Serif", "size": "14vh", "color": "#F4EFE6", "letterSpacing": "-0.02em",
    "x": "8%", "y": "62%", "anchor": "topLeft", "enter": "mask by line up" },
  { "layers": [ { "shape": "rect", "width": 300, "height": 6, "fill": "#F4EFE6" } ], "x": "8%", "y": "88%", "anchor": "left" }
] } ] }
```

- **text**: `text` (`**bold**`, `*italic*`, `[word]{$accent}`), `font` (Inter, Inter Display, Instrument Serif, JetBrains Mono, or your file via top-level `fonts`), `size weight color align lineHeight letterSpacing uppercase maxLines`, `fit: true`, `stroke`, `split: chars|words|lines`.
- **shape**: `rect circle line arrow polygon star path` · `fill` (colour, `linear-gradient(...)`, `radial-gradient(...)`) · `stroke strokeWidth radius` · `trimStart trimEnd` (draw-on) · `d` (SVG path, local px; the layer sits at the path's top-left with `anchor: topLeft`).
- **image/video**: `src` · `fit: cover|contain` · video `trimStart speed loop volume`.
- **group**: `layers: [...]` (transforms apply to all). **box**: `children` + flexbox `style`.
- Colours: hex, `oklch(...)`, theme tokens `$primary $accent $text $bg`.

Every component (`info --components`) is built from these; when none fits, compose your own.
