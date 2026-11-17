---
slug: linear-introducing-linear-agent-2026
category: ui-product
title: "Introducing Linear Agent"
studio: "Linear (in-house; individual credits not published)"
client: "Linear"
year: 2026
url: https://www.youtube.com/watch?v=mRql2VJ99gM
alt_urls: [https://linear.app/changelog/2026-03-24-introducing-linear-agent]
length_s: 55
verified: oembed
aspect: "16:9"
medium: [ui, 3d-cg]
pace: steady
palette: [dark, mono, limited]
type_role: lead
mood: [premium, minimal, technical]
purpose: launch
techniques: [ui-zoom, camera-move, depth-of-field, kinetic-type, logo-build]
audio: music-led
anikit_features: [composition, scene3d, camera, text, component.browser, motionBlur, post, "GAP: real app UI rendered as a tilted 3D plane with DOF and per-row reveal (composition-as-texture works; live UI data needs authoring)"]
awards: []
stills:
  - https://i.ytimg.com/vi/mRql2VJ99gM/maxresdefault.jpg
  - https://i.ytimg.com/vi/mRql2VJ99gM/hq1.jpg
  - https://i.ytimg.com/vi/mRql2VJ99gM/hq2.jpg
  - https://i.ytimg.com/vi/mRql2VJ99gM/hq3.jpg
measured: null
---

# Introducing Linear Agent — Linear (2026)

**Why it's in the set:** the current reference for "dark, quiet, expensive" SaaS launch films: real product UI, shown at a raking 3D angle in near-black space with shallow focus, no mock placeholders, and a single typed prompt as the story beat.

## The idea
Linear Agent is built into Linear and "understands your roadmap, issues, and code, and can synthesize context, make recommendations, and take action" (*source*: video description and changelog, 24 Mar 2026). The film shows exactly that loop: real issue list → a typed request in the "Ask Linear" composer → the agent's output.

## Structure and pacing
- 55 s (*source*: YouTube listing); published 24 Mar 2026 (*source*: YouTube).
- Frames at 25/50/75 % (YouTube hq1–3): (25 %) the bottom-right corner of the app window as a dark tilted slab, only the "Ask Linear" button lit; (50 %) a wireframe-like diagram of linked nodes (a plan/flow) on a dark tilted plane; (75 %) black frame with the white Linear logomark centred, i.e. the end card arrives by ~75 % and holds.
- *est.* 10–14 shots/moves, median ≈ 3 s: slow, continuous camera glides over the UI rather than cuts.

## Typography
- UI type is the product's own (a neo-grotesque in the Inter family, *est.* from glyph shapes: single-storey "a"-free, tall x-height, tabular IDs like "ENG-1316").
- The thumbnail frame: issue titles ("ETA refresh is too slow", "Trip history loads slowly") in light grey semibold; composer prompt "Make a plan to refactor data loading to be optimistic" in white; placeholder "Ask Linear…" in 30 % grey. Type is never overlaid as a headline on top of the UI in the frames seen; the UI text *is* the copy.

## Colour and light
- Near-black ground (#08090A–#121316 *est.*), UI surfaces one or two steps lighter (#1A1C20 *est.*), 1 px hairline borders, white text.
- Colour is reserved for status icons only (orange/yellow priority and status glyphs) and avatar photos.
- A soft top-left light falls across the tilted UI plane; far edges dissolve into black via depth of field and vignette.

## Motion techniques
- UI rendered as a flat plane in 3D, viewed from a low raking angle (≈ 30–40° tilt *est.* from thumbnail perspective), camera dollies along rows; shallow DOF keeps one row/composer sharp.
- Typed prompt in the composer as the narrative beat; output appears as structured nodes (50 % frame).
- Logo resolve on black at the end.

## Transitions
Continuous camera travel across one UI surface; dark fades between sections (*est.* from the 25 % frame, which is mostly black with only the button visible).

## Sound design and music
Not documented; *est.* minimal electronic/ambient bed with soft UI ticks.

## What makes it world-class
1. Real UI, real copy: every visible string is plausible product data (issue IDs, titles, avatars); zero grey placeholder bars.
2. Restraint in colour: monochrome surfaces; colour only where the product uses it (status icons).
3. UI treated as an object in space (tilted plane, DOF, light falloff) instead of a flat screen recording, but text in focus stays crisp.
4. One story beat per section (type a request → see the plan), so a 55 s film explains one feature.
5. The end card (logo on pure black) lands with ≥ 25 % of runtime left for the brand to breathe.

## What Anikit would need
- `composition` of the UI (real strings, Linear-like list, composer) used as a texture on a `scene3d` plane; `camera` with aperture for DOF; `motionBlur` on camera moves; `text` typewriter (`kinetic: "typewriter"`) for the prompt; `post` "clean" with vignette.
- GAP: authoring dense, realistic product UI quickly (a UI kit of lists/composers with real data).

## Judge's checklist
- Is every UI string real, specific copy (no lorem, no grey bars)?
- Is colour limited to what the product UI itself uses (status/brand accents) on a near-black ground?
- Is the UI shown as a lit object in 3D space with DOF, while the focused text stays pixel-crisp?
- Does each section carry exactly one product action (type → result)?
- Does the logo end card hold on clean black for ≥ 2 s?

## Sources
- YouTube oEmbed (title/author), YouTube description and date: https://www.youtube.com/watch?v=mRql2VJ99gM
- Linear changelog, 24 Mar 2026: https://linear.app/changelog/2026-03-24-introducing-linear-agent
- Stills viewed: maxresdefault, hq1, hq2, hq3.
