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
  Grotesque. Missing glyphs fall back to Inter per glyph. In the top-level `fonts` map, each key is an alias: `{"display": "brand.otf"}` or `{"heading": "Fraunces"}` lets you write `"font": "display"`.
- **Paragraphs:** `maxWidth` wraps without fixing the box. Multi-line text with a left anchor (`topLeft`, `left`…) is set flush left unless `align` says otherwise.
- **`axes`** sets variable-font coordinates: `{"wght": 720, "wdth": 125, "opsz": 144, "slnt": -10}`, or custom upper-case tags
  (`SOFT`, `WONK`, `CASL`, `MONO`, `XROT`, `MORF`, `SCAN`…). `fonts NAME` shows axes and ranges; values are clamped.
- **Animate axes** with `"axes.TAG"` in `animate` (in `from`/`to`/`keyframes`). The text is laid out again each frame, so
  width and weight changes reflow and stay centred on the box. An axis named only in `animate` starts at the face's default.
  `letterSpacing` in `animate` (px) also reflows now: a tracking-out over a 2–4 s hold is the classic title move.
- **Optical size is automatic.** On faces with an `opsz` axis (Fraunces, Bodoni Moda, Playfair, Newsreader, Literata, Source
  Serif 4, Big Shoulders, Bricolage Grotesque, DM Sans, Roboto Flex, Texturina…), `opsz` follows the font size in px. A
  200 px headline gets the hairline display cut and a 24 px caption gets the sturdy text cut. Set `axes.opsz` to override it.
- **`features`** turns on OpenType features: `"tnum"` (tabular figures for counters and tables), `"onum"` (old-style figures in
  serif text), `"smcp"` / `"c2sc"` (small caps), `"case"` (punctuation for all-caps), `"ss01"`–`"ss20"` (stylistic sets),
  `"dlig"`, `"zero"` and `"-liga"` (turn one off). Separate them with spaces or commas; `ss02=1` and the CSS form also work.

## Choosing (from docs/video/craft/typography.md)

- Give a reason in one sentence ("compressed gothic because it's sport and the frame is wide"). One family in 2 weights plus
  one accent face beats three families. Make contrast with size first, then weight, then face.
- Size contrast in motion is 10:1 to 25:1: a headline at 30–60 vh beside a caption at 2.2–3 vh. 9 vh over 3 vh reads as a slide.
- Set text flush left on a grid. Centre only one word, a date or a logo.
- Track display lowercase −0.01 to −0.05 em; track caps and small labels +0.05 to +0.12 em. Leading: display 0.85–1.0, text 1.25–1.45.
- Hold time ≈ 0.5 s + 0.3 s per word.
| Brief | Faces (try) |
|---|---|
| Swiss / corporate-modern | Inter Tight, Instrument Sans, Archivo (wdth), Schibsted Grotesk + IBM Plex Mono |
| Editorial / fashion | Bodoni Moda or Playfair (opsz) + Jost or Darker Grotesque |
| Documentary / news | Newsreader + Schibsted Grotesk |
| Sport / energy | Big Shoulders, Anton, Barlow Condensed italic, Saira (wdth 50–125) |
| Warm / 70s | Fraunces (SOFT 100), Young Serif, Shrikhand, Corben |
| Brutalist / art-school | Archivo Black, Syne 800, Anybody (wdth 50→150), Six Caps |
| Tech / terminal | Martian Mono (wdth), Space Mono, VT323, Recursive (MONO/CASL) |
| Luxury / whisper | Cormorant Garamond light, tracked +0.2em caps, Mrs Saint Delafield |
| Gothic / metal | UnifrakturCook, Grenze Gotisch (wght), Pirata One |
| Retro computing | Sixtyfour / Workbench (animate SCAN, BLED), Silkscreen, Press Start 2P |
| Playful | Kablammo (animate MORF), Shantell Sans (BNCE), Gluten, Bagel Fat One |
| Fake 3D | Tilt Warp / Prism / Neon: animate XROT, YROT ±45 |
Colour fonts (Nabla, Honk, Bungee Spice) draw their own palette; `color` doesn't tint them.

## Type as material and as a window

- **Gradients:** `color: "linear-gradient(90deg, #ff3d6e, #ffd23d)"` works on plain, split and kinetic text. On kinetic text
  the gradient is drawn through each glyph's animated alpha.
- **Anything inside the letters** (footage, image, shader, 3D, a group): give the text an `id` and put the fill layer *after* it with
  `"matte": "<id>"`. The text then becomes the window and isn't drawn itself. Kinetic and split animations still drive the window:
  ```json
  { "id": "word", "text": "MOLTEN", "font": "Big Shoulders", "weight": 900, "size": "62vh", "kinetic": "mask by char up 0.7s 50ms outExpo" },
  { "shader": "liquid-chrome", "width": "100%", "height": "100%", "matte": "word" }
  ```
  `{"layer": "word", "mode": "alphaInverted"}` knocks the word *out* of a plate. Footage inside a 50–70 vh compressed sans is the
  classic title move; use heavy weights (800+) so enough of the image shows.
- **Outline:** `stroke` + `strokeWidth`; `color: "none"` leaves only the outline. Layering a solid copy with a hollow copy
  offset by 0.04 em gives an inline/offset print look.
