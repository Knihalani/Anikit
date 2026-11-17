---
slug: halloran-fallen-of-wwii-2015
category: data-story
title: "The Fallen of World War II"
studio: "Neil Halloran (writer, director, coder, narrator)"
client: "Self-initiated"
year: 2015
url: https://www.youtube.com/watch?v=DwKPFT-RioU
alt_urls: [https://www.neilfilms.com/ww2/, https://informationisbeautifulawards.com/showcase?award=2015]
length_s: null
verified: oembed
aspect: "16:9"
medium: [data-viz, 3d-cg]
pace: calm
palette: [dark, limited, high-contrast]
type_role: supporting
mood: [emotional, editorial, epic]
purpose: data-story
techniques: [chart-build, scale-contrast, camera-move, instancing, long-take]
audio: vo-led
anikit_features: [scene3d, "component bar-chart", "text value counters", camera, "mograph.repeat (instanced figures)", "post vignette/grain", audio, "GAP: thousands of instanced figure glyphs stacked into bars with a long camera crane up the stack"]
awards: ["Information is Beautiful Awards 2015, Motion Infographic Gold"]
stills:
  - https://i.ytimg.com/vi/DwKPFT-RioU/maxresdefault.jpg
  - https://i.ytimg.com/vi/DwKPFT-RioU/hq1.jpg
  - https://i.ytimg.com/vi/DwKPFT-RioU/hq2.jpg
  - https://i.ytimg.com/vi/DwKPFT-RioU/hq3.jpg
measured: null
---

# The Fallen of World War II — Neil Halloran (2015)

**The canonical data documentary: one narrator, one chart language, and scale made physical, so 70 million deaths are felt rather than read.**

## The idea
A "data-driven documentary about the human cost of the second World War and the decline in battle deaths since" (*source*: neilfilms.com). Halloran wrote, directed, coded and narrated it; it exists as an interactive WebGL piece that pauses so viewers can explore the charts, and as a linear YouTube video (*source*). Data: mainly Wikipedia's WWII casualties article, with post-war data from the Uppsala Conflict Data Program (*source*).

Length: 18 min (*source*: neilfilms.com); `length_s` null (no oEmbed duration). Longer than 90 s: analyse the 60 s of the Asia/Soviet military-deaths build (the hq2 still, "ASIA MILITARY", running counter "41 M"), as representative of the method.

## Structure and pacing
- Long continuous camera moves over a single evolving chart, punctuated by chapter cards (*observed*: hq1 shows a white map-pin glyph with "80" on black, a chapter/marker device). Calm pace: *est.* median shot 8–15 s, but the chart is always growing or the camera always moving.
- The famous beat: the camera rises up a single bar (Soviet military deaths) for many seconds to convey its height (*est.*, widely described; consistent with long-take structure).

## Typography
- Condensed, tall sans in all caps for labels ("ASIA / MILITARY") in white, set right of the chart; a large condensed numeral counter at lower left ("41 M") with a thin green progress bar beneath (*observed* hq2). Country names tiny under each bar with small flags (*observed*).
- Type is set like a broadcast lower-third: minimal, never covering the data.

## Colour and light
- Pure black ground (*observed*). Bars in deep crimson (~#B0101C) for one nation and pink-red (~#D46A78) for another, with small coloured bars for others — red reserved for loss (*observed* hq2).
- Chapter cards: soft-vignetted charcoal with paper texture behind a white pin (*observed* hq1).
- No decoration: light is only the white type and the coloured data.

## Motion techniques
- Bars built from small human figures stacking up (*est.*: Halloran's described figure-stack technique; the hq2 bars appear solid at thumbnail scale).
- Counter ticks up in sync with the stacking; camera dolly/crane across a 3D chart space (*est.* from WebGL build).

## Transitions
Camera-continuous: the chart persists and the camera travels; hard cuts only to chapter cards (*est.*).

## Sound design and music
Original score and sound design by Andy Dollerson (soundtrack sold on Bandcamp, *source*). VO-led with sombre piano/strings beds and silence used for weight (*est.*).

## What makes it world-class
1. One honest chart grammar (bars of people, a counter, a label) used consistently for 18 minutes.
2. Scale is felt through time: the camera takes as long as the number deserves.
3. Colour discipline: black ground, red for death, white for words — nothing else competes.
4. Narration is personal and plain; the visuals carry the numbers so the VO can carry the meaning.
5. Data provenance is stated.

## What Anikit would need
`scene3d` or `group` with `camera` crane moves over a long chart; `text` with `value` counters; `shape` bars or instanced figure glyphs via `mograph.repeat`; `post` vignette; `audio` VO + score. GAP: tens of thousands of instanced glyphs in a stack with a smooth long camera move (needs instancing performance check).

## Judge's checklist
- Is the palette restricted so the key quantity (here red = deaths) is the only saturated colour?
- Are numbers animated in sync with the visual that represents them (counter matches bar growth)?
- Does at least one moment use time and camera movement to convey magnitude?
- Do labels stay out of the data and remain readable (≥4.5:1, steady, not moving while read)?
- Are data sources stated somewhere in the piece?

## Sources
- YouTube oEmbed for DwKPFT-RioU (title, author Neil Halloran); hq1 and hq2 stills viewed
- https://www.neilfilms.com/ww2/ (format, credits, data sources, music)
- https://www.informationisbeautifulawards.com/news/116-2015-the-winners
