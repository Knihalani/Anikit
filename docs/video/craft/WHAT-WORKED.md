# What moved the blind judge tonight (and what didn't)

Evidence from `scripts/judge.py` runs on 2026-11-17/18: r1 and r2 (12 briefs each, paired on the same refs), the A/B arms, and the demo rebuilds (`docs/video/judge/DEMO-CANDIDATES.md`). **Noise:** about ±0.4 per piece between identical 3-trial runs, and ±0.5–0.75 across rebuilds that change several things at once. Treat any single move under about 0.5 as nothing. Every Anikit piece still FAILs; one has been judged professional.

## Moved the score

- **Rebuilding the structure: about +1 (the largest effect we measured).** "Structure" here means a different composition every shot, transitions that carry something (zoomThrough, carry, a match move), a colour arc, and one idea built from the subject.
  - lumen-light 3.50 → lumen-v3 4.38 (+0.88, same piece);
  - c-event r2 2.50 → OCTAVA v2 4.62 (6 trials);
  - MOGRAPH's A/B: +1.1.
- **The idea process: +0.6 to +0.75, when it leaves the obvious idea family.**
  - b-launch A/B: 3.38 → 4.00 (idea 3 → 5).
  - h-ident: 2.88 → 3.62 when the demo dropped "bird + film strip" for the hover's fixed eye (`deliverables/demo/ident/`).
  - When both arms stayed in the family, the gain was zero: h-ident A/B 3.25 = 3.25.
- **Illustration with a rig and life in the surface.** illustration_illo-portrait scored 5.25 and was judged professional 3/3, the first Anikit piece to get that. It missed PASS only on the Δ-to-refs rule. d-illustration (illo) went +1.0 r1 → r2.
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
- **Self-review in prose was a rubber stamp** (the coordinator's audit of the r2 sessions): pieces passed their own review, while the jury called 0/36 trials professional. The skill now asks an independent art-director subagent to review instead (commit 1e322278).

## What the jury keeps naming (r1 → r2 → demos)

- the literal idea ("name = animal", "the feature diagrammed");
- one layout with the words swapped;
- type that is "set, not designed" (a centred serif or tracked caps on empty ground);
- a subject at mid fidelity hidden by effects (echo blur, uniform grain);
- an ending that holds still on two near-identical frames.

## Highest-leverage next steps

1. **Make structure the default plan, not advice in a menu.** Treat it as a shot list of different frames joined by idea transitions, and judge the 6-frame contact sheet before rendering. This is the one change worth about +1.
2. **Make the idea step name and leave the idea family** ("what would ten other agents make?"), and check that the idea is *visible in 6 evenly spaced frames*. The ident's device scored "literal" because its set-up fell between samples.
3. **Commit to a fidelity the engine reaches at 100%.** The rigged illustration (5.25) beat every mid-fidelity 3D or SDF creature. Stylise deliberately rather than approximate realism.
4. **Use type as part of the image.** Light it, cast its shadow, crop it, build it from the subject. A new face changes nothing on its own.
5. **Keep the independent reviewer and the critic in the loop.** Spend agent time on 2–3 judged passes of one piece, not on more features.
