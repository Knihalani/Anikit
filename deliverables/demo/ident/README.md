# h-ident demo rebuild: Kestrel, "the still point"

Brief (docs/video/judge/BRIEFS.md): Kestrel is an independent film distributor. Make its 6 s 16:9 logo ident, the one that plays before every film it releases.

## Idea

r1 and r2, and both arms of the A/B, all landed in the same idea family: a bird, a film strip and the name. This version leaves that family and builds on one fact about the subject instead. A hovering kestrel holds its head perfectly still in a gale while its wings, tail, body and the whole world move around it ("motionless by moving").

The film has one device, and it escalates:
- **0–3.3 s:** the camera is locked to the kestrel's eye. The world rolls and heaves around that fixed point as the gale builds (the horizon swings about 30°), and the camera pulls back from a backlit close-up to the whole bird.
- **3.3 s:** the stoop. The bird folds and drops out of frame under gravity.
- **3.3–4.3 s:** the eye stays where it was, then cools and swells into a full stop. This is the visible transition the r2 critic asked for.
- **3.7–6 s:** the word *kestrel* blows in on the wind with weight (staggered drag plus inertia), up to the full stop that never moved. It stands in the field, backlit by the same sun, throws long shadows towards the viewer, and has grass in front of its feet. A slow push-in carries the hold.

For a distributor, the device stands for patience, then commitment: the still point before the film.

## Craft choices

- **Light:** one low sun, off frame upper left. It gives a rim on the feather fringe, light through the flight feathers, a catchlight on the sun side of the eye, cloud edges lit from the left, backlit seed heads, and the rim and shadows on the letters.
- **Palette:** taken from the bird itself (slate head, rufous back, yellow orbital ring) and from a dry field at golden hour.
- **Type:** Gloock, lowercase, -0.02em. The full stop is the conceit.
- **Sound from the subject:**
  - the gale (filtered noise whose cutoff rises with the gust);
  - a 6.1 Hz wingbeat flutter;
  - a whoosh on the stoop, then a planned near-silence;
  - a low cello under the word;
  - the kee-kee-kee call over the blink of the full stop.
  - Loudness is -16.1 LUFS.
- **Everything in the image is one GLSL shader** (`kestrel.glsl`). The type comes in through a channel (`iChannel0`), so the shader can light it, cast its shadow and put grass in front of it. The wing motion blur is 10 sub-frame samples over a 1/90 s shutter, with per-pixel jitter.

## Judge (JUDGE's paired invocation: brief h-ident, seed 7, 3 trials)

Scores are blind / full.

| | Blind | Full | Idea | Comp | Type | Colour | Light | Motion | Craft | Orig |
|---|---|---|---|---|---|---|---|---|---|---|
| r1 | 2.75 | 3.17 | | | | | | | | |
| r2 | 2.88 | 3.56 | 3 | 3 | 3 | 4 | 3 | 2 | 3 | 2 |
| v1 | 3.38 | 3.56 | 3 | 3 | 3 | 5 | 4 | 3 | 3 | 3 |
| **v3 (final)** | **3.62** | **3.72** | 3 | 4 | 3 | 5 | 4 | 4 | 3 | 3 |
| v4 | 3.25 | 3.61 | 3 | 4 | 3 | 5 | 4 | 3 | 2 | 2 |

What changed between passes:
- **v1 → v3:**
  - the world rolls around the pinned eye, so frames differ;
  - the type is composited into the field with backlight, shadow and grass;
  - more blur samples.
- **v3 → v4:**
  - wing flex;
  - a longer push and a dusk shift;
  - lighter grain.

v4 sits inside the noise of v3 (±0.4), so I stopped at that plateau. Every pass is still a FAIL, judged professional 0/3.

## What still holds it back, in the judge's words

- **The bird:** "flat cards hinged at the shoulder", with stacked echo blur. An SDF bird drawn in 2D caps craft at about 3. A rigged glTF bird, or a committed stylised language (cut paper, ink), is the next step.
- **The idea doesn't read in 6 sampled frames:** the gaps ask for "the wordmark … held in the bird's still point", which is this film's idea. The judge sees the dot in the air (frame 4) but not the pinned eye that sets it up. One fix to try: hold a tighter frame on the eye just before the stoop.
- **The type is called a "tasteful lowercase serif + full stop" default,** despite carrying the conceit.

## Reproduce

```bash
ANIKIT_RENDER_CACHE=0 anikit-video render spec.json -o final.mp4 --quiet
python3 scripts/defects.py deliverables/demo/ident/final.mp4      # OK: no flat, frozen or duplicate frames
python3 scripts/judge.py brief h-ident deliverables/demo/ident/final.mp4 --out deliverables/demo/ident/judge
```

The render runs about 2m40 on 4 cores.

`--quiet` is needed because the pre-render estimate panics on this spec ("range end index 2 out of range for slice of length 1" in `pipeline::estimate`); this has been reported. The spec's invisible full-frame rect was a failed attempt to dodge that panic. It changes nothing in the picture, and it stays because the spec is exactly what rendered `final.mp4`.
