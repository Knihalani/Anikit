---
slug: maxime-causeret-order-from-chaos-2016
category: generative
title: "Max Cooper - Order from Chaos - Official Video by Maxime Causeret"
studio: "Maxime Causeret (Teresuac)"
client: "Max Cooper (album: Emergence)"
year: 2016
url: https://vimeo.com/196269431
alt_urls: [https://maxcooper.net/order-from-chaos, https://www.thisiscolossal.com/2016/12/order-from-chaos-video-maxime-causeret/]
length_s: 258
verified: oembed
aspect: "16:9"                      # est.
medium: [generative, sim, 3d-cg]
pace: steady
palette: [dark, saturated, limited]
type_role: none
mood: [calm, technical, emotional]
purpose: music-video
techniques: [sim, particles, noise-field, soft-body, fluid, beat-sync, macro, depth-of-field, instancing]
audio: music-led
anikit_features: [scene3d.particles, shader, camera, post, motionBlur, audio, "GAP: reaction-diffusion and cellular / flocking agent sims driven by audio onsets", "GAP: translucent membrane (SSS) shading for cells"]
awards: []
stills:
  - https://i.vimeocdn.com/video/608700439-a6aa42e40721d672ccd6529ca24726cdc4064a51e7cd7f1dfbbbbe34a401921a-d_1280?region=us
  - https://maxcooper.net/lib/content/32_landscape.jpg
  - https://www.thisiscolossal.com/wp-content/uploads/2016/12/Screen-Shot-2016-12-20-at-1.14.19-PM.png
  - https://www.onepointfour.co/wp-content/uploads/2017/01/maxime.jpg
measured: null
---

# Order from Chaos — Maxime Causeret for Max Cooper (2016)

**The canonical "science-made-beautiful" music video: raindrop transients become a rhythm, and the rhythm becomes cells, colonies, symbiosis and flocks. The benchmark for simulations that are *about* something and locked to the music.**

## The idea
Max Cooper recorded heavy rain on a roof window with binaural mics, "mapped the transients for the raindrop hits, and then forced the mapped points towards the nearest drumming grid positions", so "a percussive rhythm emerged". Causeret's brief: "map the emergent rhythm to an exploration of emergence in living form" (*source*: Vimeo description by Max Cooper).

## Structure and pacing
- Length 258 s (*source*: oEmbed).
- Progression (*source*: Vimeo description): raindrops → simple cellular forms → cooperation between cells forming colonies → endosymbiosis (small organisms living inside larger cells) → competition for resources → flocking → animated reaction-diffusion patterns (Turing).
- Shots: about 40–60, median about 4–5 s (*est.*; slow macro holds; to be *measured*). Steady pace with evolving motion inside each hold.

## Typography
None.

## Colour and light
Deep dark grounds with luminous, translucent organic forms: membranes, nuclei and bands of colour in reaction-diffusion passages. Light reads as microscopic (rim, transmission, depth of field) (*est.*, from the thumbnail and Colossal stills).

## Motion techniques
- Built in Houdini: "a lot of small experiments with dynamic systems around my main idea of living micro organism"; "I try to setup something fast to Cook or at least fast to preview in order to animate the shots". Made over 4 months part-time (*source*: Causeret, translated, in the Vimeo description).
- Particle and agent systems (flocking), soft-body cell division and engulfing, reaction-diffusion on surfaces (*source*: description of content; techniques *est.*).
- Raindrop hits are visualised first, so the audio's seeding is shown before it becomes life (*source*).

## Transitions
Scale-based continuity: macro to colony, colony to organism. Dissolves and match-on-form between stages (*est.*).

## Sound design and music
Max Cooper's track: raindrop-quantised percussion, sansula, pads, saturation layers, vocal snippets from Kathrin deBoer (*source*). Visual events follow the percussive transients (*est.*; the brief explicitly mapped rhythm to emergence).

## What makes it world-class
1. **Concept-to-image fidelity.** Every stage visualises a real biological idea in order (symbiosis, competition, flocking, Turing patterns); nothing is decorative.
2. **Sims that feel alive.** Agents and cells have intent: approach, merge, flee. This is not noise-driven drift.
3. **Audio-driven events.** Raindrop transients trigger visible events, so the rhythm is seen.
4. **Microscopy lighting.** Translucency, rim light and shallow depth of field sell scale and wetness.

## What Anikit would need
- `scene3d` particles and instances for cells and agents, `camera` macro with `aperture`/`focus` pulls, `post` (bloom, vignette), `motionBlur`, `audio` onset and beat data (`iAudio`/`iBeat` in `shader`).
- `shader` layers for reaction-diffusion (`buffers` with feedback "A" channel make Gray–Scott possible today).
- **GAP:** agent-based flocking and cell-division sims; translucent SSS membranes; onset-driven event triggering (beyond beat-grid pulses).

## Judge's checklist
- Does the piece progress through distinct stages of increasing complexity rather than looping one effect?
- Are visual events triggered by audible transients (check 5 random hits)?
- Do agents behave with intent (cohesion, avoidance, merging), not just drift on noise?
- Is the microscopic look sold by translucency and depth of field, without banding in dark gradients?

## Sources
- https://vimeo.com/api/oembed.json?url=https://vimeo.com/196269431 (title, author Max Cooper, duration, full description incl. Causeret's process notes)
- https://www.thisiscolossal.com/2016/12/order-from-chaos-video-maxime-causeret/ (search result)
- https://www.dezeen.com/?p=1028980 (search result: 4 months part-time; name Teresuac)
