# 3 — One world

- **Material:** a watchmaker's bench by a window. One silver engine-turned (guilloché) dial: barleycorn field, a radiating rose centre, a satin chapter ring, polished steel batons, heat-blued dauphine hands, a polished steel case, dark calf strap, oiled walnut bench. All of it is one procedural shader (`src/dial.glsl`), built so the relief is real geometry for the light (normals from the engraved height field), not a texture.
- **Fidelity:** every shot is the real dial at full resolution; macro shots show the individual engine-turned grooves, the printed minute track and the faceted batons at 100% crop.
- **Key light:** a single window. The sun crosses it: raking amber from the left (Friday 17:00), cold moonlight at night, white-gold dawn from the right (Monday). The window's mullions throw a cross of shadow that slides over the dial as the sun moves; the case wall, batons and hands cast shadows. Falloff = the edge of the window patch; outside it the room is near-black.
- **Palette family:** silver / walnut / steel, tinted by the light arc amber → blue night → white-gold dawn. One accent reserved for the device: heat-blued steel of the hands (the thing that keeps moving).
- **Type voice:** Noto Serif Display — a high-contrast condensed display serif, the voice of traditional Swiss dial printing (it's what's printed on the dial). The name exists only as dial printing in black ink ("ATELIER VAUCLAIR", "CALIBRE 9"), revealed when Monday's light reaches it. Day words ("Friday.", "Monday.") are cast as shadow into the window light, so they live in the same light as the dial.
- **Sound world:** the escapement (8 ticks a second, 28,800 vph) as foley; a solo piano and string pad, D major, 100 bpm, 5 bars = 12 s. The time-lapse is a piano ostinato that accelerates; one half-bar of nothing but ticking before dawn.
- **Anikit means:** one full-frame GLSL shader layer (dial, light, hands with angular motion blur) whose dial printing is a composition channel; text layers; hand-written music tracks + explicit tick cues; `sound` mastering.

## Changes shot to shot
Subject (seconds-hand tip / hand centre / whole dial / a baton and the blurring minute hand / the tip in the dark / the whole watch on the bench), scale (macro 0.18 → wide 1.45 dial radii), camera angle (rotation −0.35 … +0.8 rad) and move (drift, push, rotate, pull-back).

## Style frames (`frames/frame-1..3.png`, same key moment: Monday morning reveal, varying the dial material)
1. silver — **picked**
2. salmon
3. slate

**Why silver:** the silver dial is a neutral screen for the light arc — amber Friday, blue night, white Monday all read on it, and the blued hands stay the only colour accent; salmon fights the amber light and slate swallows the night shots.
Fixes noted from the frames: dial printing too small and drowned by the relief (enlarge, flatten the engraving under the ink); end type was laid over the dark — move the day words into the light as cast shadow.
