# Judge: overnight summary (2026-11-17/18)

## What the judge is
A blind, style-matched comparison against real studio work (`RUBRIC.md`, `scripts/judge.py`).
- **Frames:** six frames sampled evenly in time from our film and from 3 references with matching tags (pinned per brief in `pins.json`). All frames are normalised identically, shuffled and labelled A–D.
- **Jury:** a fresh `claude -p` session that can only read the images. It is told that 0 to N of the entries may be non-professional, and has to say which ones and why, scoring each entry on 8 dimensions.
- **Trials:** 3 or 6 shuffles, taking the median per dimension.
- **Critic:** a second fresh session scores motion, sound and craft from full-resolution frames, frame pairs, a 100% crop, pace metrics and the spectrogram.
- **Verdicts:**
  - **PASS:** judged professional in ≥ 2/3 of trials, within 1.0 of the refs, and motion ≥ 4.
  - **PRO:** judged professional, score ≥ 5 and motion ≥ 4.
  - **FAIL:** anything else.
- **References:** 95 refs with ≥ 4 real frames each: 41 of ours (`refs/`, fair-use thumbnails) plus REFS's benchmark (stills rebuilt into `.cache/` by `judge.py sync-bench`).

## Does it reject what the user rejected? (`VALIDATION.md`)
- **Rejected work:** all 20 FAIL (div-a…g, blind-ak2, launch-test, sift/fernway/margin and the 8 rejected stills). In **0 of 60 trials** was any of them judged professional. Scores 1.75–4.38.
- **Controls:** held-out professional refs in the candidate seat scored 5.38–8.12. **10 of 10 were PASS or PRO, with 0 false rejects.** One early false reject came from 480p YouTube grabs and was fixed with real 1280 px frames.
- **The user's words:** the jury's critiques mention "generic/template/AI" 3.4× and "typography" 4.3× more often for rejected work than for the controls. TASTE's independent review of the same pieces found the same failure modes.
- **Noise:** identical runs differ by up to about 0.5, so treat |Δ| < 0.75 as a tie.

## Eval rounds (`ROUNDS.md`, `rounds/*/`)

| Round | Main | Briefs | Blind score | Judged pro | Notes |
|---|---|---|---|---|---|
| div (rejected) | – | 7 | mean 3.21, median 3.12 (re-judged on r1 refs) | 0 | |
| r1 | 75cc69ab | 12 (+3 HyperFrames) | median 3.44, mean 3.43 | 0/45 trials | HyperFrames was below us on all 3 shared briefs |
| r2 | 41e6b737 | 12 | median 3.50 | 0/36 | paired Δ vs r1: −0.01; font, sound and FX uptake didn't move the jury; RSS regression to 5.7 GB |
| r3 | a59b63b8 | 6 | mean 3.70, median 3.84 (same 6 in r2: mean 3.02) | 0/36 | +0.68 vs r2; reviewer audit: the gain is not attributable to the reviewer; +40% cost |

What every round's critiques keep asking for (`rounds/r1/CRITIQUES.md` and later):
1. One idea or device specific to the subject, escalated through the film.
2. Typography designed into the image, not slide captions in a default face.
3. Each frame a different poster.
4. Real material and motivated light.
5. Motion with weight and hand-offs.
6. Sound effects drawn from the subject.

The toolkit features landed in the specs. These composition-level gaps remain.

## Demo candidates (`DEMO-CANDIDATES.md`; pooled 6-trial paired runs)
Best per brief:

| Brief | Score | Source |
|---|---|---|
| e-data | ~5.1 | GRAPHIC v11 (v9 scored ~5.4 but had a 9 vs 18 million km² contradiction) |
| d-illustration | ~4.9 | COMPONENTS |
| OCTAVA (event promo) | ~4.6 | CEILING-1 |
| k-sports | ~4.2 | r1 (pooled 12 trials: 4.5 r1, 4.25 six-trial re-judge, 3.75 MOGRAPH re-judge); MOGRAPH rebuild passes 3.5 |
| b-launch | ~4.4 | CEILING-2 |
| j-music | ~4.4 | MUSIC |
| g-titles | ~4.3 | TYPE |
| i-explainer | ~4.0 | SOUND / r3 |
| c-event | ~3.9 | CEILING-1 / r3 |
| a-social | 3.94 | r3 |
| f-3d | 3.62 | r2 (3D rebuild ties at 3.62 / 6 trials, typography 2) |
| l-generative | 3.62 | r2 |
| h-ident | ~3.4 | TASTE |

No output tonight reached PASS or PRO. The best sits about 2 points below its references (7–8.5).

## Known limits
- **Motion:** it is inferred from 6 stills. A blind motion stage on strips was built and validated twice, and both times it was kept off. The only lawful clips are CC/PD (Blender, NASA, VPRO), which let two rejected films through. With real reference clips (user approval needed) it can be validated properly.
- **Short single-device pieces:** idents built on one device can hide that device between the 6 sampled frames. The strip critic (`JUDGE_STRIPS=1`) sees it; read the two together.
- **Sound:** judged from a spectrogram only (weak evidence).
- **Cost:** about $0.6 per 3-trial judgement and $1.1 per 6-trial judgement.
