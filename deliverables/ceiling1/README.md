# CEILING-1: what the engine can do at its best, and what held it back

Three pieces, each in a different style, each built from real references gathered first and compared side by side at full size. Per-piece details, references and ranked gaps are in each folder's `README.md` and `GAPS.md`.

| Piece | Style | Length | Render | Folder |
|---|---|---|---|---|
| **NIGHT CHORUS** | Kinetic-typography title sequence (tradition: *Enter the Void*, Saul Bass, *Se7en*) | 12.7 s, 30 fps | 252–264 s (title shots ~2.5 s/frame) | `nightchorus/` |
| **LAST LIGHT** | Cinematic 3D product film: a turntable in a sunbeam (references: Deakins, *Phantom Thread*, Vermeer) | 10 s, 24 fps | ~550 s (ray tracing + volumetrics, `quality: high`) | `lastlight/` |
| **OCTAVA** | Graphic-design-led brand film where the grid *is* the score (references: Müller-Brockmann, DIA) | 10.8 s, 30 fps | 7–20 s | `octava/` |

Fonts: `fonts/` holds 27 OFL families downloaded for these pieces (the engine bundles 3).

## Blind-judge scores (`scripts/judge.py run`, 3 trials, style-matched references)

| Piece | Version | Score | Refs | Delta | Judged pro | What changed |
|---|---|---:|---:|---:|---|---|
| NIGHT CHORUS | v1: 2D type cards (`nightchorus.mp4`) | 3.25 | 8.79 | −5.54 | 0/3 | |
| NIGHT CHORUS | v2: ray-traced street of signs, continuous dolly, choir | 3.62 | 8.83 | −5.21 | 0/3 | the credits became real signs in a world |
| NIGHT CHORUS | v3: + brick, rain, louder choir | 3.38 | 8.71 | −5.33 | 0/3 | within trial noise of v2 |
| NIGHT CHORUS | v4: signs ignite one by one with their choir voices; per-letter title | 3.38 | 8.83 | −5.45 | 0/3 | critic motion/sound/craft 6/6/6; the judge: "the signs should actually illuminate wet ground, brick and fog" (engine gaps 1–2 below) |
| NIGHT CHORUS | v5: + textured prints lit by local lights (engine fix), mid-tone brick | 3.62 | 8.79 | −5.17 | 0/3 | lit brick, falloff, spill; the judge's remaining notes are about concept and type system |
| NIGHT CHORUS | v6: + live GLSL sign treatments on 3D (engine fix) (`v2/street.mp4`, current) | 3.38 | 8.79 | −5.41 | 0/3 | the judge sees 6 stills, so animation barely registers; asks for a typographic system and varied staging |
| LAST LIGHT | v1 | 3.50 | 7.71 | −4.21 | 0/3 | |
| LAST LIGHT | v2: opener into the sun, wordmark projected by the light | **4.12** | 7.25 | −3.13 | **1/3** | end card replaced by an in-world gobo; establishing shot |
| LAST LIGHT | v3: + the dust settles into orbits around the record after the drop (current) | 4.00 | 7.29 | −3.29 | 0/3 | the 6 sampled frames rarely catch the orbit; same critique (model detail, one mood) |
| OCTAVA | v1 | 4.00 | 7.87 | −3.87 | 0/3 | |
| OCTAVA | v2: risograph print material, composer list cut | **5.12** | 7.63 | −2.51 | 0/3 | print material |
| OCTAVA | v3: layered outline wordmark, typographic lockup (current MP4) | 4.25 | 7.71 | −3.46 | 0/3 | same critique as v2; the spread is judge noise |

Raw judge results (JSON, no images) are in `judge_results/`.

**What the judge keeps asking for**, in its words: "an idea specific to the subject that generates the imagery", "real material and motivated light", "a typographic point of view", "choreography between frames". Its second-order asks for the 3D pieces map directly onto engine gaps: textured surfaces lit by local lights; emissive signs that light their surroundings; animated (shader) textures on 3D surfaces; a material library with normal and roughness maps. Those are the ceiling for the 3D work tonight.

## Engine change made on this branch
- **scene3d: texture prints are lit by local point/spot lights** (`trace.rs` `print_light`; commit c099f42). Before: brick-textured walls ignored lamps and sign spill, which was the judge's #1 complaint about the street ("greybox, signs light nothing"). After: pools of lamp light and coloured spill on textured facades. Proof (before | after): `proof_print_local_lights.png`. Test: `scene3d::tests::textured_prints_are_lit_by_local_lights`; all 50 scene3d tests and clippy pass. Cost: one shadow ray per shadowed light per textured pixel (the street went from about 2.1 to 4.2 s per still with 20 shadowed lamps).

- **Fix: shader compositions rendered blank as 3D textures** (commit cb4a8ea6). A composition used as a scene3d texture was first evaluated during the frame's scale-collecting pass (thread-local), where shader layers emit nothing, and the empty display list was cached. Texture evaluation now runs outside that pass (`eval::outside_scale_pass`). Test: `tests/textures.rs` `shader_composition_textures_a_3d_surface`. Proof: `proof_shader_texture_3d.png`. This lets NIGHT CHORUS's street signs carry their live GLSL treatments (bulb chase, moiré, neon flicker, settling chroma).

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
