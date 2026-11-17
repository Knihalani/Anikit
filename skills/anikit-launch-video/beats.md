# Beat sheets (128 BPM, 4/4: beat = 0.46875 s, bar = 1.875 s)

Scene durations are multiples of a beat, every transition is `"cut"` (or a short whip inside the incoming scene),
so every cut lands on the grid of the generated score. `anikit-video check` must report exactly the target length.
Other tempi: 96 BPM (calmer, bar 2.5 s, 15 s = 6 bars) or 140 BPM (trap/hype, 15 s ≈ 8.75 bars → use 8 bars = 13.71 s
plus a 1.29 s end hold). Use `"hits": ["@bar3", ...]` to put the music's impacts on the reveal.

## 15 s (8 bars, 32 beats)

| Beats | Start (s) | Dur (s) | Section | Notes |
|---|---|---|---|---|
| 1–2 | 0.000 | 0.938 | Hook | motion in frame 1; the concept's world, unexplained |
| 3–4 | 0.938 | 0.938 | Tension 1 | the problem as an image + 1–3 words |
| 5–6 | 1.875 | 0.938 | Tension 2 | escalate (faster, closer) |
| 7–8 | 2.813 | 0.938 | Tension 3 / riser | riser sfx peaks at 3.750 |
| 9–12 | 3.750 | 1.875 | **Reveal** (`@bar3`) | impact + music hit; hero full-frame; the turn |
| 13–14 · 15–16 | 5.625 | 2 × 0.938 | Feature 1 | two shots: the feature happening, then a second angle / its result |
| 17–18 · 19–20 | 7.500 | 2 × 0.938 | Feature 2 | whip in; second shot = closer or the payoff |
| 21–22 · 23–24 | 9.375 | 2 × 0.938 | Feature 3 | |
| 25–26 | 11.250 | 0.938 | Proof | one fact, huge (or a demonstration) |
| 27–28 | 12.188 | 0.938 | Logo slam | the motif resolves into the mark |
| 29–32 | 13.125 | 1.875 | CTA + end hold | name/URL, alive (drift, light); music `"end": "ring"` |

Why two shots per feature: `pace.py` measures *cuts* (ffmpeg scene detection). A 1.875 s shot with a change inside
reads as one shot, and two of them in a row fail "median ≤ 1.5 s". Also alternate light and dark frames across
cuts: a dark-to-dark cut can go undetected and merge two shots into one long one.

## 30 s (16 bars)

Hook 2 beats · tension 6 beats (3 cuts) · reveal 2 bars (`@bar3`, a second angle at beat 3 of bar 4) · features
3 × 2 bars (each: the feature, then a cut on bar+1 to its result) · proof 1 bar · breath 2 beats (pre-drop silence:
`mute` or a `riser`) · finale 1 bar (all features in a fast montage, 2 beats each) · logo + CTA 2 bars minus 2 beats.

## 60 s (32 bars)

Two acts. Act 1 (bars 1–12): hook, tension (2 bars), reveal (2 bars), features 1–2 at 2 bars each with a closer look
inside each. Breakdown (bars 13–16): one slow, beautiful hero orbit — the only long hold, still alive (`steady`
inside a `fast` film is fine for ≤ 2.5 s per shot change). Act 2 (bars 17–28): features 3–5, proof (2 bars, a
customer + a number), finale montage. Bars 29–32: logo + CTA with the end hold.

## Seconds for a beat `b` (1-based) at 128 BPM

`start = (b - 1) * 0.46875`. Bars: bar `n` starts at `(n - 1) * 1.875`. `@beatN` / `@barN` in `at` and `hits`
use the same grid, so you can write `"at": "@bar3"` instead of computing it.
