---
name: anikit-launch-video
description: Make a product launch, announcement or teaser video with anikit-video from a short brief. Principles, process and craft advice, not a template - every film gets a structure, tempo and look designed from the product's own story - with a fast look-and-fix loop. Use when asked for a launch, product, announcement, teaser or feature video.
---

# Launch video

This adds launch-specific advice to `skills/anikit-video/SKILL.md`; follow that skill's process, loop, ship gate and single final render. Here: `concepts.md` (finding the idea) and `beats.md` (timing to music). **There is no template:** if the film you're planning would fit any product, rethink it.

## Principles
1. **Earn the first second:** frame 1 moves and raises a question. It isn't a logo or a fade from black.
2. **The product, or its effect, is the hero:** the real thing working, with real copy from the brief.
3. **One idea carried through:** a motif or device that pays off at the end.
4. **Show, don't claim:** demonstrations over adjectives; few, specific words.
5. **The ending resolves the idea,** and the logo owns the frame.
6. **Honesty:** no invented stats, customers or features.

## Finding the film
- **Brief:** ask nothing you can infer (features, audience, brand, length; 9:16 for Reels/TikTok, 1:1 for feed).
- **Idea** (`concepts.md`): the product's nouns and verbs, before → after, one motif. Look at references from outside tech and SaaS (print, title design, illustration, posters, documentary, architecture). Sketch 3 directions that differ on every axis, pick one, and write `concept.txt`.
- **Shape:** invent the structure this product needs. For inspiration only: one impossible camera move · a reverse reveal · one object transforming · a wordless demo · type as the set · a loop that returns changed · a time-lapse · a scale journey.
- **Tempo and music** come from the brief and the idea (`beats.md`, `anikit-video docs music`). Acoustic and organic styles suit most products; synth styles suit electronic ideas.

## What tends to work, what tends not to (advice; depth in `docs/video/taste/TASTE.md`)
- **Medium:** the one the idea lives in. 3D for physical ideas, type for word ideas, simulation for data. Forced 3D and flat headlines on fills both read as templates.
- **Freshness:** the rejected looks so far are tile storms and dominoes on a cyc with chrome type; fogged terrain; cream paper with an editorial serif and aphorisms; and landing-page furniture (numbered eyebrows, pill labels, LIVE dots). If you can name the trend a frame resembles, rethink it.
- **Density:** rich frames of made imagery (subject, texture, secondary motion, light) beat empty restraint.
- **Copy:** the brief's real nouns, numbers and actions. Aphorisms that fit any brand are a tell.
- **UI:** the product's real screens as compositions, textures on devices or panes, or `product-shot`, not a screenshot beside copy.

## Gotchas
- A `path` layer sits at its bounding box: set `x`/`y` to the path's top-left.
- `scene3d`: an expression `scale` is `"=[s, s, s]"`; `card` is a reflector (invisible); `phone` is 16 cm; keep the camera above a `heightfield`.
- Type-heavy films: render with `--crf 18` or lower. Past rejected examples are in `docs/video/skills/launch-examples/`.
