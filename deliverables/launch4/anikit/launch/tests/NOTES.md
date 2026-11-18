# 3b · Play (technique tests)

- **t1, script layer + expressions (shells.rhai):** hanabi-catalogue shells (peony, pistil, chrysanthemum, ring, willow) as rule-driven dot figures. *Surprised:* a 60-line Rhai script gives a whole vocabulary of shells, all on one physics rule (`1-exp(-6.5t)` spread, `30t²` fall). *Stock risk:* perfectly even rings read as a p5.js sketch, so pass 2 added angle, radius and fade jitter.
- **t2, print treatments (`inkTexture`, `riso`, `halftone` on a group):** new to me. `riso` re-inks the colours and halftone is too fine at this scale. Script layers wrapped in a group also lose their frame coordinates, so I dropped groups altogether (this is also why the pass-1 seam broke). *Artificial?* No, just wrong for this world.
- **Sound (`sound.cues` from the cue rows, orchestral `music` with `sections`/`stop`/`hits`):** booms and mortar thumps generated from the same rows as the picture. The hard stop before the finale is a 4-beat `stop`.
- **Light (blurred flash + core at each break):** added in pass 2 after both critics found "no luminance". It stays daylight, never glow on black.

Capabilities used: script/draw layers + expressions, print filters (rejected), generated music + sound cues, post grain.
