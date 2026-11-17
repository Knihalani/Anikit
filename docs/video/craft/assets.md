# Assets: photographed light, scanned surfaces, real objects

Why a design team's 3D looks expensive and an engine's default looks like a demo comes down to
three borrowed realities: **light that was photographed**, **surfaces that were scanned**, and
**objects that were modelled by someone who cared**. Anikit gets all three on demand from CC0
libraries (`anikit-video docs assets`). This page covers how studios use them, with numbers,
and what is still missing.

## Principles

1. **Light from a real place beats invented light.** An HDRI carries the sky's colour gradient,
   the sun's size and the bounce from the ground, so everything in the frame agrees. Reflections
   are where viewers notice fakes first. Chrome on a gradient looks CG; chrome reflecting trees
   and a horizon looks real ([Poly Haven HDRIs](https://polyhaven.com/hdris)).
2. **Choose the HDRI for its light, not its picture.** Lookdev artists pick by *time of day*,
   *contrast* and *sky view* (Poly Haven's own tags). Use a low-contrast overcast or studio HDRI
   for product clarity, a low sun (sunrise/sunset) for drama and long shadows, and night city for
   coloured practicals (tags on [polyhaven.com/hdris](https://polyhaven.com/hdris)).
3. **Rotate the world, not the camera.** Put the sun or brightest window 30–60° off the camera
   axis for modelling light. Behind the subject (120–180°) gives a rim, which suits glass and hair.
   `env.rotate` turns the whole world, light and reflections together.
4. **Blur the background, keep the light.** A sharp HDRI behind a product competes with it.
   Studios defocus it (long lens, shallow depth of field) or replace it. `background: "blur"`
   is about 3.4°, roughly f/2 at 85 mm on a distant background.
5. **Ground the object.** A floating object gives the fake away. A floor whose albedo matches
   the HDRI's ground, with contact shadows and AO, is how compositors "shadow catch". Anikit
   derives that albedo from the HDRI's own ground radiance divided by the sky's irradiance.
6. **Scale is the most common material error.** A plank texture at the wrong scale reads as a
   toy. ambientCG and Poly Haven publish physical dimensions; `scale: 1` honours them. Change it
   only on purpose ([ambientCG docs](https://docs.ambientcg.com/)).
7. **Roughness variation sells reality more than albedo.** Fingerprints, wear on edges and
   polish where hands touch are all roughness-map detail. A constant roughness is the clearest
   CG tell ([Adobe PBR guide, part 2](https://substance3d.adobe.com/tutorials/courses/the-pbr-guide-part-2)).
8. **Grade last, with a look, not with sliders.** A film LUT applied after tone mapping gives a
   consistent print response. Teams grade a sequence with one look and trim exposure per shot.

## Techniques with numbers

| What | Number | Source |
|---|---|---|
| HDRI resolution for lighting only | 1k (1024×512) is enough: diffuse uses 9 SH terms | [Ramamoorthi & Hanrahan 2001](https://graphics.stanford.edu/papers/envmap/) |
| HDRI resolution for sharp reflections / background at 1080p | 2k–4k (a 50 mm lens sees about 40° → 4k gives ~450 px across the frame) | geometry |
| Rough reflections | split-sum: pre-filter the map per roughness, GGX lobe width ≈ α = r² | [Karis 2013](https://blog.selfshadow.com/publications/s2013-shading-course/karis/s2013_pbs_epic_notes_v2.pdf), [Filament](https://google.github.io/filament/Filament.md.html) |
| Sun as a light | about 0.53° disc; an unclipped noon sun holds 50–90% of an outdoor HDRI's energy (measured here: noon_grass 56%, je_gray_park 90%) | measured |
| Texture density | about 1 texel per pixel at the subject; 2k sets cover ~1 m surfaces in a 1080p close-up | rule of thumb |
| Triplanar blend | weights = abs(n)^4, "whiteout" normal blend | [Ben Golus](https://bgolus.medium.com/normal-mapping-for-a-triplanar-shader-10bf39dca05a) |
| Foliage cutout | alpha test at 0.5 (glTF `alphaCutoff` default) | [glTF 2.0 spec](https://registry.khronos.org/glTF/specs/2.0/glTF-2.0.html) |
| LUT size | 33³ is the industry default (Resolve, Adobe); 17³ is visibly coarse on skin | Adobe Cube LUT Specification 1.0; Resolve's default export |

## Studio work vs amateur work

- **Studio:** one HDRI chosen for the brief, rotated deliberately, background defocused or
  replaced, a matched ground with contact shadows, materials at true scale with roughness
  breakup, one grade for the whole piece.
  **Amateur:** the default HDRI at 0°, sharp and in focus behind the product, objects floating,
  a plank texture ten times too big, glossy everything.
- **Studio:** props from one world (one period, one palette, one wear level).
  **Amateur:** a random mix of scanned rocks, toy-like primitives and stock models.
- **Studio:** credits the sources even when CC0 doesn't require it (`assets credits`).

## Licences (checked 2026-11-17)

- Poly Haven: every asset CC0 1.0 (https://polyhaven.com/license).
- ambientCG: every asset CC0 1.0 (https://docs.ambientcg.com/license/).
- Khronos glTF-Sample-Assets: licences differ per model. The index takes only models whose
  every `metadata.json` licence entry is CC0.
- Kenney: kenney.nl returned 403 from the build environment, so the index takes the 3D models
  from Kenney's starter kits on GitHub (`KenneyNL/Starter-Kit-*`), whose README and pack
  `License.txt` state CC0. They are pinned to the commit (72 stylised low-poly models).
- Film LUTs: generated by anikit's own parametric print model (`assetlib/lut.rs`), CC0.

## Gap list (Anikit vs a lookdev team)

| Gap | Status |
|---|---|
| HDRI env, PBR sets, textured glTF, alpha cutouts, LUTs, 2D image refs | done |
| Floor/ground PBR texture (`floor.texture`) | done |
| Displacement maps (only normal maps today: silhouettes stay smooth) | to do (needs tessellation or parallax) |
| EXR HDRIs, 8k+ backgrounds | `.hdr` only; 4k max per the index |
| Kenney's full catalogue (40k assets), Quaternius | kenney.nl blocked (403): 72 starter-kit models indexed |
| Search by image (a thumbnail contact sheet for `assets search`) | to do |
| Per-object HDRI light linking, multiple HDRIs blended | to do |
