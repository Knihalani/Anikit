# Anikit Video spec (v1)

You describe a video as one JSON (or YAML) document; `anikit-video render spec.json -o out.mp4` turns it into a file. This page is the complete guide. The machine-readable schema is [`spec.schema.json`](spec.schema.json) (`anikit-video schema` prints it).

**Design rules you can rely on**

- **Everything is optional except `scenes`.** Each scene needs `layers`. Defaults look good: a dark theme, Inter, centred layers, and a 0.5 s crossfade between scenes.
- **Times are seconds.** Write numbers (`1.5`) or strings: `"1.5s"`, `"500ms"`, `"12f"` (frames), or a marker `"@drop"` / `"@drop+0.25"`.
- **Lengths are pixels.** Write numbers (`120`) or strings: `"50%"` (of the parent: width for x/width, height for y/height), `"10vw"`, `"8vh"`, `"5vmin"`, `"5vmax"`.
- **Colours are CSS strings**: `"#6d5dfc"`, `"#fff8"`, `"rgb(20 20 30)"`, `"oklch(70% 0.15 250)"`, `"tomato"`, or a theme token `"$primary"`. Gradients: `"linear-gradient(135deg, #6d5dfc, #ff6ab0)"`, `"radial-gradient(#fff, #000)"`.
- **Positions are the layer's anchor point.** A layer's `x`/`y` default to the centre of its parent, and its `anchor` defaults to its own centre. `"x": 0, "anchor": "left"` puts it flush left.
- **Unknown keys are warnings, not errors**, and every message carries a path and a suggestion: `scenes[2].layers[4].animate[0].ease: unknown easing 'easeOutQuartic', did you mean 'outQuart'?`
- Keys are camelCase. Run `anikit-video check spec.json` (or `--json`) before rendering.

## The smallest video

```json
{
  "scenes": [
    { "layers": [ { "text": "Hello, world", "enter": "fadeUp" } ] }
  ]
}
```

That is 1920×1080, 30 fps, 3 seconds, dark background, white Inter text in the centre that fades up.

## A complete example

```json
{
  "size": "16:9",
  "fps": 60,
  "theme": { "colors": { "primary": "#7c5cff", "accent": "#ff5ca8", "bg": "#0c0b14", "text": "#f4f2ff" } },
  "markers": { "drop": 3.0 },
  "scenes": [
    {
      "id": "intro",
      "duration": 3.5,
      "background": "radial-gradient(#1c1640, $bg)",
      "camera": { "animate": [ { "to": { "zoom": 1.08 }, "duration": 3.5, "ease": "inOutSine" } ] },
      "layers": [
        { "type": "shape", "shape": "circle", "width": 420, "height": 420, "fill": "$primary", "opacity": 0.25, "blur": 80 },
        { "text": "Anikit Video", "style": "title", "split": "chars", "enter": { "preset": "popIn", "stagger": 0.03 } },
        { "text": "JSON in. MP4 out.", "y": "62%", "style": "subtitle", "enter": { "preset": "fadeUp", "delay": 0.6 } }
      ]
    },
    {
      "id": "features",
      "duration": 4,
      "transition": { "type": "push", "direction": "left", "duration": 0.6 },
      "layers": [
        {
          "type": "box",
          "style": { "direction": "row", "gap": 32 },
          "stagger": 0.12,
          "children": [
            { "type": "box", "style": { "padding": 40, "radius": 24, "background": "#ffffff10" }, "enter": "slideUp",
              "children": [ { "text": "Fast", "style": "heading" } ] },
            { "type": "box", "style": { "padding": 40, "radius": 24, "background": "#ffffff10" }, "enter": "slideUp",
              "children": [ { "text": "Native", "style": "heading" } ] },
            { "type": "box", "style": { "padding": 40, "radius": 24, "background": "#ffffff10" }, "enter": "slideUp",
              "children": [ { "text": "Agent-first", "style": "heading" } ] }
          ]
        }
      ]
    }
  ],
  "audio": [ { "src": "music.mp3", "volume": 0.8, "fadeOut": 1 } ]
}
```

## Top level

| Key | Type | Default | Meaning |
|---|---|---|---|
| `size` | preset or `[w, h]` | `"16:9"` | `"16:9"` 1920×1080, `"9:16"` 1080×1920, `"1:1"` 1080×1080, `"4:5"` 1080×1350, `"720p"`, `"1080p"`, `"1440p"`, `"4k"`, `"vertical"` (=9:16), `"square"`, `"portrait"` (=4:5), or `[1280, 720]`. |
| `fps` | number | `30` | Frames per second. |
| `background` | colour or gradient | `"$bg"` | Default background of every scene. |
| `theme` | object | dark theme | Colour tokens, fonts and text styles (see **Theme**). |
| `scenes` | array | **required** | Played in order. |
| `transition` | transition | `{ "type": "crossfade", "duration": 0.5 }` | Default transition between scenes. Use `"cut"` for none. |
| `markers` | `{ name: time }` | `{}` | Named times on the global timeline, used as `"@name"`. |
| `audio` | array | `[]` | Music and sound effects (see **Audio**). |
| `captions` | array or `{ "src": "subs.srt" }` | none | Burned-in captions (see **Captions**). |
| `fonts` | `{ family: path }` | bundled | Extra font files. Bundled: `Inter`, `Inter Display`, `JetBrains Mono`. |
| `duration` | time | sum of scenes | Truncates or pads (holding the last frame) the whole video. |

## Theme

```json
"theme": {
  "colors": { "primary": "#7c5cff", "secondary": "#22d3ee", "accent": "#ff5ca8", "bg": "#0b0b12", "surface": "#171725", "text": "#f5f5fa", "muted": "#9a9ab0" },
  "fonts": { "heading": "Inter Display", "body": "Inter", "mono": "JetBrains Mono" },
  "radius": 20,
  "styles": { "title": { "size": "9vh", "weight": 800, "font": "$heading" } }
}
```

- Any colour field accepts `"$name"` for `theme.colors.name`; any `font` accepts `"$heading"`, `"$body"` or `"$mono"`.
- The default colours are the ones shown above. Override any subset.
- Built-in text `style`s (override or add your own under `theme.styles`):

| Style | Font | Size | Weight | Colour |
|---|---|---|---|---|
| `title` | `$heading` | `9vh` | 800 | `$text` |
| `heading` | `$heading` | `6vh` | 700 | `$text` |
| `subtitle` | `$body` | `3.6vh` | 500 | `$muted` |
| `body` (default) | `$body` | `3vh` | 400 | `$text` |
| `caption` | `$body` | `2.2vh` | 500 | `$muted` |
| `label` | `$body` | `1.8vh` | 600 | `$muted` (uppercase, letterSpacing 0.08em) |
| `code` | `$mono` | `2.6vh` | 400 | `$text` |

## Scenes

| Key | Type | Default | Meaning |
|---|---|---|---|
| `id` | string | `"scene{n}"` | Name, used in messages and by `--from @id`. |
| `duration` | time | `3`, or `"auto"` | `"auto"` = end of the last layer animation + 0.5 s. |
| `background` | colour / gradient | top-level `background` | |
| `transition` | transition | top-level `transition` | How this scene comes **in** from the previous one. The two scenes overlap for its duration, so total length = Σ durations − Σ transitions. |
| `camera` | camera | none | Moves the whole scene (see **Camera**). |
| `markers` | `{ name: time }` | `{}` | Times relative to the scene start, usable as `"@name"` inside this scene. |
| `layers` | array | **required** | Drawn bottom to top (first is at the back). |
| `audio` | array | `[]` | Audio cues whose `at` is relative to the scene start. |

### Transitions

`"transition": "crossfade"` or `{ "type": "slide", "duration": 0.6, "ease": "inOutCubic", "direction": "left" }`.

| Type | Effect | Options |
|---|---|---|
| `cut` | Hard cut (duration ignored) | |
| `crossfade` (alias `fade`) | Dissolve | |
| `fadeBlack` / `fadeWhite` | Dip through a colour | `color` |
| `slide` | New scene slides in over the old | `direction`: `left` (default; comes from the right moving left), `right`, `up`, `down` |
| `push` | New scene pushes the old one out | `direction` |
| `zoom` | Old zooms in and fades, new settles from 0.85× | |
| `wipe` | Hard-edged reveal | `direction`, `softness` (px) |
| `iris` | Circle reveal from a point | `origin`: `[x, y]` (default centre) |
| `blur` | Blur out, blur in | `amount` (px, default 40) |
| `glitch` | Sliced offsets and RGB split, then cut | `intensity` 0..1 |
| `morphCut` | Short soft blend with a slight scale, to hide a jump | |

Defaults: duration 0.5 s (glitch 0.3 s), ease `inOutCubic`.

### Camera

```json
"camera": { "zoom": 1, "x": 0, "y": 0, "rotation": 0,
            "animate": [ { "to": { "zoom": 1.2, "x": -200 }, "at": 1, "duration": 2, "ease": "inOutSine" } ],
            "shake": { "amount": 8, "frequency": 6, "at": "@drop", "duration": 0.4 } }
```

`x`/`y` pan the view (px; positive `x` moves the camera right, so content moves left), `zoom` scales about the centre of the frame, and `rotation` is in degrees. `shake` adds smooth noise (`amount` px, `frequency` Hz), optionally limited to a window that decays to zero at its end.

## Layers

Every layer has a `type`. You may leave it out when it is obvious: a layer with `text` is `text`, with `shape` is `shape`, with `src` is `image` (or `video` for `.mp4/.mov/.webm`), with `children` is `box`, with `layers` is `group`, and with `component` is `component`.

### Common keys (all layer types)

| Key | Type | Default | Meaning |
|---|---|---|---|
| `id` | string | | Used in messages and to target layers. |
| `x`, `y` | length | `"50%"` | Anchor position inside the parent. |
| `anchor` | `[ax, ay]` (0..1) or name | `"center"` | Point of the layer placed at `x, y`: `center`, `top`, `bottom`, `left`, `right`, `topLeft`, `topRight`, `bottomLeft`, `bottomRight`. |
| `width`, `height` | length | content size | Text wraps to `width`. Shapes and images need a size (images default to their natural size). |
| `scale` | number or `[sx, sy]` | `1` | Also `scaleX`, `scaleY`. |
| `rotation` | degrees | `0` | Clockwise. |
| `skewX`, `skewY` | degrees | `0` | |
| `opacity` | 0..1 | `1` | |
| `blur` | px | `0` | Gaussian blur of the layer. |
| `blend` | string | `"normal"` | `normal`, `add`, `multiply`, `screen`, `overlay`, `darken`, `lighten`, `softLight`, `difference`. |
| `shadow` | shadow | none | `{ "x": 0, "y": 12, "blur": 32, "color": "#0008" }` or `true` for a soft default. |
| `glow` | glow | none | `{ "radius": 24, "intensity": 1, "color": "$primary" }` or a colour string. |
| `clip` | bool or shape | `false` | `true` clips children to the layer's box (with its `radius`); or a shape object `{ "shape": "circle", ... }` in the layer's local space. |
| `mask` | layer | none | Another layer whose alpha masks this one. |
| `start` | time | `0` | When the layer appears (relative to the scene, or to the parent group). |
| `end` / `duration` | time | scene end | When it disappears. |
| `enter` | preset | none | Entrance animation, at `start`. |
| `exit` | preset | none | Exit animation, finishing at `end`. |
| `animate` | array of tweens | `[]` | Explicit animation (see **Animation**). |
| `loop` | preset | none | A repeating motion while visible: `float`, `pulse`, `spin`, `wiggle`, `breathe`, `blink`. |
| `visible` | bool | `true` | `false` skips the layer. |

### `text`

| Key | Type | Default | Meaning |
|---|---|---|---|
| `text` | string | **required** | `\n` breaks lines. Inline spans: `"Make it **bold**, *italic* or [coloured]{$accent}"`. |
| `style` | string | `"body"` | A theme text style; other keys override it. |
| `font` | string | from style | Family name or `$heading` / `$body` / `$mono`. |
| `size` | length | from style | Font size (`"6vh"` scales with the frame). |
| `weight` | 100..900 or name | from style | `"bold"`, `"semibold"`, … |
| `italic` | bool | `false` | |
| `color` | colour / gradient | from style | Gradients fill the whole text box. |
| `align` | `left` / `center` / `right` | `"center"` | Within the text box. |
| `lineHeight` | number | `1.2` | Multiple of size. |
| `letterSpacing` | length or `"0.05em"` | `0` | |
| `uppercase` | bool | `false` | |
| `maxLines` | integer | none | Truncates with an ellipsis. |
| `stroke` | stroke | none | Outline text: `{ "color": "#fff", "width": 2 }`. |
| `split` | `chars` / `words` / `lines` | none | Splits the text into units so `enter`/`exit` presets stagger across them (kinetic type). |
| `reveal` | 0..1 | `1` | Fraction of characters shown (animate it for a typewriter; the `typewriter` preset does). |
| `value` | number | | With `"text": "{value}"` (or `"{value:0.0}"`, `"${value:,}"`), shows an animatable number. Animate `value` for counters. |

### `shape`

| Key | Type | Default | Meaning |
|---|---|---|---|
| `shape` | string | **required** | `rect`, `circle` (`ellipse`), `line`, `arrow`, `polygon`, `star`, `path`. |
| `width`, `height` | length | `200` | Box of `rect`, `circle`, `polygon`, `star`. |
| `radius` | length | `0` | Corner radius of `rect`. |
| `fill` | colour / gradient / `"none"` | `"$primary"` (`"none"` for `line`, `arrow`, open `path`) | |
| `stroke` | colour or stroke | none | `"#fff"` or `{ "color": "#fff", "width": 4, "cap": "round", "join": "round", "dash": [10, 6] }`. |
| `strokeWidth` | px | `4` | Shorthand for `stroke.width`. |
| `points` | `[[x, y], …]` | | `line` / `arrow` (2 points; polyline if more) and `polygon` vertices, in local px. |
| `sides` | integer | `6` | Regular `polygon` when `points` is absent; `star` points (default 5). |
| `innerRadius` | 0..1 | `0.5` | `star` inner radius as a fraction. |
| `d` | SVG path string | | `path` data (`"M0 0 C 50 -40 …"`), local px. |
| `headSize` | px | `4 × strokeWidth` | `arrow` head size. |
| `trimStart`, `trimEnd` | 0..1 | `0`, `1` | Draw part of the outline (animate for draw-on; see the `drawOn` preset). |

### `image` and `video`

| Key | Type | Default | Meaning |
|---|---|---|---|
| `src` | path or URL | **required** | PNG, JPEG, WebP, GIF, SVG; video: MP4, MOV, WebM. Relative to the spec file. |
| `fit` | `contain` / `cover` / `fill` / `none` | `"contain"` | How it fits `width × height`. |
| `radius` | length | `0` | Rounded corners. |
| `trimStart` | time | `0` | (video) Start offset into the clip. |
| `speed` | number | `1` | (video) Playback rate. |
| `volume` | 0..1 | `0` | (video) Clip audio, muted by default. |
| `loop` | bool | `false` | (video) Loop the clip. |

### `group`

`{ "type": "group", "layers": [...] }` — a transformable, animatable container. Children are positioned relative to the group's box (`width`/`height`, default the parent's size); their `start` is relative to the group's `start`. `stagger` (see below) offsets each child's animations.

### `box` (layout)

A flexbox container (CSS semantics) that lays out `children` automatically, so you don't compute coordinates. Children are any layers; their `x`/`y` are ignored (they're placed by layout), but their transforms and animation still apply on top.

```json
{ "type": "box", "style": { "direction": "column", "gap": 24, "padding": [48, 64], "align": "start",
                           "background": "$surface", "radius": 24, "border": "1px #ffffff20" },
  "width": "60%", "children": [ ... ] }
```

`style` keys: `direction` (`row` default / `column`), `gap`, `padding` (number, `[v, h]` or `[t, r, b, l]`), `align` (`start`/`center`/`end`/`stretch`, cross axis, default `center`), `justify` (`start`/`center`/`end`/`spaceBetween`/`spaceAround`/`spaceEvenly`), `wrap` (bool), `grow` (on a child's `style`: flex-grow), `background`, `radius`, `border` (`"2px #fff"`), `columns` (number → a grid with that many equal columns).

### `component`

`{ "type": "component", "component": "barChart", "props": { ... } }` — a pluggable high-level block (code blocks, charts, device mockups, counters, particles, lower thirds, …). Components have their own guide (`docs/video/components.md`); `anikit-video info --components` lists them with their props. Common keys (position, transforms, `enter`, `animate`) apply. A component also exposes a `progress` property (0..1 over its lifetime by default) that you can animate.

## Animation

Three layers of control, from simplest to most precise.

### 1. Presets: `enter`, `exit`, `loop`

```json
"enter": "fadeUp"
"enter": { "preset": "popIn", "delay": 0.2, "duration": 0.5, "ease": "outBack", "stagger": 0.04 }
"exit": "fadeOut"
"loop": { "preset": "float", "amount": 12, "period": 3 }
```

| Preset | Does | Default duration / ease |
|---|---|---|
| `fadeIn` / `fadeOut` | opacity | 0.5 s / `outCubic` |
| `fadeUp`, `fadeDown`, `fadeLeft`, `fadeRight` | opacity + 40 px move | 0.6 s / `outCubic` |
| `slideUp`, `slideDown`, `slideLeft`, `slideRight` | moves in from 15% of the frame away, with fade | 0.7 s / `outExpo` |
| `popIn` / `popOut` | scale 0 → 1 with overshoot, fade | 0.5 s / `outBack` |
| `scaleIn` / `scaleOut` | scale 0.8 → 1, fade | 0.5 s / `outCubic` |
| `zoomIn` / `zoomOut` | scale 1.4 → 1, blur, fade | 0.7 s / `outExpo` |
| `blurIn` / `blurOut` | blur 30 → 0, fade | 0.6 s / `outCubic` |
| `rotateIn` | rotation −90° → 0, scale, fade | 0.6 s / `outBack` |
| `flipIn` | scaleX 0 → 1 | 0.5 s / `outCubic` |
| `bounceIn` | drops from above with a spring | spring `bouncy` |
| `drawOn` / `drawOff` | stroke `trimEnd` 0 → 1 (shapes) | 1 s / `inOutCubic` |
| `typewriter` | text `reveal` 0 → 1 at `cps` chars/s (default 20) | linear |
| `wipeIn` / `wipeOut` | clip reveal, `direction` | 0.6 s / `inOutCubic` |
| `countUp` | text `value` from 0 to its value | 1.5 s / `outCubic` |

`exit` versions run backwards and end at the layer's `end`. Presets take `delay`, `duration`, `ease`, `stagger` (for `split` text and containers), and preset-specific options (`distance` for moves, `direction` for wipes).

### 2. Tweens: `animate`

```json
"animate": [
  { "from": { "opacity": 0, "y": "+60" }, "duration": 0.6, "ease": "outCubic" },
  { "to": { "x": 1400, "rotation": 15 }, "at": 1.5, "duration": 0.8, "ease": "inOutQuart" },
  { "to": { "fill": "$accent" }, "at": "@drop", "duration": 0.3 },
  { "keyframes": { "scale": [1, 1.2, 0.9, 1] }, "at": 3, "duration": 0.6 },
  { "to": { "scale": 1.5 }, "at": 4, "spring": "bouncy" }
]
```

- `at` (default `0`) is relative to the layer's `start`. Use `"+0.2"` for "0.2 s after the previous tween ends".
- `from`: animates **from** these values to the layer's own value. Before `at`, the layer holds the `from` value (so entrances don't flash).
- `to`: animates from the current value **to** these values, and holds them afterwards.
- `from` and `to` together: an explicit range.
- Relative values: `"+60"`, `"-20"`, `"*1.5"` are relative to the value being animated from or to.
- `keyframes`: `{ prop: [v0, v1, …] }`, evenly spaced over `duration`, or `{ prop: [[time, value], …] }` with times in 0..1 of the tween. `ease` applies between each pair.
- `duration` default `0.6`. `ease` default `outCubic`.
- `spring`: instead of `ease` + `duration`: a preset (`gentle`, `wobbly`, `stiff`, `slow`, `molasses`, `snappy`, `bouncy`) or `{ "stiffness": 180, "damping": 12, "mass": 1 }`; duration becomes the settle time.
- `repeat` (count, `-1` = forever) and `yoyo` (bool) repeat a tween.

**Animatable properties:** `x`, `y`, `width`, `height`, `scale`, `scaleX`, `scaleY`, `rotation`, `skewX`, `skewY`, `opacity`, `blur`, `radius`, `fill`/`color` (interpolated in OKLab), `stroke`(colour), `strokeWidth`, `trimStart`, `trimEnd`, `reveal`, `value`, `size` (text), `letterSpacing`, `progress` (components), `zoom` (camera).

**Easings:** `linear`, `ease`, `easeIn`, `easeOut`, `easeInOut`; Penner names as `in`/`out`/`inOut` + `Sine`, `Quad`, `Cubic`, `Quart`, `Quint`, `Expo`, `Circ`, `Back`, `Elastic`, `Bounce` (e.g. `outCubic`, `inOutBack`; `easeOutCubic` also works); `cubicBezier(0.2, 0, 0, 1)`; `steps(4)`; springs `gentle`, `wobbly`, `stiff`, `slow`, `molasses`, `snappy`, `bouncy`.

### 3. Expressions

Any numeric property (static or inside `to`/`from`) can be an expression string beginning with `=`:

```json
{ "shape": "circle", "x": "=960 + 300 * sin(t * 2)", "y": "=540 + 300 * cos(t * 2)", "rotation": "=t * 90" }
```

Variables: `t` (seconds since the layer's start), `T` (seconds since the scene's start), `gt` (global time), `p` (layer progress 0..1), `i` (index within the parent), `n` (sibling count), `w`, `h` (frame size), `fps`, `frame`. Functions: `sin cos tan asin acos atan atan2 abs min max floor ceil round sqrt pow exp log clamp(x, a, b) mix(a, b, f) smoothstep(a, b, x) noise(x) noise(x, y) rand(i) pi`. Expressions are compiled once and are cheap.

### Stagger

On a `group`/`box` (or a `split` text), `"stagger": 0.08` delays each child's animations (`enter`, `exit`, `animate`) by `index × 0.08 s`. Object form: `{ "each": 0.08, "from": "start" | "center" | "end" | "random", "ease": "inQuad" }`; or `"total": 0.6` to spread the delays over a fixed time.

## Audio

```json
"audio": [
  { "src": "music.mp3", "volume": 0.7, "fadeIn": 0.5, "fadeOut": 1.5, "duck": 0.4 },
  { "src": "whoosh.wav", "at": "@drop", "volume": 1 }
]
```

`src`, `at` (time, default `0`), `volume` (0..1, or `"-6dB"`), `trimStart`, `duration`, `fadeIn`, `fadeOut`, `loop`, `duck` (lower this track to this volume while voice tracks play), `voice` (bool, marks a voice-over track used for ducking and captions).

## Captions

```json
"captions": { "src": "voice.srt", "style": "caption", "y": "88%", "highlight": "$accent" }
"captions": [ { "text": "Meet Anikit Video", "at": 0.5, "end": 2.2 }, { "text": "JSON in, MP4 out", "at": 2.4, "end": 4 } ]
```

Captions are text layers drawn over every scene. `highlight` colours the current word when word timings are known (`words: [[text, at, end], …]`).

## Validation and messages

`anikit-video check spec.json` prints errors (the video cannot render) and warnings (it renders, but probably not as intended), each with a path:

```
error   scenes[1].layers[0].type: unknown layer type 'txt', did you mean 'text'?
error   scenes[0].layers[2].animate[0].ease: unknown easing 'easeOutQuartic', did you mean 'outQuart'?
warning scenes[0].layers[1].colour: unknown key, did you mean 'color'?
warning scenes[2].layers[3].start: 4.5 s is after the scene ends (4 s); the layer is never visible
```

With `--json`, the output is `{ "ok": bool, "errors": [{ "path", "message", "suggestion" }], "warnings": [...], "duration": s, "frames": n, "scenes": [{ "id", "start", "end" }] }`.

## Recipes

**Title card, vertical**

```json
{ "size": "9:16", "scenes": [ { "duration": 2.5, "layers": [
  { "text": "Ship faster", "style": "title", "split": "words", "enter": { "preset": "fadeUp", "stagger": 0.12 } },
  { "type": "shape", "shape": "line", "points": [[0, 0], [400, 0]], "y": "58%", "stroke": { "color": "$accent", "width": 6, "cap": "round" }, "enter": { "preset": "drawOn", "delay": 0.4 } }
] } ] }
```

**Counter**

```json
{ "text": "{value:,}+", "value": 12000, "style": "title", "enter": "countUp" }
```

**Logo orbit with expressions**

```json
{ "type": "group", "loop": "spin", "layers": [
  { "shape": "circle", "width": 24, "height": 24, "fill": "$accent", "x": "=50% + 200 * cos(t * 3 + i)", "y": "=50% + 200 * sin(t * 3 + i)" }
] }
```

**Three cards that build in**

```json
{ "type": "box", "style": { "direction": "row", "gap": 24 }, "stagger": 0.1, "children": [
  { "type": "box", "width": 360, "height": 220, "style": { "background": "$surface", "radius": 24 }, "enter": "popIn" },
  { "type": "box", "width": 360, "height": 220, "style": { "background": "$surface", "radius": 24 }, "enter": "popIn" },
  { "type": "box", "width": 360, "height": 220, "style": { "background": "$surface", "radius": 24 }, "enter": "popIn" }
] }
```

**Camera punch-in on a beat**

```json
"camera": { "animate": [ { "keyframes": { "zoom": [1, 1.12, 1.05] }, "at": "@drop", "duration": 0.5, "ease": "outExpo" } ],
            "shake": { "amount": 10, "at": "@drop", "duration": 0.35 } }
```

## Notes for implementers

- `spec` (serde types + schema) → `eval::Evaluator::new(spec, assets)` resolves themes, units, presets, stagger and expressions into per-property tracks once; `eval(t)` produces a `dl::DisplayList`.
- Expressions with `%`/`vw`/`vh` units inside (e.g. `"=50% + 200*cos(t)"`) are resolved against the property's reference length.
- Component plugins implement `eval::Component` (see `crates/anikit-video/src/eval/component.rs`).
