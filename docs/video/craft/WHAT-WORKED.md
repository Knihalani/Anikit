# What moved the blind judge tonight (and what didn't)

Evidence from `scripts/judge.py` runs on 2026-11-17/18: r1 and r2 (12 briefs each, paired on the same refs), the A/B arms, and the demo rebuilds (`docs/video/judge/DEMO-CANDIDATES.md`). **Noise:** run-to-run spread is about ±0.5 even at 6 trials (JUDGE's calibration), so treat any move under 0.75 as a tie. Every Anikit piece still FAILs.

## Moved the score

- **Rebuilding the structure: about +1 (the largest effect we measured).** "Structure" here means a different composition every shot, transitions that carry something (zoomThrough, carry, a match move), a colour arc, and one idea built from the subject.
  - lumen-light 3.50 → lumen-v3 4.38 (+0.88, same piece);
  - c-event r2 2.50 → OCTAVA v2 4.62 (6 trials);
  - MOGRAPH's A/B: +1.1.
- **Rebuild results from the demo round** (category gains over r2, pooled where re-judged):
  - i-explainer (SOUND): about +1.1, pooled ~4.0 vs 2.88;
  - b-launch (CEILING-2 board): about +0.9, best 4.62;
  - g-titles (TYPE): +0.4, a tie;
  - h-ident (this session): ~3.4 pooled over 9 trials vs 2.88, a tie by the band;
  - l-generative: no gain.
- **The idea process raises the *idea* score, not yet the overall.**
  - b-launch A/B: idea 3 → 5, overall 3.38 → 4.00 (+0.62, inside the tie band).
  - h-ident: leaving "bird + film strip" for the hover's fixed eye passed the strip critic's idea check once it saw consecutive frames (idea, invariant, arc and ending all YES; motion 5). But the blind stills jury still scored craft 2.5 and originality 2 (`deliverables/demo/ident/`).
  - When both arms stayed in the family, the gain was zero: h-ident A/B 3.25 = 3.25.
- **Illustration with a rig and life in the surface reads as professional in stills.** illustration_illo-portrait was judged pro 3/3 on stills (5.25). It still FAILs under the current rule, because it is a near-static loop and its jury motion score is 2, below the required 4. d-illustration (illo) went +1.0 r1 → r2.
- **Sound, the one critic dimension that rose across a whole round.** In r2, sound keys were used in 9 briefs, the critic's sound median went 4 → 5.5 and sync fails fell 5 → 3.

## Didn't move it

- **Fonts: +0.**
  - lumen-type with library faces scored 4.38, identical to the Inter version.
  - Across r2, 5 briefs adopted variable axes and new faces, yet the typography median stayed at 3, and the critic's "type carries meaning" check failed 9/12 (r1: 5/12).
  - A better face set like a caption still reads as a default.
- **Guide menus and feature lists raised uptake, not scores.** r2 used `sound`, `axes`, `mask by line up`, `halation` and `scene3d` far more than r1. The paired mean Δ was **−0.01** over 12 briefs: 3 up and 2 down beyond noise.
- **Engine fidelity moved the critic, not the jury.**
  - The critic's material fails fell 6 → 2 and "flat light" mentions fell 7.2 → 4.8 per piece.
  - The blind light/texture median stayed at 3.
  - In the ident demo, a wing-flex, dusk and grain pass changed the blind score by −0.37 (noise).
- **Self-review in prose was a rubber stamp.** AGENTS audited the r2 transcripts: after looking at the contact sheet, 2 of 12 sessions made substantive changes, 9 made cosmetic ones and 1 made none, and no session wrote out answers to the review questions. The skill now asks an independent art-director subagent to review instead (commit 1e322278).

## What the jury keeps naming (r1 → r2 → demos)

- the literal idea ("name = animal", "the feature diagrammed");
- one layout with the words swapped;
- type that is "set, not designed" (a centred serif or tracked caps on empty ground);
- a subject at mid fidelity hidden by effects (echo blur, uniform grain);
- an ending that holds still on two near-identical frames.

## Highest-leverage next steps

1. **Make structure the default plan, not advice in a menu.** Treat it as a shot list of different frames joined by idea transitions, and judge the 6-frame contact sheet before rendering. This is the one change worth about +1.
2. **Make the idea step name and leave the idea family** ("what would ten other agents make?"), and check that the idea is *visible in 6 evenly spaced frames*. The ident's device scored "literal" because its set-up fell between samples.
3. **Commit to a fidelity the engine reaches at 100%, and keep it moving.** The rigged illustration (pro on stills) beat every mid-fidelity 3D or SDF creature, but it needs motion ≥ 4 to pass. Stylise deliberately rather than approximate realism.
4. **Use type as part of the image.** Light it, cast its shadow, crop it, build it from the subject. A new face changes nothing on its own.
5. **Keep the independent reviewer and the critic in the loop.** Spend agent time on 2–3 judged passes of one piece, not on more features.
