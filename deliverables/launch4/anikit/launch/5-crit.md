# 7–8 · Critiques (3 passes, the maximum)

Blind judge: `scripts/judge.py run … --practice --trials 2 --tags product-launch,generative,kinetic-type` vs Vertigo, Halt and Catch Fire, Da Vinci's Demons (refs mean ≈ 8.2). Results in `crit1/`, `crit2/`, `crit3/`.

| pass | judge overall | rank | judged pro | key full scores |
|---|---|---|---|---|
| 1 | 3.75 | last (2/2) | 0/2 | composition 3, light 2, type 4 |
| 2 | 3.50 | last (2/2) | 0/2 | |
| 3 | 3.44 | last (2/2) | 0/2 | idea 4.5, composition 3, type 3, light 2, motion 4.5, sound 4 |

All three are within 0.75 of each other: **a tie**. Per the method, the fake-look check and the gap count decide → **pass 3 shipped**.

## Fake-look critic (fresh subagent, 6 stills + brief)
**Pass 1:** "Not AI-generated… reads as a Processing/p5.js sketch… Frame 5 is screensaver confetti… Nothing says JSON spec → MP4 until the last frame… Cruelest: *A p5.js fireworks sketch wearing a Helvetica Now press release.* … One change: tie the spec to the picture throughout; let each JSON line trigger its burst; end on two matching renders."
**Pass 2:** "Not AI imagery. Reads as a generative-art demo/screensaver… frame 5 is mud, no focal point… Cruelest: *A JSON pretty-printer dressed up as a fireworks screensaver, for a product that makes videos and never shows you one.* … One change: one JSON line → its one burst; two renders side by side; half the bursts."

## Gaps, sorted
- **IDEA:** "visuals illustrate fireworks, not the product" (critic 1). → *Pass 2 changed the system, not the polish:* the firing script on screen became the real cue objects (`{"at", "shell", "ink", "x", "y", "r", "lift"}`) from frame 0, and each row's trail rises out of its own line into its burst. Still open, by the judge's reading: it wants the JSON itself choreographed at expressive scale.
- **WORLD:** "no luminance / flat light" (judge, all passes; light_texture 2). → Pass 2 added a sky flash and white core at each break and a deeper dusk for the finale. The judge keeps asking for a night sky with bloom and smoke. **Declined:** that is the "glow on black" / screensaver look the skill bans, and a simulated real sky. This is the main reason the judge scores low, and a knowing trade.
- **WORLD:** "near-empty frames / slide pacing" → pass 1 to 2: frame 0 now opens on a breaking shell; the seam fires from 0.15 s; the S2 rows are present from the cut.
- **CRAFT:** finale mud (critic 2) → pass 3 cut the finale from 30 to 18 cues, lowered the flash, and cleared the sky before the name. The "IT" collision in the dot-matrix → narrower I. "Video never comes through" → end claim "One JSON spec in. A finished film out." Float noise in rows (1.6500000001) → rounded. Smart quotes in JSON → `typeset: false`.
- **Cut the weakest third:** the pass-1 "flip" from cue-sheet text to JSON, half the finale cues, the hold's oversized timecode.

## Honest verdict
The judge never rated it professional (0/6 trials). The fake-look critics never called it AI or simulated, and its idea (a firing script that fires the same show every night) is specific to Anikit. Its weakness is light and density: a daylight print world reads thinner than night-sky spectacle. A pass 4 (outside the 3-pass limit) would set the JSON at poster scale and let its values visibly move the bursts.
