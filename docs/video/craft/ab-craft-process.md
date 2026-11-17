# Does the art-direction process move the blind judge? (A/B, 2026-11-17)

**Setup.** Fresh agents ran two briefs from JUDGE's fixed set, `b-launch` (Railyard, a payments API, 15 s) and `h-ident` (Kestrel, a film distributor, 6 s), in two arms. All four built from the same branch (`-v-taste` at 08e4984), so the engine and skills were identical.
- **base:** the standard eval wrapper (read `skills/anikit-video/SKILL.md`, make it as good as a top studio).
- **craft:** the same wrapper, plus "develop the idea first" with `craft/art-direction.md` §2/§5, `craft/storytelling.md` §2–3/§6 and `craft/award-level.md` §1/§6, keeping a `concept.md`.

Each arm was judged with `scripts/judge.py run --seed 7`: 3 blind trials against the same 3 style-matched references per brief. Outputs are on `claude/epic-ramanujan-qe7x4w-v-ab-{b-launch,h-ident}-{base,craft}`.

## Results

| Brief | Arm | Overall | Idea | Originality | Composition | Type | Colour | Light/texture | Motion | Craft | Judged pro | Refs mean |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| b-launch | base | 3.38 | 3 | 3 | 4 | 3 | 4 | 3 | 4 | 3 | 0/3 | 7.66 |
| b-launch | **craft** | **4.00** | **5** | **4** | 4 | **4** | **5** | 3 | 4 | 3 | 0/3 | 7.09 |
| h-ident | base | 3.25 | 4 | 3 | 4 | 3 | 3 | 3 | 3 | 3 | 0/3 | 8.63 |
| h-ident | craft | 3.25 | 4 | 3 | 3 | 2 | 4 | 4 | 3 | 3 | 0/3 | 8.50 |

n = 2 briefs × 3 trials, so this is a direction, not proof.

## What it says

1. **The process makes better ideas, and on one brief the jury saw it.**
   - Railyard-craft listed 18 clichés, 25 facts and 33 concepts, discarded the obvious ten, and landed on "a split-flap departure board where every row reads DELAYED, then one row doesn't". The judge's idea score went 3 → 5 and originality 3 → 4.
   - Kestrel-craft's concept work was also strong (the hover as "motionless by moving", projector intermittent movement). But both Kestrel arms converged on the same Muybridge film strip, and the jury called both literal ("name equals bird"). The process didn't escape the obvious *idea family*. It only got a better version of it.
2. **Execution caps the score at about 4, whatever the idea.** All four FAIL by 3–5 points, and the judges' tells are about execution, not concept:
   - **Type.** "Widely tracked generic sans/serif caps, centred" in all four. The engine's 3 bundled faces push every piece to the same few looks.
   - **Material and mechanism.** "The split-flap board is rendered as flat grey tiles with no mechanism, depth, hinge shadow or real light"; the bird is "a symmetrical vector-default silhouette".
   - **Layout repetition.** "Same layout every frame … it is a slide deck"; "frames 4–6 are essentially identical".
   - **The tasteful-AI palette.** A dark ground with one amber or rust accent and small tracked labels appears in 3 of 4.
3. **So the next lever is execution, in this order:**
   1. type (faces with character, set as image, choreographed): TYPE;
   2. rendering the idea's *material* at full fidelity (the flap mechanism with hinge shadow, film emulsion, feathers): 3D, COMPONENTS, SHADER;
   3. a different composition every shot (scale, angle, crop; the storyboard should forbid repeats);
   4. palettes taken from the idea's world, not "dark + one accent".

## Changes made from this

- **Director prompt.** Step 2 now asks the agent to name the idea *family* that ten other agents would reach for (a film strip for a film brand, a bird for a bird name) and to turn away from it, not just from the first ten lines.
- **Storyboard advice.** No two consecutive shots share scale, angle and layout.
- **TASTE §11.** The idea's material at full fidelity is listed as the place to spend effort: a flap that is a mechanism, not a grey tile.
