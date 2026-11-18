# Blind judge: rubric and procedure

The parent's judgement and every automated score so far have praised work the user rejected
("boring", "AI slop typography", "generic", "not impressive"; see `../quality/BAR.md`). This judge exists to
say no to that work before the user has to. It never sees names, branches or which entry is ours.

## The question it answers

> Put our piece next to 2–3 pieces of real, style-matched studio work. Can a hard-to-impress jury tell
> which one isn't professional? If it can, how far behind is it, and what are the three biggest gaps?

Three verdicts, from 3 shuffled trials:

| Verdict | Rule | Meaning |
|---|---|---|
| **PASS** | called professional in ≥ 2 of 3 trials **and** median score within 1.0 of the refs' mean (Δ ≥ −1.0) **and** median motion ≥ 4 | indistinguishable from the style-matched studio work |
| **PRO** | called professional in ≥ 2 of 3 **and** score ≥ 5.0 **and** median motion ≥ 4 | passes as real studio work, but below the best of its style |
| **FAIL** | anything else | the jury can tell |

The thresholds were set from the controls (held-out professional refs in the candidate's seat), before the rejected work was
scored: all 6 controls were called professional in 17 of 18 trials, 4 PASS, and the two below the refs scored 5.38 (Calm, Coinbase).
Motion floor (added 01:45): a beautifully made but near-static loop (illo-portrait, motion 2) was judged professional on its stills. A film must move, so PASS/PRO also need the jury's median motion ≥ 4; all 10 controls score 5–8. The goal for the user's bar is PASS. Anything that FAILs never reaches the user.

## Procedure (`scripts/judge.py`)

```bash
python3 scripts/judge.py run out.mp4 --tags kinetic-type,product-launch [--format 9:16]
python3 scripts/judge.py batch docs/video/judge/briefs/round.json --out docs/video/judge/rounds/r1
python3 scripts/judge.py control <ref-slug>          # a held-out pro ref in the candidate seat
python3 scripts/judge.py table docs/video/judge/results/*/result.json
```

1. **Frames.** Six frames sampled **evenly in time** at (i + 0.5)/6 of the duration. No cherry-picking:
   the jury sees what a viewer sees. (References are editorially chosen stills, so the comparison leans
   *against* us. That is deliberate: a judge that is easy to pass is useless.)
2. **References.** 3 refs from `refs/index.json` with the most overlapping style tags (+1 for matching format),
   seeded tie-break. Tags are listed in `refs/INDEX.md`.
3. **Normalisation.** Every frame, ours and theirs, is downscaled to 640 px longest side, JPEG q80, then laid
   out in the same 3×2 panel (480×300 cells, dark gutter). Resolution, compression and layout carry no signal.
4. **Blind compare (stage 1).** The panels are shuffled into `A.jpg … D.jpg` in an empty temp directory and a
   **fresh `claude -p` session** (default model `claude-opus-5-5`, override with `JUDGE_MODEL`) runs there with
   only the Read tool. It is told that *between 0 and N* entries are non-professional (no forced quota), given
   the tells below, and returns per-entry verdict, confidence, scores, tells and three gaps. 3 trials with
   different shuffles; per-dimension medians count.
5. **Critic (stage 2, MP4 only).** Another fresh session sees the candidate alone at full resolution: the six
   frames, three frame pairs 1/6 s apart (motion), a 100% crop of the most detailed region (finish), the
   `pace.py` metrics and a soundtrack spectrogram + integrated LUFS. It scores motion, sound and craft. These are
   averaged into `full_scores`; the pass/fail Δ uses stage 1 only so ours and the refs are scored on equal terms.
6. **Controls.** `control <slug>` puts a held-out reference in the candidate seat. A judge that rejects
   professional work is as broken as one that accepts slop; controls must PASS.

Everything the jury saw is kept in `results/<name>/trial*/` and `critic/` so a human can audit any verdict.

## Scale (integers, every dimension)

| Score | Meaning |
|---|---|
| 10 | Among the best pieces of the year (Art of the Title feature, Ciclope/D&AD winner) |
| 8 | Strong studio work; a studio would put it on its reel |
| 6 | Competent professional, unremarkable |
| 4 | Template or student work |
| 2 | Obvious AI or automated output |

## Dimensions

| Dimension | What earns a high score | What the user rejected |
|---|---|---|
| **Idea** | A specific concept only this piece could have; memorable, shareable, surprising | Literal: product name + a diagram of the feature |
| **Composition** | Every frame a poster: dense, layered, deliberate scale and tension, uses the whole frame | Small content in an empty frame; the same layout every shot |
| **Typography** | A typographic point of view: chosen or custom faces, expressive scale contrast, type integrated with image, real layout | "AI slop typography": Inter/grotesk or generic editorial serif, tracked-caps label over a headline, set like a slide |
| **Colour** | A deliberate, ownable palette with value structure | Dark/cream ground + one accent; rainbow; mesh gradient |
| **Light / texture** | Motivated light, material, grain, depth where the subject wants it | Flat ambient fill, blown or muddy, no texture; plastic CG |
| **Motion** | Choreography, weight, variety, transitions that carry meaning (from frame pairs and the sequence) | Slow drift, one dolly, slide-deck cuts, nothing happening |
| **Sound** | Produced music and sound design, synced to picture, suited to the concept | Thin generated synth; generic score |
| **Craft** | Finish at 100%: crisp type, no banding/aliasing/stepping, consistent detail | Soft or low-res text, pops, glitches |
| **Originality** | A designer would not recognise it as a trend | "Generic", "tasteful AI", stock-demo look |

## Tells given to the jury

Non-professional: the "tasteful AI" look (dark or cream ground, small tracked caps over a serif or bold-sans
headline, one accent, empty frame); default typography with no idea; centred headline on a gradient or blob;
placeholder UI; stock 3D primitives; effect without subject; same composition repeated; flat unmotivated light;
clip-art illustration; a literal, forgettable idea.
Professional: a specific idea; dense, layered frames; typography with a point of view; an unusual deliberate
palette; real light and texture; every frame a poster; choreography; surprise.

## Why it is hard to fool

- No names, paths or captions reach the jury; the run directory holds only `A.jpg…`; the key stays outside it.
- Identical resize, recompression and panel layout for every entry; resolution is declared non-evidence.
- Evenly spaced frames: a piece cannot win on its one good frame.
- No quota: the jury may call every entry professional, so it must find *specific* tells, not pick the weakest.
- Three shuffles; medians. Controls measure the false-reject rate.
- The pass bar is relative to real studio work, not an absolute number a grader can inflate.

## Known limits

- References are stills, so their motion is inferred from frames, the same way ours is in stage 1.
- Sound is judged from a spectrogram, not heard. Treat sound scores as weak evidence.
- Fair-use thumbnails of famous work can be recognised; recognition only makes the jury surer which entry is
  professional, which keeps the bar hard, never easier.
