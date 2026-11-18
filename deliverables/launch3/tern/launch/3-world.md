# 3 · One world

- **Material:** a rehearsal room in late afternoon. Lime plaster wall (warm off-white, trowel texture, bumped by the sun), oak floorboards, a skirting board, and the air itself: dust motes in three depths. One GLSL world shader (`room.glsl`) builds every shot, so the material can't drift. The recorder is drawn in the same shader (graphite body, perforated grille, one red pip), lit by the same sun.
- **Key light:** low sun through a four-pane window off-frame left, ≈3400 K. It lands as a hard-edged, skewed sun patch on the plaster (mullion shadows, penumbra widening with distance, leaves breaking it) and crosses the air as a visible shaft where dust catches it. Fill: cool sky bounce (≈7000 K) from the room, falling off toward the corners. Arc: afternoon → deeper gold at the end.
- **Palette family:** plaster bone / umber shadow / slate-blue fill / sun amber. One accent reserved for the device: the recorder's red pip (`#E5402A`) — nothing else red.
- **Type voice:** Bricolage Grotesque ExtraBold — ink-trapped, warm, punchy at display size; its heavy strokes survive being *projected*. Type is never laid over the image: every word is **cast in sunlight** — the window light on the wall is the type (the shader uses the text as part of the window's light mask, so letters are skewed with the sun, lit on the plaster texture and broken by the mullions/leaves). One line at a time.
- **Sound world:** the room's own sounds — a nylon-guitar pluck, a bird outside the window, a piano chord, a floor-tom hit — each one *is* a ring in the dust. Under them a sparse piano + strings score at 96 bpm in D (8 bars = 20 s), one silent half-bar before the name.
- **Anikit means:** one shader layer per shot (uniforms for camera, sound sources, freeze time, device, name), text layers rendered into the shader's `iChannel0` so type becomes light; generated music with hand-written parts; `sound` cues on each ring.

## Changes shot to shot
| shot | subject | scale | camera |
|---|---|---|---|
| 1 | a pluck's ring through the shaft | medium wall + shaft | slow push |
| 2 | bird outside: ring sweeps the floor | low, boards fill frame | low lateral drift |
| 3 | the recorder rises into the sun | macro on grille + pip | rise with the object |
| 4 | chord ring stops and hangs | medium, device foreground left | locked, then slow drift |
| 5 | tom hit: rings bigger than the frame | tight → wide | pull-back reveals every ring whole |
| 6 | silence: room full of kept rings | wide | slow drift |
| 7 | the sun patch becomes the name | wide wall, device on the sill | slow push |

## Style frames
(see below — filled after step 4)

**Style frames (`frames/frames.json`, `frames/frames-strip.png`)** — same key moment (the chord's ring hanging, recorder in the light, "already listening."), one decision varied: the type treatment. (a) letters on the window glass cast as *shadow* inside the sun patch; (b) letters cast as *light* (the window replaced by the words); (c) as (a) under a warmer, lower sun.
**Picked (a):** it is the only one that is a finished poster — the window, its mullions and the words are one physical light event on real plaster, so the type is unmistakably *in* the room; (b) loses the window (the light no longer has a source) and is kept for the ending only, where the light *becoming* the name is the payoff; (c) was indistinguishable from (a) and lost contrast.
