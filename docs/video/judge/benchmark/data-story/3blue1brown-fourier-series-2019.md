---
slug: 3blue1brown-fourier-series-2019
category: data-story
title: "But what is a Fourier series?  From heat flow to drawing with circles | DE4"
studio: "3Blue1Brown (Grant Sanderson)"
client: "Self-initiated"
year: 2019
url: https://www.youtube.com/watch?v=r6sGWTCMz2k
alt_urls: [https://github.com/3b1b/manim]
length_s: null
verified: oembed
aspect: "16:9"
medium: [2d-vector, data-viz, kinetic-type]
pace: steady
palette: [dark, limited, cool]
type_role: supporting
mood: [technical, calm, witty]
purpose: data-story
techniques: [chart-build, morph, type-morph, loop, long-take, transitions-in-camera]
audio: vo-led
anikit_features: [shape, "shape path trimStart/trimEnd (traced curves)", "mograph path/progress (epicycles)", text, "text (LaTeX maths: GAP)", "animate (transform)", camera, audio, "GAP: native LaTeX/maths typesetting with glyph-level morphs between equations", "GAP: programmatic/procedural animation driven by computed data (epicycle sums)"]
awards: []
stills:
  - https://i.ytimg.com/vi/r6sGWTCMz2k/maxresdefault.jpg
  - https://i.ytimg.com/vi/r6sGWTCMz2k/hq1.jpg
  - https://i.ytimg.com/vi/r6sGWTCMz2k/hq2.jpg
  - https://i.ytimg.com/vi/r6sGWTCMz2k/hq3.jpg
measured: null
---

# But what is a Fourier series? — 3Blue1Brown (2019)

**The reference for computed motion: every curve, vector and equation is generated from the maths itself (in Manim), so the animation *is* the proof.**

## The idea
Start from the heat equation, build up to complex Fourier series, and end with rotating vectors (epicycles) that sum to draw arbitrary shapes. The video description says the animations were mostly made with Manim, Grant Sanderson's open-source Python library (*source*: description mirrored on yt.chop.dev). Posted June 2019 (*source*: same).

Length: `null` (no oEmbed duration). It runs well past 90 s (*est.* ~25 min; chapter "DE4" of a series). Analyse the 60 s epicycle-drawing sequence, which is the series' best-known passage.

## Structure and pacing
Steady lecture pacing: long continuous scenes in which objects transform rather than cut; *est.* median "shot" 5–10 s, but something moves at all times.

## Typography
*observed* (hq3): Computer Modern serif (LaTeX), in quotes ("Vector", "Complex number"), colour-coded — light teal (~#58C4DD) and yellow (~#FFFF00 family) on black; a speech bubble in the same serif. Equations are typeset and morph glyph-by-glyph between steps (*est.*: Manim's TransformMatchingTex behaviour).

## Colour and light
Pure black ground; a small Manim palette: blue/teal shades (~#236B8E → #58C4DD), yellow, white, a warm grey-brown for the narrator "pi" character (*observed* hq3). Colour is semantic: the same quantity keeps the same colour across all scenes.

## Motion techniques
- Pi creatures (cartoon π glyphs with eyes) react as an audience (*observed*).
- Epicycles: chains of rotating arrows, each a Fourier term, whose tip traces a path that draws on (*est.*, the title's "drawing with circles").
- Equations and graphs transform into one another; curves draw on along their length.

## Transitions
Transform-based: one diagram becomes the next; few cuts (*est.*, Manim house style).

## Sound design and music
VO-led by Sanderson; quiet piano bed (Vincent Rubinetti scores the channel — *est.*, widely credited but not verified here).

## What makes it world-class
1. Motion is computed from the subject: the paths are exact, not hand-keyed approximations.
2. Colour is a stable encoding across the whole video.
3. Equations morph term by term, so the viewer sees *which* symbol becomes which.
4. A black ground and thin anti-aliased strokes stay crisp at 100% crop.
5. Characters (pi creatures) add warmth without clutter.

## What Anikit would need
`shape` paths with `trimEnd` draw-on; `path`/`progress` and expressions (`"=..."`) for rotating vectors; `text` for labels; transforms between `shape`s; `audio`. GAP: LaTeX typesetting with glyph-matched morphs; computed/procedural geometry from a script (sum of rotating vectors) in the spec.

## Judge's checklist
- Is the motion derived from the data/maths (exact) rather than eyeballed?
- Does each quantity keep one colour throughout?
- Do equations/labels transform in place so viewers can track each term?
- Are thin strokes and type crisp at 100% crop with no shimmer while moving?

## Sources
- YouTube oEmbed for r6sGWTCMz2k (title, author 3Blue1Brown); hq3 still viewed
- https://yt.chop.dev/en/v/r6sGWTCMz2k (mirror of the description: Manim, June 2019)
- https://nibble-app.com/blog/manim (search summary)
