# Round r3 (main a59b63b8, 6 briefs, 6 trials each, pinned refs)

**Change under test:** SKILL's prose self-review was replaced by an independent art-director subagent ("second pair of eyes") that critiques the contact sheet before the final render. Main also moved from r2's 41e6b737 to a59b63b8, so other merged work is a confound and the gains can't be attributed to the reviewer alone. Reviewer uptake per session: see the AGENTS audit below (pending).

**6 of 6 FAIL, 0 of 36 trials judged professional.**

| Brief | r1 | r2 | **r3** | Δ vs r2 | Δ vs mean(r1, r2) | Demo rebuild (6 trials) |
|---|---|---|---|---|---|---|
| a-social | 3.25 | 3.38 | **3.94** | +0.56 | +0.62 | – |
| c-event | 3.75 | 2.50 | **3.75** | **+1.25** | +0.62 | 3.69 (CEILING-1) |
| e-data | 3.62 | 3.38 | **4.31** | **+0.93** | **+0.81** | 4.88 (GRAPHIC) |
| h-ident | 2.75 | 2.88 | **3.00** | +0.12 | +0.18 | 3.31 (TASTE) |
| i-explainer | 2.75 | 2.88 | **3.94** | **+1.06** | **+1.12** | 3.88 (SOUND) |
| k-sports | 4.50 | 3.12 | **3.25** | +0.13 | −0.56 | – |
| **mean** | 3.44 | 3.02 | **3.70** | **+0.68** | +0.47 | |

Tie band is about 0.75: identical 6-trial runs differ by up to 0.5. Against r2, 3 of 6 briefs are up beyond noise and none are down. Against r1, only i-explainer is clearly up, and k-sports is down (r1's 4.5 remains its best).

**Per-dimension medians, r2 → r3 (same 6 briefs):**
- composition 3 → 4
- typography 3 → 3.75
- craft 3 → 4.75
- motion 2.5 → 3
- originality 2 → 3
- idea 3 → 3.25
- light 3 → 3.25

The biggest gains are craft and composition, which a contact-sheet reviewer can see. Idea barely moved.

**Gaps (unchanged themes):**
- **Idea:** every brief is told to find a concept specific to the subject. Examples: "electricity actually drives the typography" (c), "a kestrel's hover-and-stoop drives the edit and the type" (h), "built from the clubs' actual cities and crests" (k).
- **Typography:** a point of view, and type that interacts with the image (displaced by the wave, flowing through the stencil gaps).
- **Composition:** layouts that change beat to beat, with camera and change of scale.

## Cost, wall time, performance
- **Cost:** $16.41 for 6 briefs, against $11.69 for the same 6 in r2 (**+40%**). Tokens 30.0 M vs 21.0 M. Median session wall 9.3 min vs 6.9 min. This is the price of the reviewer pass plus revisions.
- **Peak RSS:** 0.46–1.42 GB. That is down sharply from r2's peaks of 3.6–5.7 GB on other briefs; none of r3's briefs were the r2 offenders.
- **Final render:** e-data took 650 s, the rest 53–85 s.
- **Defects:** `defects.py` is clean on all 6.
