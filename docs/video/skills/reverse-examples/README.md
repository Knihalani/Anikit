# Reverse-engineering examples

## `fernway-to-tideline/` (the good one)

- **Reference:** `docs/video/skills/launch-examples/fernway/fernway.mp4`, our own 15 s 9:16 motion-design launch.
- **Recreation:** the same style for a different product, *Tideline* (offline sea charts).
- **Method:** following `skills/anikit-reverse-video/SKILL.md`:
  1. `analyze` gives 12 shots, median 0.93 s, 128 BPM, 100% of cuts on a beat, and a grade of lifted blacks with low contrast.
  2. Looking at the key frames gives the style breakdown: dark rendered terrain at dusk with a warm key light; an **amber glowing bead route as the recurring motif**; a high-contrast serif for statements (one glowing amber word); a heavy grotesk plus italic serif for feature lines; a mono for data; chart-paper inserts with an orange route and a dark label bar; a phone in the world; fast snaps on the beat; a serif wordmark end card.
  3. Each shot is rebuilt with the same medium, type system, light and motif, but new subject and copy: a swell instead of ridges, a sea route, depth soundings, "No coverage. / No chart. / Every *channel.*".
- `make.py` writes `tideline.json`.

**Open `pairs.jpg`:** each reference key frame (left) next to the recreation at the same time (right).

| | Fernway (reference) | Tideline (recreation) |
|---|---|---|
| shots / median / longest | 12 / 0.93 s / 1.90 s | 12 / 0.93 s / 1.90 s |
| pace / tempo / cuts on a beat | fast / 128 BPM / 100% | fast / 128 BPM / 100% |
| analyze time (15 s, 1080×1920) | 2.4 s | 2.9 s |
| render (`--scale 0.5`, 4 cores) | | 1 min 48 s |

Honest gaps:
- Shot 3 shows mirrored fragments of the extruded word reflecting in the water.
- Shot 11 has a stray glint.
- The sea reads as rolling hills more than water.

Files:
- `ref-analysis.json`, `ref-sheet.jpg`: analysis of the reference.
- `draft.json`: the skeleton `--spec`, with `notes` per scene and no invented layers.
- `recreate-analysis.json`, `recreate-sheet.jpg`: analysis of the recreation.
- `tideline.mp4`: the recreation.

**Limit:** motion design recreates well. Live-action or character-animation footage can't be rebuilt in a motion-graphics engine, only its cutting, framing, grade and type.

## First attempt (rejected): Big Buck Bunny and Sintel trailers

The files below are kept as a record of what *not* to do. The timing matched, but the recreations were placeholder art (centred caps on white, circles on a stock shader, a torus), because the old draft invented layers and the agent built from numbers without studying the frames. Their `*-draft.json` files predate the skeleton draft.

`anikit-video analyze` on two public trailers, then a recreation of each one's *structure, pacing and style* for a made-up product, following `skills/anikit-reverse-video/SKILL.md`. No footage, characters, logos or music from the references are in the recreations.

References (Blender Foundation open movies, CC BY 3.0, © Blender Foundation, www.blender.org; used here only for analysis and the side-by-side comparison):
- *Big Buck Bunny* trailer (1080p, 25 fps, 33.0 s): `bbb-*`
- *Sintel* trailer (1080p, 24 fps, 52.2 s): `sintel-*`

| File | What |
|---|---|
| `*-ref-analysis.json`, `*-ref-sheet.jpg` | `analyze ref -o … --sheet …` on the reference |
| `*-draft.json` | `analyze --spec` output (passes `check`) |
| `*-recreate-spec.json` | the draft adapted to new content by `adapt-draft.py` (the edits step 4 of the skill describes: new copy and subjects, same timing, transitions, camera and tempo) |
| `*-recreate.mp4` | render at `--scale 0.5` (960×540) |
| `*-recreate-analysis.json`, `*-recreate-sheet.jpg` | `analyze` on the recreation, to compare with the reference |
| `*-side-by-side.mp4` | reference (left) and recreation (right), 640×360 each |

## Measured (4 cores)

| | BBB reference | BBB recreation | Sintel reference | Sintel recreation |
|---|---|---|---|---|
| analyze time | 2.66 s (32.5 s of 1080p) | 1.51 s | 4.27 s (52.2 s of 1080p) | 2.03 s |
| shots | 17 | 17 | 12 | 10 |
| median / longest shot | 2.00 / 4.20 s | 2.00 / 4.21 s | 2.08 / 19.5 s | 2.08 / 19.5 s |
| pace | steady | steady | steady | steady |
| transitions | 12 cut, 2 crossfade, 2 zoom | 13 cut, 3 zoom | 7 cut, 4 crossfade | 7 cut, 2 crossfade |
| mean luma | 0.81 | 0.76 | 0.26 | 0.25 |
| tempo / cuts on a beat | 123 BPM / 56% | 123 BPM / 50% | 122.9 BPM / 36% | 123 BPM / 56% |
| render time | | 56 s | | 6 min 37 s (shaders + scene3d) |

Gaps the side-by-side shows, which an agent would fix next:
- **BBB:** the reference's world shots are character animation, and the recreation uses shader backdrops with an icon instead. Its two opening crossfades came out as cuts or zooms.
- **Sintel:** two of the four crossfades were lost, because they fall within a few frames of a cut. The 19.5 s "shot" is the logo build and end card, which really is a slow fade sequence.

Known limits of `analyze` seen on these clips:
- Cuts inside very fast action (BBB 20–22 s) merge into one shot.
- Fades from black into a mostly black card aren't detected.
- `look`/`technique` is a heuristic: the sheet and the key frames decide.
