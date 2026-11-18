# 3 · One world

- **Material:** land. Four ridges of weathered stone-and-moor over a still estuary, with grain locked to the land and mist pooled at each foot. The nearest ridge's crest is drawn in vermilion ink: it is the recording. Its mirror in the water closes the shape into a waveform. One shader (`land.glsl`) builds every shot, so the material can't drift.
- **Key light:** a low sun to the right of frame, warm (≈3000 K at the horizon, cold slate overhead). Faces that fall toward it catch light; light falls off with distance from it; far ranges dissolve into the sky (atmospheric perspective). Arc: blue hour (shot 1) → first gold (end).
- **Palette family:** slate / ink blue-black, stone grey-brown, dawn amber. One accent reserved for the device: vermilion `#FF5424` — only the recording line, the object's record pip, and nothing else.
- **Type voice:** Gloock — a sharp, high-contrast wedge serif; it reads like a field-guide plate title, cinematic, not product UI. Type sits *behind* the near ridge (the land occludes it when sound rises), in bone `#F4E9DA`, lit by the same sky. Big, left, one line at a time. No eyebrows, no caps labels.
- **Sound world:** the estuary itself — wind, water, a drip, birds, thunder, rain — as foley (`sound` organic, nature ambience, outdoor space), under a small orchestral score (strings, piano, harp) at 96 bpm in D, 8 bars. One planned silence.
- **Anikit means:** one GLSL world shader drawn twice (back plate, then near ridge with alpha) so type lives between them; shape layers for the object, lit from the same right-hand key; generated music; `sound` cues on the device's moments.

## Changes shot to shot
| shot | subject | scale | camera |
|---|---|---|---|
| 1 | a drip on the water | macro, horizon high | low over the water, slow push |
| 2 | birdsong raising hills | wide | lateral drift right |
| 3 | wind rolling a range across | medium, horizon low, huge sky | tilt down |
| 4 | thunder throws a mountain | wide → wider | pull-back to keep the peak whole |
| 5 | the summit's crest, rain | extreme close on the line | static, line fills frame |
| stop | flat land, silence | wide | locked |
| 6 | the object lifted into frame | object close, land behind | rise with the object |
| 7 | the name in the land | wide, gold | slow push |

## Style frames (`frames/frames-strip.png`)
Same key moment (thunder peak, "Nothing clips.") — varied light and type: (a) blue hour + heavy grotesk, (b) full gold + dark grotesk, (c) mid dawn + Gloock.
**Picked (c):** the half-lit sky gives the key light a direction and source while the bone serif still holds 7:1 on the dusk sky — (a) had no light to point to, (b) flattened the sky into a wash and the dark type went muddy. Keep (a)→(b) as the film's light arc around (c).
