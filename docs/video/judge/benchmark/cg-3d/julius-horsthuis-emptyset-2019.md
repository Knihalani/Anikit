---
slug: julius-horsthuis-emptyset-2019
category: cg-3d
title: "Max Cooper - Emptyset - Official video by Julius Horsthuis"
studio: "Julius Horsthuis"
client: "Max Cooper (One Hundred Billion Sparks)"
year: 2019
url: https://vimeo.com/321969608
alt_urls: ["https://www.onehundredbillionsparks.net"]
length_s: 314
verified: oembed
aspect: "16:9"                      # est.
medium: [3d-cg, generative]
pace: calm
palette: [mono, light, limited]
type_role: none
mood: [calm, eerie, technical]
purpose: music-video
techniques: [shader, noise-field, camera-move, long-take, depth-of-field, beat-sync]
audio: music-led
anikit_features: [shader, camera, post, audio, "GAP: none fundamental: raymarched fractals run in the shader layer; a fly-through camera path for a shader (uniform-driven) is manual today"]
awards: []
stills:
  - https://i.vimeocdn.com/video/764809882-3f0f1517ebc6fca1e1487bbc9fe133fc2d9025d21b20306f148d21e9d7cab4eb-d_1280?region=us
  - https://images.squarespace-cdn.com/content/v1/553fb34be4b01d70400fc6dd/1551976407719-NPLQS7FXMOGJZUM09OLP/Emptyset_WIDE_FINAL+%280-02-25-00%29.jpg
measured: null
---

# Emptyset — Julius Horsthuis for Max Cooper (2019)

**Fractal CG made not to look like fractals: noisy, pencil-sketch-like structures grown by iteration, flown through slowly to an arpeggio. The benchmark for raymarched or fractal imagery with taste, a direct target for Anikit's `shader` and SDF work.**

## The idea
Max Cooper's chapter on set theory: numbers built from the empty set by iteration. He mirrored "this process of construction by iteration with a constant arpeggiated synth line throughout", and Horsthuis "applied fractal image generation to mirror the process visually, whereby simple structures repeat, grow and evolve to create something that looks like a pencil sketch of a real-world scene" (*source*: Vimeo description, Max Cooper). Part of the "One Hundred Billion Sparks" project.

## Structure and pacing
- Length 314 s (*source*: oEmbed). Long continuous fly-throughs of evolving fractal structures, with few cuts (*est.*; Horsthuis's signature style). Shots: about 10–20, median about 15 s (*est.*; to be *measured*). It is calm, but alive: structures continuously grow and change.

## Typography
None.

## Colour and light
Near-monochrome, "pencil sketch" look: pale, desaturated structures with fine grain detail (*source*: Max Cooper's "pencil sketch" description; palette *est.*).

## Motion techniques
- Made in Mandelbulb3D, but "the usual aesthetics of the Mandelbulb3D software had to remain hidden… I experimented by creating some fractals that where extremely high in noise, and sometimes, interesting patterns start to appear in this noise" (*source*: Horsthuis, Vimeo description).
- "Lots of disorder and fine grained noise to try and get that balance between certain form, and chaos" (*source*: Max Cooper).
- Iteration-parameter animation (structures growing in detail), with slow camera travel through the fractal space (*est.*).

## Transitions
Slow dissolves or continuous camera travel between fractal "scenes" (*est.*).

## Sound design and music
Max Cooper, "Emptyset": a constant arpeggiated synth line as "a never-ending mechanism of creation" (*source*). Visual growth paces with the arpeggio (*est.*).

## What makes it world-class
1. **Anti-cliché fractals.** It avoids the recognisable Mandelbulb look (rainbow, glossy bulbs) through noise and disorder.
2. **Concept match.** Iterative growth visualises iterative set construction.
3. **Fine-grain detail.** Dense, sketch-like micro-structure that rewards a 100% crop.
4. **Patience.** Long, steady camera travel lets the structure evolve in front of the viewer.

## What Anikit would need
- `shader` layer (GLSL raymarch of a fractal distance estimator plus noise), uniforms animated for iteration and camera, `supersample` AA, `post` grain; `audio` `iBeat` for arpeggio-locked growth.
- **GAP:** a convenient camera-path tool for shader cameras (uniform keyframes along a spline); temporal stability of fine noisy detail (shimmer) needs supersampling budget.

## Judge's checklist
- Does the fractal avoid the stock look (no rainbow palette, no glossy Mandelbulb bulbs)?
- Is fine detail temporally stable (no sparkle or shimmer as the camera moves)?
- Does structure evolve (grow or iterate) over time, not only through camera motion?
- Is the tonal palette restrained and intentional (sketch-like monochrome)?

## Sources
- https://vimeo.com/api/oembed.json?url=https://vimeo.com/321969608 (title, author Max Cooper, duration, full description with Max Cooper and Julius Horsthuis quotes)
