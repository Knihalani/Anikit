---
slug: nature-tulp-how-many-trees-2015
category: data-story
title: "How many trees are there in the world?"
studio: "Nature Video (data visualization: Jan Willem Tulp; art direction: Kelly Krause)"
client: "Nature"
year: 2015
url: https://www.youtube.com/watch?v=jqdOkXQngw8
alt_urls: [https://informationisbeautifulawards.com/showcase/772-how-many-trees-are-there-in-the-world]
length_s: null
verified: oembed
aspect: "16:9"
medium: [data-viz, 3d-cg, generative]
pace: steady
palette: [dark, limited]
type_role: lead
mood: [technical, calm, editorial]
purpose: data-story
techniques: [map, instancing, particles, camera-move, scale-contrast, kinetic-type]
audio: vo-led
anikit_features: [scene3d, "text value (counter)", camera, "particles/instancing (point globe)", glow, audio, "GAP: data-driven point-cloud globe (lat/long density → instanced points)"]
awards: ["Information is Beautiful Awards 2015, Motion Infographic Silver"]
stills:
  - https://i.ytimg.com/vi/jqdOkXQngw8/maxresdefault.jpg
  - https://i.ytimg.com/vi/jqdOkXQngw8/hq1.jpg
  - https://i.ytimg.com/vi/jqdOkXQngw8/hq2.jpg
  - https://i.ytimg.com/vi/jqdOkXQngw8/hq3.jpg
  - https://tulpinteractive.com/img/trillions-of-trees.png
measured: null
---

# How many trees are there in the world? — Nature Video / Jan Willem Tulp (2015)

**A data visualization studio's 3D tree-density globe turned into a news explainer: every point is data, and a single big number does the storytelling.**

## The idea
Present the Crowther et al. (Nature, 2015) result: about 3.04 trillion trees, ~422 per person, from 421,529 field measurements, and a loss of about 10 billion trees a year (*source*: coverage summarised in search results). Nature briefed Jan Willem Tulp, who built a 3D visualization of tree density and the video; stills from the 3D animation were reused for the print graphic (*source*: jasiek.co.uk; IIB listing).

## Structure and pacing
*est.* 2–3 min, steady: big-number cards alternate with camera moves around a point-cloud globe. `length_s` null.

## Typography
*observed* hq2: a very large white numeral "10,000,000,000" in a clean grotesk (Helvetica-like) centred over the dark globe; "nature video" wordmark top right in light grey. Numbers are the lead visual element; full digits with commas, not "10bn", to make scale felt.

## Colour and light
*observed* hq2: near-black green ground (~#0B1A12); the globe rendered as thousands of green points (dark olive to bright leaf green, ~#3E8F2E → #8BD64A) whose density maps tree density, with a subtle rim light on the sphere edge; oceans dark. Monochrome green palette = one variable.

## Motion techniques
Globe rotation; points accumulate or fade to show counts/loss (*est.*); counter rolls up (*est.*).

## Transitions
Number cards over the persistent globe (type fades in/out over the scene), camera continues (*est.*).

## Sound design and music
Nature Video VO narration with a light ambient bed (*est.*).

## What makes it world-class
1. The globe is made of the data itself (density → points), not a texture.
2. One hue for one variable: the palette is the encoding.
3. Full-digit numbers give scale weight.
4. Made by a data-vis specialist inside a science publisher's video format: rigorous and clear.

## What Anikit would need
`scene3d` point-cloud sphere from lat/long density data (instanced points), `camera` orbit, `glow` rim, `text` with `value` counters, `audio`. GAP: data-driven point-cloud globe from a raster (density map) input.

## Judge's checklist
- Is the hero visual built from the real data (points, bars) rather than an illustration of it?
- Is colour used as an encoding (one hue = one variable) rather than decoration?
- Are big numbers legible over the visual (contrast, size, held long enough)?
- Does camera motion reveal more data rather than just spin?

## Sources
- YouTube oEmbed for jqdOkXQngw8 (title, author "nature video"); hq2 still viewed
- https://informationisbeautifulawards.com/showcase/772-how-many-trees-are-there-in-the-world
- https://jasiek.co.uk/graphics/trees
- https://www.informationisbeautifulawards.com/news/116-2015-the-winners
