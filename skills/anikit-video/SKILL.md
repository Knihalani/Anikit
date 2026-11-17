---
name: anikit-video
description: Direct and render distinctive motion videos (launches, explainers, teasers, data stories, brand films, changelogs, demos) with the native anikit-video renderer: a JSON spec with art-direction looks, 3D, depth and post. Use when asked to create, review or render a video or animated clip.
---

**Setup:** if `anikit-video` isn't on PATH, install the prebuilt binary (a few seconds; don't compile it): `curl -fsSL https://raw.githubusercontent.com/knihalani/anikit/claude/epic-ramanujan-qe7x4w/install.sh | sh`, then `export PATH="$HOME/.local/bin:$PATH"`.

You are the director. A video that looks like every other generated video has failed, however clean it is. So has a slow one: single long dollies and 6 s holds read as boring, however beautiful.

**1. Direct before you build** (write it down, 10 lines max):
- *Logline*: who it is for, what changes for them, in one sentence.
- *One feeling* the viewer leaves with (calm precision, dread, joy, awe…).
- *Shot list*, cut to the music: per shot, **what the viewer understands** after it, the image, and its length in beats. Aim for 12–25 shots in a 15 s test, on a 120–140 BPM track. Each shot earns its place or goes; the story must make sense with the sound off.

**2. Choose a world:** `anikit-video looks` lists 10 (daylight-lab, editorial, swiss, brutalist, y2k-chrome, noir, risograph, neon-night, soft-clay, documentary). Pick from the brief's audience and feeling, never by habit, and pick a different one for different briefs. `"look": "noir"` at the top of the spec sets the palette, type, motion, transitions, camera, backdrop and grain. Compose (`["editorial", "noir"]`) or override (`{"base": "swiss", "theme": {"colors": {"primary": "#0050FF"}}}`). `anikit-video looks <name>` gives its direction in one paragraph: follow it.

**3. Build the world from the brief's own nouns.** Sailing means water, tide lines and rope; coffee means cherries, hands, soil and a map; horror means sea, rock and fog. Draw them with shapes and paths (`shape: "path"` with `d`, lines that draw on with `trimEnd`, `morph`) or with real photos. Stock effects (a halftone or kinetic grid, bokeh, a mesh) are seasoning, never the subject, and never the same one across your videos.

**Then build one signature image** that no template has, and give it the longest hold (`docs/video/authoring.md`: expressions, `script` loops, raw `draw` paths):
- a shader world: `{"type": "shader", "src": "scene.glsl"}` (Shadertoy GLSL, raymarched light, fog and water; `docs/video/shader.md`). Animate its uniforms hard, and cut between angles;
- 3D: `text-3d` (extruded chrome or matte type), `model` (rounded box, torus, knot, an extruded logo or an `.obj`), `globe`, and `scene3d` when `anikit-video info --components` lists it;
- generative: `particle-text`, `metaballs`, `kinetic-grid`, `morph`, `god-rays`, `tile-reveal`;
- real: photos or footage via `ken-burns` or `src` with `fit: "cover"`;
- depth: give layers `depth` (+ farther), then move `camera.x`: parallax. `camera.aperture` with an animated `focus` gives a rack focus.

**4. Pace: fast, dense and on the beat** (BAR's pace test; `python3 docs/video/quality/pace.py out.mp4`):
- Cut or make a major change every 0.5–1.5 s on the beat (`"at": "@beat17"`). No shot holds over 2.5 s; the median shot is 1.5 s or less; under 20% of seconds look like the second before.
- Camera with energy:
  - whip pans (`camera.x` across the frame in 0.15 s, with `motionBlur`);
  - snap zooms (`zoom` punch with `outExpo` in 0.2 s);
  - speed ramps (`inOutExpo` over a long move);
  - impacts (`shake` at a beat);
  - match cuts (one shape carried across a cut).
- Density: several things move in counterpoint, with layers in depth and parallax. Never one element alone on a still frame.

**5. Show what browsers can't.** Each shot shows something a browser tool (Remotion, or WebGL/three.js in HyperFrames) can't realistically render: see `docs/video/quality/ONLY_ANIKIT.md`. Examples are heavy per-frame compute at full resolution, true motion blur, and huge particle or ray counts. A pretty shader or a 3D logo alone doesn't count.

**6. Vary the grammar shot by shot:**
- Change the layout every shot: full-bleed type, one giant word, a split, a macro crop of the UI, data as the hero, an object in space.
- Use scale contrast: one huge thing per frame, everything else small.
- Entrances follow role: a headline `"mask by line up"`, a name `"pop by char from center"`, data `countUp`/`drawOn`.
- Motivate transitions: `zoom` into the object, `iris` on a circle, `cut` on the beat.
- Hand off between shots: an outgoing text never crossfades over an incoming one. Use a push, slide or cut, or an exit and then the entrance.
- Fill the format. In 9:16 and 1:1, stack the composition top to bottom: no band of empty frame bigger than a third.
- Match materials to the look. In flat looks (daylight-lab, swiss, editorial, risograph), 3D is matte (`metallic: 0`, `roughness: 0.8`, the look's colours); chrome belongs to y2k-chrome and noir. Show the brand once, at the end.

**7. Write the spec:**
- [`docs/video/spec.md`](../../docs/video/spec.md) is the one-page cheat sheet. `defs`/`use`/`repeat` keep specs short.
- Templates (`anikit-video templates`) are quick starts only. Restyle them with `--look`, then make them yours.

**8. Judge in motion against the bar** ([`docs/video/quality/BAR.md`](../../docs/video/quality/BAR.md)). A frame fails if it shows:
- a centred headline on a gradient;
- a blob or mesh backdrop doing the design;
- placeholder UI (grey bars, fake cards);
- small content in an empty frame;
- flat 2D with no light, material, depth or lens;
- a slide-deck structure;
- a stock-demo look (primitives, default Inter-on-dark, generic dashboard);
- an effect with no subject;
- any judder, banding, aliasing or clipped text.

A critique score is not evidence:
- `anikit-video critique spec.json --render` checks rendered frames for contrast, overlaps and cut text, and flags *sameness*.
- `anikit-video preview spec.json` makes a contact sheet. Look at it and ask, shot by shot, "do I understand X within 1 s?" and "would I screenshot this frame?"
- For motion, check stills at transitions and at entrances (`--at 2.3`). Fix three things, then re-check.

**9. Render, then run the pace check on the MP4:** `anikit-video render spec.json -o out.mp4 && python3 docs/video/quality/pace.py out.mp4`, with `--format 9:16` on templates or `"size": "9:16"` in a spec.

Over MCP the same tools exist (`list_looks`, `create_from_template`, `critique_spec` with `render: true`, `render_contact_sheet`, `render_video`).
