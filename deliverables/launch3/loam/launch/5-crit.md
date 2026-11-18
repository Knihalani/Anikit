# 5 · Crit 1 (pass 1, blind comparison vs professional films)

`judge.py run launch/pass1.mp4 --practice --trials 2 --format 9:16 --tags product-launch,cg-3d --out launch/crit1`

**Candidate overall 3.25** (full with critic 3.56), judged professional 0/2; ref mean 6.44 (delta -3.19).
Refs: wealthsimple-trade 3.56, tiffany-holiday 7.88, da-vincis-demons 7.88. Ranking: candidate last or 3rd of 4 in both trials.

## Gaps (verbatim) and sort

- **IDEA/CRAFT** — It needs a real typographic point of view. Treat 'loam' as a designed wordmark, with scale contrast and type that interacts with soil, light or growth, instead of a default sans pasted on the floor.
- **WORLD** — The lighting and materials need to be rebuilt. Terracotta, soil and leaves deserve real texture, subsurface translucency on the leaves, and a motivated light that changes over time to sell the passing days.
- **IDEA** — It needs a less literal idea and more cinematography. Vary scale and camera (macro roots, a time-lapse arc, a surprising reveal) so the film has a memorable image like A's or D's.
- **IDEA** — It needs an idea beyond 'plants appear plus tagline'. Give the passage of time or growth a specific visual metaphor
- **CRAFT** — The typography needs a point of view: a chosen face, real scale contrast, and type that interacts with soil, light or shadow
- **WORLD** — The materials and light need real craft. Use richer soil, ceramic and leaf texture and dramatic, motivated light rather than a flat beige set

## Tells (verbatim)

- Stock 3D assets (pots and houseplants) sit on a flat grey floor against a drab olive wall. The materials are basic, the textures are low detail and the shadows are soft and generic.
- Typography uses default bold or italic grotesk text ('Not yet.', 'Sunday.', 'loam', 'Out now.') laid flat on the floor plane with no scale idea, kerning care or integration with the scene.
- The camera and composition barely change: a centred pot, then three pots in a row. The 'window light' frame is a single gobo plane.
- The central idea (plants growing until launch day) is literal and obvious, with a product name and 'Out now.' as the payoff, like a template.
- The muddy beige-olive palette has no accent and no intent. Light is flat and unmotivated except for one window cookie.
- A literal, obvious idea: plants grow, 'Not yet.', 'Sunday.', then 'loam / Out now.' It is a product launch reduced to a caption sequence
- Default bold grotesk set in perspective on the floor like a slide. 'loam' is cropped mid-frame. There is no typographic idea or scale system
- Stock 3D assets: generic pot and plant models, a bland stucco wall and a beige floor. The pot in frame 2 has an odd modelling artefact
- Muted, single-note beige and olive palette. The window-light gobo is the only lighting idea, and it is a stock move
- The same tabletop set repeats with little change. The motion amounts to a camera push and objects appearing

## Critic (motion/sound/craft)

- 100% crop: the wall/floor horizon line shows hard stair-step aliasing; it reads as a raw real-time render edge, not a resolved one
- Plant foliage has cutout or alpha-card edges with hard dark fringes and no soft self-shadowing, so the leaves look like pasted textures against the softly lit wall
- Contact shadows are weak or missing: pots seem to float on the table (frame 2 close-up, frame 3 urn), and the large blurry wall shadows aren't anchored to any object
- The white pot carries an odd melted-looking tag or spout attachment that reads as a modelling artefact at close-up
- Floor type ('Not yet.', 'Sunday.', 'A splash.') is a generic heavy sans with soft, blurry perspective rendering, and the 'loam' wordmark is cropped at the frame edge mid-move
- Pairs show almost no change 1/6 s apart in every row: the shots are slow, uneased camera drifts over static sets, which counts as dead air
- There is a 5.0 s hold, and 3/14 seconds look the same as the previous second; the end tag sits static for about 4 s with only a light change
- End card: 'Out now.' is set in a second, smaller green type treatment that clashes with the grey floor-projected system
- gap: Motion: no real choreography. Nothing grows, gets watered, sways or reacts, and the 'splash' moment shows no splash. The idea (the plant's answer written on the floor) needs a physical event in each shot: soil darkening, leaves lifting, a shadow sweeping across as time passes. Instead it's a camera dolly over still life. Top studios would also use the window-light shadow sweep as the transition between shots rather than hard cuts.
- gap: Sound: the spectrogram shows one repeated pluck or percussion loop with a flat, static wideband noise bed (5.6–7.6 s) and a dropout to silence (~9–10 s). That's the only dynamic gesture, and it isn't clearly tied to a cut or to the reveal. There's no foley (water, soil, ceramic) and no sound design selling the material world, and nothing visible lands on the transients.
- gap: Finish: real-time-engine quality, with an aliased horizon, cutout foliage, floating pots and soft, low-contrast floor type. The wall texture and light falloff are pleasant, but the render needs AA, proper contact occlusion, leaf subsurface and crisper projected type to reach studio level. The energy curve is also flat, with no build to a peak before the logo.

## Reading it

- **IDEA gaps present** ("less literal idea", "an idea beyond plants appear plus tagline", "a caption sequence"). The jury read the shadow words as captions on a floor — the device didn't read as *the light speaking*; it read as type laid on a set. Rule 1: replace the idea from a different family, not polish it.
- **WORLD:** "flat beige set", "muddy beige-olive palette", "light flat and unmotivated except for one window cookie" — and the one frame they singled out as having a lighting idea was the window-cookie frame. Pass 2 builds the whole world on that hard window light.
- **CRAFT:** default bold grotesk on the floor; "loam" cropped mid-move; the sensor reads as a melted tag; nothing moves in the frames (dead air, 5 s hold); "Out now." in a clashing second type treatment; weak contact shadows.
- **Cut the weakest third:** the floor captions (all four), the 5 s static end hold, the cloud shot as a dark beat.

## Decision
Back to step 2 with a different family: a **weather** device (rain falls indoors on exactly one pot), not a **type-in-light** device. New world: hard window-pane sun on a deep green wall. Type only at the end, made of the film's own material (water).

---

# 5 · Crit 2 (pass 2: indoor weather)

`judge.py run launch/pass2.mp4 --practice --trials 2 --format 9:16 --tags product-launch,cg-3d --out launch/crit2`

**Candidate overall 4.12** (full with critic 4.47), judged professional 0/2; ref mean 5.94 (delta -1.82). Ranked 3rd of 4 in both trials (above wealthsimple-trade).

## Gaps (verbatim) and sort

- **IDEA** — Find a specific idea: make the soil, roots or moisture the visual world instead of filming a pot being rained on
- **CRAFT** — Typography needs a point of view, with a custom or chosen face, scale and integration with the image instead of a centred wordmark plus a default tagline
- **WORLD** — Build depth and material: dense, layered frames with real soil, terracotta and leaf texture, as in A, and varied shot design instead of one repeated set-up
- **IDEA** — It needs an idea beyond literal watering: a point of view on what the sensor means (time, care, the plant's perspective).
- **WORLD** — Model consistency and craft need work: the pot, soil and sensor should be one coherent, detailed asset across every shot, with real material response to the water.
- **CRAFT** — The typography needs a designed logotype and integration with the image instead of a centred default grotesk with a particle effect.

## Tells (verbatim)

- The same set-up repeats: one terracotta pot, one plant, one diagonal window shadow, with the camera only pushing in and out
- The idea is literal: water falls on the plant, a sensor sits in the soil, the product name appears. Nothing would be remembered or shared
- Rain and spray are default particle emitters with no designed shape or weight
- 'loam / Out now.' is set in a generic bold grotesk, centred like a slide, with a particle-dissolve effect standing in for a typographic idea
- Window-shadow light is the one real asset, but it is the standard hobbyist 3D lighting trick, and the materials (pot, soil, sensor) read as untextured defaults
- One idea stated literally: a smart plant sensor, falling water, then the product name. It amounts to the product plus a diagram of what it does.
- Nearly the same wide shot of the pot against a green wall with window-shadow stripes repeats in four of six frames.
- The pot's shape and decoration are inconsistent between frames, and in frame 3 the pot warps. That looks like an automated or generative inconsistency.
- The end card is a generic bold sans 'loam' with a particle-dissolve effect and a default 'Out now.' line placed centrally, like a slide.
- The window-shadow light is the one considered element, and it is a stock trope.

## Critic

- Rain is drawn as rows of evenly spaced dots instead of motion-blurred streaks, so each drop reads as a stepped stack of samples (very visible in the close-up shots).
- The rain looks pasted on as a flat 2D layer: it doesn't pick up the scene's depth of field, it sits in front of the leaves without proper depth sorting, and it leaves no splashes or wet spots on leaves or soil.
- The pot changes design between shots, from a round pot with a lip to a footed urn, with no motivation, which breaks continuity.
- The wide three-pot shot has a large, flat, unlit dark-grey area of floor in the lower half that reads as an empty render void.
- The 'l' of the 'loam' logo forms more sparsely and raggedly than the other letters, so the wordmark looks unfinished when it settles; the tagline 'Out now.' uses a generic bold sans that doesn't relate to the particle logo.
- In the burst shot the particle field looks uniform, with no clear direction, focal point or variation in density; at 100% the particles are soft, with no light falloff or interaction with the scene.
- There is a 5.0 s shot with low change per second, which is dead air for a 15 s piece.

## Decision

- Pass 2 (4.12) beats pass 1 (3.25) by 0.87, more than the 0.75 tie band: **ship pass 2** (`final.mp4` = `launch/pass2.mp4`).
- Crit 2 still has an IDEA gap ("literal watering"; make the soil and moisture the world). A third pass was being scoped (a below-the-soil world) when the session was stopped, so it wasn't built.
- Known craft debts in the shipped pass: rain drawn as stepped dots in the macros, no splashes or wet soil, the empty dark floor in the wide three-pot shot, a ragged 'l' in the particle name, and a 5 s macro hold.
