# Components

High-level blocks for Anikit Video specs: code, charts, devices, UI, backgrounds, particles, 3D, Lottie and transitions. Every component is a pure function of time, so frames render in any order on any core.

```json
{ "type": "component", "component": "barChart", "props": { "values": [3, 7, 5, 9], "labels": ["Q1", "Q2", "Q3", "Q4"] }, "width": 900, "height": 500 }
```

**Names and props:** kind names are written in camelCase (`barChart`) or kebab-case (`bar-chart`). Props are written in camelCase (`fontSize`) or snake_case (`font_size`). An unknown name or prop is an error with a "did you mean" suggestion.

**The box:** a component draws inside its layer's `width × height`, which defaults to the whole frame. The layer's position, transforms, `enter`/`exit` and `opacity` apply on top.

**Sizes** are in output pixels; the defaults are tuned for 1080p.

**Time:** `t` is seconds since the layer started. Most components have their own entrance, controlled by:
- `delay`: when the entrance starts, in seconds;
- `out`: when the exit starts, in seconds (leave it out and the component stays);
- `duration`: how long a build-on lasts;
- `stagger`: the delay between successive items.

**Colours:** any CSS colour (`"#7c6cff"`, `"oklch(70% 0.2 280)"`, `"tomato"`) or an `[r, g, b, a]` array with components from 0 to 1.

**Theme:** components default to the scene theme, a dark launch-video look. Override it per component with `"theme": "light"` or `{ "accent": "#ff5ca8", "palette": [...] }`. The theme keys are `background`, `surface`, `surface2`, `border`, `text`, `muted`, `accent`, `positive`, `negative`, `palette` and `radius`.

**Determinism:** particles and backgrounds take a `seed`. The same seed always gives the same frames.

## Code

### `code`: editor window
The window has syntax highlighting (TypeScript, JavaScript, Rust, Python, Go, Bash, JSON, YAML, HTML, CSS, SQL and more) and a human-rhythm typing animation.

| Prop | Default | Meaning |
|---|---|---|
| `code` | | Source text. |
| `language` | `"typescript"` | Language name or extension. |
| `title` | `""` | File name shown in the title bar. |
| `fontSize` | `26` | |
| `lineNumbers` | `true` | |
| `chrome` | `"mac"` | `mac` (traffic lights), `minimal` or `none`. |
| `typing` | `0` | Characters per second; `0` shows the code at once. |
| `delay` | `0` | When typing starts. |
| `diff` | `false` | Lines starting with `+` or `-` are shown as added or removed. |
| `highlights` | `[]` | `[{ "lines": [3, 4], "range": [6, 9], "at": 1.5, "until": 3, "dim": true }]` |
| `scroll` | `[]` | `[{ "line": 20, "at": 2, "duration": 0.6 }]` scrolls so that `line` is at the top. Typing auto-scrolls on its own. |
| `cursor` | `true` | Blinking caret at the typing position. |
| `background` | theme | Window colour. |
| `glow` | none | A coloured glow behind the window. |

### `terminal`
Lines starting with `$ ` or `> ` are typed. Other lines print one after another, coloured green for success and red for errors.

| Prop | Default | Meaning |
|---|---|---|
| `lines` | | Commands and output. |
| `title` | | |
| `fontSize` | `26` | |
| `typing` | `28` | Characters per second. |
| `pause` | `0.35` | Pause after a command. |
| `lineDelay` | `0.06` | Delay between output lines. |
| `delay` | `0` | |
| `promptColor` | theme | |

## Charts

Charts build on with springs.

### `barChart`
| Prop | Default | Meaning |
|---|---|---|
| `values` | | |
| `labels` | | |
| `colors` | accent | One colour per bar, or one colour for all. |
| `multicolor` | `false` | Use the palette. |
| `horizontal` | `false` | |
| `max` | nice | Axis maximum. |
| `highlight` | none | Index of a bar to emphasise; the others dim. |
| `valuesShown` | `true` | |
| `grid` | `true` | |
| `barWidth` | `0.62` | |
| `fontSize` | `22` | |
| `format` | | `{ "prefix": "$", "suffix": "%", "decimals": 1, "compact": true }` |
| `delay`, `duration`, `stagger` | | |

### `lineChart` and `areaChart`
| Prop | Default | Meaning |
|---|---|---|
| `series` | | `[[...], [...]]`, one array per line. |
| `names` | | Legend entries. |
| `labels` | | X-axis labels. |
| `colors` | palette | |
| `area` | | `true` by default for `areaChart`. |
| `dots` | `false` | |
| `endLabel` | `true` | A value bubble that rides the head of the line. |
| `smooth` | `true` | |
| `min`, `max` | | |
| `grid` | `true` | |
| `width` | `4` | Line width. |
| `format`, `fontSize`, `delay`, `duration`, `stagger` | | |

### `donutChart` and `pieChart`
| Prop | Default | Meaning |
|---|---|---|
| `values`, `labels`, `colors` | | |
| `hole` | `0.62` (`0` for a pie) | |
| `gap` | `1.5` | Degrees between segments. |
| `center` | the total | Centre text. |
| `centerLabel` | | Caption under the centre text. |
| `legend` | `true` | |
| `percents` | `true` | |
| `format`, `delay`, `duration` | | |

### `counter`: big counting number
| Prop | Default | Meaning |
|---|---|---|
| `from`, `to` | | |
| `prefix`, `suffix` | | |
| `decimals` | | |
| `compact` | `false` | Abbreviations: 12K, 3.4M. |
| `separator` | `true` | Thousands separators. |
| `size` | `160` | |
| `weight` | `700` | |
| `color` | | |
| `gradient` | | `[top, bottom]` |
| `align` | `0.5` | 0 left, 1 right. |
| `label` | | Caption. |
| `delay`, `duration` | | |

Digits are tabular, and the width is reserved for the final value, so the number never jitters.

### `stat`: stat tile
| Prop | Meaning |
|---|---|
| `label`, `value`, `prefix`, `suffix`, `decimals`, `compact` | |
| `delta` | Percentage change; shown as ▲ in green or ▼ in red. |
| `spark` | Sparkline values. |
| `color` | |
| `delay`, `duration` | |

## Devices

Devices keep their aspect ratio and are centred in the box. Screen props are shared by all of them:
- `src`: an image or video;
- `colors`: a placeholder gradient with skeleton UI, used when there is no `src`;
- `scrollTo` (0 to 1), `scrollAt` and `scrollDuration`: scroll a tall screenshot;
- `delay`: spring entrance;
- `float`: bobbing amplitude in px.

| Kind | Extra props |
|---|---|
| `phone` | `color` (frame), `reflection` (0–1), `shadow`, `time` (status bar). |
| `laptop` | `color`, `reflection`, `shadow`, `open` (seconds for the lid to open). |
| `browser` | `url`, `tabs` (titles; the first is active), `typeUrl` (seconds to type the URL), `shadow`, `aspect` (default 1.6). |

## UI kit

| Kind | Props |
|---|---|
| `card` | `title`, `body`, `icon` (`spark`, `bolt`, `check`, `plus`, `arrow`, `info`, `warning`, `x`), `accent`, `glow`, `delay`, `out` |
| `button` | `label`, `variant` (`primary`, `secondary`, `ghost`, `gradient`), `size`, `color`, `press` (times), `arrow`, `delay`, `out` |
| `toggle` | `at` (flip time), `on` (initial state), `size`, `color`, `label` |
| `badge` | `label`, `color`, `size`, `dot` (pulsing "live" dot), `solid`, `delay`, `out` |
| `avatar` | `name` (gives the initials and colour), `src`, `size`, `status` (`online`, `away`, `busy`), `delay` |
| `avatarStack` | `names`, `srcs`, `size`, `overlap`, `max` (then "+N"), `delay`, `stagger` |
| `progressBar` | `from`, `to`, `duration`, `delay`, `thickness`, `color`, `label`, `percent` |
| `progressRing` | `from`, `to`, `duration`, `delay`, `size`, `thickness`, `color`, `label` |
| `toast` | `title`, `body`, `kind` (`success`, `info`, `warning`, `error`), `delay`, `hold`, `from` (`top`, `bottom`, `right`), `width` |
| `cursor` | `path` (`[[x, y], ...]` waypoints in box px), `times` (arrival time of each), `clicks` (times; each gives a ripple), `size`, `style` (`arrow` or `hand`), `ripple` (colour) |
| `tooltip` | `text`, `at` (`[x, y]` arrow tip), `side` (`top` or `bottom`), `size`, `delay`, `out` |
| `callout` | `from` (the annotated point), `to` (label position), `text`, `color`, `size`, `delay`, `out` |
| `arrow` | `from`, `to`, `bend` (−1 to 1), `color`, `width`, `dashed`, `delay`, `duration` |
| `lowerThird` | `title`, `subtitle`, `accent`, `size`, `align` (`left` or `center`), `margin`, `delay`, `out` |
| `titleCard` | `title`, `eyebrow`, `subtitle`, `size`, `gradient` (`[from, to]`), `reveal` (`word`, `line`, `fade`), `stagger`, `align`, `delay`, `out` |

## Content blocks

| Kind | Props |
|---|---|
| `chat` | `messages` (`[{ "from": "user" \| "assistant" \| name, "text", "at"? }]`), `fontSize`, `stream` (words per second for replies), `thinking` (seconds of typing dots), `gap`, `delay`, `maxWidth`. Replies stream word by word; the thread scrolls up. |
| `prompt` | `text`, `placeholder`, `typing` (characters per second), `delay`, `sendAt`, `fontSize`, `glow` (an animated gradient rim). It types, presses send and clears. |
| `checklist` | `items`, `stagger`, `checkAfter`, `fontSize`, `color`, `strike` (to-do style), `delay` |
| `quote` | `text`, `name`, `title`, `avatar` (src), `stars` (0–5), `reveal` (words per second), `fontSize`, `delay` |
| `table` | `columns`, `rows` (cells are text, or `true`/`false`/`"✓"`/`"✗"` for icons), `highlight` (index of the column to emphasise), `fontSize`, `delay`, `stagger` |
| `timeline` | `items` (`[{ "label", "title", "body" }]`), `current` (pulses; later items dim), `vertical`, `fontSize`, `step` (seconds per milestone), `delay` |
| `spotlight` | `targets` (`[[x, y, w, h], ...]` in box px), `times`, `dim` (0–1), `radius`, `padding`, `color`, `delay`, `out`. Everything outside the focus darkens; the focus glides between targets. |
| `keys` | `keys` (e.g. `["⌘", "K"]`), `press` (times), `size`, `delay`, `out` |
| `marquee` | `items` (words, or image srcs with `images: true`), `speed` (px/s), `fontSize`, `gap`, `separator`, `fade`, `color` |

## Backgrounds

Place backgrounds first in a scene's layers.

| Kind | Props |
|---|---|
| `meshGradient` | `colors` (3–6 blobs), `base`, `speed`, `warp` (0 gives round blobs, 1 gives swirls), `intensity` (default 0.75), `seed`, `detail`, `rate` (field keyframes per second, blended in between; default 12) |
| `aurora` | `colors` (ribbons), `sky` (`[top, horizon]`), `speed`, `intensity`, `seed`, `rate` (default 15) |
| `gradient` | `colors`, `kind` (`linear` or `radial`), `angle`, `spin` (degrees per second) |
| `grain` | `amount` (0.03–0.1 is subtle), `size`, `rate` (new patterns per second), `seed`. Put it last, as an overlay. |
| `grid` | `cell`, `color`, `lineWidth`, `fade`, `perspective` (receding floor), `horizon`, `speed` (scroll), `reveal` (seconds to draw on) |
| `dots` | `spacing`, `radius`, `color`, `fade`, `wave` (a travelling highlight) |
| `lightLeak` | `colors`, `intensity`, `speed`, `seed` |
| `bokeh` | `count`, `colors`, `minSize`, `maxSize`, `speed`, `seed` |
| `starfield` | `count`, `color`, `fly` (warp speed), `speed`, `seed` |

## Particles

| Kind | Props |
|---|---|
| `confetti` | `mode` (`burst`, `cannons` or `rain`), `at` (fire time), `origin` (`[fx, fy]` as fractions of the box), `count`, `colors`, `power`, `gravity`, `size`, `life`, `seed` |
| `sparks` | `bursts` (times), `origin`, `direction` and `spread` (degrees), `count`, `colors` (`[hot, cool]`), `power`, `gravity`, `life`, `glow`, `seed` |
| `snow` | `count`, `color`, `speed`, `wind`, `size`, `seed` |
| `dust` | `count`, `color`, `size`, `speed`, `glow`, `seed` |

### `particleText`: words made of particles
Thousands of glowing particles fly in from all around, form the text, hold with a soft shimmer, re-form into the next word, then blow away. Points are sampled from the real glyph outlines, so any phrase works.

```json
{ "type": "component", "component": "particleText", "props": { "words": ["Hello", "World"] } }
```

| Prop | Default | Meaning |
|---|---|---|
| `text` | `"Hello"` | The word or phrase (or use `words`). |
| `words` | | Several words; the particles re-form from one into the next. |
| `count` | `4000` | Number of particles. |
| `size` | `2.4` | Particle radius in px at 1080p. |
| `colors` | theme | Gradient across the text, left to right. |
| `fontSize` | fit | Default fits about 80% of the width and at most 30% of the height. |
| `weight`, `face` | `800`, `display` | |
| `form` | `1.6` | Seconds to fly in and to re-form. |
| `hold` | `1.4` | Seconds each word holds. |
| `exit` | `"blow"` | `blow` (wind), `explode`, `fade`, or `none` to stay. |
| `exitTime` | `1.6` | |
| `delay`, `glow`, `seed` | `0`, `true`, `1` | |

## Shapes and light

### `morph`: shape morphing
A gradient-filled shape that morphs smoothly through a list of named shapes or SVG paths, with a little squash, turn and a glossy top light.

```json
{ "type": "component", "component": "morph", "props": { "shapes": ["circle", "star", "heart"] } }
```

| Prop | Default | Meaning |
|---|---|---|
| `shapes` | circle, star, heart, blob | Names (`circle`, `square`, `star`, `heart`, `blob`, `triangle`, `hexagon`, `diamond`) or SVG path data (`"M0,0 L10,0 L5,8 Z"`). Every shape is centred and scaled to a similar visual size. |
| `size` | `0.62` | Fraction of the box's shorter side. |
| `colors` | theme | Fill gradient (2–3 colours). |
| `duration` | `0.9` | Seconds per morph. |
| `hold` | `0.7` | Seconds each shape holds. |
| `loop` | `true` | Morph back to the first shape and repeat. |
| `turn` | `0` | Degrees each morph rotates the shape. |
| `glow`, `stroke`, `delay`, `seed` | `true`, none, `0`, `1` | `seed` varies the `blob` shape. |

### `metaballs`: liquid blobs
Gooey blobs drift, merge and split. The edge is an iso-contour extracted as a vector path each frame, so it stays crisp at any resolution.

| Prop | Default | Meaning |
|---|---|---|
| `count` | `9` | Number of blobs. |
| `colors` | theme | Gradient (2–4 colours); it drifts slowly. |
| `size` | `0.085` | Blob radius as a fraction of the box height. |
| `spread` | `1` | How far the blobs wander. |
| `speed` | `1` | |
| `glow`, `gloss` | `true` | Coloured bloom; soft specular light. |
| `detail` | `12` | Grid cell in px at 1080p (smaller is smoother). |
| `seed` | `3` | |

### `godRays`: volumetric light
Soft beams fan out from a light source, slowly turning and breathing, with dust motes that light up inside the beams. Screen-blended: place it above the background.

| Prop | Default | Meaning |
|---|---|---|
| `at` | `[0.32, -0.08]` | Source position as fractions of the box (may be outside it). |
| `color` | warm white | |
| `angle` | `62` | Direction of the fan in degrees (90 points straight down). |
| `spread` | `70` | Fan width in degrees (360 for all around). |
| `count` | `14` | Number of beams. |
| `length` | `1.15` | Beam length as a fraction of the box diagonal. |
| `intensity` | `0.8` | |
| `spin` | `1.5` | Fan rotation in degrees per second. |
| `dust` | `120` | Number of dust motes (0 for none). |
| `seed` | `4` | |

### `lensFlare`
An anamorphic flare: hot core, halo, faint rainbow ring, a blue horizontal streak, and ghosts along the line from the light through the frame centre. The ghosts slide as the light moves.

```json
{ "type": "component", "component": "lensFlare", "props": { "path": [[0.15, 0.25], [0.85, 0.35]], "times": [0, 3] } }
```

| Prop | Default | Meaning |
|---|---|---|
| `at` | `[0.3, 0.3]` | Light position as fractions of the box. |
| `path` | | Positions to travel through (fractions); overrides `at`. |
| `times` | even | Arrival time of each `path` point, in seconds. |
| `duration` | `4` | Travel time over the whole `path` when `times` is omitted. |
| `color` | warm | Core and halo tint. |
| `streakColor` | blue | |
| `streak` | `1` | Streak length as a fraction of the box width (0 for none). |
| `ghosts` | `7` | Number of ghosts (0 for none). |
| `intensity`, `seed` | `1`, `2` | |

## 3D

### `model`: product spin and orbit
Rendered with `anikit-core`'s CPU rasteriser at native resolution, supersampled 2× for anti-aliasing, with studio lighting and a transparent background.

| Prop | Default | Meaning |
|---|---|---|
| `shape` | `roundedBox` | `roundedBox`, `sphere`, `torus`, `knot`, `cylinder`, `cube` or `logo`. |
| `points`, `depth` | lightning bolt | For `logo`: the polygon outline and its extrusion depth. |
| `obj` | | Inline Wavefront OBJ text, used instead of `shape`. |
| `size`, `radius` | | Rounded-box proportions and corner radius. |
| `color` | accent | |
| `metallic` | `0.3` | |
| `roughness` | `0.3` | |
| `spin` | `28` | Degrees per second about Y. |
| `angle` | `-30` | Start angle. |
| `tilt` | `16` | Degrees. |
| `orbit` | `0` | Camera orbit, in degrees per second. |
| `float` | `0.035` | Bobbing amount. |
| `scale` | `1` | |
| `delay` | | Spring entrance. |
| `shadow` | `0.6` | Contact shadow (a number or a bool). |
| `resolution` | `1` | Capped at 2048 px. |

**Cost** per frame at 1280×720 on 4 cores: 5–23 ms.

## Lottie

### `lottie`
| Prop | Meaning |
|---|---|
| `src` | Path to a `.json` file. |
| `data` | An inline Lottie object, used instead of `src`. |
| `speed` | |
| `loop` | |
| `fit` | `contain`, `cover` or `none`. |
| `delay` | |
| `color` | Recolours everything. |

**Supported:** shape, null, solid and precomp layers; parenting; transforms; rect, ellipse, path and star shapes; fill and stroke (with dashes); linear and radial gradients; trim paths; bezier and hold keyframes; spatial position curves.

**Skipped:** masks, mattes, effects, text, image layers, repeaters and merge paths. Skipped features are reported in diagnostics.

## Transitions

Scene transitions: `"transition": { "type": "wipe", "duration": 0.6, "direction": "left", "edge": "#fff" }`.

**Types:** `fade`, `wipe` (`direction` or `angle`, optional glowing `edge`), `slide`, `push`, `zoom`, `iris` (`center`), `glitch`, `blur`, `whip` and `lightLeak`.

**Overlay components** cover a cut:

| Kind | Effect | Props |
|---|---|---|
| `wipe` | Layered colour panels sweep across, hold, then sweep away. Cut your scenes inside the `hold` window. | `at`, `duration`, `hold`, `colors`, `direction`, `slant` |
| `iris` | The iris closes and reopens. | `at`, `duration`, `hold`, `color`, `center` |
| `glitch` | Glitch bars. | `at`, `duration`, `intensity`, `colors`, `seed` |

## Audio (`src/audio/`)

The audio module implements the spec's `audio` tracks:

```json
"audio": [
  { "src": "music.mp3", "fadeIn": 1, "fadeOut": 2, "loop": true, "duck": 0.4 },
  { "src": "voiceover.wav", "at": 1.5, "voice": true },
  { "src": "sfx:whoosh", "at": 3.0, "volume": "-6dB" },
  { "src": { "sfx": "typing", "params": { "duration": 2 } }, "at": 4 }
]
```

**Track keys:**
- `at` (or `start`), `trimStart` (or `trim: [in, out]`), `duration`;
- `volume`: linear, or `"-6dB"`; `db` adds gain;
- `fadeIn`, `fadeOut` (equal-power), `loop`;
- `voice: true` (or `role`: `music`, `voice` or `sfx`);
- `duck`: the level to duck to (0–1), `true`, or `{ under, amountDb, attack, release }`.

Music ducks smoothly under voice tracks. The master bus has a soft limiter.

**Built-in SFX** (`"sfx:<name>"`) are synthesised procedurally, so no asset files are needed: `click`, `tick`, `pop`, `whoosh`, `swoosh`, `rise`, `drop`/`impact`, `ding`, `chime`, `typing`, `success`.

**Music analysis** (`audio::analyze_pcm`) gives `bpm`, `beats[]`, `downbeats[]`, `onsets[]` and `energy` (100 values per second). Use it to snap cuts and animations to the beat.
