# Type: the font library, variable axes, OpenType features

Three faces were bundled (Inter, Instrument Serif, JetBrains Mono), and those are the tells of AI-made design. Pick a voice instead.
`anikit-video fonts` lists ~130 curated open-licence families by genre (grotesque, geometric, humanist, didone, serif, slab,
condensed, wide, blackletter, script, hand, stencil, mono, pixel, display). Each comes with a one-line character note and pairings.
`anikit-video fonts didone`, `fonts wdth`, `fonts Fraunces` (one card) and `fonts --json` filter the list.

```json
{ "scenes": [ { "duration": 4, "background": "#0d0c0b", "layers": [
  { "text": "Soft power", "font": "Fraunces", "size": "34vh", "color": "#f2e9dc", "align": "left", "x": "6vw", "y": "52vh",
    "letterSpacing": "-0.04em", "lineHeight": 0.9, "axes": { "wght": 300, "SOFT": 0, "WONK": 1 },
    "animate": [ { "keyframes": { "axes.wght": [300, 900], "axes.SOFT": [0, 100] }, "duration": 2.5, "ease": "inOutCubic" } ] },
  { "text": "VOL. 04 — 2026", "font": "Martian Mono", "size": "2.4vh", "letterSpacing": "0.12em", "color": "#f2e9dc99",
    "align": "left", "x": "6vw", "y": "88vh", "axes": { "wdth": 112.5 }, "features": "tnum" } ] } ] }
```

- **`"font": "Name"`**: a library family is downloaded once (50–500 KB, from a pinned google/fonts commit, SHA-256 checked) to
  `~/.cache/anikit/fonts` (`$ANIKIT_FONT_DIR` overrides it). After that it renders offline and gives identical output on
  every machine. If a fetch fails, `check` reports an error that names the file and folder to put it in. `ANIKIT_OFFLINE=1`
  turns fetching off. `anikit-video fonts --fetch all` pre-warms the cache (~80 MB). An unknown name gives a warning with
  suggestions. It works everywhere a font is named: layers, spans, `theme.fonts`, styles and components.
- **Names are forgiving:** `"Bodoni Moda Black"` → Bodoni Moda (with a warning to set `weight` separately), and `"Bricolage"` → Bricolage
  Grotesque. In the top-level `fonts` map, each key is an alias: `{"display": "brand.otf"}` or `{"heading": "Fraunces"}` lets you write `"font": "display"`.
  Characters a face lacks (₂ → ½ ←) fall back to Inter per glyph instead of drawing as boxes.
- **Paragraphs:** `maxWidth` wraps without fixing the box. Multi-line text with a left anchor (`topLeft`, `left`…) is set flush left unless `align` says otherwise.
- **`axes`** sets variable-font coordinates: `{"wght": 720, "wdth": 125, "opsz": 144, "slnt": -10}`, or custom upper-case tags
  (`SOFT`, `WONK`, `CASL`, `MONO`, `XROT`, `YEAR`, `MORF`, `BLED`, `SCAN`…). `fonts NAME` shows a face's axes and ranges.
  Values outside the range are clamped.
- **Animate axes** with `"axes.TAG"` in `animate` (in `from`/`to`/`keyframes`). The text is laid out again each frame, so
  width and weight changes reflow and stay centred on the box. An axis named only in `animate` starts at the face's default.
  `letterSpacing` in `animate` (px) also reflows now: a tracking-out over a 2–4 s hold is the classic title move.
- **Per-glyph axes** (a weight wave): the text `animator` takes `axes`, the values at full selection:
  `"animator": {"by": "char", "axes": {"wght": 900, "SOFT": 100}, "start": 0, "end": 0.25, "smooth": 0.3, "offset": "=-0.3 + 1.6*fract(t/2.5)"}`.
  Advances are compensated so heavier letters push their neighbours, and the line stays aligned.
  Combine it with the animator's `x`/`y`/`scale`/`color` for a wave that rises as it thickens.
- **Optical size is automatic.** On faces with an `opsz` axis (Fraunces, Bodoni Moda, Playfair, Newsreader, Literata, Source
  Serif 4, Big Shoulders, Bricolage Grotesque, DM Sans, Roboto Flex, Texturina…), `opsz` follows the font size in px. A
  200 px headline gets the hairline display cut and a 24 px caption gets the sturdy text cut. Set `axes.opsz` to override it.
  Inter keeps its behaviour (`"Inter Display"` = opsz 32).
- **`features`** turns on OpenType features: `"tnum"` (tabular figures for counters and tables), `"onum"` (old-style figures in
  serif text), `"smcp"` / `"c2sc"` (small caps), `"case"` (punctuation for all-caps), `"ss01"`–`"ss20"` (stylistic sets),
  `"dlig"`, `"zero"` and `"-liga"` (turn one off). Separate them with spaces or commas; `ss02=1` and the CSS form also work.

Next: `anikit-video docs type-craft` (choosing faces by brief, scale and tracking numbers, gradient fills, type as a window onto footage or shaders).
Research with sources: docs/video/craft/typography.md.
