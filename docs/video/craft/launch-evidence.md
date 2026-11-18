# Launch films: what separates ours from the references (internal evidence)

Sources: jury and critic JSON for our b-launch films (r1, r2, `v-sound_railyard`, the 5 s `mograph_atlas`/`velo` stingers, Railyard "Arrivals" passes 1–5, `deliverables/anikit-launch`), the other demo rebuilds, and the `product-launch/` and `ui-product/` benchmark notes.

**Noise.** Identical runs differ by up to about 0.5. r1 b-launch: 3.75 and 4.12 in two runs; Railyard pass 2: 4.62 (3 trials), 4.31 (6). Treat any |Δ| < 0.75 as a tie, and a single 3-trial run as an anecdote.

## 1. What the professional launch films do

**Numbers are estimates.** `measured.json` holds one piece (Blender *Charge*). All 12 launch notes have `measured: null`; shot counts are *est.* from length and stills.

| | Product launch | UI launch |
|---|---|---|
| Length | 26–65 s (mostly ~30 s) | 38–57 s |
| Shots (est.) | 8–28; median ≈ 1.5–3 s (benchmark median 2.0 s) | 10–35; median ≈ 1–3 s (benchmark 1.5 s) |
| Type role | supporting 5, none 1: a wordmark lock-up, or type printed *on* the object (Air Max) | lead 4, supporting 2: the UI's own content ("Ask Linear…"), never a caption |
| Worlds / palettes | **one per film**: XREAL cobalt; Devialet red/gold/ivory; Nothing, Oppo mono | one ground per film: Framer black, Whisk yellow, Raycast mono + copper |
| Product | is the environment (Nothing); emerges from a material (Oppo snow, XREAL embryo); is moved by a mechanism (Devialet clockwork); is withheld to the end (B&O) | real UI and copy, at extreme scale or as a lit object in space; no placeholder bars |
| Ending | lock-up on the product's centre line, group hero, or loop to frame 1 | logo on black with ≥ 25% left (Linear); CTA over the payoff image (Whisk) |

**The pinned b-launch refs,** in the jury's own words (trial JSON):
- **Coinbase:** "one strict system: every image is made of monospaced ASCII glyphs on a saturated … blue", yet "every frame is a different structure: a running figure, a coin trailing glyphs, a planet with orbital rings…".
- **JPMorgan:** "raked window-shadow light … consistent across every frame" on cream, gold and glass, and "each shot reworks the same token into a new form".
- **XREAL:** "real variety between frames … the product turned into a material metaphor, carried consistently".

**The rule.** Hold material, light and palette family constant (a scripted arc inside one world is fine; Air Max and Material 3 vary ground colour but keep light and grammar). Change object, scale and composition every frame.

**Judge asymmetry.** Ref "frames" are six studio case-study stills, i.e. hero frames. Ours are six frames sampled evenly in time. Each of our six sample times must land on a poster frame.

## 2. Recurring jury complaints about our launch films (7 films: r1, r2, v-sound, atlas, velo, Railyard, Anikit)

Ranked by films whose tells or gaps name it.

1. **The literal idea: the product name plus a diagram of the feature. 7/7.** Railyard became rails, then a departure board; Anikit "shows its own JSON spec as the picture".
   - *Moved the idea score, not the total:* the A/B "craft" idea process raised idea 3 → 5, overall 3.38 → 4.00 (a tie). Its split-flap board was still "a literal pun" in every trial.
   - *Moved the total, but on other briefs:* a device made from the subject's own behaviour or data. e-data r2 3.38 → v1 4.38 (the ice cover *is* the extent). i-explainer r2 2.88 → v2 4.0 (liquid-surface device; confounded: r3 reached 3.94 without it). j-music r2 3.75 → pass 1 4.38 (the lamps write the lyric).
   - No b-launch film has had such a device, and none has gone above ~4.4. The best launch-type film is Burrow (d-illustration, 5.25 owner 6-trial / 4.62 JUDGE): one story in one storybook world, some type placed in the scene; its tagline was still called literal.
2. **Type set like a slide, in a default face; a stock end card. 7/7.** The critic's "type carries meaning" check fails in 5/7.
   - *Didn't move it:* new faces (r2: 5 briefs adopted the library and variable axes, typography median stayed 3). Railyard pass 5 reframed its end cards: 4.12, no gain.
   - *Moved it:* type physically inside the material. e-data v1 4.38 → v4 5.25 (3 trials; ~4.9–5.0 at 6), from years painted on the floes that drown as the water opens.
3. **Flat or unmotivated light, no material. Named in 7/7; critic material check fails 5/7.**
   - *Didn't move the jury:*
     - Railyard pass 3 (lamp shaft in haze, rim light, green wave) 4.25, then pass 4 4.0, after pass 2's 4.62;
     - ray-traced 3D board (pass 1) 3.88 vs the TASTE 2D board 4.0;
     - r2's halation and bloom: the light median stayed 3.
   - The refs are praised for light that is *consistent across frames*, not for fidelity.
4. **One layout with the words swapped / one space. 6/7.**
   - *Moved it (at the noise edge):*
     - r2 b-launch was 2 shots, median 7.5 s: 3.62;
     - Railyard pass 1, one drifting camera: 3.88;
     - pass 2, 7 authored shots with hard cuts on flips: 4.62 (3 trials), 4.31 (6 trials), pooled ~4.4.
   - Cross-brief, a structural rebuild is the largest effect measured: c-event 2.50 → OCTAVA 4.62, lumen 3.50 → 4.38.
   - Railyard's ceiling is still "every frame is the same board".
5. **The opposite failure: no coherent world. 1/7 films, but it dominates that one.** The Anikit launch scored 3.62. Six tells name it ("three templates stitched together", "the palette changes from frame to frame"), and the critic noted "the palette swaps completely every shot". Its concept planned a different palette per beat.
6. **The "tasteful-AI" kit. 4/7:** a dark ground, one accent, a tiny tracked mono label and a bold grotesk.
7. **Visible defects and empty frames:**
   - garbled glyphs ("NNNP 7PR4k") and text clipped at the frame edge in Railyard (repeatedly), v-sound and Anikit;
   - a mostly empty grey frame in Anikit.
   - *Fixing them didn't raise the score* (pass 5: 4.12), but the jury cites them as "generator artifacts". Don't ship them.
8. **No poster frames, no arc, a dead ending.** The critic's `freeze_frame` check fails **7/7**, `arc` fails in 3 (atlas, Railyard, Anikit), and both Railyard and Anikit end on 4 s or more of near-static cards. No fix has been tested.
9. **Copy:**
   - a feature list on screen: Railyard's end card is the brief's three facts verbatim;
   - a "fictional-sounding brand", flagged twice in Railyard pass 2;
   - self-describing tool copy: the jury's first Anikit tell was "self-promo for an automated 'spec in, film out' tool".

**Pace is necessary but not sufficient.** Velo and Atlas pass `pace.py` (median 0.8 s and 1.4 s) and scored 2.75 and 3.0, the lowest of the set.

## 3. Binary checks to run on your own six frames

Use 6 frames evenly spaced in time, as the judge does. The thresholds are heuristics and have not been validated against the judge.

**Hold constant:**
1. Do all 6 frames share one light logic and palette family? An arc within one world passes; a full look swap between shots fails.
2. Is there one material world (the same surface, medium and grain) in all 6? (JPMorgan: cream/gold/glass under window light, every frame.)
3. Can you point to a key light with falloff in every frame, rather than a radial-gradient glow?

**Change:**
4. Does each frame differ from every other in subject, scale or camera angle? Fail if any two share a layout with only the words changed.
5. Is every frame a poster: one clear focal subject, made imagery, with no frame holding more than ~40% flat, untextured background?

**Idea and type:**
6. Literal test: describe the idea without the product name or the feature's mechanism. Is anything left?
7. Swap test: put a competitor's name in. Does any frame still work?
8. Does any frame exist to show an engine feature, or carry a caption that names a technique ("simulated", "fps", "API call")?
9. In at least half of the frames with text, is the type inside the scene (painted, lit, casting a shadow, made of the material, or the UI's own content), rather than laid over it?
10. Does any frame use the kit: a tiny tracked-caps or mono label plus a headline, or a centred headline over a backdrop?
11. Is there more than one claim line on screen in any frame, or is the brief's fact list repeated verbatim?

**Defects and ending:**
12. Is any glyph string garbled, or any word clipped by the frame edge unintentionally?
13. Are frames 5 and 6 visibly different, with no ≥ 2 s static card at the end?
14. Does the end frame resolve the device (the product and the lock-up made of the film's world), rather than a logo-plus-tagline slide?
15. ≥ 6 shots or major changes in 15 s (necessary, not sufficient)?

## 4. Failure patterns caused by the brief or the skill

- **Fact-list briefs become feature-beat films.** The Railyard brief lists three facts (one call, instant settlement, a webhook per state). Agents give each fact a beat and a caption, and the end card prints all three. The Anikit concept assigned one beat per engine feature (music, shader, fluid, sound, speed) and captioned each, and the jury called it "unrelated effect demos". Demand *one* demonstrated truth; leave the rest unsaid.
- **Metaphor-finding breeds name or mechanism films.** Skill-era agents drew Railyard as rails; the A/B craft process gave the board, still in the name's family; the Anikit piano roll shows its own mechanism. Make the gate explicit: the idea must survive deleting the product name.
- **"Differ on every axis" and the colour arc, read inside one film.** The skill says to vary medium and look *between films* (BAR §"repeats a look") and to sketch directions that "differ on every axis". The Anikit concept applied this within one film: six palettes. State it plainly: vary across films, hold one world within a film, and vary only the composition.
- **Hypothesis (untested):** the rejected-looks lists leave "dark ground + one accent + mono label" as the safe default (4/7 films).
- **Self-promo films show the tool.** The jury read "spec in, film out" copy as a sign the film was generated. Show what the product makes possible, not its claims.
- **The polish loop plateaus.** Railyard passes 3–5 changed how it looks (haze, flap timing, framing), not what it is, and scored 4.25 → 4.0 → 4.12. Stop after about two judged passes without a concept change.
- **Brief and judge limits we can't fix in the skill:**
  - fictional brand names read as an AI tell;
  - the Anikit film was judged against *payments* refs (b-launch tags), in one 3-trial run, so 3.62 is ±0.5 and mismatched in genre;
