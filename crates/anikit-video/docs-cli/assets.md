# Assets: CC0 HDRIs, materials, models, LUTs on demand

Reference professional, freely licensed (CC0) assets by name. They download once
(pinned URL + checksum) into `~/.cache/anikit/assets` (`ANIKIT_ASSET_CACHE` to move it)
and are reused from then on. Nothing is bundled in the binary.

## Find one (don't browse: search)

    anikit-video assets search sunset --type hdri
    anikit-video assets search "brushed metal" --type texture
    anikit-video assets search plant --type model
    anikit-video assets list --type lut          # all 16 film looks
    anikit-video assets info polyhaven:hdri/kloppenheim_06
    anikit-video assets sheet sunset --type hdri -o pick.jpg   # look before choosing

Each line: `id  type  one-line description  [tags]  size`. Types: `hdri texture model lut image`.

## Refs

| ref | what |
|---|---|
| `polyhaven:hdri/<slug>` | HDRI environment (1k default; `@2k`, `@4k`) |
| `polyhaven:texture/<slug>` | PBR set: albedo, normal, roughness, AO, displacement (2k; `@1k`) |
| `ambientcg:<AssetId>` | PBR set from ambientCG (e.g. `ambientcg:Wood049`, 2k) |
| `polyhaven:model/<slug>` | glTF model with its textures (1k; `@2k`) |
| `khronos:<Name>` | CC0 glTF sample model (GLB) |
| `anikit:lut/<name>` | film LUT, generated locally (no download) |

Any Poly Haven slug or ambientCG id works even if it isn't in the curated index
(looked up live). The curated ~230 are pinned and have descriptions.

## Use them

    "scene3d": {
      "env": { "hdri": "polyhaven:hdri/studio_small_09", "rotate": 120, "exposure": 0.3, "background": "blur" },
      "objects": [
        { "type": "sphere", "material": { "texture": "ambientcg:Marble012", "scale": 1.5 } },
        { "type": "model", "src": "polyhaven:model/marble_bust_01", "pos": [1.2, 0, 0] }
      ]
    }

- `env.hdri`: lighting, reflections and background from the HDRI. `rotate` (degrees, spins the
  world), `exposure` (stops), `background`: `true` (sharp), `"blur"` (soft, keeps focus on the
  subject), `false` (lighting only; the floor/backdrop stays). The sun in an outdoor HDRI becomes a
  real key light with traced shadows.
- `material.texture`: a PBR set. `scale` = tiles per metre multiplier, `color` tints the albedo,
  `roughness`/`metallic` still override. Triplanar mapped, so no UVs are needed.
- `"floor": {"texture": "ambientcg:WoodFloor051", "scale": 1}`: the same on the ground.
- `"src": "polyhaven:model/…"` on a model object: any glTF from the library.
- 2D image layers: `{"type": "image", "src": "polyhaven:texture/concrete_wall_008"}` uses the
  albedo map (a texture, paper, plaster, fabric background).
- Grade: `"post": {"preset": "cinematic", "lut": "anikit:lut/teal-orange", "lutMix": 0.8}`;
  a `.cube` path works too.

## Offline, licences

`assets get <ref…>` pre-fetches (prints paths). Offline, a missing asset fails with a clear
message; `ANIKIT_OFFLINE=1` never touches the network. Downloads use the system `curl`.
All CC0 1.0: Poly Haven (polyhaven.com/license), ambientCG (docs.ambientcg.com/license),
CC0-only Khronos samples, anikit's own LUTs. `assets credits spec.json` lists what a spec uses.
