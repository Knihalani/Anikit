---
slug: nasa-year-in-the-life-of-co2-2014
category: data-story
title: "NASA | A Year in the Life of Earth's CO2"
studio: "NASA Goddard (GMAO; animator/scientist: Bill Putman)"
client: "NASA"
year: 2014
url: https://www.youtube.com/watch?v=x1SgmFa0r04
alt_urls: ["https://svs.gsfc.nasa.gov/11719"]
length_s: null
verified: oembed
aspect: "16:9"
medium: [data-viz, sim]
pace: calm
palette: [saturated, high-contrast]
type_role: supporting
mood: [technical, epic, calm]
purpose: data-story
techniques: [map, fluid, noise-field, long-take, camera-move]
audio: vo-led
anikit_features: [shader, "shader buffers (advection/feedback)", "image (map plates)", "text value (date counter)", camera, scene3d, audio, "GAP: colour-mapped scalar-field renderer with legend (rainbow/diverging LUT)"]
awards: []
stills:
  - https://i.ytimg.com/vi/x1SgmFa0r04/maxresdefault.jpg
  - https://i.ytimg.com/vi/x1SgmFa0r04/hq1.jpg
  - https://i.ytimg.com/vi/x1SgmFa0r04/hq2.jpg
  - https://i.ytimg.com/vi/x1SgmFa0r04/hq3.jpg
  - https://www.geeksaresexy.net/wp-content/uploads/2016/03/carbon.jpg
measured: null
---

# NASA | A Year in the Life of Earth's CO2 — NASA Goddard (2014)

**Simulation as data story: a full year of atmospheric CO₂ from a 7 km global model, played as swirling fluid, where the shapes of the weather are the argument.**

## The idea
A GEOS-5 "Nature Run" (NASA Goddard's Global Modeling and Assimilation Office) ingests real atmospheric and emissions data, then runs freely, simulating January–December 2006; the visualization shows CO₂ (and carbon monoxide) moving with the weather (*source*: SVS page via search summary; EarthSky). The GMAO identifies the run as the non-hydrostatic 7 km global mesoscale simulation (*source*). Released 17 Nov 2014 (*source*). Story: the northern-hemisphere growing season pulls CO₂ down; emissions plumes stream from industrial centres.

Length: ~3 min (*source*: Vice headline); `length_s` null.

## Structure and pacing
- A few long takes: global equirectangular map (*observed* hq2), plus regional/globe views (*est.*). Calm; the date counter advances ~ several days per second (*est.*).

## Typography
Lower-left NASA meatball and a large date stamp "2006 / 05 / 07" in white sans, with "Global Modeling and Assimilation Office" beneath; two thin colour-bar legends at bottom right (CO column, CO₂ ppm) with small tick labels (*observed* hq2). Functional broadcast-science layout.

## Colour and light
- *observed* hq2: CO₂ mapped with a rainbow LUT — deep blue (low) through cyan, yellow, orange to red (high), with the northern hemisphere red/orange swirls in spring and the south in blue; CO overlaid in greys/white as a separate layer; Antarctica white. Continents grey beneath.
- Not "tasteful", but the high-contrast LUT makes the swirls legible as structure.

## Motion techniques
Fluid advection: filaments, eddies and plumes stretch and roll continuously (*observed* in the still's swirl structure; it is model output, not stylised). Seasonal breathing of the colour field.

## Transitions
Slow dissolves or camera moves between global and regional views (*est.*).

## Sound design and music
NASA Goddard videos of this series carry a narrator (*est.*) and an ambient score; VO explains the colour legend.

## What makes it world-class
1. The data has its own motion; the piece simply makes it visible at high resolution.
2. One continuous field with real physical structure (eddies, plumes) — impossible to fake with stock particles.
3. A date counter and legend keep the viewer anchored while the field evolves.
4. Scientific authority: model, run and dates are documented.

## What Anikit would need
`shader` with `buffers` feedback for advected fields over an `image` base map; colour LUT in the shader; `text` with `value` date counter; `camera` for regional moves. GAP: a scalar-field renderer that maps real gridded data (NetCDF-style) through a named LUT with an auto legend.

## Clip availability
NASA-produced media is generally not copyrighted in the US (NASA media guidelines); the SVS page (https://svs.gsfc.nasa.gov/11719) usually offers direct downloads. The page returned 503 when checked, so the lead should confirm the download link and licence there before analysis.

## Judge's checklist
- Does motion come from (or convincingly match) a physical/data process rather than generic noise?
- Is there a legend and time anchor the viewer can read at any frame?
- Are fine structures (filaments, eddies) crisp at 100% crop, without banding in the colour field?
- Does the colour map make high vs low unambiguous?

## Sources
- YouTube oEmbed for x1SgmFa0r04 (title, author NASA Goddard); hq2 still viewed
- https://svs.gsfc.nasa.gov/11719 (search summary; page 503 when fetched)
- https://earthsky.org/earth/video-a-year-in-the-life-of-earths-co2
- https://www.vice.com/en/article/539wyn/watch-earths-yearly-carbon-cycle-in-just-three-minutes
- https://gmao.gsfc.nasa.gov/global_mesoscale/7km-G5NR/animations (search summary: 7 km G5NR)
