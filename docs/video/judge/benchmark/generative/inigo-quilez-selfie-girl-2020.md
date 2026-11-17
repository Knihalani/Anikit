---
slug: inigo-quilez-selfie-girl-2020
category: generative
title: "Painting a Character with Maths"
studio: "Inigo Quilez"
client: "Self-initiated"
year: 2020
url: https://www.youtube.com/watch?v=8--5LwHRhjk
alt_urls: [https://www.shadertoy.com/view/WsSBzh]
length_s: null                      # making-of is about 25 min (MetaFilter, via search; not opened); the shader itself is a short real-time loop
verified: oembed
aspect: "16:9"
medium: [generative, 3d-cg, stylised-3d]
pace: calm
palette: [warm, saturated, light]
type_role: none
mood: [playful, technical, warm]
purpose: art
techniques: [shader, noise-field, depth-of-field, camera-move, loop]
audio: vo-led
anikit_features: [shader, "GAP: none for the shader itself (Anikit runs Shadertoy GLSL via naga + Cranelift); performance at 1080p supersampled is the open question"]
awards: []
stills:
  - https://i.ytimg.com/vi/8--5LwHRhjk/maxresdefault.jpg
  - https://i.ytimg.com/vi/8--5LwHRhjk/hq1.jpg
  - https://i.ytimg.com/vi/8--5LwHRhjk/hq2.jpg
  - https://i.ytimg.com/vi/8--5LwHRhjk/hq3.jpg
  - https://www.shadertoy.com/media/shaders/WsSBzh.jpg
measured: null
---

# Selfie Girl (and its making-of, "Painting a Character with Maths") — Inigo Quilez (2020)

**The ceiling of pure-maths imagery: an animated, lit, Pixar-adjacent character taking a selfie, produced entirely by one raymarched fragment shader. The most direct benchmark for Anikit's `shader` layer, which compiles exactly this kind of Shadertoy GLSL.**

## The idea
A girl taking a selfie, sculpted, coloured, lit and animated with formulas only. The making-of "explains how he used mathematics to paint a girl taking a selfie, covering the formulas for sculpting, colorizing and lighting her but not the programming" (*source*: iq's site description of the 27-11-2020 production, via search result). Live source: Shadertoy WsSBzh (*source*: RISC OS forum thread "Painting a Selfie Girl, with Maths" links WsSBzh and this video).

## Structure and pacing
- The shader is a short real-time animated shot (blink, small head and arm motion, camera drift) (*est.*).
- The official video is the making-of, about 25 min (*source*: MetaFilter via search; not opened), in iq's usual format: progressive build-up from primitive SDFs to the final frame, with voice-over (*est.*).

## Typography
None in the piece.

## Colour and light
Warm, painterly, stylised-realistic. Iq's standard lighting stack: key sun with soft shadows, sky fill, bounce or ambient occlusion, subsurface-like skin warmth, gamma-correct output and a filmic curve (*est.*, from iq's published articles and tutorials).

## Motion techniques
- Raymarched signed distance fields with smooth-min blending (`smin`) for organic forms, domain deformation for hair and clothing, procedural noise (fbm) for surface detail (*est.*, iq's documented technique: iquilezles.org/articles).
- Animation by time-varying parameters in the distance functions (blink, head tilt) (*est.*).

## Transitions
Not applicable (single shot). The making-of uses progressive reveals.

## Sound design and music
Voice-over narration in the making-of (*est.*). The shader itself is silent.

## What makes it world-class
1. **Character appeal from maths.** Proportion, silhouette and expression are as appealing as hand-modelled stylised CG.
2. **Lighting quality.** Soft shadows, occlusion in creases, warm skin response, no flat ambient.
3. **Anti-aliased, clean image.** No stair-stepping on silhouettes or banding in the sky.
4. **Detail by function.** Fine detail (hair strands, fabric) comes from domain warps and noise, not from textures.

## What Anikit would need
- `shader` layer with GLSL source (Anikit already compiles Shadertoy GLSL through naga to a Cranelift JIT, with `supersample` AA).
- The test is whether Anikit's CPU JIT renders a shader of this complexity at 1920×1080 with `supersample` in acceptable time and with bit-identical output to a GPU reference (precision, `pow`/`exp` edge cases).
- **GAP (content, not engine):** authoring at this level is expert work; Anikit's built-in library should hold its own SDF characters, not ports of iq's.

## Judge's checklist
- In a raymarched Anikit render, are silhouettes anti-aliased with no stair-stepping at 100% crop?
- Do shadows soften with distance from the occluder, and is there contact occlusion in creases?
- Is the image free of raymarch artifacts (banding, overstepping holes, noise speckles)?
- Does the character or object have appeal (silhouette, proportion), not just technical correctness?

## Clip availability
The video is on iq's official YouTube channel (no download). The shader source is public to view on Shadertoy (https://www.shadertoy.com/view/WsSBzh), but iq's recent shaders carry a restrictive copyright header (all rights reserved; no redistribution or reuse) (*unverified*: Shadertoy was behind a Cloudflare check during research). Treat it as reference-only and do not copy it into Anikit's shader library or committed files. Rendering it locally for a private side-by-side comparison should follow whatever the header says.

## Sources
- https://www.youtube.com/oembed?url=https://www.youtube.com/watch?v=8--5LwHRhjk&format=json (title "Painting a Character with Maths", author Inigo Quilez)
- https://riscosopen.org/forum/forums/12/topics/15933 (thread "Painting a Selfie Girl, with Maths": links WsSBzh and the video)
- https://metafilter.com/194834/Paint-the-universe-with-geometry-smoothstep-and-noise (search result only: 25-minute video, single equation; page returned 403)
- https://hackaday.com/painting-a-character-with-maths-8-5lwhrhjk-mp4-shot0001 (search result: Hackaday coverage)
