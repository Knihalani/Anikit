---
name: anikit-launch-video
description: Make a product launch, announcement or teaser video with anikit-video from a short brief. Principles, process and craft advice, not a template - every film gets a structure, tempo and look designed from the product's own story - with a fast look-and-fix loop. Use when asked for a launch, product, announcement, teaser or feature video.
---

# Launch video

Read `skills/anikit-video/SKILL.md` first (setup, the loop, costs). Then `anikit-video docs taste` (canon:
`docs/video/taste/TASTE.md`).
Here: `concepts.md` (finding the idea), `beats.md` (timing to music). **There is no template.** If the film you are
planning would fit any product, rethink it.

## Principles
1. **Earn the first second:** frame 1 moves and raises a question; it is not a logo or a fade from black.
2. **The product, or its effect, is the hero:** the real thing working, with real copy from the brief.
3. **One idea carried through:** a motif or device that pays off at the end.
4. **Show, don't claim:** demonstrations over adjectives; few, specific words.
5. **The ending resolves the idea,** and the logo owns the frame.
6. **Sound and picture move together** (calm is fine).
7. **Honesty:** no invented stats, customers or features.

## Process
1. **Brief:** ask nothing you can infer (features, audience, brand, length, aspect: 9:16 for Reels/TikTok, 1:1 for feed).
2. **Idea** (`concepts.md`): the product's nouns and verbs, before → after, one motif. Gather 3 references from
   outside tech and SaaS (print, title design, illustration, posters, documentary, architecture), sketch 3 directions
   that differ on every axis, pick one and write `concept.txt`. Look at what earlier films used and go somewhere new.
3. **Shape:** invent the structure this product needs. Inspiration only: one impossible camera move · a reverse
   reveal · one object transforming · a wordless demo · type as the set · a loop that returns changed · a time-lapse
   · a scale journey.
4. **Tempo and music** from the brief and the idea (`docs music`, `beats.md`); declare `"intent"`.
5. **Build, then look fast:** `anikit-video preview s.json` after every change (it validates, writes a sheet with
   one frame per second and prints the critique and pace); zoom in with `preview s.json --at T -o f.png`.
6. **Before shipping:** `critique s.json --render --pack rv/` (look at every image), the taste pass (TASTE §10), then
   the final `render` at full quality. Look at frames from the MP4 at 100%.

## What works, what to avoid, why (advice, not bans; depth in TASTE.md §1–§11)
- **Medium:** pick the one the idea lives in: 3D for physical ideas, type for word ideas, simulation for data. Forced
  3D (Sift) and flat headlines on fills both read as templates (TASTE §1, §8).
- **Freshness:** rejected looks so far are tile storms and dominoes on a cyc with chrome type (Sift), fogged terrain
  (Fernway), and cream paper with an editorial serif and aphorisms (Margin, "AI slop"). If you can name the trend a
  frame resembles (screenshot beside copy, UI on dark with one accent …), rethink it (TASTE §9).
- **Density:** rich frames of made imagery (subject, texture, secondary motion, hierarchy, light) beat empty
  restraint (TASTE §3, §11).
- **Copy:** the brief's real nouns, numbers and actions; aphorisms that fit any brand are a tell.
- **Type:** full resolution (2D, or `scene3d` `"resolution": 1` without `draft` in finals), in focus, with no blur,
  bloom or grain on words being read, about 4.5:1 contrast, phone-legible sizes, and held about 0.3 s + 0.06 s per
  character (TASTE §1).
- **Light:** a motivated key, fill and rim; check stills at 100% for clipped whites and crushed blacks (TASTE §4).
- **Music:** from the brand and the idea. Acoustic or organic (`piano`, `acoustic`, `orchestral`, `cinematic`,
  `jazz`, `rock`, `pop`, or `tracks`) suits most products; synth styles suit electronic ideas (TASTE §7).

## Capabilities (where they serve the idea)
UI compositions as textures on panes or devices, and `product-shot` · ray-traced `scene3d` (PBR, DOF, fog, custom
geometry, instancing) · `physics` and `particles` · shader worlds (`recipes`) · 2D type with `split`, masks and
grids, or 3D/particle type · generated scores, `hits`, `@beatN`, `beat`/`level` expressions, sfx · keyed cameras,
whips, rack focus, motion blur.

## Gotchas
- A `path` layer sits at its bounding box: set `x`/`y` to the path's top-left.
- `scene3d`: an expression `scale` is `"=[s, s, s]"`; `card` is a reflector (invisible); `phone` is 16 cm; keep the
  camera above a `heightfield`.
- Sift's type looked low-res because finals kept `"resolution": 0.5` + `draft`, words sat in DOF under bloom and
  grain, and the file was re-encoded at CRF 22. Use `render --crf 18` or lower for type-heavy films.
- `pace.py` can miss cuts between shots of similar brightness. Examples (all rejected, for reference):
  `docs/video/skills/launch-examples/`.
