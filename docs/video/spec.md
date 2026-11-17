# Anikit Video spec: cheat sheet

A video is one JSON (or YAML) file: `anikit-video render spec.json -o out.mp4`. Everything is optional except `scenes`. Full reference: [`spec-reference.md`](spec-reference.md). Schema: [`spec.schema.json`](spec.schema.json).

```json
{ "scenes": [
  "Launch day | Everything ships at once",
  { "layers": [ { "text": "3× faster", "style": "title", "animate": "rise" }, "Built in Rust" ] },
  { "transition": "push", "layers": [ { "component": "bar-chart", "props": { "values": [3, 7, 5, 9] } } ] }
] }
```

A scene string is a title card (`"Title | subtitle"`); a layer string is centred body text. Defaults: 1920×1080, 30 fps, 3 s scenes, dark theme, Inter, 0.5 s crossfades, every layer centred.

## Values

| Kind | Write |
|---|---|
| time | `1.5` · `"1.5s"` · `"500ms"` · `"12f"` · `"@marker"` · `"@marker+0.2"` |
| length | `120` (px) · `"50%"` (of parent) · `"10vw"` · `"8vh"` · `"5vmin"` |
| colour | `"#7c5cff"` · `"#fff8"` · `"oklch(70% 0.15 250)"` · `"$primary"` · `"linear-gradient(135deg, $primary, $accent)"` |
| expression | `"=960 + 300 * sin(t * 2)"` (vars `t T gt p i n w h fps frame beat bar level bpm`; `c ? a : b`, `&& ||`; `sin mix remap smoothstep ease noise rand …`). Colours too: `"=hsl(t * 60, 70, 55)"`, `"=mix($primary, $accent, level)"`. See [`authoring.md`](authoring.md) |

## Top level

`size` (`"16:9"` `"9:16"` `"1:1"` `"4:5"` `"720p"` `"4k"` or `[w, h]`) · `fps` · `background` · `theme { colors, fonts, radius, styles }` · `scenes` · `transition` · `markers { name: time }` (plus automatic `"@beat17"`, `"@bar5+0.25"` from the first music track) · `audio [...]` · `captions` · `fonts { family: file }` · `duration` · `compositions` · `post` (`clean cinematic dreamy punchy noir vintage`, or `{ "preset": "cinematic", "bloom": 0.4 }`; per scene too) · `motionBlur` (`true`, or `{ "shutter": 90 }`; default only with `--quality high`) · `encode` (`{ "crf": 26, "aq": 0.8 }`: quantiser 0–51, default 23, lower is bigger; `render --crf N` overrides; aq 0–2 for gradients and skies).

Theme colours (used as `$name`): `primary secondary accent bg surface text muted`. Text styles: `title heading subtitle body caption label code`.

**Generate:** `{ "script": "let out = []; for k in 0..24 { out.push(#{ shape: \"circle\", x: 80 * k }) } out" }` in any layer or scene list (Rhai: loops, maths, `rand`, `hsl`); a top-level `script` can rewrite `spec`. See [`authoring.md`](authoring.md).

**Reuse (fewer tokens):** `"defs": { "card": {…layer or scene…} }`, then `{ "use": "card", "with": { "title": "Hi" }, "x": 200 }` anywhere a layer or scene goes. Overrides deep-merge; `"$title"` in the def is replaced. A def can declare `"params": { "size": 34, "color": "$text" }` (defaults), and the use site passes them directly: `{ "use": "label", "size": 40 }`. `"repeat": 3` copies a layer or scene, with `"$i"` (0-based), `"$n"` and arithmetic `"=160 + $i*420"`. For scatters, vary each copy: `"x": "=rand($i, 0, 1920)"`, `"width": "=pick(rand($i + 7) * 3, 18, 24, 32)"`, `"fill": "=pick(rand($i) * 2, #fff, $accent)"`. `{ "repeat": 3, "scenes": [ … ] }` loops a sequence of scenes.

## Scenes

`id` · `duration` (default 3, `"auto"`, or musical: `"2 beats"`, `"1 bar"`, `"4b"`; top-level `"cut": "bar"` sets the default for every scene) · `stack` (`true`: free text layers as one centred column, no x/y needed) · `background` · `transition` (into this scene) · `camera` · `markers` · `layers` · `audio`.

Transitions: `cut crossfade fadeBlack fadeWhite slide push zoom wipe iris blur glitch morphCut`, shader transitions `burn ripple pixelate swirl ink-bleed warp-wipe luma-melt zoom-blur`, or your own `{ "type": "shader", "shader": "vec4 transition(vec2 uv) {…}" }` (gl-transitions API). Write one as a string or as `{ "type": "slide", "direction": "left", "duration": 0.6 }`.

Camera: `{ "zoom": 1, "x": 0, "y": 0, "rotation": 0, "focus": 0, "aperture": 0, "animate": [...], "shake": { "amount": 8, "at": "@drop", "duration": 0.4 } }`.

## Layers

`type` is inferred: `text` → text, `shape` → shape, `src` → image/video, `children` → box, `layers` → group, `component` → component.

Every layer: `x y` (default centre) · `anchor` (`center`, `topLeft`, …) · `width height` · `scale rotation skewX skewY opacity blur` · `blend` · `shadow` (`true` or `{x, y, blur, color}`) · `glow` · `clip` · `mask` · `depth` · `start end duration` · `enter exit loop animate` · `stagger`.

| Type | Keys |
|---|---|
| text | `text` (`**bold**`, `*italic*`, `[word]{$accent}`) · `style` · `font size weight color align lineHeight letterSpacing uppercase maxLines` · `fit: true` (shrink to `width`) · `stroke` · `split` (`chars words lines`) · `kinetic` · `reveal` · `value` with `"{value:,}"` |
| shape | `shape` (`rect circle line arrow polygon star path`) · `fill` · `stroke` · `strokeWidth` · `radius` · `points` · `d` · `sides` · `trimStart trimEnd` |
| image / video | `src` · `fit` (`contain cover fill none`) · `radius` · video: `trimStart speed volume loop` |
| box | flexbox: `style { direction gap padding align justify wrap background radius border columns }` · `children` |
| group | `layers` |
| draw | `draw: [ { "d": "M0 0 C…", "stroke": "$accent", "width": 4 }, { "points": [[x, y], …], "smooth": true, "colors": [...] }, { "circle": [x, y, r], "fill": "#fff" }, { "rect": [x, y, w, h] }, { "line": [x1, y1, x2, y2] } ]` in frame px |
| dots | `dots: { "data": [[x, y], …] or "points.csv" (`x,y[,colour]`), "size": 4, "color": "$accent", "appear": "=clamp(t * 2000 - i, 0, 1)" }`, thousands of points in frame px (maps, scatter plots) |
| composition | `composition` (a name from top-level `"compositions": { "ui": { "size": [800, 500], "duration": 4, "background": "#111", "layers": [...] } }`, or `scenes`) · `fit` · `trimStart speed` · `loop: true`. It has its own clock and is clipped to its box; 3D layers use it as a texture |
| component | `component` (`bar-chart line-chart donut-chart counter stat code terminal phone laptop browser card button toast cursor lower-third title-card mesh-gradient aurora grid confetti model lottie …`; `anikit-video info --components`) · `props` |
| shader | `shader` (Shadertoy GLSL `mainImage`, WGSL, or a built-in: `hero mesh-gradient ocean light-shafts particles liquid-chrome sdf-scene aurora caustics flow-ink turing film-burn glitch lake-mirror`; style filters `halftone risograph linocut watercolour crosshatch film dither screenprint print-offset`; see `info --shaders`) or `src: "fx.glsl"` · `uniforms` (animatable by name) · `channels` (`below noise` · image/video · `#composition` · `{layers}` · buffers `A`–`D`) · `buffers` (multi-pass, feedback) · `filter` · `supersample` · `alpha` |

**Shaders** run JIT-compiled on the CPU at full resolution: `{ "shader": "aurora", "uniforms": { "intensity": 1.4 } }`. `"filter": true` reads the composited layers below as `iChannel0` and replaces them. `iAudio`/`iBeat` follow the music; `still` prints each shader's ms/frame. Guide, built-ins, costs and limits: [`shader.md`](shader.md).

## Animation

```json
"enter": "rise"                                   // presets, below
"enter": { "preset": "popIn", "delay": 0.2, "stagger": 0.04 }
"exit": "fade"
"loop": "float"                                   // float pulse spin wiggle breathe blink
"animate": [
  { "from": { "opacity": 0, "y": "+60" }, "duration": 0.6 },
  { "to": { "x": 1400, "rotation": 15 }, "at": 1.5, "ease": "inOutQuart" },
  { "keyframes": { "scale": [1, 1.2, 1] }, "at": "@drop", "duration": 0.5 },
  { "to": { "scale": 1.3 }, "at": 3, "spring": "bouncy" } ]
```

Presets: `fadeIn fadeUp fadeDown fadeLeft fadeRight slideUp slideDown slideLeft slideRight popIn scaleIn zoomIn blurIn rotateIn flipIn bounceIn drawOn typewriter wipeIn countUp`, and their exits. Aliases: `rise drop fade pop grow zoom blur draw type count wipe slide bounce`. On text, kinetic styles work too: `"scramble"`, `"wave by word"`, `"pop by char from center"`, `"highlight words"`, `"mask by line up"`.

`from` holds until `at` (no flash). `to` animates from the current value. Relative values: `"+60"`, `"-20"`, `"*1.5"`. `at` is relative to the layer start; `"+0.2"` means after the previous tween. Easings: `outCubic` (default), `inOutSine`, `outBack`, `outExpo`, `cubicBezier(…)`, `steps(4)`, springs `gentle wobbly stiff snappy bouncy`.

Animatable: `x y width height scale scaleX scaleY rotation skewX skewY opacity blur radius fill color stroke strokeWidth trimStart trimEnd reveal value wipe depth`, plus camera `zoom focus aperture`.

## Audio and captions

```json
"audio": [ { "src": "music.mp3", "volume": 0.7, "fadeOut": 1.5, "duck": true }, { "sfx": "whoosh", "at": "@drop" } ],
"captions": { "src": "voice.srt", "highlightBox": "$accent", "maxWords": 3 }   // or "background" for a pill behind the line
```

SFX: `click tick pop whoosh swoosh rise riser drop impact ding chime typing success`.

## CLI

Edit in place instead of rewriting: `anikit-video set spec.json scenes.intro.layers.title.text "New title"` (path by key, index or `id`; `--delete` removes).

```
anikit-video check  spec.json [--json]          errors and warnings with paths and "did you mean"
anikit-video still  spec.json --at 2.5 -o f.png [--width 640] [--watch]
anikit-video sheet  spec.json -o sheet.png [--frames 12]
anikit-video render spec.json -o out.mp4 [--quality draft|standard|high] [--from 2 --to 5] [--profile]
anikit-video beats  music.wav [--json]           tempo, beat and bar times ("@beatN" / "@barN")
```

`--json` prints one compact line, for example `{"ok":false,"errors":["scenes[0].layers[1].type: unknown layer type 'txt', did you mean 'text'?"],"warnings":[]}`. Output by extension: `.mp4` (our ak264 encoder; x264 with `--quality high`), `.webm`, `.mov`, `.gif`, `.png` sequence.
