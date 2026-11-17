---
slug: panoply-manifold-2024
category: cg-3d
title: "Manifold"
studio: "Panoply (CD: Mark Lindner)"
client: "Self-initiated"
year: 2024
url: https://vimeo.com/1022123584
alt_urls: [https://www.stashmedia.tv/data-dimensions-panoply-drops-manifold-short-film/]
length_s: 89
verified: oembed
aspect: "16:9"                      # est.
medium: [3d-cg, sim, ui]
pace: steady
palette: [dark, limited, high-contrast]
type_role: supporting
mood: [technical, minimal, eerie]
purpose: art
techniques: [cloth, sim, morph, hard-cut, camera-move, 3d-type, flash-frame, grid]
audio: sound-design-led
anikit_features: [scene3d, scene3d.physics, composition, text, camera, post, motionBlur, audio, "GAP: cloth (vellum-style) sim driven by an external field", "GAP: polygon slice-and-displace type effect on 3D text"]
awards: []
stills:
  - https://i.vimeocdn.com/video/1941015928-3482f0ccb1e173e546994220c38c7d3469d3d8d66fe68165167771cb8575d8aa-d_1280?region=us
  - https://panoply.co.uk/api/media/file/MAN24_HoldFrame.jpg
  - https://panoply.co.uk/api/media/file/MAN24_16x9_09.jpg
  - https://panoply.co.uk/api/media/file/MAN24_16x9_10.jpg
  - https://panoply.co.uk/api/media/file/MAN24_16x9_12.jpg
  - https://panoply.co.uk/api/media/file/MAN24_16x9_13.jpg
  - https://panoply.co.uk/api/media/file/MAN24_16x9_14.jpg
measured: null
---

# Manifold — Panoply (2024)

**Abstract CG with a rigorous idea: higher-dimensional shapes slicing through 3D space drive the behaviour of fabric, while CRT-style monitors show the raw data. The benchmark for sim-driven abstraction where every motion has a cause, plus integrated "data UI" and type.**

## The idea
"Inspired by the directness of early computers, where hardware and data felt raw and immediate, combined with the exploration of how higher dimensions might intersect with our 3D reality… much like a 3D object creating a 2D shape when intersecting a plane. These intersections drive the motion of fabric through time, creating a dynamic interplay between rigid geometric shapes and their fluid influence" (*source*: Vimeo description; CD Mark Lindner in Stash).

## Structure and pacing
- Length 89 s (*source*: oEmbed). Alternates fabric-sim hero shots with monitor and console inserts showing the captured data, ending on glitch-sliced end titles (*source*: Stash on monitors and end titles; order *est.*).
- Shots: about 25–35, median about 2.5 s (*est.*; to be *measured*).

## Typography
End titles: "we broke up the type by slicing and packing the polygons. Using a bounding box, we created a falloff that distorted the positions of the polygons, mirroring the motion of the scrolling consoles… These polygons were continuously recut to create a glitchy appearance. The type itself visually stretches through time, as if being scanned like the 3D objects… We used SDF shapes in COPs to create simple tileable patterns, which were then mapped onto the UVs" (*source*: Stash).

## Colour and light
Dark, restrained, hardware-like world: monitors as light sources, with fabric lit by hard key and practical glow (*est.*, from the "early computers" brief and the thumbnail).

## Motion techniques
- "We started by recording attributes from the 3D objects using rasterization within Houdini (COPs), generating image sequences that captured essential properties such as ID, color, and density. These image sequences are later displayed on the monitors" (*source*: Stash).
- "The data from these attributes are fed into the Houdini vellum solver, which continuously updates the density and rest lengths of the fabrics… frame-by-frame… a constantly shifting and responsive surface" (*source*: Stash).

## Transitions
Cuts between the 3D world and its data readouts; scan-line and glitch accents at section changes (*est.*).

## Sound design and music
Audio by Human Robot Soul (*source*: Vimeo credits). Expect granular, data-noise textures synced to fabric motion and console scrolls (*est.*).

## What makes it world-class
1. **Causality.** Fabric deformation is driven by the intersecting geometry's data, so motion looks motivated, not random.
2. **Diegetic data.** The monitors show the actual captured attributes: the UI is part of the world, not an overlay.
3. **Type treated with the same system.** End titles are sliced and scanned like the objects.
4. **Restraint.** A limited palette, dark world and a few materials.

## What Anikit would need
- `scene3d` with fabric-like surfaces and hard key light; `composition` rendered onto in-scene monitors (scene3d supports 2D compositions on 3D surfaces); `text` with `animator` for scan or stretch reveals; `post` (scan lines via `shader` filter); `audio`.
- **GAP:** cloth sim with per-frame rest-length and density control from an external field.
- **GAP:** polygon slice-and-displace effect on 3D type.

## Judge's checklist
- Does each deformation have a visible cause (an intersecting form, a field), rather than generic noise wobble?
- Do in-scene screens show content derived from the scene itself (diegetic data), not filler UI?
- Is the type treated with the same visual system as the imagery?
- Is the palette restrained (dark plus 1–2 accents), with clean blacks at 100% crop?

## Sources
- https://vimeo.com/api/oembed.json?url=https://vimeo.com/1022123584 (title, author, duration, description, credits)
- https://www.stashmedia.tv/data-dimensions-panoply-drops-manifold-short-film/ (Mark Lindner process quotes: COPs rasterisation, vellum, end-title technique)
