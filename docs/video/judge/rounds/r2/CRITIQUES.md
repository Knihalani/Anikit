# Round r2 (main 41e6b737): what changed vs r1

**12 of 12 FAIL, 0 of 36 trials judged professional.** Median blind score 3.50 (r1: 3.44). Paired with r1 on the same refs (`PAIRED.md`), the mean Δ is **−0.01**. Beyond the ±0.4 noise band:
- up: **d-illustration +1.0** (4.38, the best eval output so far), j-music +0.63, l-generative +0.5;
- down: **k-sports −1.38** and **c-event −1.25**. c-event is now "a bulleted list fading in, no image or world at all".

## The new toolkit is being used, but the score hasn't moved (`UPTAKE.md`)
- **Fonts:** the font library and variable axes are taken up. 5 briefs use `axes`, `wght` or `wdth`, with faces such as Bricolage Grotesque, Familjen Grotesk, Fraunces, Archivo, Cormorant Garamond and IBM Plex Mono instead of Inter or Instrument Serif. The jury's typography median **stays at 3**, and the critic's "type carries meaning" check fails in **9 of 12** (r1: 5). A better face, still set like a slide caption, doesn't read as a typographic idea.
- **Sound:** sound-design keys appear (`sound` 9 briefs, `ambience` 6, `cues` 6). The critic's sound median rises **4 → 5.5** and sync fails drop from 5 to 3. This is the one real gain.
- **Material and finish:** `halation`, `bloom` and `scene3d` (f-3d) are used, and `component=illo` (d). The critic's material check fails drop from 6 to 2, and "flat light/material" mentions from 7.2 to 4.8 per piece. The jury's light/texture median **stays at 3**.
- **Text presets:** `mask by line up` appears in 5 briefs. Motion median **stays at 3**. Gaps still read "a linear fade-in", "lines draw on", "no camera journey".

## Recurring critiques (unchanged from r1)
1. **Idea:** a literal map of the feature. Idea median **fell 3.5 → 3**.
2. **Typography:** set, not designed. The new faces are centred or captioned over the image and not integrated with it.
3. **No world:** c-event and k-sports have no built place at all; the frames are a diagram.
4. **Motion:** no camera journey, no escalation, no hand-offs.

## Performance regressions (PERF)
- **Peak RSS:** f-3d 5.7 GB, g-titles 5.0 GB, b-launch 3.6 GB, d-illustration 2.6 GB. r1's maximum was 1.75 GB.
- **Final render:** g-titles 1,157 s, f-3d 711 s, l-generative 422 s, j-music 339 s, e-data 267 s.
- **Spec size:** d-illustration's spec is 12.7 MB (hand-built illustration shape lists). Is the illo component missing a compact form?
- **Agents:** $29.67 for 12 briefs (r1: $24.64), 59.4 M tokens (r1: 45.4 M). Median session wall 11.4 min (r1: 12.3, which included the build). d and f each cost over $4.
- **Defects:** none. r1's j-music dead holds are gone. Flags were a deliberate black pause (l-generative 13.6 s) and single dark frames.
