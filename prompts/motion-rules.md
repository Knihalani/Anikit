---
name: motion-rules
description: The motion-design rules every Anikit Video prompt follows (timing, hierarchy, easing, reading time, safe areas). Include it in any system prompt that writes or reviews a spec.
---

# Motion-design rules for Anikit Video

These are rules, not suggestions. Each has a number so it can be checked, and `anikit-video critique` checks most of them automatically.

## 1. Attention: the hook

- **On screen by 1 s, understood by 2–3 s.** The first scene carries a hook line of **3–7 words**: a tension ("Your renders take 3 minutes."), a promise ("Video at the speed of JSON.") or a surprising fact ("935 frames per second. On a laptop."). Never open on a logo, on "Introducing…", or on a slow fade from black.
- Motion starts on frame 1. Lead with the hook word itself: animate it in with `fadeUp`/`popIn` at delay 0–0.2 s.
- The product name usually comes second, as the answer to the hook.

## 2. Reading time

- A viewer reads about **3 words per second**, plus **1 s** to notice the text: `hold ≥ words / 3 + 1` seconds, and never less than 1.5 s. The template engine's `readtime(text)` computes exactly this.
- Count from when the text is **readable**: after its entrance has mostly played (the delay, plus half the duration, plus the whole stagger), not from when it starts animating in.
- **One idea per screen.** About 12 words is comfortable, 20 the maximum; 30 or more is a paragraph, which nobody reads in a video.
- Headlines: 2–6 words. Supporting lines: at most 12–16 words. Labels and kickers: 1–3 words.

## 3. Pacing

- A beat (one idea) lasts **2–5 s**. Under 1.2 s doesn't register, except single kinetic words on a beat in a teaser; over 8 s without motion feels stalled.
- Video length by format: a social teaser is 6–15 s; a feature announcement 12–25 s; a launch film 20–45 s; a demo 30–90 s.
- Vary the rhythm: a quick hook, a breath on the reveal, an even cadence through the features, then a held end card of **at least 3 s**, so the call to action can be read and acted on.
- Transitions take **0.3–0.7 s**, and never more than a third of a scene. Use one transition family per video, plus at most one "special" one, such as `zoom` into the reveal or `blur` into the end card.

## 4. Hierarchy

- **One focal point per moment.** The most important element is the biggest, the brightest and the first to move; secondary elements follow it 0.15–0.5 s later.
- **Three type levels at most per screen:** kicker or label (small, uppercase, accent colour), headline (big, tight tracking), support (muted, about 40% of the headline's size).
- **Accent colour on one thing only**: a key word, a number or a button. When everything is highlighted, nothing is.

## 5. Easing and motion

| Motion | Ease | Duration |
|---|---|---|
| Entrances | `outExpo` (snappy, premium) or `outCubic` (calm) | 0.5–0.8 s |
| Exits | `inCubic` (or a transition), faster than entrances | 0.3–0.5 s |
| Moves between two states | `inOutCubic` / `inOutQuart` | 0.6–1 s |
| Playful pops (icons, badges, buttons) | `outBack`, spring `snappy` | |
| Camera drift | `inOutSine`, zoom 1 → 1.03–1.08 over the whole scene | |

- **Never `linear`** for spatial motion; it's only for typewriters, progress and loops.
- Staggers are **0.04–0.12 s** per item: 0.03 for characters, 0.08 for words, 0.12 for cards. A whole staggered group should finish within 1 s.
- Movement distances are small: 3–6vmin for fades and slides. Large moves read as transitions, not as entrances.
- Prefer one coherent motion direction per video (for example, everything rises).
- Something should always be moving a little, like a slow camera push or a floating glow, but only one thing should move fast at a time.

## 6. Legibility and contrast

- Minimum text size is **2.2vmin** (about 24 px at 1080p). Body copy is 3vmin or more; on 9:16, 4vmin or more, because phones shrink everything.
- Contrast: **4.5:1** for body text and **3:1** for large text (4vmin or more) against what is actually behind it. Muted text on a gradient is the usual failure, so check the darkest and the lightest point.
- Tight tracking (`-0.02` to `-0.035em`) on big display type; open tracking (`0.08–0.14em`) on small uppercase labels.
- Set a `width` on every text that may wrap, so lines break where you choose (a 45–60 % width for supporting text in 16:9).

## 7. Safe areas and formats

- **16:9:** keep text 7% from the sides and 9% from the top and bottom.
- **1:1:** keep text 8% from every edge.
- **9:16:** keep text 8% from the sides, **12% from the top and 20% from the bottom**, where platform UI (captions, buttons, the progress bar) covers the video.
- Design each aspect deliberately: rows become columns on 9:16; screenshots go full-width under the text; type is about 10% larger relative to the short side.

## 8. Brand

- Use the brand kit's tokens (`$primary`, `$accent`, `$bg`, `$text`, `$muted`, `$surface`) and fonts (`$heading`, `$body`, `$mono`), never hard-coded colours.
- The logo appears **once or twice**: at the reveal and on the end card. It is never in the hook.
- Respect `voice.avoid`, and use `voice.cta` on the end card unless the brief says otherwise.

## 9. Copy

- Benefits over features ("Renders in seconds", not "Uses a SIMD rasteriser"); concrete numbers over adjectives ("10× faster", not "blazing fast").
- No filler ("Introducing", "We're excited", "revolutionary", "seamless", "next-generation", "unlock", "leverage").
- Sentence case for headlines; a full stop is allowed on short punchy lines ("JSON in. MP4 out.").
- Each scene's copy must make sense **with the sound off**.
