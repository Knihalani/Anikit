# 5 — Crit 1 (pass 1)

`python3 scripts/judge.py run launch/pass1.mp4 --practice --trials 2 --format 16:9 --tags product-launch,cg-3d --out launch/crit1`

**Overall 4.81** (full incl. sound 4.86) · refs: da-vincis-demons 7.75 (rank 1), tiffany-holiday 7.38 (rank 2), wealthsimple-trade 2.88 (rank 4) · ref mean 6.0 · delta −1.19 · judged professional 0/2 · mean rank 3 · verdict FAIL.
Scores: idea 5.5 · composition 5.5 · typography 3.5 · colour 6.0 · light/texture 6.0 · motion 4.5 · craft 4.0 · originality 3.5 · sound 5.

## Gaps (verbatim) → kind
1. "Lock the dial lettering and model the engraving with real micro-geometry and anisotropic metal, as D does with its jewellery" → **WORLD**
2. "Give the type a point of view (scale contrast, interaction with the light bands, a deliberate face) instead of a default serif placed like a slide" → **CRAFT** (type)
3. "Vary the shots (movement close-ups, wrist or human context, a cut on the light change) so the sequence has choreography rather than one push-in" → **WORLD** (same camera every shot)
4. "Build the typography as a system (a chosen face, scale contrast, a grid tied to the dial geometry) and do not drop captions at the edges" → **CRAFT**
5. "Render the dial printing and engraving as real material, with crisp transfer printing, reflective indices and lens behaviour, so the object earns the luxury claim" → **WORLD**
6. "Vary the shots and the story beats (wrist, workshop, movement, light on a different surface) so the time-passing idea develops and does not repeat" → **WORLD**

Critic defects (motion/sound stage), all **CRAFT**: guilloché moiré/aliasing; dial wordmark broken; window gobo too soft and flat; hands/indices CG-clean, plastic rim on the case; ~1 s dead air near 8 s; blue macro underlit and muddy; end-card type generic, small line undersized.

Tells worth recording: one juror called the device "some intelligence: window-blind shadows sweep across the dial from Friday to Monday, so time passes as light", and also "a familiar luxury-watch trope… literal (time passing)". No item in the gaps list is an IDEA gap (the critic's own `idea` check passed), so under the rules the idea stays and the world and craft get rebuilt. The tell is noted as a risk: pass 2 has to make the device develop, not repeat.

## Pass 2 plan
- **WORLD:** rebuild the engraving as crisp V-cut grooves (one rose-engine wave family plus a V-cut sunburst, no interfering cosines, so no moiré); sharper window gobo with a harder sun edge; dial printing sampled with mipmaps, shown legibly only in the reveal shot (it's withheld in the macro and time-lapse shots, where it read as garbled).
- **Shot variety / beats:** new opening beat, the crown being **wound** on Friday (a macro of the knurled crown turning, with ratchet clicks), so the story becomes wound → time passes → still running. Different camera moves per shot: crown (lateral track), tip (static, rotated), centre (rotation), wide (whip-rotate), baton (lateral slide), dark tip (static), reveal (pull back).
- **Type:** the day words are now *inlaid in the bench* in the shader and lit by the window, so the mullion shadows cut through the letters and they exist only where the light falls. Scale contrast: one huge day word, one small line; no captions at frame edges.
- **Cut the weakest third:** the ~1 s braking tail of shot 4 (dead air) is cut, and the ambient-only dark stretch of shot 2 is shortened.

---

# Crit 2 (pass 2)

`python3 scripts/judge.py run launch/pass2.mp4 --practice --trials 2 --format 16:9 --tags product-launch,cg-3d --out launch/crit2`

**Overall 4.62** (full 4.64) · refs: da-vincis-demons 7.81, tiffany-holiday 7.5, wealthsimple-trade 3.0 · ref mean 6.1 · delta −1.48 · judged professional 0/2 · mean rank 3 · FAIL.
Scores: idea 4.0 · composition 5.5 · typography 3.0 · colour 6.0 · light/texture 6.0 · motion 4.5 · craft 5.0 · originality 3.0.

## Gaps (verbatim) → kind
1. "It needs a real concept beyond 'shadows move across a watch'. Compare A, where the visual language comes from the subject itself" → **IDEA**
2. "The typography should be designed rather than a default serif dropped on black: scale, integration with the dial geometry, a considered secondary line" → **CRAFT**
3. "The asset should stay consistent and the macro shots should be choreographed into an arc (assembly, time passing, a reveal) instead of interchangeable close-ups" → **WORLD**
4. "No concept beyond 'macro of a watch in moody light'; A builds a whole world out of its subject's source material, and this needs a comparable idea" → **IDEA**
5. "Type has no point of view: choose or draw a face tied to watchmaking (dial numerals, engraving) and integrate it with the image rather than adding a slide at the end" → **CRAFT**
6. "Vary the cinematography: movement, the mechanism, scale shifts, a human presence, rather than the same dial graded twice" → **WORLD**

Critic defects (CRAFT): dial printing still fragmented at 100%; moiré on the V-cut guilloché and sunburst; window bars cross "Monday." and the secondary line is low-contrast; ~1 s near-static stretch at 8–9 s; hand motion blur reads as a ghost trail; opening crown frame cropped with empty black; music changes in rigid 2.4 s blocks.

## Decision
- Pass 1 4.81 vs pass 2 4.62: the difference (0.19) is under 0.75, so it's a **tie**.
- Tie-break, fewer IDEA/WORLD gaps: pass 1 has 0 IDEA + 4 WORLD; pass 2 has 2 IDEA + 2 WORLD. Pass 2's crit raised the idea itself ("shadows move across a watch"), so **pass 1 ships** (`final.mp4` = `launch/pass1.mp4`, `spec.json` = `launch/pass1.json`, shader `src/dial.glsl`). Pass 2's sources are kept as `src/dial-pass2.glsl`, `src/build-pass2.py` and `launch/pass2.json`.
- Not done: a third pass with a replacement idea from a different family. Candidates for next time, from `2-ideas.md` and these gaps: idea C (the empty weekend, with human presence) or a mechanism-led world built from the movement's own parts. These need a new world, not more polish on this dial.

Lesson recorded: rebuilding the craft (V-cut engraving, bench inlay type, a winding shot) didn't move the score. Both juries ranked the film 3rd of 4 behind the two films that build their world out of their subject's source material. The device "light passes over the dial" is legible but reads as a familiar watch trope.
