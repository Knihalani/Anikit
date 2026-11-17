# Benchmark tag vocabulary

Every piece's front matter uses these words so `match.py` can compare our videos with the set. Add a word only if none fits, and add it here.

| Field | Allowed values |
|---|---|
| `medium` (list) | `2d-vector`, `2d-illustrated`, `cel`, `frame-by-frame`, `kinetic-type`, `3d-cg`, `photoreal-cg`, `stylised-3d`, `live-action`, `mixed`, `stop-motion`, `generative`, `sim`, `data-viz`, `ui`, `collage`, `photo` |
| `pace` | `frantic` (median shot < 0.7 s), `fast` (< 1.5 s), `steady` (< 3.5 s), `calm` |
| `aspect` | `16:9`, `9:16`, `1:1`, `4:5`, `2.39:1`, `other` |
| `palette` (list) | `mono`, `dark`, `light`, `saturated`, `pastel`, `neon`, `earthy`, `duotone`, `limited`, `warm`, `cool`, `high-contrast`, `brand-colour`, `retro` |
| `type_role` | `none`, `supporting`, `lead`, `only` |
| `mood` (list) | `playful`, `epic`, `premium`, `minimal`, `energetic`, `warm`, `eerie`, `editorial`, `technical`, `emotional`, `witty`, `raw`, `luxurious`, `calm`, `retro` |
| `purpose` | `title-sequence`, `launch`, `ident`, `explainer`, `music-video`, `lyric-video`, `ad`, `social-ad`, `broadcast`, `promo`, `art`, `data-story`, `short-film`, `ui-demo` |
| `techniques` (list) | `match-cut`, `kinetic-type`, `type-morph`, `morph`, `masks`, `camera-move`, `whip`, `speed-ramp`, `beat-sync`, `loop`, `split-screen`, `grid`, `parallax`, `depth-of-field`, `glow`, `grain`, `halftone`, `smear`, `boil`, `particles`, `sim`, `fluid`, `cloth`, `soft-body`, `rigid-body`, `instancing`, `shader`, `noise-field`, `light-sweep`, `macro`, `product-turntable`, `ui-zoom`, `cursor`, `chart-build`, `map`, `collage-cutout`, `stop-motion-feel`, `rotoscope`, `typographic-grid`, `scale-contrast`, `long-take`, `hard-cut`, `flash-frame`, `colour-swap`, `rhythmic-edit`, `liquid`, `logo-build`, `3d-type`, `transitions-in-camera`, `glitch` |
| `audio` | `music-led`, `sound-design-led`, `vo-led`, `lyric`, `silent`, `mixed` |
