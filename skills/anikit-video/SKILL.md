---
name: anikit-video
description: Direct and render distinctive motion videos (launches, explainers, teasers, data stories, brand films, changelogs, demos) with the native anikit-video renderer: a JSON spec with art-direction looks, 3D, depth and post. Use when asked to create, review or render a video or animated clip.
---

You are the director. A video that looks like every other generated video has failed, however clean it is.

**1. Direct before you build** (write it down, 10 lines max):
- *Logline*: who it is for, what changes for them, in one sentence.
- *One feeling* the viewer leaves with (calm precision, dread, joy, awe…).
- *Shot list*: per shot, **what the viewer understands** after it (not what is on screen), the single image that makes them understand it, and seconds. 5–9 shots, 15–40 s. Each shot earns its place or goes; the story must make sense with the sound off.

**2. Choose a world:** `anikit-video looks` lists 10 (daylight-lab, editorial, swiss, brutalist, y2k-chrome, noir, risograph, neon-night, soft-clay, documentary). Pick from the brief's audience and feeling, never by habit, and pick a different one for different briefs. `"look": "noir"` at the top of the spec sets the palette, type, motion, transitions, camera, backdrop and grain. Compose (`["editorial", "noir"]`) or override (`{"base": "swiss", "theme": {"colors": {"primary": "#0050FF"}}}`). `anikit-video looks <name>` gives its direction in one paragraph: follow it.

**3. Build the world from the brief's own nouns.** Sailing means water, tide lines and rope; coffee means cherries, hands, soil and a map; horror means sea, rock and fog. Draw them with shapes and paths (`shape: "path"` with `d`, lines that draw on with `trimEnd`, `morph`) or with real photos. Stock effects (a halftone or kinetic grid, bokeh, a mesh) are seasoning, never the subject, and never the same one across your videos.

**Then build one signature image** that no template has, and give it the longest hold:
- 3D: `text-3d` (extruded chrome or matte type), `model` (rounded box, torus, knot, an extruded logo or an `.obj`), `globe`, and `scene3d` when `anikit-video info --components` lists it;
- generative: `particle-text`, `metaballs`, `kinetic-grid`, `morph`, `god-rays`, `tile-reveal`;
- real: photos or footage via `ken-burns` or `src` with `fit: "cover"`;
- depth: give layers `depth` (+ farther), then move `camera.x`: parallax. `camera.aperture` with an animated `focus` gives a rack focus.

**4. Vary the grammar shot by shot:**
- Change the layout every shot: full-bleed type, one giant word, a split, a macro crop of the UI, data as the hero, an object in space.
- Use scale contrast: one huge thing per frame, everything else small.
- Entrances follow role: a headline `"mask by line up"`, a name `"pop by char from center"`, data `countUp`/`drawOn`.
- Motivate transitions: `zoom` into the object, `iris` on a circle, `cut` on the beat.
- Hold the frame once a shot has landed, but never freeze for more than 2 s: give the hold a slow push (`zoom` 1 → 1.04), drifting dust or a breathing light.
- Hand off between shots: an outgoing text never crossfades over an incoming one. Use a push, slide or cut, or an exit and then the entrance.
- Fill the format. In 9:16 and 1:1, stack the composition top to bottom: no band of empty frame bigger than a third.
- Match materials to the look. In flat looks (daylight-lab, swiss, editorial, risograph), 3D is matte (`metallic: 0`, `roughness: 0.8`, the look's colours); chrome belongs to y2k-chrome and noir. Show the brand once, at the end.

**5. Write the spec:**
- [`docs/video/spec.md`](../../docs/video/spec.md) is the one-page cheat sheet. `defs`/`use`/`repeat` keep specs short.
- Templates (`anikit-video templates`) are quick starts only. Restyle them with `--look`, then make them yours.

**6. Judge in motion and fix:**
- `anikit-video critique spec.json --render` checks rendered frames for contrast, overlaps and cut text, and flags *sameness*.
- `anikit-video preview spec.json` makes a contact sheet. Look at it and ask, shot by shot, "do I understand X within 1 s?" and "would I screenshot this frame?"
- For motion, check stills at transitions and at entrances (`--at 2.3`). Fix three things, then re-check.

**7. Render:** `anikit-video render spec.json -o out.mp4`, with `--format 9:16` on templates or `"size": "9:16"` in a spec.

Over MCP the same tools exist (`list_looks`, `create_from_template`, `critique_spec` with `render: true`, `render_contact_sheet`, `render_video`). A worked example is [`examples/video/directed/`](../../examples/video/directed/).
