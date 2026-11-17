# Post: grade, bloom, grain, motion blur

```json
{ "post": { "preset": "cinematic", "bloom": 0.4, "halation": 0.3, "vignette": 0.3, "grain": 0.02, "aberration": 0.15 },
  "motionBlur": { "shutter": 180 },
  "scenes": [ { "duration": 2, "post": "punchy", "layers": [ { "text": "GRADED", "size": "16vh" } ] } ] }
```

- Presets `clean cinematic dreamy punchy noir vintage`, or an object with overrides: `bloom bloomThreshold bloomRadius bloomTint halation aberration distortion exposure temperature tint lift gamma gain contrast saturation tonemap vignette grain grainSize dither`.
- Per video or per scene (`"none"` turns it off for a scene).
- `motionBlur`: real sub-frame blur on moving frames; still frames cost nothing. Use it on whips and fast moves.
- Looks (`anikit-video looks`) set a grade for you; spec keys win.
