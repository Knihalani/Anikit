---
name: motion-rules
description: Motion-design rules for Anikit Video specs with the numbers practice gives (hook, reading time, pacing, hierarchy, easing, legibility, safe areas, sound, copy), as ranges to choose from, not one house style. `anikit-video critique` checks most of them.
---

# Motion rules

Full canon and sources: `docs/video/taste/TASTE.md` (`anikit-video docs taste` for the pass). Every number below is a range or a floor; where it isn't, the canon gives the reason.

1. **Hook:** something moves and raises a question by 1 s (a tension, a promise, a number, an image). Opening on a logo, "Introducing…" or a fade from black tends to lose people.
2. **Reading time:** hold text ≥ max(0.83 s, chars/15, words × 0.3 s) **after it settles** (≈ `words/3 + 1` s for a line); motion time doesn't count. Single words cut to a beat can read in 0.4–0.6 s. ≤ ~12 words per screen, lines ≤ 42 characters.
3. **Pacing from intent:** fast = cuts every 0.5–1.5 s, none over 2.5 s; steady ≤ 6 s; calm holds long but every hold develops. Whatever the pace, vary shot lengths (a run, then a breath; CV ≳ 0.6), and cut on action or comprehension; save exact beat hits for key moments. Transitions 0.3–0.7 s, hard cuts by default, decorative wipes rarely. End on a held frame or a button.
4. **Hierarchy:** one focal point per moment, entering first. 2–4 type sizes from one ratio (2:1 or 1.618 for statements; 1.25–1.333 only for dense data); adjacent sizes ≥ 1.2× apart; hero ≥ 3:1. The accent colour goes where the meaning is, ≲ 10% of the frame.
5. **Easing with weight:** fast attack, long settle. Snappy moves 150–300 ms (`outCubic`/`outQuart`, Material `(0.2,0,0,1)`); hero moves 0.5–1 s (`outExpo`); exits shorter than entrances (`inCubic`). Linear spatial motion tends to read as mechanical, which is right only when that is the point. Springs: bounce ≤ 0.3 (≤ ~5% overshoot) unless the piece is playful; opacity and colour never overshoot. Overlap parts by 1–3 frames; stagger items 20–80 ms with the total ≲ 0.5 s. Vary entrances by role; fade-and-rise on everything is a tell.
6. **Legibility:** ≥ ~36 px type at 1080p for anything read (comfortable: lines 7–8% of frame height); no thin weights under ~40 px. Contrast ≥ 4.5:1 (3:1 for large), in *luma*: MP4 halves colour detail, so equal-brightness colour pairs smear. Words stay crisp while read: no blur, defocus, motion blur or bloom on them. Tighten display tracking ~−0.02 em; caps and labels +0.05–0.1 em; set a `width` on text that may wrap.
7. **Safe areas:** 16:9 graphics-safe 5% (EBU R95) with 7% sides/9% top-bottom as the comfortable default; 1:1 8% all round; 9:16 8% sides, 12% top, 20% bottom. Bleed off-frame only on purpose.
8. **Brand:** tokens (`$primary`, `$text`, …), not hex. Palette from the brand's real assets, values first. The logo tends to work best at most twice (reveal and end), not in the hook.
9. **Sound:** instruments from the concept (synths only if it's electronic), one planned silence, ≤ 1 riser per 30 s, hits on the frame rather than early, −14 to −16 LUFS at ≤ −1 dBTP.
10. **Copy:** benefits with real numbers; passes the swap test. No filler (seamless, revolutionary, unlock, leverage, "We're excited"). It makes sense with the sound off.
