---
slug: raven-kwok-skyline-2015
category: generative
title: "Skyline"
studio: "Raven Kwok"
client: "Karma Fields (Monstercat)"
year: 2015
url: https://vimeo.com/139977231
alt_urls: ["http://ravenkwok.com/skyline/", "https://www.stashmedia.tv/?p=25149"]
length_s: 296
verified: oembed
aspect: "16:9"                      # est.
medium: [generative, 2d-vector]
pace: fast
palette: [mono, high-contrast, dark]
type_role: none
mood: [energetic, technical, eerie]
purpose: music-video
techniques: [beat-sync, grid, noise-field, particles, morph, rotoscope, rhythmic-edit]
audio: music-led
anikit_features: [shader, mograph, audio, "GAP: Voronoi agent system driven by audio spectrum (beyond iAudio 4-band levels)", "GAP: performer-silhouette seeding from video (rotoscope)"]
awards: ["FILE Electronic Language International Festival 2016, 3rd Place (Anima+Games)"]
stills:
  - https://i.vimeocdn.com/video/536121164-490b06d21f38e83c464dc12c44ff61990f236d15864ca15cf49f7c6ff8d36464-d_1280?region=us
measured: null
---

# Skyline — Raven Kwok for Karma Fields (2015)

**Code-only music video: Voronoi cells whose seeds are agents driven by the song's spectrum and by the vocalist's animated silhouette. The benchmark for audio-reactive 2D generative systems that stay graphic and legible.**

## The idea
"A code-based generative music video directed and programmed by Raven Kwok… The entire music video consists of multiple stages that are programmed and generated using Processing. One of the core principles for generating the visual patterns in Skyline is Voronoi tessellation." "Seeds for generating the diagram are sorted into various types of agents following certain behaviors and appearance transformations. They are driven by either the song's audio spectrum with different customized layouts, or animated sequence of the vocalist" (*source*: Vimeo description).

## Structure and pacing
- Length 296 s (*source*: oEmbed). "Multiple stages", each a different agent and Voronoi configuration matched to song sections (*source*). Stage changes on section boundaries; within stages, constant beat-locked motion (*est.*). Equivalent "shot" rhythm about 1–2 s (*est.*; to be *measured*).

## Typography
None in the main video.

## Colour and light
Predominantly monochrome line work (white or black cells and edges) with high contrast; density and line weight carry tone (*est.*, from the thumbnail and Kwok's style).

## Motion techniques
- Voronoi tessellation with seeds as agents (flocking, orbiting, spectrum-mapped layouts) (*source*).
- Seeds sampled from an animated sequence of the vocalist, so a human figure emerges from cells (*source*).
- Each of Kwok's Karma Fields projects took 1–2 months to develop (*source*: Vice, search result).

## Transitions
System-state changes at song sections: the seed layout re-organises (morph) rather than cutting (*est.*).

## Sound design and music
Karma Fields, "Skyline" (Monstercat). Visual parameters map to the audio spectrum (*source*).

## What makes it world-class
1. **One system, many expressions.** A single geometric rule (Voronoi) yields many distinct stages through agent behaviours.
2. **Visible audio mapping.** Spectrum layouts make the music's frequency content legible.
3. **A human emerges from geometry.** The vocalist's silhouette appears in the cell field, giving the abstraction an anchor.
4. **Graphic restraint.** Monochrome line work at high resolution and crisp anti-aliasing.

## What Anikit would need
- `shader` with `buffers` (Voronoi via jump-flood or brute force for a few hundred seeds) fed by `iAudio`/`iBeat`.
- `mograph` for line styling; `audio` analysis.
- **GAP:** per-band spectrum arrays (more than 4 levels) exposed to shaders and layers; agent state that persists across frames (buffers give feedback, but particle-agent state needs a data channel); silhouette sampling from a video channel.

## Judge's checklist
- Can you see which frequency band drives which element (does a kick visibly move something)?
- Are cell edges crisp and anti-aliased at 100% crop, with no shimmer as cells move?
- Does each section introduce a new system behaviour rather than the same pattern recoloured?
- Is there a readable figure or form emerging from the abstraction at key moments?

## Sources
- https://vimeo.com/api/oembed.json?url=https://vimeo.com/139977231 (title, author, duration, full technical description, FILE 2016 award list)
- https://www.vice.com/en/article/mathematic-models-become-creatures-driven-by-code (search result: Creators Project coverage)
- https://www.stashmedia.tv/?p=25149 (search result)
