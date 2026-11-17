---
slug: memo-akten-learning-to-see-gloomy-sunday-2017
category: generative
title: "Learning to see: Gloomy Sunday (2017)"
studio: "Memo Akten"
client: "Self-initiated"
year: 2017
url: https://vimeo.com/260612034
alt_urls: [https://www.memo.tv/works/learning-to-see/]
length_s: 182
verified: oembed
aspect: "16:9"                      # est.
medium: [generative, live-action, mixed]
pace: steady
palette: [saturated, dark]
type_role: none
mood: [emotional, eerie, editorial]
purpose: art
techniques: [split-screen, morph, long-take, hard-cut]
audio: music-led
anikit_features: [video, shader, composition, "GAP: neural image-to-image (pix2pix-style) inference", "GAP: live split-screen of source vs generated in sync"]
awards: []
stills:
  - https://i.vimeocdn.com/video/1604699068-89cc1952bce06668bac2f0dde8dab2c39eda16184d06fb4ca2d3328742b7fcb4-d_1280?region=us
  - https://media.superradiance.net/cdn-cgi/image/width=1200,fit=scale-down,format=auto,onerror=redirect/memotv/projects/2017/learning-to-see/images/learning-to-see.jpg
  - https://media.superradiance.net/cdn-cgi/image/width=640,fit=scale-down,format=auto,onerror=redirect/memotv/projects/2017/gloomy-sunday/gallery/01_Gloomy Sunday still 003.jpg
measured: null
---

# Learning to See: Gloomy Sunday — Memo Akten (2017)

**Generative imagery with a thesis: neural networks trained on waves, fire, clouds and flowers "see" keys, cables and cloth on a table as oceans and blooms. The benchmark for a generative piece where the system's limits *are* the message.**

## The idea
"An artificial neural network looks out onto the world, and tries to make sense of what it is seeing. But it can only see through the filter of what it already knows. Because we too, see things not as they are, but as we are" (*source*: memo.tv). A live camera points at a table of everyday objects; custom networks "attempt to reconstruct it in real time, drawing on their prior" training (*source*: memo.tv).

## Structure and pacing
- Length 182 s (*source*: oEmbed; memo.tv gives 3:02, consistent).
- Four chapters: Fire, Water, Air, Earth (Flower) (*source*: gallery listing via search). Each pairs the live input (hands moving objects) with the network's interpretation, side by side or alternating (*est.*).
- Long, continuous hand-manipulation takes; chapter cuts at musical sections (*est.*).

## Typography
Minimal or none (*est.*).

## Colour and light
Each chapter inherits its training set's palette: fire oranges, ocean blues and whites, sky gradients, saturated flowers. The source footage is plain, neutral tabletop footage (*est.*, from chapter names and stills).

## Motion techniques
- pix2pix-style conditional GAN image-to-image translation (*source*: search results summarising project texts).
- Motion is "puppeteered" by the human hands in the source footage; the generated layer follows frame by frame, with characteristic GAN shimmer and texture boil (*est.*).

## Transitions
Chapter changes are hard cuts (*est.*).

## Sound design and music
Diamanda Galás, "Gloomy Sunday" (*source*: Vimeo description). The music's slow, mournful pacing frames the machine's "longing" (*est.*).

## What makes it world-class
1. **Concept-first generative.** The visible gap between input and output is the content; you understand it in two seconds.
2. **Human puppeteering.** Hands moving cables become crashing waves; physical causality makes the generated image feel alive.
3. **Emotional score.** A torch song turns a technical demo into elegy.
4. **Honest artefacts.** Texture boil and shimmer are embraced as the network's "hallucination", not hidden.

## What Anikit would need
- `video` input layers, `composition` for split-screen, `shader` for colour mapping.
- **GAP:** neural inference (image-to-image) is out of scope for a deterministic motion engine; the closest Anikit analogue is a procedural "reinterpretation" shader (e.g. luminance-driven fluid, fire or flower textures) applied to a source video.

## Judge's checklist
- Is the relationship between input and generated output immediately legible?
- Does the generated layer track the source motion frame-accurately (cause and effect visible)?
- Is each chapter's palette and texture coherent with its "world" (fire, water, air, flowers)?
- Does the music or pacing give the system's behaviour an emotional reading?

## Sources
- https://vimeo.com/api/oembed.json?url=https://vimeo.com/260612034 (title, author, duration, music credit)
- https://www.memo.tv/works/learning-to-see/ (concept text, stills)
- https://katevassgalerie.com/print/p/memaktenlearningtosee (search result: four chapters, Vimeo link)
- https://machinevisionuib.github.io/machine-vision-situation/learning-see-gloomy-sunday-machine-learns-see (search result: pix2pix technique)
