# Shots at the quality bar

Two 7 s shots made to pass [`docs/video/quality/BAR.md`](../../../docs/video/quality/BAR.md), each in one look. The renders are in `docs/video/quality/shots/`.

| Shot | Look | What carries it |
|---|---|---|
| [`lighthouse/`](lighthouse/) | `noir` with a desaturated cinematic grade | A raymarched lighthouse on a rock in sea fog: a volumetric beam that sweeps across the lens (glare), moonlit swell with sparkle, and a lamp that fails at the end. Serif title set low left. |
| [`studio/`](studio/) | `daylight-lab` | A phone drops onto a white studio cove into a pool of sunlight through water (caustics, soft cast shadow). Its screen is a real 2D composition (tide curve, safe window) passed in as a shader channel. A precise flush-left headline. |

```bash
anikit-video render lighthouse/spec.json -o lighthouse.mp4   # about 0.35 s per frame on 4 cores
anikit-video render studio/spec.json -o tidewater.mp4        # about 0.23 s per frame
```

How they were judged: full-size stills at several times, 100% crops of edges, and consecutive frames from the MP4 for judder. Defects found that way and fixed:
- glitter speckle on the water;
- a smeared moon reflection;
- banding on the lantern glass;
- a bloom ring around the lamp: the grade's bloom is off here; reported to SPEC.
