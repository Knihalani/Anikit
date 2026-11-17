---
name: anikit-video
description: Make extraordinary motion videos fast with anikit-video, a native renderer for JSON specs with ray-traced 3D, GLSL shaders, particles, physics, generated music and a reviewer that judges frames. Use when asked to create, review or render a video or animated clip.
---

**Setup:** if `anikit-video` isn't on PATH, install the prebuilt binary (seconds; don't compile it): `curl -fsSL https://raw.githubusercontent.com/knihalani/anikit/claude/epic-ramanujan-qe7x4w/install.sh | sh`, then `export PATH="$HOME/.local/bin:$PATH"`.

**The loop:** write `s.json` → `anikit-video still s.json --at 2.4 --width 640 -o f.png` (look) → `anikit-video render s.json -o out.mp4` → `anikit-video critique s.json --render --pack rv/` (then look at `rv/`).

**Need detail? Don't grep the docs: run `anikit-video docs <topic>`** (30 lines each, with a working example): `primitives animation camera compositions shader 3d particles physics music post pacing critique make-anything`.

## Build from the open primitives (anything is possible)

A spec: `{ "size": "16:9", "intent": {"pace": "fast"}, "music": "synthwave", "scenes": [ { "duration": 1.2, "layers": [ … ] } ] }`.

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
8. **Music and grade:** `"music": {"style": "synthwave", "bpm": 128}` (generated; cut with `"duration": "@bar2"`, `"at": "@beat9"`; never hand-write a synth); `"post": {"preset": "cinematic", "bloom": 0.4}`.

Combine them. `anikit-video docs make-anything` lists the escape hatches.

No limits: expressions on any number or colour (`"=hsl(beat * 30, 80, 60)"`, `level`, `c ? a : b`), `{ "script": … }` loops that generate layers, and `draw` layers of raw paths and points (`docs/video/authoring.md`).

## Shortcuts (optional; each is made from the primitives above)

- `anikit-video recipes`: drop-in high-end shots to adapt (kinetic-slam, whip-transition, chrome-hero, particle-ignite, domino-cascade, raymarch-world) and the shader library.
- `anikit-video looks`: art direction (palette, type, motion, grade). Use `"look": "noir"`.
- `anikit-video info --components`: prebuilt widgets (charts, devices, UI).
- `anikit-video templates`: quick starts. They are optional; don't let them define the piece.

## Direct it

- **Logline and feeling first,** then a shot list where each shot says what the viewer *understands*.
- **Declare `intent`** (`calm|steady|fast|frantic`). Fast means a cut or major change every 0.5–1.5 s on the beat, with whips, snaps, speed ramps and impacts. Calm can hold long, but never lifeless.
- **Show what browsers can't:** heavy per-frame compute such as ray-traced 3D, a million particles, real motion blur and raymarched worlds.
- **Build the world from the brief's own nouns,** with one subject per shot that fills the frame. Avoid centred headlines on gradients, placeholder UI and stock looks (docs/video/quality/BAR.md).
- **Judge by looking:** `critique --render --pack rv/` writes a strip, the cut frames and the worst frames boxed, plus `review.md` questions. The score is not a quality verdict; the frames are.

Over MCP (`anikit-video mcp`) the same exists: `docs`, `recipes`, `list_looks`, `validate_spec`, `critique_spec` (`render`, `detail`), `render_still`, `render_contact_sheet`, `render_video`.
