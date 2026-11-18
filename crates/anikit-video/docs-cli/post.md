# Post: grade, bloom, grain, motion blur

```json
{ "post": { "preset": "cinematic", "bloom": 0.4, "halation": 0.3, "vignette": 0.3, "grain": 0.02, "aberration": 0.15 },
  "motionBlur": { "shutter": 180 },
  "scenes": [ { "duration": 2, "post": "punchy", "layers": [ { "text": "GRADED", "size": "16vh" } ] } ] }
```

- Presets `clean cinematic dreamy punchy noir vintage`, or an object with overrides: `bloom bloomThreshold bloomRadius bloomTint halation aberration distortion exposure temperature tint lift gamma gain contrast saturation tonemap vignette grain grainSize dither`.
- Film LUTs: `"lut": "anikit:lut/teal-orange"` (+ `lutMix` 0..1), applied after tone mapping. 16 built-in looks, made by anikit (CC0): `neutral-print warm-negative teal-orange bleach-bypass faded-matte golden-hour cool-steel vivid-reversal pastel-dream cross-process day-for-night mono-silver mono-sepia cyber-green kodachrome-warm noir-crush`, any `.cube` path, or the film model itself: `"lut": {"base": "teal-orange", "contrast": 0.6, "sat": 1.1, "shadows": [0, 0.01, 0.03], "highs": [0.03, 0.01, -0.02], "fade": 0.04, "gamma": [1, 1, 0.95], "tealOrange": 0.5, "bleach": 0, "mono": false, "night": 0}`. `assets lut <name> -o x.cube` exports one.
- Per video or per scene (`"none"` turns it off for a scene).
- `motionBlur`: real sub-frame blur on moving frames; still frames cost nothing. Use it on whips and fast moves.
- Looks (`anikit-video looks`) set a grade for you; spec keys win.
