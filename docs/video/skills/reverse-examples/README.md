# Reverse-engineering examples

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
