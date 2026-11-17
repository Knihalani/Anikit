# CEILING-1: what the engine can do at its best, and what held it back

Three pieces, each in a different style, each built from real references gathered first and compared side by side at full size. Per-piece details, references and ranked gaps are in each folder's `README.md` and `GAPS.md`.

| Piece | Style | Length | Render | Folder |
|---|---|---|---|---|
| **NIGHT CHORUS** | Kinetic-typography title sequence (tradition: *Enter the Void*, Saul Bass, *Se7en*) | 12.7 s, 30 fps | 252–264 s (title shots ~2.5 s/frame) | `nightchorus/` |
| **LAST LIGHT** | Cinematic 3D product film: a turntable in a sunbeam (references: Deakins, *Phantom Thread*, Vermeer) | 10 s, 24 fps | ~550 s (ray tracing + volumetrics, `quality: high`) | `lastlight/` |
| **OCTAVA** | Graphic-design-led brand film where the grid *is* the score (references: Müller-Brockmann, DIA) | 10.8 s, 30 fps | 7–20 s | `octava/` |

Fonts: `fonts/` holds 27 OFL families downloaded for these pieces (the engine bundles 3).

## The ceiling, in one paragraph
With custom fonts, JIT GLSL over text channels, ray-traced 3D with volumetric light, and an authored score, the engine can already make frames that sit next to studio references. The title card of NIGHT CHORUS, the reveal of LAST LIGHT and the dates and lockup of OCTAVA are the evidence. What it can't do yet is let an agent get there *from the spec alone*. Every piece needed a Python generator, a downloaded font library, and workarounds for silent failures (wrong font, no texture, black or white exposure). The biggest wins are not new effects. They are removing silent failures and adding three primitives: per-glyph materials and SDF text channels, music-event-driven layers, and scene3d rigs and UVs.

## Cross-cutting gaps, ranked (all three pieces)

1. **Fonts [TYPE].** Only 3 bundled faces. Worse, failures are *silent*: the `fonts` map key is ignored; families register under names that are neither name ID 1 nor 16 ("Archivo Expanded Black" → "Archivo Expanded"); an unknown `font` falls back to Inter with no warning. This hit all three pieces. Fix: a bundled library; register IDs 1/16/21 plus the key; error on unresolved names; `info --fonts spec.json`.
2. **Silent failures in scene3d [3D].** Textures only print on faces with local normal ±Z (cylinders have no UVs); `exposure: 0` = black; `interior` auto-exposes a dark backdrop to white; `camera:false` objects cast no volumetric shadow; `over` particles ignore local lights. Each cost 2–6 iterations, and none warned.
3. **No music→picture event binding [SPEC/MUSIC].** OCTAVA (each note lights a cell) and NIGHT CHORUS (one credit per beat) both had to be generated in Python from one note list. Fix: layers driven by score events (`onNotes`, with `$pitch`, `$t`, `$vel`), or score events exposed to `script`.
4. **Text as material [SHADER/TEXT].** No per-glyph material (11 full-frame shader layers for an 11-letter title, 80% of render time) and no glyph SDF channel (neon, outlines and extrusion approximated by mip-blur). Fix: a glyph-index channel plus `sdf: true` channels.
5. **Bad defaults for cut-driven work [SPEC].** A 0.5 s crossfade between scenes by default; previews and `still --width` render shaders at reduced size, so procedural textures look broken in sheets; static contrast checks ignore shape layers under text; the pace critique flags deliberate holds.
6. **scene3d authoring [3D].** No parenting/rigs (a tonearm took per-part pivot expressions); no anisotropic sheen (vinyl grooves); volumetric shafts are grainy and miss thin occluders (blind stripes impossible); the ~5 m scale assumption is undocumented.
7. **Sound [MUSIC].** No crackle, hiss or room-tone sfx (synthesised from noise); the documented `music fetch salamander` fails (`--pack` works); `beats` misplaces bar 1; track-level `transpose` breaks tracks with `clips`. I also can't audition audio, only check envelopes; a `music --envelope` (loudness per 100 ms plus onsets) would help any agent verify silences and hits.
8. **Render cache granularity [PERF].** Moving one label in one 0.47 s shot re-rendered 15 of 16 chunks.

## Method notes (for other agents)
- Gather references first (artofthetitle.com contact sheets; film-grab.com for cinematography; Wikimedia Commons; the-brandidentity.com) and write down what you take from each.
- Iterate on **full-size** stills and downscale yourself; don't trust `--width` thumbnails for shader or procedural work.
- For 3D, build at ~5 m scale, find the light on single stills (~2.5 s each), and only then render.
- Generate specs with a script when picture and sound must share a structure, so they cannot drift.
