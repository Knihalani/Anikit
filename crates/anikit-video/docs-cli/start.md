# Start here: from zero to a render in 2 minutes

Install the prebuilt binary (seconds; do not build from source): `curl -fsSL https://raw.githubusercontent.com/knihalani/anikit/claude/epic-ramanujan-qe7x4w/install.sh | sh`. Then:

```bash
anikit-video docs <topic>        # one compact page per primitive (this list)
anikit-video preview s.json      # THE loop: validate + s-sheet.png (1 frame/s) + critique + pace, ~1 s for 2D
anikit-video preview s.json --at 2.4 -o f.png               # one moment at full size
anikit-video render s.json -o out.mp4
anikit-video critique s.json --render --pack review/        # judged against your "intent"; then LOOK at review/
```

A spec is JSON: `{ "size": "16:9", "scenes": [ { "duration": 2, "layers": [ ... ] } ] }`.
**Build anything from the open primitives**; the shortcuts on top are optional.

| Topic | What you get |
|---|---|
| `docs taste` | **read before designing:** routes to a striking frame (and the tools for each), what tends to land with a viewer and listener, what tends to fail |
| `docs primitives` | text, shapes, SVG paths, images, groups: full styling |
| `docs animation` | keyframes and `=expressions` on any property, presets, kinetic type |
| `docs camera` | zoom/pan/whip, shake, depth + rack focus, motion blur |
| `docs compositions` | nested timelines you reuse, loop, or put on 3D surfaces |
| `docs shader` | your own GLSL as a layer, a filter or a transition |
| `docs 3d` | `scene3d`: ray-traced PBR, glass/chrome, glTF, instancing, DOF |
| `docs particles` | millions of particles: emit from text/images, flow, attractors |
| `docs physics` | rigid bodies: piles, dominoes, impulses, speed ramps |
| `docs music` | generated music and `@beatN` cuts; sfx; voice-over ducking |
| `docs post` | grades, bloom, halation, grain, motion blur |
| `docs pacing` | intent, beat-cut timing, holds that stay alive |
| `docs critique` | the reviewer and the review pack |
| `docs analyze` | reverse-engineer a reference video: shots, pacing, palette, camera, beats → a draft spec |
| `docs board` | a look from reference images (paintings, photos, posters): palette, value, texture, composition → theme tokens |
| `docs make-anything` | escape hatches and how to combine them |
| `docs launch` | launch/product videos: principles, process, quality checks; no template (skill: skills/anikit-launch-video) |
| `docs expressions` · `docs script` · `docs draw` | per-frame maths and colours (music-reactive), generator loops, raw paths and points |
| `docs mograph` | After Effects-style motion design: repeaters, echo trails, motion paths, text on a path, shape modifiers, marching dashes |

Shortcuts (optional): `anikit-video recipes` (drop-in high-end shots), `looks` (art direction),
`info --components` (prebuilt widgets), `templates` (quick starts).
Rules that always hold: hook in ≤ 2 s · cut words, not type size · one focal point · every choice made for this subject (`docs taste`) · judge frames, not scores.
