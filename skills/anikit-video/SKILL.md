---
name: anikit-video
description: Make extraordinary motion videos fast with anikit-video, a native renderer for JSON specs with ray-traced 3D, GLSL shaders, particles, physics, generated music and a reviewer that judges frames. Use when asked to create, review or render a video or animated clip.
---

**Setup:** `anikit-video --version` prints the build commit. If it's missing, install the prebuilt binary (seconds; compiling takes minutes): `sh install.sh` from a checkout of this repo, otherwise `curl -fsSL https://raw.githubusercontent.com/knihalani/anikit/claude/epic-ramanujan-qe7x4w/install.sh | sh`, then `export PATH="$HOME/.local/bin:$PATH"`. Every command takes `--help`.

## Process (each step is cheap; skipping them is what costs)

1. **Concept, three lines:** start from the image, not the copy (in most world-class pieces type is absent or supporting): the one image only *this* brief could have, built from its own nouns; the medium the idea asks for (2D type, illustration, paths and shaders are first-class; 3D when the idea is physical or material); the sound (acoustic or orchestral unless the idea is electronic). `anikit-video docs taste` (one page) has what tends to land. Depth when a shot needs it: `docs/video/craft/` (graphic-design, illustration, art-direction, 3d-design, editing, grading, music, storytelling, ai-tells); these are long (20–50k), so list a file's headings with `grep -n '^##'` and read one section (≤ 3k). The crate source isn't documentation; `docs key <name>` answers spec questions.
2. **Reference:** with web access, look at a few pieces of top work in this genre and name what makes them good. Local images: `anikit-video board refs/` turns them into palette, type and texture tokens.
3. **Style frames:** write 2–3 contrasting first frames as one-scene specs in one file, `preview` it, and pick the strongest before building the rest.
4. **Build, then loop:** `anikit-video preview s.json` (~1 s for 2D) validates, prints unknown keys first, writes `s-sheet.png` (one frame per second), the top notes, pace and a final-render estimate. Look at the sheet; fix; repeat. `--at 1,4.5,8` writes several moments in one image at full detail.
5. **Ship gate** (below), then **ONE** `anikit-video render s.json -o out.mp4 --sheet contact.png`. Follow `preview`'s advice on how to run it: in the foreground with a 10-minute timeout, or, for a longer final, in the background while you wait for that process to exit in the same turn. Don't end your turn until the file exists. Later fixes: `render --from A --to B`.

If you build from source instead of installing, run `cargo build --release -p anikit-video` once in the foreground with a 10-minute timeout; polling a background build costs many turns.

Everything here is a starting point, never a ceiling: anything the spec keys don't cover, a GLSL `shader`, a `draw` layer of raw paths and points, or a `script` layer can build. Write the spec directly. For repetitive layers use `"repeat": 40` with `$i`, `defs`/`use` (`anikit-video docs reuse`), or a `{ "script": … }` layer, not a Python generator. Patch values with `anikit-video set s.json scenes.2.duration=3 scenes.2.layers.title.text="Salt"` (list indexes or layer ids as path steps) or an edit tool, rather than a script that rewrites the whole file.

## The spec in one screen

`{ "size": "16:9", "intent": {"pace": "fast"}, "music": "acoustic", "fonts": {"Display": "fonts/Display.ttf"}, "scenes": [ { "duration": 1.2, "background": "#0E0E10", "transition": "push", "camera": {…}, "layers": [ … ] } ] }`

- **Text:** `{ "text": "Make **anything**.", "font": "Display", "size": "14vh", "x": "8%", "y": "50%", "anchor": "left", "color": "#F4F1EA", "enter": "mask by line up" }`. `"font"` takes any of ~130 library families by name (`anikit-video fonts` lists them) or a TTF/OTF you load through `fonts`.
- **Shapes and paths:** `{ "shape": "path", "d": "M0 200 C 300 0, 600 400, 900 120", "stroke": "#FF5A1F", "strokeWidth": 6, "animate": [{ "from": {"trimEnd": 0}, "to": {"trimEnd": 1}, "duration": 1 }] }`; `"shape": "rect"|"circle"` with `width`, `height`, `fill`. A path sits at its bounding box: its `x`/`y` place the box's top-left, not the `d` coordinates.
- **Motion:** `"animate": [{ "from": {"scale": 3, "blur": 20}, "duration": 0.25, "ease": "outExpo" }]`; expressions on any number or colour: `"x": "=960 + 300*sin(t*2)"`, `"rotation": "=t*90"` (`t` is layer time, `gt` global). A layer's own timing is `start`/`end`; an entrance delay goes inside it (`"enter": {"preset": "fadeUp", "delay": 0.2}`) and a tween's start is `"at"`. No transition is a hard cut; a transition plays in the last part of the outgoing scene, so the video is exactly the sum of the scene durations.
- **Camera:** `"camera": { "animate": [{ "from": {"zoom": 1.3}, "to": {"zoom": 1}, "duration": 0.3, "ease": "outExpo" }], "shake": {"amount": 8, "at": 0} }`, plus `"motionBlur": true`. Layer `depth` gives parallax.
- **Shaders:** `{ "shader": "void mainImage(out vec4 o, in vec2 f) { … iTime … }", "uniforms": {"speed": 1} }`, a file as its own layer `{ "type": "shader", "src": "x.glsl" }` (not inside the `shader` key), or a library name (`anikit-video recipes` lists them). `"filter": true` treats the layers below.
- **3D, particles, physics:** `{ "component": "scene3d", "props": { "objects": [ … ], "rig": "rembrandt", "set": "sweep", "camera": { "lens": "portrait85", "shake": "breath", "keys": [ … ] } } }`. Light it with a `rig`, build a `set`, pick a `lens` and vary them every time; `check` flags the cheap-CG tells (`docs 3d`, `docs particles`, `docs physics`).
- **Real light, surfaces, objects (CC0, fetched once):** `"env": {"hdri": "polyhaven:hdri/studio_small_09"}`, `"material": {"texture": "ambientcg:Wood049"}`, `{"type": "model", "src": "polyhaven:model/Camera_01"}`, image `src` refs, `"post": {"lut": "anikit:lut/teal-orange"}`. Find them with `anikit-video assets search <words> --type hdri|texture|model|lut` (`docs assets`).
- **Compositions:** `"compositions": { "ui": {…} }` → `{ "composition": "ui", "loop": true }`.
- **Music and grade:** `"music": "orchestral"` or `{"style": "acoustic", "bpm": 96, "key": "D", "hits": ["@bar5"]}`; cut on `"@beat9"`/`"@bar2"`. Hand-written parts: `{"inst": "strings", "notes": "o4 A4 C5 E5/2 | [A3 C4 E4]/1"}`; length after a slash (`/8`, `/8t`, `/4.`), then `!` accent or `?` ghost (`C4/8!`); `'` and `,` shift scale degrees only (`5'`). The mix, sfx and loudness are built in: `docs music`. Sound design: `"sound": "auto"` (or a palette, `"luxury"`) sounds moves, landings and reveals from the picture, tuned to the key and mastered; `anikit-video sound plan s.json` lists the cues. `"post": {"preset": "cinematic", "bloom": 0.15}`.
- **Look up one key:** `anikit-video docs key shadow` (its type, fields and examples, a few lines); a topic: `anikit-video docs <topic>` (1–3k characters each): `primitives animation expressions camera compositions shader 3d particles physics music post pacing draw script graphic` (`graphic`: modular grids and cells, squircles, patterns and custom tiles, duotone, halftone, riso inks, cutouts). `anikit-video snippets` has paste-able fragments.

## What's possible (a menu to widen the options; each line is the minimal key, `docs <topic>` for more)

- **Real materials and light:** scene3d `"env": {"hdri": "polyhaven:hdri/…"}`, materials from `ambientcg:Wood049` (`anikit-video assets search oak`), `"rig": "rembrandt|window|product|…"`, `"set": {"preset": "plinth"}`, `"camera": {"lens": "portrait85", "preset": "orbit"}` (`docs 3d`, `docs assets`).
- **Motion with weight:** `"inertia": "heavy"`, `"smear": true`, group `"drag": 0.06` (`docs key inertia`).
- **Type as image:** `"matte": {"layer": "photo"}` (type as a window), text on a path, variable axes (`docs mograph`, `docs type`).
- **Idea transitions:** `{"type": "zoomThrough", "layer": "logo"}` on the incoming scene; `"carry": ["logo"]` on any transition moves a shared layer from the old layout to the new one (a match move, for continuity between layouts) (`docs mograph`).
- **Sound from the picture:** `"onNotes": {"track": "drums", "keys": ["kick"], "layer": {…}}` makes a layer per hit; `beat`/`level` in expressions (`docs rhythm`, `docs music`).
- **Illustration:** components `illo` (lit vector shapes, a consistent hand), `landscape`, `plants` (`docs component illo`).
- **Print and hand:** `"brush": true` on strokes, `"halftone": {…}`, riso/screenprint filters, patterns and grids (`docs graphic`).
- **Living systems:** components `fluid`, `physarum`, scene3d particles and physics; library shaders (`anikit-video recipes`) (`docs fluid`, `docs physarum`, `docs particles`).

**Costs:** 2D renders at 100+ fps; shaders 50–400 ms a frame; ray-traced 3D and particles 0.5–1.5 s a frame (keep 3D shots to 2–4 s). `motionBlur` only where motion is fast. `--quality best` is several times slower; standard is the final.

## Self-review, then ship gate (one more iteration here is worth it)

Look at your contact sheet and answer, then fix and preview again. These are the questions the blind judge fails pieces on most:
- **The device:** is it literal? Say the idea without the brand name and without the feature's mechanism; if nothing is left, it's the name drawn or the feature diagrammed. Does the one device *escalate* every beat, or does it play once?
- **The poster test:** pause anywhere. Would that frame hold up as a poster (14 of 15 round-1 films failed this)? Does any layout repeat with the words swapped?
- **The arc:** does energy build to a peak at about ⅔ and land (11 of 15 had a flat arc)? Is there one surprise?
- **Colour and continuity:** does colour change with the arc (sodium orange → water cyan), or is it one accent on near-black throughout? Do layouts flow into each other, with shared elements carried across (`carry`), rather than being swapped like slides? (A MOGRAPH A/B: restructuring alone lifted a piece 3.25 → 4.38; fonts on top added 0.)
- **Material at 100%:** does a crop show material from the subject (grain, edge, light falloff), or flat vector and filter noise?
- **Type and sound:** is the type a choice for this world, living in the scene rather than an eyebrow-plus-headline lockup? Do motion, camera and sound carry weight from the subject's own materials?
`critique --render` flags the measurable ones (`layout`, `label`, `motion`, `arc`, `eye-trace`).

### Ship gate (before the one final render)

Open the sheet and three full-size stills and answer in writing, a line each:

1. **Is every frame visually striking on its own, a still someone would save?** It can be rich illustration, bold graphic design, beautiful light, spectacle, precise data design, texture or motion. What tends to get there: one strong image idea per shot, considered detail, real light and texture, contrast of scale. What tends not to: empty frames, a UI screenshot beside copy, default looks, generic type, and landing-page furniture (numbered eyebrows like "02 / PROCESS", pill labels, LIVE dots, corner metadata) unless the world itself has labels (a timetable, a map legend).
2. **What would make a viewer stop scrolling?** Spectacle, wit, beauty, clarity, rhythm, surprise or craft: name the one this piece is going for.
3. **Where is the weakest frame, and why?**
4. **Does the sound do as much as the picture?** `anikit-video critique s.json --render` measures loudness and whether cuts land on beats.
5. **Is every word legible?** Full resolution, in focus, 4.5:1 contrast, held long enough to read.

If an answer is weak, improve that one thing and preview again, then render once.

## Shortcuts (optional)

Launch or product video: `skills/anikit-launch-video/SKILL.md`. A reference video to match: `anikit-video analyze ref.mp4 --spec draft.json` and `skills/anikit-reverse-video/SKILL.md`. Worked examples of techniques: `anikit-video recipes`. Art-direction presets: `anikit-video looks`. Prebuilt widgets: `anikit-video info --components`. MCP: `anikit-video mcp`.
