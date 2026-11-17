# Concept generator: one world per video

A launch video fails when it looks like the last one. Before writing any JSON, pick **one concept**, then fill its four
slots from the brief. The concept is the world; the four slots make it this product's world.

## 1. Find the product's metaphor (30 seconds, no questions)

Write three lines in your notes:
- **Noun:** the physical thing the product is most like (a ledger, a lens, a loom, a lock, a tide, a switchboard).
- **Verb:** what it does to the user's problem (sorts, shields, compresses, connects, reveals, accelerates).
- **Before → after:** the problem as an image, then the same image resolved (tangle → straight line, noise → signal).

The concept below that best stages *that verb on that noun* wins. If two fit equally, take the one at index
`(sum of the product name's character codes) mod 12`, so different products drift to different worlds.
If earlier launch videos exist in the working directory, never reuse their concept.

## 2. The twelve worlds

| # | Concept | World and hero | Motif carried through every shot (the match-cut device) | Signature Anikit capability | Music |
|---|---|---|---|---|---|
| 1 | **Material world** | The product's UI printed onto a physical material: glass panes, anodised aluminium cards, ceramic tiles, in a ray-traced set | one material (e.g. smoked glass) that every element is made of | `scene3d` panes with `texture: "#ui"`, real DOF, chrome reflections | house 124–128 |
| 2 | **Macro product** | Extreme close-ups of the device/screen: pixels, bezels, a cursor the size of a car, racked focus | the cursor / caret as a physical object | `scene3d` `phone`/`card` + `fstop` 1.4–2, `focus` keys, motion blur | tech 128 |
| 3 | **Typographic** | The words *are* the set: giant extruded letters, type as architecture, the camera flies through counters | one word that keeps transforming (morphs into the next feature) | `extrude`/`text` objects in chrome or matte + kinetic 2D type slams on beats | trap 140 / dnb 172 |
| 4 | **Data-driven** | The product's own numbers become a landscape: heightfields, bar forests, flowing streams of records | one line/curve that rises across the whole film and becomes the logo underline | `heightfield`/`count` instancing with `"=expr"` per instance, 100k+ particles as records | tech / synthwave |
| 5 | **Physical simulation** | The problem is a pile of stuff (boxes, tiles, coins) that the product sorts, stacks or topples | one object type (a tile) that falls, stacks, then forms the logo | `physics` bodies with `timeScale` speed ramps, dominoes, piles | cinematic-hybrid hits |
| 6 | **Particle alchemy** | Chaos → order: a cloud that condenses into the UI, the feature words, then the logo | the particle colour ramp = brand colours, re-used on every hit | `particles` `emit: {text}`, `flow`, 400k+ points, additive bloom | synthwave 118–124 |
| 7 | **Shader world** | A procedural landscape that embodies the verb (a tide, a field of light, a tunnel) the product flies through | the horizon line, kept at the same height across cuts | raymarched GLSL layer (`recipes` shader library: `sdf-scene`, `liquid-chrome`, `caustics`, `light-shafts`, `turing` …) with 2D type on top | dnb / tech |
| 8 | **Blueprint / exploded view** | The product disassembled into labelled layers that snap together on the beat | the assembly axis: every shot moves along it | `scene3d` `slab`/`pane` stack with `keys`, `draw` path callouts that trim on | tech 120 |
| 9 | **Light & shadow** | A dark room; the product is revealed only by moving light (blinds, a spot sweep, god rays) | one light source that sweeps each shot | `product-shot` component (blinds), spot lights + `fog.shafts` | cinematic 96–110 |
| 10 | **Collage / editorial** | Magazine layouts in motion: serif headlines, cutout UI, paper and ink, hard cuts | a red rule line / a crop mark | 2D primitives at 100+ fps: dense type, masks, `split: chars`, many cuts | lofi / jazz 90 |
| 11 | **Miniature city** | The workflow as a tiny world: instanced buildings = files, roads = pipelines, tilt-shift DOF | the camera's fixed tilt-shift angle | instancing (`count`, `lanes`) + shallow `fstop` + `cyc` env | pop / corporate-bright |
| 12 | **Signal / sound** | The product as an instrument: waveforms, level-reactive everything, cuts are drum hits | a waveform that becomes the logo stroke | `level`/`beat` expressions on every layer, `visualizer`, sfx on each hit | house / techno 128 |

## 3. Fill the four slots

1. **Palette:** brand colours if given. Otherwise derive from the noun (tide → ink navy + foam white + one
   signal coral). Max one accent; it marks the hits only. Never rainbow, never purple-blue SaaS gradient.
2. **Type:** one display face + one text face. Brand fonts if given, else: technical → `Inter Display` 800–900 tight
   tracking + `JetBrains Mono` labels; human/crafted → `Instrument Serif` + `Inter`. Never default Inter-on-dark alone.
3. **Hero treatment:** how the user's screenshots appear (as a composition texture on panes/phone/card, or flat in
   collage). Real copy from the brief only; no grey placeholder bars.
4. **The turn:** the single moment (usually the reveal on `@bar3`) where the world changes state: tangle snaps straight,
   pile becomes logo, dark room lights. Everything before builds to it; everything after rides it.

Write the concept, motif and turn into the spec's top-level `"about"` so the next run can avoid it.
