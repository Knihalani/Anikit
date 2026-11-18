# Round r1 under the blind judge: what every piece got wrong

I judged the 11 delivered pieces from eval round r1 with `scripts/judge.py` (3 blind trials each against 3 style-matched references; branches `claude/epic-ramanujan-qe7x4w-v-eval-r1-*`). `l-generative` delivered no video.

**All 11 FAIL, and 0 of 33 trials judged them professional.** Scores run 2.88–4.12 against reference means of 6.9–8.6.

The lowest dimensions:
- originality, median 2–3;
- motion, median 3;
- typography, median 3;
- light and texture, median 3.

Idea scores sit at 3–5.

| Piece | Score | Refs | Piece | Score | Refs |
|---|---|---|---|---|---|
| b-launch | 4.12 | 7.54 | g-titles | 3.62 | 8.58 |
| k-sports | 4.12 | 8.55 | d-illustration | 3.50 | 7.46 |
| c-event | 3.88 | 7.79 | f-3d | 3.50 | 7.29 |
| e-data | 3.75 | 7.21 | a-social | 3.25 | 6.88 |
| j-music | 3.25 | 8.58 | h-ident | 2.88 | 8.63 |
| i-explainer | 2.88 | 7.34 | | | |

## The same seven tells, across eleven different briefs

These are counts of pieces whose jury tells name the pattern.

| Tell | Pieces | The jury's words |
|---|---|---|
| **The idea is literal: the name, or a diagram of the feature** | 11/11 | "product name plus a diagram of the feature"; Railyard = rail tracks; Kestrel = a bird; Northbridge = a bridge icon; Lisbon = azulejo; noise cancelling = a waveform and its inverse |
| **The same composition in every frame, with the words swapped** | 11/11 | "one layout with the words swapped, like a slide deck"; "frames 3–6 are nearly the same composition with only a card added" |
| **Flat, unmotivated light; glow as an effect** | 10/11 | "a flat radial-gradient glow with no motivation"; "flat studio grey"; "neon glow … an effect with no material" |
| **No texture or material, or texture applied as a filter** | 10/11 | "a generic noise texture inside flat circles"; "paper-cut texture applied as a filter rather than built from material"; "a procedural wave displacement, not a cut engraving" |
| **Stock effects and presets** | 10/11 | blur-in word reveal, mirrored text reflection, fake misregistration, drop-shadow offset puns, dot grids, a stock vanishing-point move, muddy crossfades |
| **A tiny tracked-caps label over or under a headline** | 9/11 | "small tracked-caps label ('SEPTEMBER SEA-ICE EXTENT') above a large generic editorial serif headline" is called "a signature generic tell" |
| **The "tasteful AI" formula** | 8/11 | a dark navy, charcoal or cream ground, a generic editorial serif or condensed grotesk, and one accent colour |
| Clip-art, vector-default illustration | 6/11 | "symmetrical, vector-default", "clip-art steam lines", "thin-line bridge" |
| A centred logo-and-tagline end card on a glow | 4/11 | "the stock endframe" |

## What this says

1. **"Literal" is the dominant failure, and it has a precise shape: the brand name drawn, or the feature diagrammed.**
   - The fix is upstream, in the idea: `craft/art-direction.md` §2.3 (turn the obvious one) and the director's "leave the idea family".
   - A quick self-check: write the idea without the brand name or the feature's mechanism. If nothing is left, it's literal.
2. **One layout per piece is the second.**
   - The engine can vary everything, but agents build one scene template and swap the words.
   - Plan the shot list as a sequence of *different frames*: scale, angle, crop and what fills the frame should all change.
   - Repeat a layout only as a deliberate rhyme.
3. **Light, material and texture are the third.**
   - Glows and noise filters stand in for light and surface.
   - `craft/finish-and-life.md` covers texture locked to the object and following its form; `3d-design.md` covers motivated light; SIM's lit particles help.
4. **The label-and-headline kit is now the most recognisable type tell.**
   - That kit is a tiny tracked-caps eyebrow, a big generic headline and one accent.
   - Type in support, with a role taken from the world (a timetable, a map legend), reads as designed; the eyebrow reads as a template (`craft/typography.md` §3.6).

## Changes made from this

- **`critique` tips:**
  - `label` fires when tiny tracked-caps labels recur across scenes.
  - `--render` `layout` fires when most shots share one composition (measured on the sampled frames).
- **`docs taste`:** a "what the judge keeps saying" line pointing here.
- **Director prompt:** the literal-idea self-check, and "plan a sequence of different frames".

## Round 2 update (same briefs, same refs)

12/12 FAIL again, paired mean Δ −0.01 (`docs/video/judge/rounds/r2/`). Dropped or eased: dead holds (gone), flat material (critic material fails 6 → 2), weak sound (critic sound 4 → 5.5). Persisting: the literal idea (idea median fell 3.5 → 3), one layout with the words swapped, type set like a caption (critic "type carries meaning" fails rose 5 → 9 of 12 even with new faces), no camera journey or escalation. New: c-event and k-sports had no built world at all. What moved scores since is in `WHAT-WORKED.md`.
