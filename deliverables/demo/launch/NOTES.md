# Railyard, "Arrivals" (b-launch demo rebuild, 15 s, 16:9)

**Idea:** a departure board for money. Every row of waiting money (wire, ACH, payroll, invoice) clatters to DELAYED, QUEUED or HELD. One API call lands its row, then CREATED → PENDING → SETTLED, with a webhook row and a chime at every state change. A green wave runs down the board, which then turns in one flap to RAILYARD / ONE API CALL. / SETTLED INSTANTLY. The button rhymes with frame 1: DELAYS: NONE.

**Build:** `gen.py` simulates a 26×7 Solari board and writes `spec.json`, a single `scene3d` with 548 ray-traced objects. Each cell has a static top half, a static bottom half and a hinged flap that falls under gravity and bounces off its stop. Cells flip through their alphabet in order; there are no glyph swaps. Per-object `=expressions` of t drive each flap's angle and the texture frame of the glyph compositions (3 inks × 46 glyphs, plus a vertically flipped back face).

The rest of the build:
- **Light:** a hooded warm lamp shaft in thin haze, plus a green settle light for the colour arc (amber waiting → white → green).
- **Shots:** seven, with hard cuts on flips (dense linear camera keys, eased per shot).
- **Sound:** the flap simulation drives it (clacks binned per frame and panned), with chimes on webhooks, a riser and impact into the peak, and a wood thump on the one-flap payoff, over a piano/cello/strings bed. Mastered to −14 LUFS.

**Render:** `ANIKIT_RENDER_CACHE=0 anikit-video render spec.json --crf 17 -o final.mp4`. 438 s on 4 cores. `scripts/defects.py` is clean.

## Scores (pinned: `python3 scripts/judge.py brief b-launch final.mp4`; refs coinbase, jpmorgan, xreal; noise ±0.4)

| Run | Score (full) | What changed |
|---|---|---|
| r1 (eval agent) | 3.75 (4.28) | rail diagram |
| r2 (eval agent) | 3.62 (4.11) | switch diagram |
| TASTE craft arm | 4.0 | 2D split-flap board |
| pass 1 | 3.88 (4.22) | 3D ray-traced mechanism, one drifting camera |
| **pass 2 (final.mp4)** | **4.62 (4.94)** | 7 authored shots with cuts; drum runs ≤ 13 flaps (no garbage glyphs); clean end lockup |
| pass 3 | 4.25 (4.78) | shorter copy, framing that fits, lamp shaft in haze, a grazing rim light, a green wave as the peak |
| pass 4 | 4.0 (4.44) | flaps at 0.07 s (the hinge reads); Railyard events land in ONE synchronized flap |

That is a plateau at about 4.2 ± 0.4, so I stopped. Pass 2 is kept as `final.mp4` (`score.json`). Passes 3–4 changed *how* it looks; the jury's verdict on *what* it is didn't move.

## What still holds it back (jury, all passes)
1. **Idea:** "a departure board for a company called Railyard" reads as a literal pun in every trial. The jury asks for the board to break, transform, or open into a physical rail world. That's a concept limit, not an engine limit.
2. **One space:** every frame is the same board. The references (Coinbase, JPMorgan, XREAL) move through several spaces and scales.
3. **Type:** the condensed sans on the flaps reads as a default. The end cards read as centred slides.
4. **Dead air** in the last 4 s (static cards).

## Engine gaps found
- **scene3d `cylinder` `size`** didn't match the docs as I read them: rods meant to be 6 mm came out tens of centimetres thick, so I dropped the hinge axles.
- **No per-instance timeline data:** a board where each cell runs its own state machine needs about 2 KB of nested ternary expressions per object (generated). A `keys`-style per-object track for texture `frame`, or an array-lookup expression, would make it authorable by hand.
- **Composition frames as a glyph atlas:** the frame windows must be centred on frame times (`start = (f − 0.3)/fps`). Windows of `[f/fps + ε, (f + 1)/fps − ε]` silently dropped letters.
- **Sound for mechanisms:** `"sound": "auto"` can't see a scene3d texture/expression mechanism. I had to bin about 3,600 simulated clacks into explicit cues myself.
