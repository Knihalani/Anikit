---
name: anikit-video
description: Make extraordinary motion videos fast with anikit-video, a native renderer for JSON specs with ray-traced 3D, GLSL shaders, particles, physics, generated music and a reviewer that judges frames. Use when asked to create, review or render a video or animated clip.
---

**Setup:** `anikit-video --version` prints the build commit. If it's missing, install the prebuilt binary (seconds; compiling takes minutes): `sh install.sh` from a checkout of this repo, otherwise `curl -fsSL https://raw.githubusercontent.com/knihalani/anikit/claude/epic-ramanujan-qe7x4w/install.sh | sh`, then `export PATH="$HOME/.local/bin:$PATH"`. Every command takes `--help`.

## Process (each step is cheap; skipping them is what costs)

1. **Concept, three lines:** the one image only *this* brief could have, built from its own nouns; the medium the idea asks for (2D type, illustration, paths and shaders are first-class; 3D when the idea is physical or material); the sound (acoustic or orchestral unless the idea is electronic). `anikit-video docs taste` (one page) has what tends to land.
2. **Reference:** with web access, look at a few pieces of top work in this genre and name what makes them good. Local images: `anikit-video board refs/` turns them into palette, type and texture tokens.
3. **Style frames:** write 2–3 contrasting first frames as one-scene specs in one file, `preview` it, and pick the strongest before building the rest.
4. **Build, then loop:** `anikit-video preview s.json` (~1 s for 2D) validates, prints unknown keys first, writes `s-sheet.png` (one frame per second), the top notes, pace and a final-render estimate. Look at the sheet; fix; repeat. `--at 1,4.5,8` writes several moments in one image at full detail.
5. **Ship gate** (below), then **ONE** `anikit-video render s.json -o out.mp4 --sheet contact.png` (background it when the estimate is minutes; it prints one line every 15 s). Later fixes: `render --from A --to B`.

Write the spec directly. For repetitive layers use `"repeat": 40` with `$i`, `defs`/`use`, or a `{ "script": … }` layer, not a Python generator. Patch one value with `anikit-video set s.json <path> <value>`.

## The spec in one screen

`{ "size": "16:9", "intent": {"pace": "fast"}, "music": "acoustic", "fonts": {"Display": "fonts/Display.ttf"}, "scenes": [ { "duration": 1.2, "background": "#0E0E10", "transition": "push", "camera": {…}, "layers": [ … ] } ] }`

- **Text:** `{ "text": "Make **anything**.", "font": "Display", "size": "14vh", "x": "8%", "y": "50%", "anchor": "left", "color": "#F4F1EA", "enter": "mask by line up" }`. Built-in faces are Inter, Instrument Serif and JetBrains Mono; load any TTF/OTF through `fonts`.
- **Shapes and paths:** `{ "shape": "path", "d": "M0 200 C 300 0, 600 400, 900 120", "stroke": "#FF5A1F", "strokeWidth": 6, "animate": [{ "from": {"trimEnd": 0}, "to": {"trimEnd": 1}, "duration": 1 }] }`; `"shape": "rect"|"circle"` with `width`, `height`, `fill`.
- **Motion:** `"animate": [{ "from": {"scale": 3, "blur": 20}, "duration": 0.25, "ease": "outExpo" }]`; expressions on any number or colour: `"x": "=960 + 300*sin(t*2)"`, `"rotation": "=t*90"` (`t` is layer time, `gt` global). A layer's own timing is `start`/`end`; entrances take `delay`.
- **Camera:** `"camera": { "animate": [{ "from": {"zoom": 1.3}, "to": {"zoom": 1}, "duration": 0.3, "ease": "outExpo" }], "shake": {"amount": 8, "at": 0} }`, plus `"motionBlur": true`. Layer `depth` gives parallax.
- **Shaders:** `{ "shader": "void mainImage(out vec4 o, in vec2 f) { … iTime … }", "uniforms": {"speed": 1} }`, a file with `{ "type": "shader", "src": "x.glsl" }`, or a library name (`anikit-video recipes` lists them). `"filter": true` treats the layers below.
- **3D, particles, physics:** `{ "component": "scene3d", "props": { "objects": [ … ], "camera": "orbit" } }` (`docs 3d`, `docs particles`, `docs physics`).
- **Compositions:** `"compositions": { "ui": {…} }` → `{ "composition": "ui", "loop": true }`.
- **Music and grade:** `"music": "orchestral"` or `{"style": "acoustic", "bpm": 96, "key": "D", "hits": ["@bar5"]}`; cut on `"@beat9"`/`"@bar2"`. Sound design: `"sound": "auto"` (or a palette, `"luxury"`) sounds moves, landings and reveals from the picture, tuned to the key and mastered; `anikit-video sound plan s.json` lists the cues. `"post": {"preset": "cinematic", "bloom": 0.15}`.
- **Look up one key:** `anikit-video docs key shadow` (its type, fields and examples, a few lines); a topic: `anikit-video docs <topic>` (1–3k characters each): `primitives animation expressions camera compositions shader 3d particles physics music sound post pacing draw script`. `anikit-video snippets` has paste-able fragments.

**Costs:** 2D renders at 100+ fps; shaders 50–400 ms a frame; ray-traced 3D and particles 0.5–1.5 s a frame (keep 3D shots to 2–4 s). `motionBlur` only where motion is fast. `--quality best` is several times slower; standard is the final.

## Ship gate (before the one final render)

Open the sheet and three full-size stills and answer in writing, a line each:

1. **Is every frame visually striking on its own, a still someone would save?** It can be rich illustration, bold graphic design, beautiful light, spectacle, precise data design, texture or motion. What tends to get there: one strong image idea per shot, considered detail, real light and texture, contrast of scale. What tends not to: empty frames, a UI screenshot beside copy, default looks, generic type.
2. **What would make a viewer stop scrolling?** Spectacle, wit, beauty, clarity, rhythm, surprise or craft: name the one this piece is going for.
3. **Where is the weakest frame, and why?**
4. **Does the sound do as much as the picture?**
5. **Is every word legible?** Full resolution, in focus, 4.5:1 contrast, held long enough to read.

If an answer is weak, improve that one thing and preview again, then render once.

## Shortcuts (optional)

Launch or product video: `skills/anikit-launch-video/SKILL.md`. A reference video to match: `anikit-video analyze ref.mp4 --spec draft.json` and `skills/anikit-reverse-video/SKILL.md`. Worked examples of techniques: `anikit-video recipes`. Art-direction presets: `anikit-video looks`. Prebuilt widgets: `anikit-video info --components`. MCP: `anikit-video mcp`.
