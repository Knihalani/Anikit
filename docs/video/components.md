# Components

High-level blocks for Anikit Video specs: code, charts, devices, UI, backgrounds, particles, media and motion graphics, 3D, Lottie and transitions. Every component is a pure function of time, so frames render in any order on any core.

```json
{ "type": "component", "component": "barChart", "props": { "values": [3, 7, 5, 9], "labels": ["Q1", "Q2", "Q3", "Q4"] }, "width": 900, "height": 500 }
```

**Shorthands (fewest tokens):**
- Dotted kind names: `chart.bar`, `chart.line`, `chart.area`, `chart.donut`, `chart.pie`, `bg.mesh`, `bg.aurora`, `bg.grid`, `bg.stars`, `device.phone`, `device.laptop`, `device.browser`, `ui.toast`, `ui.cursor`, `fx.confetti`, `fx.sparks`.
- Plain words also work, for example `bar`, `line`, `donut`, `mesh`, `title`, `editor`, `testimonial`, `ticker`.
- `data` is the main data of any component: chart values, line series (a flat array is one series), a counter's target, chat messages, checklist items and table rows.
- `text` is the main text: a title, a label, or the code.
- So `{ "component": "chart.bar", "props": { "data": [3, 7, 5] } }` is a complete chart.

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
- `screen`: a generated app screen, used when there is no `src`. It is one of `app` (neutral: the product name and its items, no invented numbers or names), `dashboard` (KPI tiles, an area chart and a list), `chat`, `feed`, `editor` or `settings`. Add `title` for the product name and `accent` for the colour. `items` (the product's features or changelog entries) fill the sidebar, list and card titles, chat lines and settings rows. `kpis` (`[{"label": "Notes", "value": "12,400"}]`) replace the dashboard numbers, which count up. Screens lay out for portrait or landscape and build in with a stagger. Example: `{ "component": "device.phone", "props": { "screen": "dashboard", "title": "Northwind" } }`.

| Kind | Extra props |
|---|---|
| `phone` | `color` (frame), `reflection` (0–1), `shadow`, `time` (status bar). |
| `laptop` | `color`, `reflection`, `shadow`, `open` (seconds for the lid to open). |
| `browser` | `url`, `tabs` (titles; the first is active), `typeUrl` (seconds to type the URL), `shadow`, `aspect` (default 1.6). |

## UI kit

| Kind | Props |
|---|---|
| `card` | `title`, `body`, `fontSize` (default scales with the card), `icon` (`spark`, `bolt`, `check`, `plus`, `arrow`, `info`, `warning`, `x`), `accent`, `glow`, `delay`, `out` |
| `button` | `align` (0 left, 0.5 centre (default), 1 right; vertically centred in the box), `labelColor` (default: white or near-black by WCAG contrast with the fill), `label`, `variant` (`primary`, `secondary`, `ghost`, `gradient`), `size`, `color`, `press` (times), `arrow`, `delay`, `out` |
| `toggle` | `at` (flip time), `on` (initial state), `size`, `color`, `label` |
| `badge` | `align` (as `button`), `labelColor` (default: by contrast), `label`, `color`, `size`, `dot` (pulsing "live" dot), `solid`, `delay`, `out` |
| `avatar` | `align`, `name` (gives the initials and colour), `src`, `size`, `status` (`online`, `away`, `busy`), `delay` |
| `avatarStack` | `align`, `names`, `srcs`, `size`, `overlap`, `max` (then "+N"), `delay`, `stagger` |
| `progressBar` | `from`, `to`, `duration`, `delay`, `thickness`, `color`, `label`, `percent` |
| `progressRing` | `percent` (false hides the number, e.g. behind a countdown), `align`, `from`, `to`, `duration`, `delay`, `size`, `thickness`, `color`, `label` |
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
| `checklist` | `align` (the list is centred as a block by default), `items`, `stagger`, `checkAfter`, `fontSize`, `color`, `strike` (to-do style), `delay` |
| `quote` | `text`, `name`, `title`, `avatar` (src), `stars` (0–5), `reveal` (words per second), `fontSize`, `delay` |
| `table` | `columns`, `rows` (cells are text, or `true`/`false`/`"✓"`/`"✗"` for icons), `highlight` (index of the column to emphasise), `fontSize`, `delay`, `stagger` |
| `timeline` | `items` (`[{ "label", "title", "body" }]`), `current` (pulses; later items dim), `vertical`, `fontSize`, `step` (seconds per milestone), `delay` |
| `spotlight` | `targets` (`[[x, y, w, h], ...]` in box px), `times`, `dim` (0–1), `radius`, `padding`, `color`, `delay`, `out`. Everything outside the focus darkens; the focus glides between targets. |
| `keys` | `align` (as `button`), `keys` (e.g. `["⌘", "K"]`), `press` (times), `size`, `delay`, `out` |
| `marquee` | `items` (words, or image srcs with `images: true`), `speed` (px/s), `fontSize`, `gap`, `separator`, `fade`, `color` |

## Backgrounds

Place backgrounds first in a scene's layers.

| Kind | Props |
|---|---|
| `meshGradient` | `colors` (3–6 blobs), `base`, `speed`, `warp` (0 gives round blobs, 1 gives swirls), `intensity` (default 0.75), `seed`, `detail`, `rate` (field updates per second, held in between so frames repeat; default 8), `smooth` (blend between updates instead) |
| `aurora` | `colors` (ribbons), `sky` (`[top, horizon]`), `speed`, `intensity`, `seed`, `rate` (default 12, held), `smooth` |
| `gradient` | `colors`, `kind` (`linear` or `radial`), `angle`, `spin` (degrees per second) |
| `grain` | `amount` (0.03–0.1 is subtle), `size`, `rate` (new patterns per second; default 0 = static, which is free to encode), `seed`. Put it last, as an overlay. |
| `grid` | `cell`, `color`, `lineWidth`, `fade`, `perspective` (receding floor), `horizon`, `speed` (scroll), `reveal` (seconds to draw on) |
| `dots` | `spacing`, `radius`, `color`, `fade`, `wave` (a travelling highlight) |
| `lightLeak` | `colors`, `intensity`, `speed`, `seed`, `rate` (updates per second, held; default 10) |
| `bokeh` | `count`, `colors`, `minSize`, `maxSize`, `speed`, `seed` |
| `starfield` | `count`, `color`, `fly` (warp speed), `speed`, `seed` |

## Particles

Every particle has a depth: far ones are small and dim, near ones large and out of focus (soft bokeh), so a burst reads as a 3D volume. Fast confetti smears along its motion; sparks are tapered trails whose colour follows black-body temperature as they cool. Colours default to neutral light (snow, dust, sparks) or the theme palette (confetti); on light themes, snow and dust use the muted text colour.

| Kind | Props |
|---|---|
| `confetti` | `mode` (`burst`, `cannons` or `rain`), `at` (fire time), `origin` (`[fx, fy]` as fractions of the box), `count` (`180`), `colors` (theme palette), `material` (`paper`: matte with a soft sheen; `foil`: metallic flashes; `mixed`), `shape` (`mixed`, `strips`, `squares`, `circles`), `power`, `gravity`, `size` (`22`), `life`, `spin` (tumble speed), `depth` (`1`; `0` = flat and sharp), `blur` (motion smear, `1`), `seed` |
| `sparks` | `bursts` (times), `origin`, `direction` and `spread` (degrees), `count` (`140`), `temperature` (`4300` K; `6500` white-hot, `2500` embers), `colors` (`[hot, cool]`, overrides the temperature), `streak` (trail length, `1.6`), `power`, `gravity`, `life`, `glow`, `flash` (origin flash), `depth`, `seed` |
| `snow` | `count` (`320`), `color`, `speed`, `wind`, `turbulence` (`1`), `size`, `shape` (`soft` or `crystal`: six-armed flakes in the focus plane), `depth`, `seed` |
| `dust` | `count`, `color`, `look` (`motes`: fine specks with a few bokeh discs; `bokeh`: big soft discs; `fireflies`: pulsing glowing points), `size`, `speed`, `glow`, `depth`, `seed` |

Two different looks per kind:
- `confetti`: `{ "material": "foil", "colors": ["#f4f1ea", "#e9c46a", "#d4a373"], "shape": "strips" }` (gold-and-cream foil streamers) vs `{ "mode": "cannons", "shape": "circles", "depth": 0, "blur": 0 }` (flat, crisp, playful).
- `sparks`: `{ "temperature": 6500, "streak": 3, "spread": 50, "direction": -90 }` (a white-hot fountain with long trails) vs `{ "temperature": 2200, "streak": 0.6, "gravity": 150, "life": 2.5 }` (slow drifting embers).
- `snow`: `{ "shape": "crystal", "count": 200, "wind": 0 }` (still, crystalline) vs `{ "count": 900, "speed": 260, "wind": 160, "turbulence": 2.5 }` (a blizzard).
- `dust`: `{ "look": "bokeh", "count": 40 }` vs `{ "look": "fireflies", "count": 60 }`.

## Media and motion graphics

### `visualizer`: audio visualiser
```json
{ "component": "visualizer", "props": { "src": "music.mp3", "style": "circle", "image": "logo.png", "beats": true } }
```
The audio is decoded once and analysed with an FFT at 60 Hz into a small table of log-spaced band levels (smoothed, with peak-hold), so frames only look values up. **Without a readable `src`** it analyses a built-in synthetic groove (120 BPM, looping every 8 s), so previews still move; check the path if the bars look like a generic beat.

| Prop | Default | Meaning |
|---|---|---|
| `src` | synthetic | Audio file (mp3, wav, ogg, flac, m4a). |
| `style` | `bars` | `bars` (rounded gradient bars with peak caps and a reflection), `wave` (smooth mirrored waveform, lows in the middle) or `circle` (radial bars around a ring). |
| `bands` | 48 / 32 / 96 | Number of bars (bars / wave / circle). |
| `colors` | accent → pink → cyan | Gradient, low → high. |
| `beats` | `false` | Pulse (glow, ring, scale) on detected beats. |
| `smoothing` | `0.6` | How slowly levels fall (0–1). |
| `gain` | `1` | Level multiplier. |
| `peaks` | `true` | Peak-hold caps (`bars`, not mirrored). |
| `mirror` | `false` | `bars`: grow both ways from the centre line. |
| `reflection` | `0.25` | `bars`: faded reflection below. |
| `gap` | `0.38` | Gap between bars, as a fraction of each slot. |
| `radius` | `0.42` | `circle`: ring radius as a fraction of the half-size. |
| `image` | | `circle`: image (avatar, logo) cover-fitted inside the ring. |
| `line` | `3` | `wave`: line width. |
| `offset` | `0` | Seconds into the audio at the layer's start. |
| `delay` | `0` | The bars grow in from here. |

Put the same file in the spec's `audio` so you hear what you see; use `offset` when the audio track is trimmed.

### `kenBurns`: photo with slow zoom and pan
```json
{ "component": "kenBurns", "props": { "src": "beach.jpg", "motion": "left" } }
```
| Prop | Default | Meaning |
|---|---|---|
| `src` | placeholder | Photo, cover-fitted. Without it, a generated dusk landscape. |
| `motion` | `in` | `in`, `out`, `left`, `right`, `up` or `down` (direction the camera moves). |
| `zoom` | `1.2` | Zoom amount for the presets. |
| `from`, `to` | | `[x, y, w, h]` fractions of the image to frame at the start and end (override `motion`). |
| `duration` | `8` | Seconds for the whole move. |
| `delay` | `0` | |
| `layers` | `[]` | Parallax cut-outs `[{ "src": "fg.png", "depth": 1 }]`, cover-fitted like the photo; `depth` 0 moves with it, 1 twice as much. |
| `vignette` | `0.4` | Corner darkening (0–1). |
| `film` | `0.3` | Warm highlights, teal shadows, lifted blacks (0–1). |
| `colors` | | Placeholder sky `[top, horizon]`. |

### `tileReveal`: tiles that reveal an image
```json
{ "component": "tileReveal", "props": { "src": "shot.png", "wave": "radial", "out": 3 } }
```
| Prop | Default | Meaning |
|---|---|---|
| `src` | | Image to reveal (cover-fitted across the grid); without it a gradient of `colors`. |
| `colors` | accent → pink → cyan | |
| `cols`, `rows` | `10`, from the aspect | |
| `effect` | `flip` | `flip` (turns over from a tinted back face), `scale` (pops with a twist) or `fade` (rises and fades). |
| `wave` | `diagonal` | Order: `diagonal`, `radial`, `random`, `left`, `right`, `top` or `bottom`. |
| `duration` | `1.1` | Seconds for the wave to cross the grid. |
| `tile` | `0.7` | Seconds for one tile. |
| `gap`, `radius` | `6`, `10` | px. |
| `delay` | `0` | |
| `out` | | When the tiles leave again with the same wave. |
| `reverse` | `false` | Start revealed and hide instead. |
| `back` | tinted surface | Back-face colour for `flip`. |
| `seed` | `1` | For `random`. |

### `kineticGrid`: rippling grid backdrop
```json
{ "component": "kineticGrid", "props": { "shape": "dot", "wave": "interference" } }
```
| Prop | Default | Meaning |
|---|---|---|
| `cols` | `18` | Rows follow from the box aspect. |
| `shape` | `rounded` | `rounded` (circles at rest, rounded squares at the crest), `square`, `dot` or `diamond`. |
| `wave` | `radial` | `radial`, `diagonal`, `linear` or `interference` (two sources). |
| `colors` | accent → pink → cyan | Cycled across the grid and over time. |
| `speed` | `1` | |
| `wavelength` | `7` | In cells. |
| `size`, `min` | `0.8`, `0.16` | Shape size at the crest and trough, as fractions of a cell. |
| `rotate` | `90` (`0` for dots) | Degrees of turn at the crest. |
| `delay` | `0` | Cells pop in from the centre. |
| `step` | `0` | Seconds per step (e.g. `0.5` = one beat at 120 BPM): the wave snaps forward at each step and holds, so it moves on the beat and frames in between are identical (smaller files). |

### `reactions`: live-stream reaction burst
```json
{ "component": "reactions", "props": { "kinds": ["heart", "fire", "100"], "rate": 6 } }
```
Emoji are vector drawings (no emoji font needed): `heart`, `like` (thumbs up), `fire`, `star`, `sparkles`, `100` and `laugh` (tears of joy). Aliases such as `thumbs`, `lol` and `hundred` work too. Each one pops in with a small ring-and-sparks burst, floats up with a wobble and fades.

| Prop | Default | Meaning |
|---|---|---|
| `kinds` | `heart`, `like`, `fire`, `star` | Picked at random per reaction. |
| `count` | `24` | Reactions spread over `duration`… |
| `duration` | `3` | …seconds (front-loaded a little). |
| `rate` | | Reactions per second as an endless stream (overrides `count`). |
| `origin` | `[0.82, 0.94]` | Start point as fractions of the box (bottom right, as on live streams). |
| `size` | `72` | px (±25 %). |
| `spread` | `150` | Horizontal drift and wobble, px. |
| `rise` | `0.8` | How far up they float, as a fraction of the origin's height. |
| `life` | `2.8` | Seconds each one lives. |
| `style` | `emoji` | `emoji` (glossy colour drawings) or `flat` (one-colour silhouettes with white details, in `colors`). |
| `colors` | theme palette | Colours for `flat`, one per kind. |
| `burst` | `true` | The ring and sparks on pop. |
| `delay`, `seed` | | |

Two looks: `{ "kinds": ["heart", "like", "laugh"], "rate": 6 }` (classic live-stream emoji) vs `{ "style": "flat", "colors": ["#ffffff"], "kinds": ["heart", "star"], "burst": false }` (a minimal monochrome brand look).

### `cardStack`: swiping stack of cards
```json
{ "component": "cardStack", "props": { "cards": ["Design", { "title": "Ship", "body": "One command.", "src": "ship.png" }], "interval": 2 } }
```
Cards behind are smaller, higher and darker; every `interval` the front card swipes away and the stack advances, the swiped card returning to the back.

| Prop | Default | Meaning |
|---|---|---|
| `cards` | | Titles, or `{ "title", "body", "src", "color", "eyebrow" }`. Titled cards get a palette gradient and a number eyebrow; image cards are cover-fitted with the title over a scrim. |
| `interval` | `2.2` | Seconds per card. |
| `swipe` | `left` | `left`, `right`, `up` or `alternate`. |
| `visible` | `3` | Cards shown in the stack. |
| `aspect` | `1.5` | Card width / height. |
| `size` | `0.62` | Card width as a fraction of the box (capped by the height). |
| `depth` | `0.09` | Vertical step per depth, as a fraction of the card height. |
| `darken` | `0.22` | Darkening per depth. |
| `skew` | `-4` | Degrees of shear on the whole stack (a 2.5D tilt). |
| `radius` | theme | |
| `loop` | `true` | `false` stops on the last card. |
| `delay` | `0` | The stack springs in from the back. |

### `particleText`: words made of particles
Thousands of particles swirl in through a curl-noise flow (with depth: near ones grow and soften, far ones shrink), settle into the text with a soft bloom and the odd glint, re-form into the next word, then blow away in ribbons. Points are sampled from the real glyph outlines, so any phrase works. Colours default to the theme's text colour (warm white on dark themes).

```json
{ "type": "component", "component": "particleText", "props": { "words": ["Hello", "World"] } }
```

| Prop | Default | Meaning |
|---|---|---|
| `text` | `"Hello"` | The word or phrase (or use `words`). |
| `words` | | Several words; the particles re-form from one into the next. |
| `count` | `6000` | Number of particles. |
| `size` | `1.9` | Particle radius in px at 1080p. |
| `colors` | theme text | Gradient across the text, left to right. |
| `fontSize` | fit | Default fits about 80% of the width and at most 30% of the height. |
| `weight`, `face` | `800`, `display` | |
| `form` | `1.8` | Seconds to fly in and to re-form. |
| `hold` | `1.4` | Seconds each word holds. |
| `flow` | `curl` | How particles travel: `curl` (swirling noise flow), `direct` (gentle arcs) or `spiral` (orbit into place). |
| `turbulence` | `1` | Strength of the swirl, `0`–`3`. |
| `depth` | `1` | Depth while flying (`0` = flat). |
| `sparkle` | `0.5` | Share of particles that glint while the text holds. |
| `glow`, `bloom` | `true`, `0.7` | Soft bloom around the formed text. |
| `exit` | `"none"` | `none` (the text stays), `blow` (wind, with curl ribbons), `explode` or `fade`. |
| `exitTime` | `1.6` | |
| `delay`, `seed` | `0`, `1` | |
| `blend` | `screen` | `normal` draws solid dots, so dark particles show on a light background. |
| `stagger` | `1` | How much the exit sweeps left to right; `0` makes every particle leave at once (a burst on a beat). |

Two looks: `{ "text": "Launch", "colors": ["#ffd59e", "#ff7a59"], "exit": "blow" }` (warm swirling embers) vs `{ "text": "Launch", "flow": "spiral", "turbulence": 0, "sparkle": 0, "glow": false, "size": 2.6, "count": 3000 }` (a clean, graphic orbit-in).

## Shapes and light

### `morph`: shape morphing
A shape that morphs smoothly through a list of named shapes or SVG paths, with a little squash and turn, in one of four materials.

```json
{ "type": "component", "component": "morph", "props": { "shapes": ["circle", "star", "heart"] } }
```

| Prop | Default | Meaning |
|---|---|---|
| `shapes` | circle, star, heart, blob | Names (`circle`, `square`, `star`, `heart`, `blob`, `triangle`, `hexagon`, `diamond`) or SVG path data (`"M0,0 L10,0 L5,8 Z"`). Every shape is centred and scaled to a similar visual size. |
| `material` | `glossy` | `glossy` (shaded body, lit rim, specular), `glass` (clear, bright rim, caustic), `flat` (clean fill) or `outline` (line drawing). |
| `size` | `0.62` | Fraction of the box's shorter side. |
| `colors` | theme accent | Fill gradient (1–3 colours); defaults to shades of the accent (neutral for `glass`). |
| `duration` | `0.9` | Seconds per morph. |
| `hold` | `0.7` | Seconds each shape holds. |
| `loop` | `true` | Morph back to the first shape and repeat. |
| `turn` | `0` | Degrees each morph rotates the shape. |
| `glow`, `stroke`, `delay`, `seed` | `true`, none, `0`, `1` | `seed` varies the `blob` shape. |

Two looks: `{ "material": "glossy", "colors": ["#ffb56b", "#ff5e62"] }` (a juicy 3D-ish candy) vs `{ "material": "outline", "glow": false, "colors": ["#ffffff"] }` (a crisp line drawing).

### `metaballs`: liquid blobs
Blobs drift, merge and split. The edge is an iso-contour extracted each frame as smooth closed outlines, so it stays crisp at any resolution. Higher iso-levels of the same field follow the merged shape exactly and shade the liquid: a fresnel rim, inner glow, caustics or mirror bands, depending on the material.

| Prop | Default | Meaning |
|---|---|---|
| `count` | `9` | Number of blobs. |
| `material` | `glossy` | `glossy` (coloured liquid, rim light, inner glow, speculars), `glass` (clear with a bright rim and a caustic), `chrome` (mirror bands) or `matte` (soft clay). |
| `colors` | theme accent | 1–4 colours; the gradient drifts slowly. Defaults to shades of the accent (`glossy`, `matte`) or neutral (`glass`, `chrome`). |
| `size` | `0.085` | Blob radius as a fraction of the box height. |
| `spread` | `1` | How far the blobs wander. |
| `speed` | `1` | |
| `glow`, `gloss` | `true` | Soft halo; specular highlights. |
| `rim` | `1` | Rim light strength (`0`–`2`). |
| `detail` | `7` | Grid cell in px at 1080p (smaller is more precise). |
| `seed` | `3` | |

Two looks: `{ "material": "chrome", "count": 6, "size": 0.11 }` (liquid metal) vs `{ "material": "matte", "colors": ["#f4a261", "#e76f51"], "glow": false }` (soft clay).

### `godRays`: volumetric light
A fan of hundreds of hair-thin slices, each lit by a noise density over angle and distance, so shafts of every width emerge, drift and breathe at full frame rate, with haze around the source and dust motes that light up inside the beams (near motes are soft bokeh). Light is neutral daylight by default, nudged towards the theme accent; place it above the background.

| Prop | Default | Meaning |
|---|---|---|
| `at` | `[0.32, -0.08]` | Source position as fractions of the box (may be outside it). |
| `color` | daylight | Light colour near the source. |
| `farColor` | `color` | Colour far from the source (a warm core with cool haze, for example). |
| `angle` | `62` | Direction of the fan in degrees (90 points straight down). |
| `spread` | `70` | Fan width in degrees (360 for all around). |
| `count` | `26` | Roughly how many distinct shafts across the fan. |
| `length` | `1.15` | Beam length as a fraction of the box diagonal. |
| `intensity` | `1` | |
| `contrast` | `0.7` | `0` is an even glow, `1` hard beams with dark gaps. |
| `haze` | `0.5` | Bloom around the source, `0`–`1`. |
| `spin` | `1` | Fan rotation in degrees per second. |
| `speed` | `1` | How fast the shafts shift and breathe. |
| `dust` | `260` | Number of dust motes (0 for none). |
| `depth` | `1` | Depth of field on the dust (`0` = all sharp). |
| `seed` | `4` | |

Two looks: `{ "at": [0.5, -0.1], "angle": 90, "spread": 50, "contrast": 0.95, "count": 10, "color": "#fff1d6", "dust": 400 }` (cathedral beams) vs `{ "at": [0.5, 0.45], "spread": 360, "count": 40, "contrast": 0.4, "color": "#ffd9a0", "farColor": "#7fb4ff", "dust": 0, "spin": 4 }` (a radiant burst behind a logo).

### `lensFlare`
A physically inspired flare: veiling glare, bloom and a hot core; a diffraction starburst (blade spikes with chromatic edges and fine hair); a dispersed halo ring; a streak; and ghosts of varied shapes along the optical axis from the light through the frame centre and beyond, each split into red, green and blue copies. Ghosts strengthen as the light moves off-axis, and the flare fades as the light leaves the frame.

```json
{ "type": "component", "component": "lensFlare", "props": { "path": [[0.15, 0.25], [0.85, 0.35]], "times": [0, 3] } }
```

| Prop | Default | Meaning |
|---|---|---|
| `at` | `[0.3, 0.3]` | Light position as fractions of the box. |
| `path` | | Positions to travel through (fractions); overrides `at`. |
| `times` | even | Arrival time of each `path` point, in seconds. |
| `duration` | `4` | Travel time over the whole `path` when `times` is omitted. |
| `lens` | `spherical` | `spherical` (starburst, polygon ghosts, short neutral streak), `anamorphic` (long blue streak, oval ghosts, no starburst) or `vintage` (warm, soft, big halos, a 4-point star). |
| `color` | per lens | Core and halo tint (daylight; warm for `vintage`). |
| `streakColor` | per lens | |
| `streak` | per lens | Streak length as a fraction of the box width (0 for none). |
| `spikes` | per lens | Diffraction spikes (0 for none). |
| `blades` | `6` | Aperture blades: the ghosts' polygon shape (0 for round). |
| `chroma` | `1` | Chromatic dispersion of ghosts, spikes and the halo (`0`–`2`). |
| `halo` | `0.3` | Halo ring radius as a fraction of the box height (0 for none). |
| `rotation` | `12` | Starburst rotation in degrees. |
| `ghosts` | `8` | Number of ghosts (0 for none). |
| `intensity`, `seed` | `1`, `2` | |

Two looks: `{ "lens": "anamorphic", "path": [[0.1, 0.4], [0.9, 0.45]], "duration": 4 }` (a sci-fi sweep) vs `{ "lens": "vintage", "at": [0.75, 0.2], "chroma": 0.3 }` (a soft, warm sun).

**Cost and shortcuts.** The effect kinds keep their speed shortcuts behind one switch, `fxkit::exact()` (off): out-of-focus particles and bloom are gaussian discs and ellipses rather than blurred layers (near confetti uses a baked soft-rectangle image), light composites with source-over rather than additive layers, god-ray slices are up to ~9 px wide where they fade out, and metaballs are contoured on a `detail` grid. Per frame at 1080p (eval / full render, single thread): god-rays 0.5 / 24 ms, lens-flare 0.02 / 9 ms, confetti 0.1 / 2 ms, sparks 0.04 / 1.2 ms, snow 0.1 / 5 ms, dust 0.05 / 1.4 ms, particle-text 3.8 / 29 ms (while flying), metaballs 0.9 / 15 ms, globe 0.2 / 10 ms, morph 0.01 / 3 ms, reactions 0.02 / 2.4 ms.

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

### `text3d`: extruded chrome text or logo
`{ "text": "ANIKIT" }` is enough. Each glyph (from the bundled fonts, with kerning) is extruded with a quarter-round bevel. The bevel shrinks on its own where strokes are thin, such as serif hairlines. Letters rise in one after another, then the word sways gently and a sheen sweeps across it every 6 s. The default material is polished chrome with a hint of the theme accent, lit by a warm key, a cool fill, a fresnel rim and a studio horizon reflection, on a transparent background. Kind names: `text3d`, `text-3d`.

| Prop | Default | Meaning |
|---|---|---|
| `text` | `ANIKIT` | `\n` breaks lines. |
| `points` | | A logo polygon `[[x, y], ...]` (y down), used instead of `text`. |
| `path` | | An SVG path (`M L H V C S Q T Z`, absolute or relative; nested sub-paths make holes), used instead of `text`. |
| `font`, `weight` | `Inter`, `800` | Any loaded family or alias (`serif`, `mono`). |
| `tracking` | `0.03` | Letter spacing, in em. |
| `depth`, `bevel` | `0.2`, `0.028` | Extrusion depth and bevel size, in em. |
| `color` | chrome | Base colour. |
| `gradient` | | Colours spread across the letters from left to right, e.g. `["#7c6cff", "#ff6fb1"]`. |
| `metallic`, `roughness` | `0.85`, `0.22` | `0`/`0.6` gives matte plastic. |
| `intro` | `rise` | `rise`, `flyIn`, `drop` or `none`. |
| `stagger` | `0.07` | Seconds between letters. |
| `delay` | | Seconds before the entrance. |
| `spin` | `0` | Turntable speed, in degrees per second. |
| `angle`, `tilt` | `-10`, `8` | Start yaw. Tilt towards the camera, which shows the tops of the letters. |
| `sway` | `7` | Amplitude of the idle sway, in degrees. |
| `shine` | `1` | Strength of the sheen sweep (`0` turns it off). |
| `shadow` | `0.35` | Soft shadow under the text. |
| `scale`, `resolution` | `1`, `1` | Size, and render scale (capped at 2400 px). |

The component's duration is the end of its entrance. **Cost** per frame at 1920×1080 on 4 cores, for a full-width title: 18–25 ms (a logo: 5 ms). The cost scales with the text's area on screen.

### `globe`: dot-matrix globe with arcs
A rotating globe in the style of the Stripe and GitHub globes: land is drawn as a lattice of dots (from built-in coarse continent outlines), with an atmosphere halo and a fresnel rim. Great-circle arcs draw on one after another with a soft glow, in the theme accent warming to white. A comet (a tapered, fading tail behind a glowing head) then runs along each arc every 2.6 s. Markers pulse with expanding rings. The view starts centred on the arcs and markers. Everything is vector, and the static geometry is built once.

```json
{ "type": "component", "component": "globe", "props": { "arcs": [[37.8, -122.4, 51.5, -0.1], [51.5, -0.1, 1.35, 103.8]], "markers": [[37.8, -122.4]] } }
```

| Prop | Default | Meaning |
|---|---|---|
| `arcs` | | `[[lat1, lon1, lat2, lon2], ...]`, in degrees. |
| `markers` | | `[[lat, lon], ...]`. |
| `spin` | `8` | Degrees of longitude per second. |
| `tilt` | `18` | The latitude the camera looks from. |
| `lon` | the middle of the arcs | Longitude facing the camera at the start. |
| `color` | sky blue | Dots, rim and atmosphere (a cool blue with a touch of the accent). |
| `arcColor` | accent → white | One colour, or `[start, end]`. |
| `markerColor` | arc end colour | |
| `dots` | `land` | `land`, `all` or `none`. |
| `density` | `1` | Dot rows (`1` = 100 rows from pole to pole, max `4`). |
| `grid` | `false` | Latitude and longitude lines. |
| `glow` | `1` | Atmosphere strength. |
| `stagger`, `draw` | `0.45`, `1.4` | Seconds between arcs, and the draw-on time of each. |
| `delay`, `scale` | | Fade-in delay. A scale of `1` fills 80 % of the short side. |

Two looks: `{ "arcs": [...], "markers": [...] }` (a night-blue data globe) vs `{ "dots": "all", "density": 1.6, "color": "#ffb547", "arcColor": ["#ffffff"], "glow": 0.4 }` (a dense golden sphere with white arcs).

The component's duration is the time the last arc lands. **Cost** per frame at 1920×1080: eval under 1 ms, render 6–15 ms.

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

## Cost per frame

1080p from `cargo run --release -p anikit-video --example component_cost`:
- **eval** builds the ops (3D kinds rasterise here, on all cores; serially when called from frame-parallel renders);
- **full render** rasterises every frame from scratch;
- **incremental YUV** is the pipeline's path, which redraws only what changed since the previous frame;
- **static frames** counts frames identical to the one before (from 0.5 s to 1.5 s). Those are deduplicated and encode as P_Skip almost free.

Slow ambient effects step their time (backgrounds 8–15 updates per second, ambient particles and rays 15 per second, 3D poses 30 per second during entrances and 15 once settled). Attention pulses stop after 3 cycles, and settled springs land exactly on 1. Most frames therefore repeat once entrances end. Light effects and grain draw straight into the frame, with no full-frame blend layers.

| kind | eval ms | full render ms | incremental YUV ms | static frames |
|---|---:|---:|---:|---:|
| `code` | 0.025 | 1.31 | 0.00 | 29/29 |
| `terminal` | 0.012 | 1.26 | 0.05 | 21/29 |
| `bar-chart` | 0.054 | 1.80 | 1.72 | 7/29 |
| `line-chart` | 0.025 | 1.51 | 2.91 | 0/29 |
| `area-chart` | 0.025 | 2.41 | 3.83 | 0/29 |
| `donut-chart` | 0.036 | 1.24 | 1.15 | 0/29 |
| `pie-chart` | 0.006 | 1.38 | 0.59 | 0/29 |
| `counter` | 0.013 | 0.85 | 0.78 | 0/29 |
| `stat` | 0.016 | 5.58 | 8.03 | 0/29 |
| `phone` | 0.005 | 3.60 | 4.44 | 7/29 |
| `laptop` | 0.003 | 4.12 | 4.37 | 7/29 |
| `browser` | 0.007 | 12.02 | 12.01 | 7/29 |
| `card` | 0.007 | 5.11 | 5.68 | 14/29 |
| `button` | 0.004 | 0.75 | 0.12 | 14/29 |
| `toggle` | 0.000 | 0.65 | 0.04 | 4/29 |
| `badge` | 0.003 | 0.66 | 0.02 | 14/29 |
| `avatar` | 0.003 | 0.68 | 0.03 | 17/29 |
| `avatar-stack` | 0.010 | 0.70 | 0.05 | 14/29 |
| `progress-bar` | 0.009 | 1.61 | 0.87 | 0/29 |
| `progress-ring` | 0.010 | 0.76 | 0.20 | 0/29 |
| `toast` | 0.007 | 0.87 | 0.22 | 11/29 |
| `cursor` | 0.001 | 0.68 | 0.07 | 5/29 |
| `tooltip` | 0.003 | 0.70 | 0.08 | 14/29 |
| `callout` | 0.006 | 0.68 | 0.06 | 0/29 |
| `arrow` | 0.000 | 0.62 | 0.02 | 23/29 |
| `lower-third` | 0.006 | 0.70 | 0.08 | 14/29 |
| `title-card` | 0.015 | 0.81 | 0.26 | 19/29 |
| `mesh-gradient` | 0.299 | 2.84 | 0.57 | 22/29 |
| `aurora` | 1.549 | 2.92 | 0.86 | 18/29 |
| `grain` | 0.000 | 3.33 | 0.00 | 29/29 |
| `grid` | 0.001 | 2.29 | 0.00 | 29/29 |
| `dots` | 0.253 | 4.94 | 0.80 | 29/29 |
| `light-leak` | 0.002 | 2.97 | 1.36 | 20/29 |
| `bokeh` | 0.007 | 1.34 | 1.34 | 14/29 |
| `starfield` | 0.093 | 1.07 | 1.40 | 14/29 |
| `gradient` | 0.000 | 2.62 | 0.00 | 29/29 |
| `confetti` | 0.025 | 0.84 | 2.20 | 0/29 |
| `sparks` | 0.003 | 3.66 | 2.85 | 3/29 |
| `snow` | 0.034 | 1.01 | 1.34 | 14/29 |
| `dust` | 0.034 | 1.18 | 1.36 | 14/29 |
| `particle-text` | 0.894 | 19.39 | 24.62 | 0/29 |
| `morph` | 0.001 | 2.86 | 3.70 | 0/29 |
| `metaballs` | 0.106 | 3.53 | 2.56 | 14/29 |
| `god-rays` | 0.065 | 8.84 | 5.80 | 14/29 |
| `lens-flare` | 0.003 | 2.68 | 2.20 | 14/29 |
| `model` | 32.058 | 1.99 | 2.63 | 0/29 |
| `text-3d` | 19.764 | 1.62 | 2.05 | 0/29 |
| `globe` | 0.297 | 8.94 | 11.22 | 0/29 |
| `wipe` | 0.001 | 2.04 | 2.68 | 9/29 |
| `iris` | 0.001 | 0.66 | 1.09 | 13/29 |
| `glitch` | 0.000 | 0.63 | 0.00 | 29/29 |
| `chat` | 0.021 | 0.74 | 0.10 | 5/29 |
| `prompt` | 0.018 | 2.25 | 1.96 | 0/29 |
| `checklist` | 0.008 | 0.72 | 0.12 | 0/29 |
| `quote` | 0.023 | 0.77 | 0.08 | 11/29 |
| `table` | 0.008 | 3.09 | 0.25 | 23/29 |
| `timeline` | 0.011 | 0.69 | 0.20 | 0/29 |
| `marquee` | 0.043 | 0.71 | 0.40 | 0/29 |
| `spotlight` | 0.002 | 1.06 | 1.50 | 11/29 |
| `keys` | 0.014 | 0.74 | 0.06 | 14/29 |
| `visualizer` | 0.008 | 3.32 | 5.10 | 0/29 |
| `ken-burns` | 0.004 | 13.77 | 13.08 | 0/29 |
| `tile-reveal` | 0.005 | 2.92 | 4.06 | 0/29 |
| `kinetic-grid` | 0.016 | 1.68 | 1.59 | 14/29 |
| `reactions` | 0.009 | 1.00 | 0.43 | 0/29 |
| `card-stack` | 0.020 | 7.16 | 7.92 | 4/29 |
| `lottie` | 0.000 | 0.66 | 0.00 | 29/29 |

Totals: eval 56.01 ms, full render 178.56 ms, incremental 157.14 ms
