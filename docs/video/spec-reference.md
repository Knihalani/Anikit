# Anikit Video spec: full reference (v1)

The one-page cheat sheet is [`spec.md`](spec.md); this page is the complete reference.

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

## Shorthand (fewest tokens)

```json
{ "scenes": [
  "Launch day | Everything ships at once",
  { "layers": [ { "text": "3× faster", "style": "title", "animate": "rise" }, "Built in Rust" ] },
  { "layers": [ { "component": "bar-chart", "props": { "values": [3, 7, 5, 9] } } ] }
] }
```

- A **scene** can be a string: a title card (`"Title | subtitle"`).
- A **layer** can be a string: a centred body-text layer.
- `"animate": "rise"` (a preset name) means `"enter": "rise"`; `animate` also takes one tween object instead of a list.
- Preset aliases: `rise` (fadeUp), `drop` (fadeDown), `fade`, `pop`, `grow` (scaleIn), `zoom`, `blur`, `draw` (drawOn), `type` (typewriter), `count` (countUp), `wipe`, `slide` (slideUp), `bounce`; in `exit` they mean the exit form (`"exit": "fade"` = fadeOut).
- On text, any kinetic style also works as `enter`/`animate`: `"animate": "scramble"`, `"enter": "pop by char from center"`.

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

## Grid

`"grid": { "columns": 12, "rows": 8, "margin": 96, "gutter": 24, "baseline": 12, "areas": { "head": "1-8/1-3" }, "show": false }` on the video, a scene (replaces it) or a group (for its children). Layers snap to it with `cell`. `margin` takes 1, 2 (`[v, h]`) or 4 lengths (default 5% of the short side); `gutter` 1 or 2 (default 2%). With `baseline`, text in a cell gets leading in whole baseline units. `show: true` draws the fields and baselines. Guide: `anikit-video docs graphic`.

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

**Depth (parallax and depth of field).** Give layers a `depth` (0 = the focal plane, positive = farther, negative = nearer). When the camera pans, a layer moves by `1 / (1 + depth)` of the pan, so backgrounds drift slower than the foreground. Set `camera.aperture` (blur σ in px per unit of |depth − focus|) to blur layers away from `camera.focus`. `depth`, `focus` and `aperture` are all animatable, which gives you focus pulls:

```json
"camera": { "aperture": 8, "animate": [ { "to": { "focus": 2, "x": 120 }, "duration": 2, "ease": "inOutSine" } ] },
"layers": [ { "component": "aurora", "depth": 3 }, { "text": "Sharp, then soft", "style": "title" }, { "src": "phone.png", "depth": 2 } ]
```

## Layers

Every layer has a `type`. You may leave it out when it is obvious: a layer with `text` is `text`, with `shape` is `shape`, with `src` is `image` (or `video` for `.mp4/.mov/.webm`), with `children` is `box`, with `layers` is `group`, with `component` is `component`, with `composition` is `composition`, and with `shader` (or `src` ending in `.glsl`/`.wgsl`) is `shader`.

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
| `depth` | number | `0` | Parallax and depth of field (see **Camera**). |
| `blend` | string | `"normal"` | `normal`, `add`, `multiply`, `screen`, `overlay`, `darken`, `lighten`, `softLight`, `difference`. |
| `shadow` | shadow | none | `{ "x": 0, "y": 12, "blur": 32, "color": "#0008" }` or `true` for a soft default. |
| `cell` | area name, `"1-6/2-4"`, `[col, row, cols, rows]` | none | Place the layer in a field of the scene's `grid` (sets x, y, width; height except for text). `anchor` picks the cell point (default `topLeft`). See `anikit-video docs graphic`. |
| `duotone`, `gradientMap` | `[dark, light]`, `[c1, c2, …]` | none | Map the layer's luma to inks (OKLab). `toneRange: [black, white]` pre-stretches, `posterize: N` steps, `toneAmount` mixes. |
| `halftone` | px or `{cell, angle, shape, ink, paper, gain}` | none | Print screen over the layer (dot/line/square; ink `"source"` keeps colours; fixed to the frame). |
| `glow` | glow | none | `{ "radius": 24, "intensity": 1, "color": "$primary" }` or a colour string. |
| `clip` | bool or shape | `false` | `true` clips to the layer's box (with its `radius`); or a static shape `{ "shape": "circle", "width": 300, "height": 300 }` positioned in the layer's box (centred by default). |
| `mask` | shape layer | none | A shape whose outline cuts this layer (hard-edged; same as a `clip` shape). |
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
| `fit` | bool | `false` | `true` shrinks the size (never grows it) so the widest line fits `width` (default 85% of the parent), keeping your line breaks. Use it for big kinetic words on 9:16. |
| `stroke` | colour or stroke | none | Outlined text: `"#000"` or `{ "color": "$accent", "width": 3 }` (px). Drawn under the fill; with `"color": "none"` only the outline shows. |
| `split` | `chars` / `words` / `lines` | none | Splits the text into units so `enter`/`exit` presets stagger across them (kinetic type). |
| `kinetic` | string or object | none | A text animation from the text engine, as shorthand: `"rise"`, `"pop by char from center 0.4s 30ms outBack"`, `"scramble"`, `"typewriter"`, `"wave by word"`, `"highlight words"`, `"mask by line up"`. Styles: `rise`, `fade`, `blur`, `pop`, `typewriter`, `scramble`, `wave`, `highlight`, `mask`. An `enter` that isn't a layer preset (for example `"enter": "scramble"`) is treated as `kinetic`. |
| `reveal` | 0..1 | `1` | Fraction of characters shown (animate it for a typewriter; the `typewriter` preset does). |
| `value` | number | | With `"text": "{value}"` (or `"{value:0.0}"`, `"${value:,}"`), shows an animatable number. Animate `value` for counters. |

### `shape`

| Key | Type | Default | Meaning |
|---|---|---|---|
| `shape` | string | **required** | `rect`, `circle` (`ellipse`), `line`, `arrow`, `polygon`, `star`, `path`, `squircle` (`superellipse`), `blob`, `arc`, `pattern`. |
| `exponent` | number | `4` | `superellipse`/`squircle`: 2 ellipse, 4–5 squircle, higher squarer. |
| `sides`, `wobble`, `seed` | | `6`, `0.3`, `1` | `blob`: lobes, irregularity 0..1, seed. |
| `startAngle`, `endAngle`, `innerRadius` | degrees, 0..1 | `0`, `270`, `0` | `arc`: 0 = 12 o'clock, clockwise; `innerRadius` > 0 makes a ring segment. |
| `pattern` | `"dots 32"` or `{kind, spacing, mark, angle, seed, jitter, ramp}` | | Fills the box: `dots grid lines stripes checker triangles diamonds crosses rings waves zigzag truchet arcs scales bricks`. `shape` may be omitted. |
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

Video clips are decoded once (via ffmpeg) at the output frame rate and at the layer's size, so playback costs the same as an image. `radius` rounds the corners of images and videos.
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

`{ "component": "bar-chart", "props": { "values": [3, 7, 5, 9], "labels": ["Q1", "Q2", "Q3", "Q4"] } }` — a ready-made block: `code`, `terminal`, `bar-chart`, `line-chart`, `area-chart`, `donut-chart`, `pie-chart`, `counter`, `stat`, `phone`, `laptop`, `browser`, `card`, `button`, `toggle`, `badge`, `avatar`, `avatar-stack`, `progress-bar`, `progress-ring`, `toast`, `cursor`, `tooltip`, `callout`, `arrow`, `lower-third`, `title-card`, backgrounds (`mesh-gradient`, `aurora`, `grain`, `grid`, `dots`, `light-leak`, `bokeh`, `starfield`, `gradient`), particles (`confetti`, `sparks`, `snow`, `dust`), `model` (3D) and `lottie`. camelCase names (`barChart`) work too. `anikit-video info --components` prints every kind with a minimal example of its `props`.

The component draws into its box: `width` × `height` (default: the whole parent), placed like any layer (`x`, `y`, `anchor`). Its own animation runs on the layer's local time (from `start`); the layer's transforms, `opacity`, `enter`/`exit` and `animate` apply on top. Components use the theme's colours (`primary` is their accent).

### Finishing: `post` and `motionBlur`

`"post"` (video level, or per scene; a scene's value overrides, `"none"` turns it off) applies a finishing look after everything is drawn: a preset `clean`, `cinematic`, `dreamy`, `punchy`, `noir`, `vintage`, or an object with a preset and overrides, `{ "preset": "cinematic", "bloom": 0.4, "vignette": 0.3, "grain": 0.02 }`. Settings: `bloom bloomThreshold bloomKnee bloomRadius bloomTint halation aberration distortion exposure temperature tint lift gamma gain contrast saturation tonemap vignette vignetteSoftness grain grainSize dither`. Default: none.

`"motionBlur"`: `true` (180° shutter), `false`, or `{ "shutter": 90 }` (degrees). Moving frames are rendered as several sub-frames averaged in linear light; still frames cost nothing extra. Default: on with `--quality high`, off otherwise (it costs several renders per moving frame), never in drafts.

### `shader`

A fragment shader, run JIT-compiled on the CPU at full resolution. Full guide: [`shader.md`](shader.md).

```json
{ "shader": "void mainImage(out vec4 fragColor, in vec2 fragCoord) { … }",
  "uniforms": { "speed": 1.5, "tint": "#ff8800", "wobble": "=sin(t)" },
  "channels": [ "below", "noise", "photo.jpg", "#dashboard", { "layers": [ … ], "wrap": "repeat" } ],
  "animate": [ { "to": { "speed": 3 }, "duration": 2 } ] }
```

| Key | Meaning |
|---|---|
| `shader` | GLSL with `void mainImage(out vec4 fragColor, in vec2 fragCoord)`, WGSL with `fn mainImage(fragCoord: vec2f) -> vec4f`, or a built-in name (`anikit-video info --shaders`). `src: "fx.glsl"` loads a file. |
| `uniforms` | name → number, `[x, y(, z, w)]`, colour or `"=expr"`. Declared with `uniform float name; // = default` in the source, or declared for you. They animate by name in `animate` (up to 12 floats per layer). |
| `channels` | `iChannel0..3`: `"below"` (everything drawn so far), `"noise"`, an image/video path, `"#name"` or `{"composition": "name", "trimStart", "speed", "loop"}`, or inline `{"layers": [...]}`. Options: `wrap` (`clamp` `repeat` `mirror`), `filter` (`linear` `nearest`), `mipmaps`. |
| `filter` | `true`: `iChannel0` defaults to `"below"` and the output replaces the whole frame. |
| `supersample` | anti-aliasing: adaptive 2×2 where neighbouring pixels differ. On by default (off for filters and for sources with `// anikit:aa off`); `false` is up to 2× faster. |
| `alpha` | use `fragColor.a` as coverage (default opaque, like Shadertoy). |
| `buffers` | Shadertoy-style passes `{"A": {"shader"\|"src", "channels"}, …"D"}`, rendered in order before the image at float precision. Channel `"A"` reads buffer A: this frame's if A ran earlier, else last frame's (feedback). Buffers see the spec's uniform values, not animated ones. |

Inputs: `fragCoord` (pixel centre, y up), `iResolution`, `iTime` (seconds since the layer started), `iFrame`, `iTimeDelta`, `iFrameRate`, `iChannelResolution[4]`, `iMouse`/`iDate` (zero / fixed), `iAudio` (music level, bass, mid, treble) and `iBeat` (pulse, phase, beat number, bar pulse). The layer fills the frame unless `x y width height` say otherwise. `opacity`, `blend`, masks and presets apply as for an image.

Shader transitions use the gl-transitions API (`vec4 transition(vec2 uv)` with `getFromColor`, `getToColor`, `progress`, `ratio`): `"transition": "burn"` or `{ "type": "shader", "shader": "…", "uniforms": { … } }`.

Errors give the line and column inside your shader and suggest names:

```
error: scenes[0].layers[1].shader: shader: line 3:12: Unknown function 'smoothStep' (did you mean `smoothstep`?)
error: scenes[0].layers[1].animate[0].to.intensty: 'intensty' is not animatable (this shader's uniforms: speed, intensity), did you mean 'intensity'?
warning: scenes[0].layers[1].channels: the shader samples iChannel1 but no channel 1 is given (it reads as black)
```

### `composition`

A 2D sub-composition: a small video of its own, defined once at the top level and placed like an image.

```json
"compositions": {
  "dashboard": { "size": [800, 500], "duration": 4, "background": "$surface", "loop": true,
                 "layers": [ { "component": "line-chart", "props": { "values": [2, 5, 3, 8] } } ] }
},
"scenes": [ { "layers": [ { "composition": "dashboard", "width": 1200, "rotation": -4, "enter": "rise" } ] } ]
```

A composition has `size` (default: the video size), `duration` (default: its layers' animation, or the sum of its `scenes`), `background` (default transparent), `layers` or `scenes` (with `transition`), and `loop`. It uses the video's fps, theme and fonts, and may place other compositions (no cycles).

The layer draws it into its box with `fit` (default `contain`), clipped to the composition's bounds. Its clock is the layer's local time: composition time = `trimStart` + local × `speed`. Past the end it holds the last frame, or wraps with `"loop": true` (on the layer, or as the composition's default). Everything stays vector: it is as sharp as drawing the layers directly. 3D layers use the same compositions as textures (`Evaluator::composition(name)` gives its evaluator, and `CompTime::map` converts the clock).

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
- `keyframes`: `{ prop: [v0, v1, …] }`, evenly spaced over `duration`, or `{ prop: [[time, value], …] }` with times in 0..1 of the tween. `ease` applies between each pair; a third item sets the ease of the move *into* that key: `[[0, 200], [0.5, 960, "outBack"], [1, 1700, [0.9, 0, 0.1, 1]]]`.
- `duration` default `0.6`. `ease` default `outCubic`.
- `spring`: instead of `ease` + `duration`: a preset (`gentle`, `wobbly`, `stiff`, `slow`, `molasses`, `snappy`, `bouncy`) or `{ "stiffness": 180, "damping": 12, "mass": 1 }`; duration becomes the settle time.
- `repeat` (count, `-1` = forever) and `yoyo` (bool) repeat a tween.

**Animatable properties:** `x`, `y`, `width`, `height`, `scale`, `scaleX`, `scaleY`, `rotation`, `skewX`, `skewY`, `opacity`, `blur`, `radius`, `fill`/`color` (interpolated in OKLab), `stroke`(colour), `strokeWidth`, `trimStart`, `trimEnd`, `reveal`, `value`, `wipe`, `depth`, `pathOffset`, `dashOffset`, `rangeStart`/`rangeEnd`/`rangeOffset` (text animator); camera: `zoom`, `focus`, `aperture`.

**Easings:** `linear`, `ease`, `easeIn`, `easeOut`, `easeInOut`; Penner names as `in`/`out`/`inOut` + `Sine`, `Quad`, `Cubic`, `Quart`, `Quint`, `Expo`, `Circ`, `Back`, `Elastic`, `Bounce` (e.g. `outCubic`, `inOutBack`; `easeOutCubic` also works); `cubicBezier(0.2, 0, 0, 1)` or an array `"ease": [0.2, 0, 0, 1]`; `steps(4)`; GSAP-style `back.out(1.7)`, `elastic.out(1, 0.3)`; springs `gentle`, `wobbly`, `stiff`, `slow`, `molasses`, `snappy`, `bouncy`.

### 3. Expressions

Any numeric property (static or inside `to`/`from`) can be an expression string beginning with `=`:

```json
{ "shape": "circle", "x": "=960 + 300 * sin(t * 2)", "y": "=540 + 300 * cos(t * 2)", "rotation": "=t * 90" }
```

Variables: `t` (seconds since the layer's start), `T` (seconds since the scene's start), `gt` (global time), `p` (layer progress 0..1), `i` (index within the parent), `n` (sibling count), `w`, `h` (frame size), `fps`, `frame`. Functions: `sin cos tan asin acos atan atan2 abs min max floor ceil round sqrt pow exp log clamp(x, a, b) mix(a, b, f) smoothstep(a, b, x) noise(x) noise(x, y) rand(i) pi`. Expressions are compiled once and are cheap.

### Stagger

On a `group`/`box` (or a `split` text), `"stagger": 0.08` delays each child's `enter` and `animate` by `index × 0.08 s` (exits are not staggered). Object form: `{ "each": 0.08, "from": "start" | "center" | "end" | "random", "ease": "inQuad" }`; or `"total": 0.6` to spread the delays over a fixed time.

### Motion graphics (`anikit-video docs mograph`)

Render-time, per layer (examples: `examples/video/mograph/`):

| Key | Does |
|---|---|
| `repeat: {count, offset: {x, y, rotate, scale, opacity}, stagger, center}` | Repeater: copy *i* moved i×(x, y), rotated i×rotate and scaled scaleⁱ about `center` (default the layer position), opacity +i×opacity, i×stagger s later. A plain number is still the spec-level copy (`$i`). |
| `echo: {count, delay, decay, blend}` | Motion trail: earlier-time copies under the layer (one blended layer for all of them). |
| `path`, `progress`, `orient` | Follow SVG path data (parent px) at constant speed; `progress` 0..1 defaults to linear over the layer's life; `orient: true` (or extra degrees) turns along the tangent. |
| `textPath`, `pathOffset` | Text on a path (layer-local px; never wraps); `pathOffset` slides it (px, animatable). |
| `roundCorners`, `offsetPath`, `puckerBloat`, `zigzag: {size, ridges, smooth}`, `twist`, `wigglePath: {amount, detail, speed, seed}` | Shape modifiers, applied in this order (rects and ellipses become paths). `wigglePath` animates, deterministically. |
| `matte: "id"` or `{layer, mode}` | Track matte through a sibling layer (`alpha`, `alphaInverted`, `luma`, `lumaInverted`); the matte layer is not drawn itself. |
| `dashOffset` | Added to `stroke.dashOffset`; animatable (`"=t*-60"` marches ants). |
| `animator: {by, x, y, rotation, scale, opacity, blur, color, start, end, offset, smooth, order, seed, duration, delay, ease}` | Text animator with a range selector (plain text). Units in `[start, end] + offset` take the props (soft edges `smooth`); with no `offset` the range sweeps away over `duration` (a reveal). `order`: `forward`, `reverse`, `center`, `random` (+`seed`). |

## Audio

```json
"audio": [
  { "src": "music.mp3", "volume": 0.7, "fadeIn": 0.5, "fadeOut": 1.5, "duck": true },
  { "src": "voiceover.wav", "at": 0.5, "voice": true },
  { "sfx": "whoosh", "at": "@drop" },
  { "sfx": "impact", "at": "@drop+0.2", "volume": "-3dB" }
]
```

A cue plays a file (`src`) or a built-in sound effect (`sfx`: `click`, `tick`, `pop`, `whoosh`, `swoosh`, `rise`, `riser`, `drop`, `impact`, `ding`, `chime`, `typing`, `success`; `pitch` and `duration` vary it). Keys: `at` (time, default `0`; inside a scene it's relative to the scene), `volume` (0..1 or `"-6dB"`), `trimStart`, `duration`, `fadeIn`, `fadeOut`, `loop`, `voice` (marks a voice-over), and `duck` (`true`, or a reduction in dB, default 12: this track dips while voice plays).

## Captions

```json
"captions": { "src": "voice.srt" }
"captions": [ { "text": "Meet Anikit Video", "at": 0.5, "end": 2.2 }, { "text": "JSON in, MP4 out", "at": 2.4, "end": 4 } ]
"captions": { "highlightBox": "$accent", "maxWords": 3, "cues": [
  { "text": "", "at": 0.5, "end": 2.4, "words": [["Meet", 0.5, 0.8], ["Anikit", 0.8, 1.3], ["Video", 1.3, 2.0]] } ] }
```

Word-synced captions over every scene, short-form style: words are grouped into pages of `maxWords` (default 4), and the spoken word pops in and lights up in `highlight` (default `$accent`), or sits on a `highlightBox`. `background` puts a pill behind the whole line. Without word timings they're estimated from the text. `src` reads SRT or WebVTT (including VTT word timestamps). Other keys: `color`, `size` (default 6% of the frame's short side), `y` (bottom of the captions, default `90%`), `uppercase`, `upcoming` (`dim`, `hidden` or `show` for words not yet spoken), and `style` (a theme text style's font and weight).

## Reuse: `defs`, `use`, `with`, `repeat`

Repeated structure costs tokens, so a spec can name it once. This is a pure JSON expansion done before anything else (`spec/reuse.rs`). `anikit-video check` reports problems in it with paths like any other key.

```json
{
  "defs": {
    "stat": { "type": "box", "width": 360, "children": [
      { "text": "$label", "style": "label" },
      { "text": "$value", "style": "title", "color": "$accent" } ] }
  },
  "scenes": [
    { "layers": [
      { "use": "stat", "repeat": 3, "x": "=200 + $i * 420", "with": { "label": "Metric $i", "value": "=10 * ($i + 1)" } }
    ] },
    { "repeat": 2, "scenes": [ { "duration": 1, "layers": ["Take $i"] }, { "duration": 1, "layers": ["Cut"] } ] }
  ]
}
```

- **`defs`** (top level): named scenes or layers. A def may itself `use` another def. Cycles are an error.
- **`use`**: allowed anywhere a scene or layer is expected. The other keys on the object deep-merge onto the def: objects merge key by key, while arrays (such as `layers`) and scalars replace.
- **`with`**: values for `"$name"` placeholders in the def. A string that is exactly `"$name"` takes the value's type (number, object, …), and inside a longer string the value is interpolated (`"Metric $i"`). Names that `with` doesn't bind are left alone, so theme tokens such as `"$accent"` keep working.
- **`repeat: N`** on a scene or layer emits N copies (up to 1000), with `"$i"` (0, 1, …) and `"$n"` bound throughout, including in `with`.
- **Arithmetic:** a string starting with `=` is evaluated after substitution, with `+ - * / %` and parentheses: `"x": "=160 + $i * 420"`, `"at": "=0.5 * $i"`.
- **Scene sequences:** in `scenes`, `{ "scenes": [ … ] }` is spliced in place. With `repeat`, it loops a whole run of scenes.
- **Errors:** `error: scenes[0].layers[0].use: unknown def 'stats' (defs: stat), did you mean 'stat'?`

**Saving on the long benchmark** (`bench/video/anikit/long.json`, the 4-scene kinetic sequence played 3×): written as `{ "repeat": 3, "scenes": [ …4 scenes… ] }` ([`examples/long-reuse.json`](examples/long-reuse.json)), it is **2,215 B instead of 6,273 B minified (≈550 instead of ≈1,570 tokens, −65%)**. It expands to an identical spec, checked by `tests/spec_reuse.rs`.

## Validation and messages

`anikit-video check spec.json` prints errors (the video cannot render) and warnings (it renders, but probably not as intended), each with a path:

```
error   scenes[1].layers[0].type: unknown layer type 'txt', did you mean 'text'?
error   scenes[0].layers[2].animate[0].ease: unknown easing 'easeOutQuartic', did you mean 'outQuart'?
warning scenes[0].layers[1].colour: unknown key, did you mean 'color'?
warning scenes[2].layers[3].start: 4.5 s is after the scene ends (4 s); the layer is never visible
```

With `--json`, the output is one compact line: `{ "ok": bool, "errors": ["scenes[1].layers[0].type: unknown layer type 'txt', did you mean 'text'?"], "warnings": [...], "duration": s, "frames": n, "scenes": [{ "id", "start", "end" }] }`.

## CLI

```
anikit-video check  spec.json [--json]                      validate, print errors/warnings and timing
anikit-video still  spec.json --at 2.5 -o f.png [--watch]   one frame (--at @sceneId works too)
anikit-video sheet  spec.json -o sheet.png [--cols 4 --rows 3 --width 480]
anikit-video render spec.json -o out.mp4 [--quality draft|good|best] [--from 2 --to 5] [--scale 0.5]
                    [--codec ak264|h264|h265|vp9|av1|prores|gif|png]
                    default by extension: .mp4 → ak264 (our encoder; x264 with --quality best), .webm vp9, .gif, .mov prores, .mkv av1
anikit-video info [--components]                              encoders and the component catalogue
anikit-video schema [-o spec.schema.json]
```

`--json` prints one machine-readable object on stdout; progress goes to stderr. `render --json` ends with `{ "ok": true, "summary": { "frames", "deduped", "seconds", "render_fps", "bytes", … } }`.

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

- `spec::parse_str` / `parse_json` / `parse_yaml` → `Parsed { spec, warnings }` (unknown keys, with paths and suggestions).
- `eval::Evaluator::new(&spec, Options { base_dir, fonts, load_assets })` compiles once: themes, units, text layout (TEXT's `FontStore`), box layout, presets, stagger and expressions become per-property keyframe tracks. Errors come back as `SpecErrors` (every diagnostic); warnings stay in `evaluator.diagnostics`.
- `eval_into(t, &mut DisplayList)` is pure and cheap (a few µs per frame). `is_static(t0, t1)` is a conservative "nothing changes in this span".
- `evaluator.assets` (`SharedAssets`) holds fonts and images (user images, decoded video frames, component-generated images), implements `components::Assets`, and hands renderers versioned `render::Assets` snapshots.
- `pipeline::render_video(&ev, &RenderOptions, &mut dyn Encoder)`: parallel batches, one `Renderer` per worker thread, dedupe of identical display lists, changed-macroblock hints for ak264, and the encoder on its own thread.
