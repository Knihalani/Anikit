---
slug: universal-everything-walking-city-2014
category: generative
title: "Walking City"
studio: "Universal Everything (creative director: Matt Pyke; animation: Chris Perry)"
client: "Self-initiated"
year: 2014
url: https://vimeo.com/85596568
alt_urls: ["https://www.universaleverything.com/art/walking-city", "https://vimeo.com/718724283"]
length_s: 468
verified: oembed
aspect: "16:9"                      # est.
medium: [3d-cg, generative, sim]
pace: calm
palette: [light, limited, mono]
type_role: none
mood: [minimal, calm, premium]
purpose: art
techniques: [long-take, morph, loop, instancing, cloth, soft-body, noise-field, camera-move]
audio: sound-design-led
anikit_features: [scene3d, scene3d.instances, camera, post, motionBlur, audio, "GAP: skinned walk-cycle character with procedural surface swaps", "GAP: cloth / inflatable / particle-shell surface treatments on a moving body"]
awards: ["Prix Ars Electronica 2014 Golden Nica"]
stills:
  - https://i.vimeocdn.com/video/477386526-70ba52542b331c855705a1a54198d51278067bef12b0ab13d30314e445b94e69-d_1280?region=us
  - https://static.universaleverything.com/wp-content/uploads/2013/11/18160857/walking.jpg
  - https://www.thisiscolossal.com/wp-content/uploads/2014/02/walking.jpg
  - https://www.stashmedia.tv/wp-content/uploads/2014/02/UE_Walking-City.png
  - https://www.cartoonbrew.com/wp-content/uploads/2014/07/walkingcity.jpg
measured: null
---

# Walking City — Universal Everything (2014)

**One walk cycle, endless material metamorphosis: the Golden Nica-winning model of a "video sculpture" where a constant figure lets form, pattern and material carry everything.**

## The idea
"Referencing the utopian visions of 1960's architecture practice Archigram, Walking City is a slowly evolving video sculpture. The language of materials and patterns seen in radical architecture transform as the nomadic city walks endlessly, adapting to the environments she encounters" (*source*: Vimeo description). The studio's page adds: "influenced by biomorphic architecture and the work of people like Zaha Hadid, our walking minimalist building shifts its shape, patterns and geometric form. Against a white background, the focus is on material evolving and forming its own kind of narrative" (*source*: universaleverything.com).

## Structure and pacing
- Length 468 s (*source*: oEmbed). A single continuous figure walking in place, centred, with the camera tracking alongside (*source*: Vimeo "walks endlessly"; framing *est.* from the thumbnail).
- Essentially one long take that morphs through "dozens of permutations" (*source*: search-result summary of Colossal); each permutation lasts about 10–20 s (*est.*).

## Typography
None.

## Colour and light
White, seamless background (*source*: studio page). Soft, even studio light; the figure's materials supply all the colour, mostly restrained architectural tones (concrete, white, black, occasional colour accents) (*est.*).

## Motion techniques
- Made in Houdini (*source*: credits via search result: "Toolkit: Houdini"). "CGI software, originally used in Hollywood visual effects and physics simulations, to create an evolving building in movement" (*source*: studio page).
- A constant mocap- or keyframe-style walk cycle as the armature. Surface systems (panels, inflatables, cloth, particle shells, extrusions) are procedurally swapped and morphed across it (*est.*, from the description and the Citizens editions text "shrouded in a digital costume shifts and breaks, reshapes and endlessly evolves").

## Transitions
No cuts (*est.*). Transitions are surface morphs on the moving body: one material system grows over or dissolves into the next.

## Sound design and music
Soundtrack by Simon Pyke (Freefarm) (*source*: Vimeo description). Footstep-locked rhythmic sound design that changes timbre with each material (*est.*).

## What makes it world-class
1. **A constant figure with variable treatment.** Locking the walk cycle and framing makes every material change legible and comparable.
2. **Material storytelling.** Each permutation is a distinct architectural idea, not a recoloured version of the previous one.
3. **Seamless morphs.** Transitions happen on the body in motion, with no cuts and no pops.
4. **Radical minimalism.** A white void with one subject; nothing competes with the form.

## What Anikit would need
- `scene3d` with a white cyclorama (`env: studio`), a rigged walking figure, instanced surface elements (panels, spheres, extrusions) attached to it, a slow tracking `camera`, `motionBlur`, and a `post` clean preset.
- **GAP:** skinned character animation (walk cycle) and procedural surface attachment on a deforming mesh; cloth and inflation sims.

## Judge's checklist
- Is the subject constant (same gait and framing) so that material changes alone carry the piece?
- Is each surface treatment a distinct design idea (form, pattern, material), not a colour swap?
- Are transitions between treatments seamless on the moving body, with no pop?
- Does the white background stay clean (no banding, a crisp contact shadow under the feet)?

## Sources
- https://vimeo.com/api/oembed.json?url=https://vimeo.com/85596568 (title, author, duration, description, Golden Nica, Simon Pyke)
- https://vimeo.com/api/oembed.json?url=https://vimeo.com/96509736 (Walking City (Citizens) description)
- https://www.universaleverything.com/art/walking-city (studio description, Golden Nica 2014, exhibitions)
- http://prix2014.aec.at/prixwinner/12662/ (linked from Vimeo; not opened)
- https://www.thisiscolossal.com/2014/02/walking-city-an-evolving-video-sculpture/ (search result; embed ID)
