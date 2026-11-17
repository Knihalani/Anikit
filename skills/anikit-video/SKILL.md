---
name: anikit-video
description: Make extraordinary motion videos fast with anikit-video, a native renderer for JSON specs with ray-traced 3D, GLSL shaders, particles, physics, generated music and a reviewer that judges frames. Use when asked to create, review or render a video or animated clip.
---

**Setup:** if `anikit-video` isn't on PATH, install the prebuilt binary (seconds; compiling takes minutes): from a checkout of this repo run `sh install.sh`, otherwise `curl -fsSL https://raw.githubusercontent.com/knihalani/anikit/claude/epic-ramanujan-qe7x4w/install.sh | sh`; then `export PATH="$HOME/.local/bin:$PATH"`.

**Taste first:** before designing, read `anikit-video docs taste` (one page: routes to a striking frame and the tools for each, what tends to land and what tends to fail; depth in `docs/video/taste/TASTE.md`).

**The loop:** write `s.json` → `anikit-video preview s.json` (repeat; ~1 s for 2D: validates, writes `s-sheet.png` with one frame per second, prints the top notes, pace and a final-render estimate; look at the sheet) → the ship gate below → ONE final `anikit-video render s.json -o out.mp4` (background it when the estimate is minutes) → `render --from A --to B` only to check or fix a shot. Zoom in with `preview s.json --at 2.4 -o f.png`. Hand-built contact sheets aren't needed: `preview` writes one.

**Need detail? Read `anikit-video docs essentials` once (one page, ~13k characters), or `anikit-video docs <topic>`** (30 lines each, with a working example): `primitives animation camera compositions shader 3d particles physics music post pacing critique make-anything graphic` (`graphic`: modular grids, squircles, patterns, duotone, halftone, riso inks).

**Concept before JSON (two minutes, three lines):** (1) the one image only *this* brief could have, built from its own nouns (a lock that shatters into particles for a security product; its real data as a physics pile); (2) the medium the idea asks for: 2D typography, illustration, paths and shaders are first-class; use 3D only when the idea is physical or material, and a studio/cyc set with chrome or glass is the most generic 3D look, so it tends to read as stock (vary the look between pieces); (3) the music: acoustic or orchestral instrumentation unless the concept is electronic, and the beat it lands on. Recipes are techniques to borrow for a shot, not the piece: `critique` flags a spec that is mostly a recipe with new words.

**Have a reference video?** "Make one like this": `anikit-video analyze ref.mp4 --sheet s.jpg --spec draft.json`, then follow `skills/anikit-reverse-video/SKILL.md`.

## Build from the open primitives (anything is possible)

A spec: `{ "size": "16:9", "intent": {"pace": "fast"}, "music": "acoustic", "scenes": [ { "duration": 1.2, "layers": [ … ] } ] }`.

1. **Text, shapes and paths, fully styled:**
   `{ "text": "Make **anything**.", "font": "Instrument Serif", "size": "14vh", "x": "8%", "anchor": "left", "enter": "mask by line up" }`
   `{ "shape": "path", "d": "M0 200 C 300 0, 600 400, 900 120", "stroke": "#FF5A1F", "strokeWidth": 6, "animate": [{ "from": {"trimEnd": 0}, "to": {"trimEnd": 1}, "duration": 1 }] }`
2. **Keyframes and expressions on any property:**
   `"animate": [{ "from": {"scale": 3, "blur": 20}, "duration": 0.25, "ease": "outExpo" }]` · `"rotation": "=t*90"` · `"x": "=960 + 300*sin(t*2)"` · `"repeat": 40` with `$i`.
3. **Camera:**
   `"camera": { "animate": [{ "from": {"zoom": 1.3, "x": -600}, "to": {"zoom": 1, "x": 0}, "duration": 0.3, "ease": "outExpo" }], "shake": {"amount": 8, "at": 0} }`, plus `"motionBlur": true`. Layer `depth` + `aperture`/`focus` = parallax and rack focus.
4. **Your own GLSL** (a layer, `"filter": true`, or a transition), JIT-compiled at full resolution:
   `{ "shader": "void mainImage(out vec4 o, in vec2 f) { … iTime … }", "uniforms": {"speed": 1} }`
5. **Ray-traced 3D** (PBR, glass and chrome, glTF, instancing, real DOF):
   `{ "component": "scene3d", "props": { "env": "studio", "objects": [{ "text": "ANIKIT", "material": "chrome" }], "camera": "orbit" } }`
6. **Particles and physics** inside scene3d: `{ "type": "particles", "count": 400000, "emit": {"text": "IGNITE"}, "flow": 0.8 }`, `"physics": { "bodies": [{ "type": "box", "count": 400, "layout": "pile" }], "timeScale": "=1 - 0.7*smoothstep(2, 2.5, t)" }`.
7. **Compositions:** nested timelines used as layers, looped, or as textures on 3D objects: `"compositions": { "ui": {…} }` → `{ "composition": "ui", "loop": true }`.
8. **Music and grade:** `"music": "orchestral"` or `{"style": "acoustic", "bpm": 96, "key": "Cm", "hits": ["@bar5"]}` generates a mastered score that fills the video (`anikit-video docs music`: styles, instruments, note language). Cut on its exact grid with `"@beat9"` / `"@bar2"`; the built-in instruments sound far better than a hand-written synth. Grade: `"post": {"preset": "cinematic", "bloom": 0.4}`.

Combine them. `anikit-video docs make-anything` lists the escape hatches.

No limits: expressions on any number or colour (`"=hsl(beat * 30, 80, 60)"`, `level`, `c ? a : b`), `{ "script": … }` loops that generate layers, and `draw` layers of raw paths and points (`docs/video/authoring.md`).

**Costs (stay on the fast path):**
- 2D layers render at 100+ fps; shaders cost 50–400 ms a frame (`still` prints it).
- Ray-traced 3D and particles cost 0.5–1.5 s a frame. **Iterate with `render --quality draft`**, which traces 3D at half size automatically, then render the final at the default (standard) quality. Keep 3D shots to 2–4 s.
- A final with 3D takes minutes (`recipes` lists draft and final costs). Start it early in the background and keep working; check one shot at final quality with `render --from 2 --to 4`. Physics, particle and floor bakes are cached on disk (`~/.cache/anikit`), so re-renders skip them.
- `motionBlur` re-renders 2D layers per sub-frame (shader and 3D images are reused), so turn it on where motion is fast, not globally.
- Iterate with `preview` (and `render --quality draft` when you need to see motion); `--quality best` is for the final only and costs several times more.
- A layer's own timing is `start`/`end`; `at` is for tweens and `audio` cues.

## Ship gate (before the one final render)

Open the sheet and three full-size stills (`preview s.json --at T -o f.png`) and answer in writing, a line each:

1. **Is every frame visually striking on its own, a still someone would save?** It can be rich illustration, bold graphic design, beautiful light, spectacle, precise data design, texture or motion. What tends to get there: one strong image idea per shot, considered detail, real light and texture, contrast of scale. What tends not to: empty frames, a UI screenshot beside copy, default looks.
2. **What would make a viewer stop scrolling here?** "Incredible" comes in many forms: spectacle, wit, beauty, clarity, rhythm, surprise, craft. Name which one this piece is going for.
3. **Where is the weakest frame, and why?**
4. **Does the sound do as much as the picture?** (Its hits on the cuts; the instrumentation suits the idea.)

If an answer is weak, improve that one thing and preview again (it costs about a second), then render once; fixes after that go through `render --from/--to`. `anikit-video docs taste` has what tends to make work land.

## Shortcuts (optional; each is made from the primitives above)

- `anikit-video snippets [name]`: paste-able fragments so you write less (type treatments, line draws, camera push/snap/whip, acoustic and orchestral music, a film grade).

- **Launch, product or announcement video?** Use the `skills/anikit-launch-video/SKILL.md` skill (`anikit-video docs launch`): principles, process and quality checks; no template.

- `anikit-video recipes`: high-end shots to learn a technique from (copy the shot you need into your own concept) (launch-sting: particles + 3D + shader + music on the beat; kinetic-slam, whip-transition, chrome-hero, particle-ignite, domino-cascade, raymarch-world) and the shader library.
- `anikit-video looks`: art direction (palette, type, motion, grade). Use `"look": "noir"`.
- `anikit-video info --components`: prebuilt widgets (charts, devices, UI).
- `anikit-video templates`: quick starts. They are optional; don't let them define the piece.

## Direct it

- **Logline and feeling first,** then a shot list where each shot says what the viewer *understands*.
- **Declare `intent`** (`calm|steady|fast|frantic`). Fast means a cut or major change every 0.5–1.5 s on the beat, with whips, snaps, speed ramps and impacts. Calm can hold long; it still needs something alive in each shot.
- **Legible and well lit first:** words render at full resolution (no `resolution` < 1, DOF, blur or bloom on text while it is read), at 4.5:1 contrast or better, held long enough to read. Check stills at 100% for exposure: no clipped highlights or crushed blacks behind type.
- **Spend compute where the idea needs it** (ray-traced materials, a million particles, real motion blur), not as decoration. A strong 2D design beats a generic 3D one.
- **Build the world from the brief's own nouns,** with one subject per shot that fills the frame. Avoid centred headlines on gradients, placeholder UI and stock looks (docs/video/quality/BAR.md).
- **Judge by looking:** `critique --render --pack rv/` writes a strip, the cut frames and the worst frames boxed, plus `review.md` questions. The score is not a quality verdict; the frames are.

Over MCP (`anikit-video mcp`) the same exists: `docs`, `recipes`, `list_looks`, `validate_spec`, `critique_spec` (`render`, `detail`), `render_still`, `render_contact_sheet`, `render_video`.
