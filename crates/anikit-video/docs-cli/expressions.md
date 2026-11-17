# Expressions: any number or colour, per frame

```json
{ "scenes": [ { "duration": 4, "layers": [
  { "shape": "circle", "width": 160, "x": "=960 + 500 * sin(t * 1.5)", "y": "=540 + 200 * cos(t * 3)",
    "fill": "=hsl(t * 90, 80, 60)", "scale": "=fract(t) < 0.1 ? 1.3 : 1" },
  { "text": "=t < 2 ? 1 : 2", "style": "title", "y": "80%", "color": "=t < 2 ? $accent : $primary" } ] } ] }
```

- Variables: `t` (layer s) `T` (scene s) `gt` (video s) `p` (0..1) `frame` `i` `n` `w` `h` `fps`, plus music: `beat` `bar` (fractional), `level` (loudness 0..1), `bpm`.
- Logic: `< <= > >= == != && || !`, `c ? a : b`, `if(c, a, b)`.
- Functions: `sin cos tan atan2 abs floor fract mod min max clamp sqrt pow exp log hypot mix lerp remap smoothstep step pingpong ease easeIn easeOut spring tri saw square noise rand rand2`.
- Text: `"text": "=floor(t * 100)"` shows a number every frame (counters, countdowns). For a format, use `"text": "{value:,} users"` with `"value": "=…"`. Results are numbers, not strings.
- Colours: `=rgb(r, g, b[, a])`, `=hsl(h, s, l[, a])`, `=oklch(L, C, h)`, `=mix($a, $b, x)`, `=c ? colourA : colourB`. They work on `fill`, `stroke`, text `color` and box `background`.
- Music-reactive: `"scale": "=1 + 0.3 * level"`, `"rotation": "=8 * (1 - fract(beat))"`. Needs `music` or an `audio` `src`.
- `t` is the layer's own clock: it starts at 0 when the layer starts. For a layer with `start: 4`, `"=t > 5 ? …"` fires at 9 s on the video. Use `gt` (video seconds) or `T` (scene seconds) when you think in absolute times; use `t` to keep a layer self-contained when you move it.
