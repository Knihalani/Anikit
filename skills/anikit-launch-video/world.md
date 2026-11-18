# One world (steps 3–4)

Professional launch films in our benchmark (Coinbase, JPMorgan, XREAL, Nothing, Bang & Olufsen) hold one material, one light logic and one palette family for the whole film. They change the subject, scale and camera in every shot. Our failed films did the opposite: a new look per beat, or one layout repeated.

## Decide once, in `3-world.md`

- **Material:** one family, taken from the subject's own world: the place, objects and rituals of the people it's for, or the product's own surfaces. Don't pick from a list of nice materials. Not "dark ground with a glow".
- **Fidelity:** the shots that carry the device are built at full fidelity. That means real geometry and texture at 100% crop, atmosphere, and secondary motion. The jury's most common complaint about our 3D was "placeholder-level modelling". If a shot can't be made convincing, choose a medium where it can, such as illustration, print or type, rather than shipping a primitive.
- **Light:** a key light with direction, falloff and colour temperature, motivated by something in the world (window, lamp, screen, sun, the product itself). An arc over the film is good (dawn → noon, cold → warm). Radial glows and gradient washes are the tell.
- **Palette:** a family drawn from the material and light, with one accent reserved for the device. An arc within the family is good; swapping to an unrelated palette between shots is not.
- **Type voice:**
  - **The face:** choose it for this world (`anikit-video fonts`), and write why in one line.
  - **Placement:** type lives *in* the scene. It's printed on, carved, cast in light, made of the material, the UI's own content, or a `matte` window onto the world.
  - **Avoid:** centred headlines on fills and tiny tracked-caps eyebrows.
  - **Scale:** keep contrast high (one huge word beats three medium lines).
- **Sound world:** the instruments and the foley come from the subject: the product's sounds, the place's room tone, materials touching. Use `"sound": "auto"` plus explicit cues on the device's moments. Generate the music for the film's tempo (`docs music`).
- **Change per shot:** subject, scale (macro ↔ wide), camera angle and move. No two shots may share a layout.

## Anikit means (pick what the world needs, not what's new)

| World | Tools |
|---|---|
| Physical product, objects, places | `scene3d` with `env.hdri`, `material.texture` (ambientcg PBR), `rig`, `set`, `camera.lens`, `focus`, `fog`, practicals, decals |
| Illustrated | `illo`, `landscape`, `plants` with one `light` and a `hand` |
| Print and graphic | halftone, riso, `print` presets, `brush`, `grid`, editorial charts |
| Generative and natural | library shaders and `ak_` blocks, `fluid`, particles with `layers`/`glint`, `physarum`, cloth |
| Type-led | variable `axes`, `animator`, `matte` (type as window), text on a path, `extrude` |
| UI products | the real screens as lit planes in scene3d, or `product-shot`, at ≥ 26 px text |

Look things up with `anikit-video docs <topic>` / `docs key <name>`, one section at a time.

## Style frames (step 4)

Render three `still`s of the same key moment, each varying one decision, and view them side by side.

**Pick:**
- the one that's a finished poster, with one focal subject and no more than ~40% flat background;
- the one that holds up at 100% crop.

**Write in `3-world.md`:** which one you picked, and the one sentence that made it the winner.
