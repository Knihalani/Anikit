---
name: motion-rules
description: Motion-design rules for Anikit Video specs (hook, reading time, pacing, hierarchy, easing, legibility, safe areas, copy). `anikit-video critique` checks most of them.
---

# Motion rules

1. **Hook:** 3–7 words on screen by 1 s (a tension, a promise or a number). Never open on a logo, "Introducing…" or a fade from black.
2. **Reading time:** hold text for at least `words/3 + 1` s (minimum 1.5 s), counted from when it settles. Aim for about 12 words per screen, 20 at most. Headlines are 2–6 words; support lines are 14 words or fewer.
3. **Pacing:** each beat lasts 2–5 s. Transitions take 0.3–0.7 s (no more than ⅓ of the scene), from one family per video plus one special. Hold the end card for at least 3 s. Lengths: teaser 6–15 s, feature 12–25 s, launch 20–45 s.
4. **Hierarchy:** one focal point per moment, entering first and largest. At most 3 type levels (label, headline, support at about 40% size). The accent colour goes on one element only.
5. **Easing:** entrances use `outExpo` or `outCubic` over 0.5–0.8 s; exits are faster (`inCubic`). Moves use `inOutCubic`, pops `outBack`, and camera drift `inOutSine` (zoom 1 → 1.05). Spatial motion is never `linear`. Staggers: chars 0.03, words 0.08, cards 0.12, finishing within 1 s. Keep distances at 3–6vmin.
6. **Legibility:** text is at least 2.2vmin (body 3vmin or more; 4vmin or more on 9:16). Contrast is at least 4.5:1, or 3:1 for large text, against what is actually behind it. Give display type tight tracking (−0.02 to −0.035em) and labels wide tracking (0.12em). Set a `width` on any text that may wrap.
7. **Safe areas:** 16:9 margins are 7% at the sides and 9% top and bottom. 1:1 is 8% all round. 9:16 is 8% at the sides, 12% at the top and 20% at the bottom.
8. **Brand:** use tokens (`$primary`, `$text`, `$muted`, …), not hex values. Show the logo at most twice (reveal and end card), never in the hook.
9. **Copy:** benefits with real numbers. No filler (seamless, revolutionary, unlock, leverage, "We're excited"). It must make sense with the sound off.
