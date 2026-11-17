---
name: anikit-launch-video
description: Make a product launch video (15/30/60 s, 16:9, 9:16 or 1:1) with anikit-video from a short brief — a hook in the first second, the product as hero in a ray-traced or simulated world, cuts on a generated score's beats, and a CTA — with a fresh visual concept every time and a render-and-look QA loop. Use when asked for a launch, product, announcement, teaser or feature video.
---

# Launch video

**First:** read `skills/anikit-video/SKILL.md` (setup, the loop, primitives, costs). Then `anikit-video docs launch`
(this skill in 30 lines) and, only when you use them, `docs 3d`, `docs particles`, `docs physics`, `docs music`.
Reference files here: `concepts.md` (the concept generator) and `beats.md` (beat sheets in seconds).

## 1. Brief → decisions (ask nothing you can infer)

| Field | If missing |
|---|---|
| product name, one-line value | required; if only a URL/README is given, extract them from it |
| 3–5 features | take them from the value line, screenshots or README; cut to 3 for 15 s |
| screenshots / URLs | build the product's UI as a composition from primitives with **real copy from the brief** |
| brand colours / fonts | derive from the product's noun (`concepts.md` §3) |
| length | 15 s · aspect | 16:9 (9:16 if "Reels/TikTok/Shorts/story", 1:1 if "feed/LinkedIn") |
| proof (a number, a customer, a quote) | use a fact from the brief; if there is none, use a *demonstration* (the product doing it), never a fake stat |

Ask only if the product name or what it does is unknowable.

## 2. Concept (never the same look twice)

Open `concepts.md`. Write noun · verb · before→after, pick one of the twelve worlds, fill its four slots
(palette, type, hero treatment, the turn) and choose **one motif** that appears in every shot and carries the match
cuts. Record them in your notes and in the output folder's `concept.txt`. A concept that could fit any product is wrong.

## 3. Structure (proven; timed to the music)

Fast intent, 128 BPM (beat 0.469 s, bar 1.875 s): 15 s = 8 bars, 30 s = 16, 60 s = 32. `beats.md` has the tables.

1. **Hook (beats 1–2):** motion in frame 1. A strange, beautiful image from the concept's world — not a logo, not a
   fade from black. The viewer must ask "what is that?" within 1 s.
2. **Tension (to `@bar2`+):** the problem as an image, 1–3 words per cut, cuts every 2 beats.
3. **Reveal (`@bar3`, the music hit):** the turn. The product appears as hero (ray-traced, full frame), impact sfx,
   camera snap. This is the peak of the first half.
4. **Three feature beats (1 bar each at 15 s):** each = the feature *shown happening* + ≤ 4 words, as **two 2-beat
   shots** (the action, then a second angle or its result), so the cuts are real cuts.
5. **Proof (2–4 beats):** one fact, big. Or the product doing the thing, faster than words.
6. **Logo + CTA (last 6 beats):** a 2-beat logo slam where the motif resolves, then name + URL/CTA held ≥ 1.5 s,
   still alive (drift, light). Music ends with `"end": "ring"`.

Copy budget: 15 s ≤ 20 words total, 30 s ≤ 40, 60 s ≤ 80. One idea per shot. Product name on screen at most twice.

## 4. Build with the top end (where it serves the story)

- **Hero:** `scene3d` with the UI composition as a texture on a `pane` (`"texture": "#ui"`; add an `aluminium` `box`
  behind it for a device body; `card` is a reflector, invisible to the camera; `phone` is life-size, 16 cm), real DOF,
  chrome/glass/aluminium, env `studio`/`night`/`cyc`; or `product-shot` (a phone on a plinth under window blinds).
- **Chaos→order:** `particles` (`emit: {text}`, `flow`, 200k–400k) or `physics` (piles, dominoes, `timeScale` ramps).
- **Worlds:** a library shader (`anikit-video recipes`) as the set, with type on top.
- **Type lives in the world — never flat type on a solid or gradient fill** (BAR rule 1; it reads as a slide).
  Every word shot uses one of these instead:
  1. **Extruded 3D type** in the concept's set: a `scene3d` `text` object (`font`, `depth`, `bevel`) among the motif's
     objects (falling tiles, terrain, fog), snapped in with `keys` (`rot` + `scale` 1.3 → 1 in 0.22 s) + `camera.shake`;
  2. **Type made of particles** (`particles` `emit: {"text": …}` + `flow`), or emissive type that lights the set;
  3. **Type as a texture** on the motif's objects (a composition of the word on panes, cards or a device screen);
  4. **Type over the product's own UI or map** (the composition full-frame), cut on the beat;
  5. **Type masked by the motif** (a group with `mask`/`clip` by the motif shape, or revealed by a moving light).
  2D kinetic type (`split: chars`, 0.12 s `outExpo` slams) is fine *on top of* a 3D or UI shot, never on an empty fill.
- **Music:** `"music": {"style": "...", "bpm": 128, "key": "...", "hits": ["@bar3", "@bar7"], "end": "ring"}`.
  Scene durations in multiples of 0.469 s so cuts land on beats; `audio` sfx (`whoosh` on whips, `impact` on the
  reveal, `riser` peaking at the reveal, `click`/`tick` on UI changes). Pulse with `"=1 + 0.04*(1 - fract(beat))"`.
- **Energy:** whip pans (`camera` x ±1500 → 0 in 0.22 s `outExpo`), snap zooms, `motionBlur` on fast shots only.
- **9:16:** stack vertically, type 12–16 vw, keep text inside the middle 80%, the hero fills the height.
- Start from `anikit-video recipes launch-sting` for syntax; compose fresh — never ship the recipe's look.

Gotchas (each cost an iteration while proving this skill):
- A `path` layer sits at its bounding box: set `x`/`y` to the path's top-left, or it piles up at (0, 0).
- `text` does not evaluate `=expr`; build a counter from a few timed layers (`start`/`end`).
- In `scene3d`, an expression `scale` must be a vector: `"=[s, s, s]"`. `0` hides an instance (use it to "draw" a
  trail of beads: `"=[(i/n < t*0.5 ? 1 : 0), …]"`).
- A dark terrain under `night` + dark fog renders black: add a low key light (`intensity` 10–16) and a lighter base.
- Keep a camera above any `heightfield`; inside it the frame turns flat white.
- The renderer has no bitrate flag: a 15 s 1080p film is ~25–45 MB. For a repo or a chat attachment, re-encode
  (`ffmpeg -i in.mp4 -c:v libx264 -crf 24 -preset slow -c:a aac out.mp4`) and say so.

Costs: 3D/particles 0.3–1.5 s a frame → `"quality": "draft"`, `"resolution": 0.5` on scene3d while iterating, keep
each 3D shot 1–3 s. 2D shots are nearly free; use many.

## 5. QA loop (must pass before the final render)

```bash
anikit-video check s.json                                  # errors + real length (must equal the target)
anikit-video still s.json --at 0.3 --width 640 -o hook.png # the hook frame; then one still per scene
anikit-video render s.json --quality draft --scale 0.5 -o draft.mp4
python3 docs/video/quality/pace.py draft.mp4 --strip strip.png   # PACE: PASS required
anikit-video critique s.json --render --pack rv/           # open rv/review.md and LOOK at every image
anikit-video render s.json -o launch.mp4                   # final, default quality
```

Then judge the frames against `docs/video/quality/BAR.md` (all nine FAIL rules) and answer in your notes:
1. Is frame 0.3 s intriguing with no context? 2. Can you name the concept and motif from the strip alone?
3. Is every shot a designed image with real light/material (no centred headline on a gradient, no placeholder UI)?
4. Do cuts and hits land on the music? 5. Would the user's brand team ship it?
Fix the worst frame first; two iterations is normal. Report wall time, render time and pace numbers honestly.
