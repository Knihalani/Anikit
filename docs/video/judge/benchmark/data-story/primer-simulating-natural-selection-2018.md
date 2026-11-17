---
slug: primer-simulating-natural-selection-2018
category: data-story
title: "Simulating Natural Selection"
studio: "Primer (Justin Helps)"
client: "Self-initiated"
year: 2018
url: https://www.youtube.com/watch?v=0ZGbIKd0XrM
alt_urls: ["https://github.com/Helpsypoo/primer", "https://flowingdata.com/2019/07/31/natural-selection-simulation/"]
length_s: null
verified: oembed
aspect: "16:9"
medium: [stylised-3d, sim, data-viz]
pace: steady
palette: [dark, limited, cool]
type_role: supporting
mood: [playful, technical, witty]
purpose: data-story
techniques: [sim, chart-build, soft-body, instancing, camera-move]
audio: vo-led
anikit_features: [scene3d, "scene3d model (blob characters)", "component (histogram/bar-chart)", text, camera, "physics world (agents)", audio, "GAP: agent-based simulation whose population feeds a live histogram in the same frame", "GAP: squash-and-stretch soft-body characters"]
awards: []
stills:
  - https://i.ytimg.com/vi/0ZGbIKd0XrM/maxresdefault.jpg
  - https://i.ytimg.com/vi/0ZGbIKd0XrM/hq1.jpg
  - https://i.ytimg.com/vi/0ZGbIKd0XrM/hq2.jpg
  - https://i.ytimg.com/vi/0ZGbIKd0XrM/hq3.jpg
measured: null
---

# Simulating Natural Selection — Primer (2018)

**Data from a live simulation, shown by charming 3D characters: the audience watches the population evolve and the histogram update at the same time.**

## The idea
Blob creatures with heritable traits (speed, size, sense) forage for food; traits cost energy, so selection favours trade-offs. The video runs the simulation and charts trait distributions over generations (*source*: search-result summaries of the video). The description says it is fifth in Primer's evolution series, "made with Blender and python", with code on GitHub (*source*: description mirror via search).

Length: `null` (no oEmbed duration); *est.* ~10 min. Analyse the 60 s where the second trait (size) is introduced and the population/histogram evolve (hq2 still).

## Structure and pacing
Steady explainer rhythm: a trait card, a small demonstration, then a sped-up simulation with a chart. *est.* median shot 4–6 s.

## Typography
*observed* (hq2): "Second trait: Size" in a light humanist serif (Libertinus/Linux-Libertine-like) in off-white on charcoal, large and centred top. Chart labels in the same family (*est.*).

## Colour and light
*observed*: charcoal ground (~#2E3133), a flat grey tabletop plane with soft contact shadow, blobs in saturated mid-blue (~#3D86B8) with simple black dot eyes, soft key from top left and gentle ambient occlusion. Trait variants are coloured to match their chart bars (*est.*).

## Motion techniques
Blobs move by squash-and-stretch hops (*est.*, consistent with their soft pear shape); simulation runs at speed with many instances on the plane; histograms grow bar by bar as generations pass (*est.*).

## Transitions
Camera moves between the simulation board and the chart, or the chart appears beside the board (*est.*).

## Sound design and music
VO-led by Justin Helps; light playful music; small boop SFX for blob events (*est.*).

## What makes it world-class
1. The data is generated on screen: viewers see the cause (creatures) and the effect (distribution) together.
2. Characters make abstract stats lovable without breaking the rigour.
3. A disciplined look: one ground, one board, one character design, colour = trait.
4. Clean, readable 3D: soft key light and contact shadows, nothing decorative.

## What Anikit would need
`scene3d` instanced blob `model`s with soft-body squash, agent logic in a `world`/expression, a histogram `component` bound to the simulation, `text` cards, `camera` moves, `audio`. GAP: an agent simulation feeding a live chart in the same render; soft-body squash-and-stretch on instanced characters.

## Judge's checklist
- Are the data shown in the chart visibly produced by the process on screen?
- Does colour encode the variable consistently between the 3D scene and the chart?
- Is the 3D lit with a clear key and contact shadows (objects sit on the ground)?
- Are characters appealing without obscuring the data?

## Sources
- YouTube oEmbed for 0ZGbIKd0XrM (title, author Primer); hq2 still viewed
- https://flowingdata.com/2019/07/31/natural-selection-simulation/ (search summary)
- Search-result mirror of the description (Blender and Python; GitHub Helpsypoo/primer)
