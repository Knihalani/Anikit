---
slug: blender-studio-charge-2022
category: cg-3d
title: "CHARGE - Blender Open Movie"
studio: "Blender Studio (director: Hjalti Hjálmarsson)"
client: "Self-initiated (Blender Studio Open Movie #14)"
year: 2022
url: https://www.youtube.com/watch?v=UXqq0ZvbOnk
alt_urls: [https://studio.blender.org/films/charge/, https://studio.blender.org/projects/charge/]
length_s: null                      # official page: "3-minutes-long"; YouTube oEmbed gives no duration
verified: oembed
aspect: "16:9"                      # est.; check on analysis
medium: [photoreal-cg, 3d-cg]
pace: steady
palette: [dark, cool, high-contrast]
type_role: none
mood: [epic, raw, technical]
purpose: short-film
techniques: [camera-move, depth-of-field, particles, glow, light-sweep, hard-cut, rhythmic-edit, speed-ramp]
audio: mixed
anikit_features: [scene3d, camera, post, motionBlur, audio, "GAP: rigged realistic human character", "GAP: volumetric light / atmospherics in scene3d", "GAP: electric arcs / sparks FX"]
awards: []
stills:
  - https://i.ytimg.com/vi/UXqq0ZvbOnk/maxresdefault.jpg
  - https://i.ytimg.com/vi/UXqq0ZvbOnk/hq1.jpg
  - https://i.ytimg.com/vi/UXqq0ZvbOnk/hq2.jpg
  - https://i.ytimg.com/vi/UXqq0ZvbOnk/hq3.jpg
  - https://studio.blender.org/files/public/header/project/f6/76/f67601c6f69b2d7f65a5e5d97e73c4d7.png
  - https://studio.blender.org/files/public/header/blog/d5/65/d565eedccd5967feda7081ecd3d6f141.png
measured: null
---

# CHARGE — Blender Studio (2022)

**An openly licensed, game-cinematic-grade action short: the reference for how far a single team can push real-time-style PBR realism (EEVEE) in about 3 minutes.**

## The idea
"In an energy-scarce dystopia, an old destitute man breaks into a battery factory" and is confronted by a security droid (*source*: official logline as quoted in listings). Blender Studio's stated goal: "a high-visual-impact, action-packed 3-minutes-long animation inspired by the game cinematics and realtime demos formats", to "pursue realism and push Blender's capabilities in the interactive PBR workflow" (*source*: studio.blender.org/films/charge).

## Structure and pacing
- Length about 3 min (*source*: official page). Shot count about 40–60, median shot about 2.5 s, with fast clusters in the action beats (*est.*, typical of game-cinematic cutting; to be *measured*).
- Three acts (*est.*): infiltration (stealth, slow reveals) → confrontation with the droid (fast cutting, impacts) → resolution.

## Typography
None in the body of the film; only the title and end credits.

## Colour and light
- The entire production was rendered in EEVEE, Blender's real-time engine (*source*: premiere blog). The producer says the image pipeline stays in linear colour space with the "Filmic" view transform (*source*: premiere blog comments).
- Industrial interior: cool, desaturated steel and concrete broken by hot practical sources (indicator lights, energy cells, sparks) (*est.*, from the battery-factory setting and the YouTube stills).

## Motion techniques
- Realistic human character with hand-keyed performance (*source*: befores & afters calls it a "highly realistic character"). Lead animator Rik Schutte; animation by Pablo Fournier and Rik Schutte (*source*: credits page).
- Cinematography is game-cinematic: layout and cinematography by the director, with weighty camera moves, rack focus and shallow depth of field (*est.*).
- FX: shading and FX by Simon Thommes; modelling, lighting and FX by Andy Goralczyk (*source*: credits).

## Transitions
Hard cuts in an action-film grammar; tension built by alternating close-ups with wide shots (*est.*).

## Sound design and music
Music by Torin Borrowdale; sound design by Sander Houtman (*source*: credits page). Expect full Foley and impact design synced to the action (*est.*).

## What makes it world-class
1. **Material realism at real-time cost.** Skin, worn metal and concrete hold up in close-up under a Filmic, linear-light pipeline.
2. **Light motivated by sources in the scene.** Practical lights and energy glows drive the look; there is no flat fill.
3. **Weight in the character animation.** The droid and the man have mass, anticipation and follow-through. (Critics note the old man can move "like a 30 year old": a judge should check age-appropriate weight.)
4. **Game-cinematic shot grammar.** Clear screen direction, readable action beats and a continuous threat geography.

## What Anikit would need
- `scene3d` for hard-surface sets (metal, glass, emissive panels), `camera` moves with `aperture`/`focus` pulls, `motionBlur`, and a `post` cinematic preset (bloom from practicals, vignette, grain).
- `audio` for the score and hit points.
- **GAP:** rigged, skinned realistic characters with performance animation.
- **GAP:** volumetrics (light shafts, haze), electric arcs and spark FX at this fidelity.
- Realistically, Anikit should compete here only on hard-surface product or environment shots, not on character drama.

## Judge's checklist
- Is every light in the frame motivated by a visible or plausible source?
- Do metal and concrete surfaces show roughness variation and wear at 100% crop?
- In action beats, is screen direction consistent across cuts?
- Do impacts have anticipation, contact and settle (no floaty motion)?
- Is the tonal range filmic (highlights roll off, nothing clipped flat white)?

## Clip availability
Licence: Blender Studio content "is generally released under Creative Commons Attribution License (CC-BY), with copyright © Blender Foundation" (*source*: https://studio.blender.org/remixing/). No free direct download of the Charge master was found. The production files on https://studio.blender.org/projects/charge/ (original video file, no-music stereo master) are marked "Subscribe to view", and https://download.blender.org/demo/movies/ has no Charge folder (checked: it holds BBB, Sintel and Tears of Steel only). The official viewing copy is the YouTube upload above. If the lead needs an analysable copy, a Blender Studio subscription gives the official master under CC-BY. Attribution: "Charge, © Blender Foundation | studio.blender.org".

## Sources
- https://www.youtube.com/oembed?url=https://www.youtube.com/watch?v=UXqq0ZvbOnk&format=json (title, channel)
- https://studio.blender.org/films/charge/ (official description, 3-minute length, goals)
- https://studio.blender.org/projects/charge/ (production files: subscriber-only)
- https://studio.blender.org/projects/charge/pages/credits/ (credits)
- https://studio.blender.org/blog/charge-premiere/ (EEVEE for the whole production, Filmic, release 15 Dec 2022)
- https://studio.blender.org/remixing/ (CC-BY statement)
- https://beforesandafters.com/2023/03/13/how-blender-studios-latest-film-with-a-realistic-human-character-was-made/
- https://cgcookie.com/posts/hot-takes-on-charge-blender-studio-s-latest-short-film (search result: critique of character weight)
