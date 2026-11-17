---
slug: andreas-wannerstedt-eternal-installations-2020
category: cg-3d
title: "Eternal Installations"
studio: "Andreas Wannerstedt"
client: "Self-initiated"
year: 2020
url: https://vimeo.com/454900647
alt_urls: [https://www.instagram.com/andreaswannerstedt/]
length_s: 60
verified: oembed
aspect: "16:9"                      # est.
medium: [photoreal-cg, 3d-cg]
pace: steady
palette: [earthy, limited, light]
type_role: none
mood: [calm, premium, minimal]
purpose: art
techniques: [loop, rigid-body, camera-move, depth-of-field, macro, long-take]
audio: sound-design-led
anikit_features: [scene3d, camera, motionBlur, post, audio, mograph.repeat, "GAP: outdoor environments (terrain, vegetation, sky) in scene3d", "GAP: seamless-loop authoring helper (period check for every animated property)"]
awards: []
stills:
  - https://i.vimeocdn.com/video/952196346-ace0f6662f641cbeb1b3fa97dc2091eeda0e66a5209eba33be528cce23400e92-d_1280?region=us
measured: null
---

# Eternal Installations — Andreas Wannerstedt (2020)

**The "oddly satisfying" loop raised to land-art scale: perfectly timed, hand-keyed perpetual-motion sculptures placed in photoreal landscapes. This is the benchmark for precision of timing and calm, premium CG.**

## The idea
"A surreal journey through a series of dreamlike environments, where large-scale art installations are stretching the boundaries of physical laws. The perpetual motion is the essence of these meditative installations". Wannerstedt's "oddly satisfying" animations "meet physical installation art… defying gravity and especially reality". "All pieces presented in the film are created as seamlessly looping animations" (*source*: Vimeo description).

## Structure and pacing
- Length 60 s (*source*: oEmbed).
- A sequence of installations, each held long enough to see at least one full loop cycle. Shots: about 10–14, median about 4–5 s (*est.*, from 60 s and loop-length conventions of his Instagram work; to be *measured*). It is calm by design; the life comes from the perpetual motion inside each hold.

## Typography
None.

## Colour and light
- Natural environments (landscapes, majestic settings) under soft daylight or overcast skies, with sculptures in clean architectural materials: polished stone, brushed metal, matte pastel solids (*est.*, from the description and his body of work).
- A restrained palette per installation, typically one or two object colours against neutral nature (*est.*).

## Motion techniques
- He animates **by hand with keyframes, not physics simulation**, "which gives him full control over motion and timing"; Cinema 4D, rendered in Arnold or Redshift (*source*: Scandinavian Mind interview).
- Mechanisms that are nearly plausible: rolling balls, swinging pendulums, sliding blocks and rotating rings, whose friction and gravity are bent just enough to loop forever (*source*: interview: "bends physical laws such as friction and gravity").
- Ease curves are precise: contact moments have a crisp stop or snap, and travel is smooth, which produces the "satisfying" feeling (*est.*).

## Transitions
Hard cuts or slow dissolves between installations; within a shot the camera drifts slowly or holds (*est.*).

## Sound design and music
Music and sound design by un.studios (*source*: Vimeo credits). In his work, every contact (click, roll, chime) is foleyed and synced exactly to the animation (*est.*).

## What makes it world-class
1. **Loop perfection.** Every installation's cycle is seamless: no pop, no speed change at the loop point.
2. **Hand-tuned timing.** Contacts, stops and handoffs are keyed for satisfaction; the rhythm feels musical even without music.
3. **Scale contrast.** Small "desk-toy" mechanisms are scaled to architecture inside real-looking landscapes.
4. **Restraint.** One mechanism per shot, a limited palette, no effects for their own sake.
5. **Sound is the second half.** Each contact has a matching sound at the exact frame.

## What Anikit would need
- `scene3d` (studio or outdoor `env`), rigid objects with chrome, stone and matte materials, `camera` slow push with depth of field, `motionBlur`, and a `post` clean or cinematic preset.
- Expressions (`"=sin(t*…)"`) and `mograph.repeat` for periodic mechanisms; `audio` with sample hits on contacts.
- **GAP:** outdoor terrain, vegetation and sky environments of photoreal quality.
- **GAP:** a loop-check tool that verifies every animated property has the same period, so the first and last frames match.

## Judge's checklist
- Is the loop point invisible (frame N+1 equals frame 0 for every moving part)?
- Does every contact moment have a crisp, deliberate timing (no mushy ease-in-out on impacts)?
- Is there exactly one clear mechanism per shot, readable in the first second?
- Do sounds hit on the exact contact frames?
- Does the material and lighting quality hold at 100% crop (no noise, no shimmer on slow camera moves)?

## Sources
- https://vimeo.com/api/oembed.json?url=https://vimeo.com/454900647 (title, author, duration, description, credits)
- https://scandinavianmind.com/how-andreas-wannerstedt-became-a-forerunner-for-oddly-satisfying-art/ (Cinema 4D, Arnold/Redshift, hand keyframes not sims, bent physics)
- https://www.vice.com/en/article/3keamw/andreas-wannerstedts-oddly-satisfying-animations-are-internet-gold (search result, series background)
