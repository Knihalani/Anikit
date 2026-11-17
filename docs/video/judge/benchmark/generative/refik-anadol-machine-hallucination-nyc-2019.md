---
slug: refik-anadol-machine-hallucination-nyc-2019
category: generative
title: "MACHINE HALLUCINATION : NYC"
studio: "Refik Anadol Studio"
client: "Artechouse New York (commission)"
year: 2019
url: https://vimeo.com/450313024
alt_urls: [https://refikanadol.com/works/machine-hallucination/]
length_s: 336
verified: oembed
aspect: "16:9"                      # the documentation video; the installation was 13689 × 7418 px across 18 channels
medium: [generative, data-viz, photo]
pace: steady
palette: [saturated, dark, cool]
type_role: none
mood: [epic, technical, eerie]
purpose: art
techniques: [morph, fluid, particles, noise-field, camera-move, long-take, glow]
audio: sound-design-led
anikit_features: [shader, scene3d.particles, post, camera, audio, "GAP: latent-space morph of photo datasets (GAN interpolation)", "GAP: point-cloud / particle 'data sculpture' of 1M+ points with fluid advection"]
awards: []
stills:
  - https://i.vimeocdn.com/video/944445700-e53023c42381d73d4dd37894f79f62e5958ec932ed9008cafe78a9e68d7889eb-d_1280?region=us
measured: null
---

# Machine Hallucination: NYC — Refik Anadol Studio (2019)

**The reference "AI data painting": 100 million photographs of New York reinterpreted as flowing latent-space architecture at 16K. This is the bar for scale, density and fluid data-driven motion in immersive generative work.**

## The idea
"A data universe of New York City in 1025 latent dimensions… by deploying machine learning algorithms on over 100 million photographic memories of New York City found publicly in social networks… a 30-minute experimental cinema, presented in 16K resolution, that visualizes the story of New York through the city's collective memories" (*source*: Vimeo description and refikanadol.com).

## Structure and pacing
- The documentation video is 336 s (*source*: oEmbed). The installation is 30 minutes (*source*).
- Installation specs: 18-channel video, 32-channel sound, custom software, 13689 × 7418 px; Artechouse New York, 6 Sep 2019 – 7 Feb 2020 (*source*: Vimeo description).
- Long continuous passages: latent-space morphs of architecture, then dissolution into pigment-like fluid and particle "data sculptures" (*est.*, from the studio's "AI Data Painting/Sculpture" framing).

## Typography
None in the artwork; the documentation may include title cards (*est.*).

## Colour and light
Saturated, painterly colour fields derived from the photo dataset, flowing over dark space; emissive, with no physical lighting (*est.*).

## Motion techniques
- GAN latent-space interpolation of the photo corpus (morphing building facades and streets) (*source*: "1025 latent dimensions"; GAN type *est.*).
- Fluid advection of colour and particle masses ("AI Data Painting" and "AI Data Sculpture", terms Anadol coined at Google AMI in 2016) (*source*: refikanadol.com quote).

## Transitions
Continuous morphs; no hard cuts in the artwork (*est.*).

## Sound design and music
Sound design by Kerim Karaoglu; 32-channel spatial sound (*source*).

## What makes it world-class
1. **Scale and density.** Millions of moving elements and 16K canvas resolution; every region of the frame is active.
2. **Fluid continuity.** Imagery flows like paint; motion has inertia and turbulence, never jitter.
3. **Recognisable subject in abstraction.** NYC architecture remains legible in the latent morphs before dissolving.
4. **Immersive framing.** Designed for a room-scale, multi-surface canvas.

## What Anikit would need
- `shader` with feedback `buffers` for fluid advection of an image (`channels` with a photo or video), `scene3d` particles for point masses, `post` bloom, and slow `camera` drift.
- **GAP:** dataset-driven latent morphing (neural); million-point particle advection at video resolution; multi-channel or very high-resolution output.

## Judge's checklist
- Is the whole frame alive (no dead regions) while the motion stays fluid and inertial rather than noisy?
- Does a recognisable subject persist through the abstraction?
- Are fine particles and colour gradients clean at 100% crop (no banding, no compression mush)?
- Does colour come from the subject data (photo palette) rather than a generic rainbow gradient?

## Sources
- https://vimeo.com/api/oembed.json?url=https://vimeo.com/450313024 (title, author, duration, full description, specs, credits)
- https://refikanadol.com/works/machine-hallucination/ (project text, AI Data Painting quote)
- https://www.dandad.org/annual/2020/editorial/behind-the-work-machine-hallucination-new-york (D&AD Annual 2020 feature; award level not shown on page)
