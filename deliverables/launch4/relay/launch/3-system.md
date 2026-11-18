# 3 · One visual system: the night clock

- **Derivation line:** *Relay works while you sleep and hands over at dawn → the film is one night on one clock; the hour sets the light of the ground, and nothing on screen is "done" until the clock says morning.* Every shot is stamped by the clock (the huge numerals behind, or the timestamps in the rows), and the ground's colour is computed from the hour (ink at night, first light only after 05:00, daylight at 07:00).
- **Subject per shot:** Relay's own UI (inbox rows, the brief, a drafted reply, the Send button), always cropped close, or the clock numerals as the set. No phone mock, no bedroom, no person, no device floating in space.
- **Material world:** flat UI surfaces (rounded cards, hairline borders) layered on a graded sky-ground; depth from overlap and scale only. One light logic: the ground is the light source (cool ink → warm horizon at dawn), cards take its tint (night cards #17253B, morning cards #FBFAF7).
- **Palette family:** night ink #0E1A2E / #17253B, UI text #EEF2F8 / #8B9BB3; morning sky #CCD8E3 → horizon #FFE2B0; morning text #14181F / #5B6573; **one accent: Relay amber #FFB23E** (the read mark, the drafted text caret, the Send button). Saturated pixels stay on the accent and on the dawn horizon.
- **Type:** two families. **Bricolage Grotesque** 800, tabular figures, for the clock and the 3 claim lines (one huge size, 40–62 vh). **Host Grotesk** for all UI (three sizes: 38 / 32 / 28 px at 1×, cropped up 1.6–2.2× by the camera). Grid: 12 columns on 1920, 96 px outer margins (5%), 24 px gutters; UI cards span columns 3–10 or 2–11.
- **Motion vocabulary:** curves — `outExpo` (entries), `inOutCubic` (camera moves and the clock ramp), `inExpo` (exits, faster than entries). Duration tiers — 0.25 / 0.5 / 1.0 s. Cause rules: rows enter only when an email arrives (the clock advances); a row changes only when Relay reads it (amber mark + gist); the camera only moves toward the thing about to change. No float, no drift, no glow pulse.
- **Texture:** one — fine film grain over the whole film (post `grain` 0.02), so holds aren't frozen.
- **Depth:** layered planes (clock behind, cards in front), camera zoom for macro crops; no perspective, never an orbit.
- **CG blacklist:** no 3D at all in this film (count 0): no humans, no phone model, no bed, no sun, no stars, no particles.
- **Banned looks checked:** not indigo→violet (ink blue → warm daylight, no purple); not cream + terracotta (morning is cool sky white + amber); not near-black + acid (the ground is ink blue and becomes daylight); no glowing serif; no sparkle "AI" icon, no orb, no robot; no tiny tracked mono label over a centred headline — the only small text is the product's own UI.
- **What changes per shot:** subject (one row / the storm column / the clock / the brief / one draft / the Send button / the empty inbox), scale (macro 2.2× → the whole column at 0.9×), camera (push, pull, track down the brief).
- **Tests that set this:** `tests/notes.md`. **Style frame chosen:** see the end of this file after step 4.

## Style frame chosen (step 4)
Frame A (daylight ground, white card, amber Send, drafted text fully legible) — see `frames/notes.md`. The first-light variant was muddy; the 2.1× macro cropped the very sentence that proves the product.
