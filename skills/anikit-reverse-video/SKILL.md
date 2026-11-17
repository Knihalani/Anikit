---
name: anikit-reverse-video
description: Reverse-engineer a reference video (MP4, WebM, GIF or a frame sequence) and recreate its structure, pacing, style and techniques as an Anikit Video spec adapted to the user's own content. Use when the user says "make one like this", shares a reference clip, or asks how a video was made.
---

**Rule zero: recreate, don't copy.** Take the *structure, pacing, style and techniques* of the reference: shot rhythm, transitions, camera language, palette mood, type treatment and finish. Never reuse its footage, frames, logos, characters, trademarks, music or copy, and never trace its artwork. Use the user's own product, words and assets. If the user asks for a pixel copy of someone else's work, say no and offer the "in the style of" version. Borrow the *grammar* (pacing curve, framing logic, how type meets image), never the shots: `docs/video/taste/TASTE.md` §8, "References vs copying". If the reference itself shows a trend tell (§9, including the "tasteful AI" look), don't recreate that part.

**What it can and can't recreate:** motion design (type, 2D/vector, UI, shader worlds, rendered 3D, particles) recreates well. Live-action footage (people, real places, camera-original photography) can't be rebuilt in a motion-graphics engine: keep its *cutting, framing, grade and type* and say plainly that the imagery itself will differ (or use the user's own footage as video layers).

**Setup:** `anikit-video` on PATH (see `skills/anikit-video/SKILL.md`), plus `ffmpeg`.

## 1. Measure

```bash
anikit-video analyze ref.mp4 -o ref.json --sheet ref-sheet.jpg --spec draft.json
```

A few seconds (one decode pass at 640 px). `anikit-video docs analyze` explains every field. Per shot you get timing and transition, palette, **grade** (black/mid/white points, warmth, tint, saturation, in words), **layout** (symmetry, centre of weight), camera move, **motion character** (snap-then-settle, builds, steady, bursts), depth of field, texture (grain, glow, vignette), look (flat-2d / rendered-3d / photographic) and **type boxes** (size in vh, alignment, light-on-dark or dark-on-light, thin/regular/bold). Plus tempo, beats and cuts-on-beat.

`draft.json` is a **timed skeleton only**: scenes at the shot times, transitions, camera moves, theme tokens from the palette, music tempo and post finish, and per scene a `notes` string describing the reference shot. Its `layers` are empty on purpose: the tool doesn't invent content.

## 2. Look, and write a style breakdown before you build

The numbers can't tell a serif from a sans or chrome from paper. You must look:

1. Open `ref-sheet.jpg`.
2. Extract every key frame at full size and look at each one:
   `ffmpeg -ss <key_time> -i ref.mp4 -frames:v 1 shotN.png`. Also look at 2–3 frames inside long or busy shots.
3. Write `breakdown.md`, one block per shot (or per group of identical shots), with:
   - **Subject and medium:** what is in frame; 2D vector, UI, paper/print, rendered 3D (which materials), shader abstraction, particles, footage.
   - **Type:** family class (serif / grotesk / geometric / mono / script), case, weight, size as vh, tracking, alignment, line count, where it sits, how it enters (mask, scale-in, typewriter…).
   - **Layout:** grid, margins, symmetry, focal point.
   - **Light and grade:** key-light direction, mood, black and white points, warm or cool, saturation (use `grade.label`).
   - **Motion:** camera move and ease (from `camera` and `motion_character`), object motion, overshoot or snap.
   - **Texture:** grain, glow or bloom, blur, vignette.
   - **Recurring motif:** the one element that carries across shots (a line, a colour, a shape). This is usually what makes the style recognisable. Keep it.
   - Vocabulary and numbers for each line are in `docs/video/taste/TASTE.md`: §1 Typography (scale ratio, tracking, read time), §2 Colour (value structure, palette size), §3 Composition (placement, negative space, lens), §4 Light (key:fill ratio, key direction), §5 Motion (durations, easing, overshoot).

Correct the tool wherever your eyes disagree.

## 3. Map each technique to Anikit

| In the reference | In Anikit (`anikit-video docs <topic>`) |
|---|---|
| Cuts on the beat | `music` bpm from the analysis + scene durations on `@beat`/`@bar` (`docs music`, `docs pacing`) |
| Crossfade, dip, wipe, zoom | scene `transition` |
| Whip, snap zoom, push-in, drift | `camera.animate` with `outExpo` snaps or `inOutSine` drifts, `motionBlur` (`docs camera`) |
| Serif/sans/mono type system, mask reveals | text `font`, `weight`, `letterSpacing`, `enter: "mask by line up"`, kinetic styles (`docs animation`) |
| Flat vector, charts, maps, UI | shapes, paths, `compositions` (`docs primitives`, `docs compositions`) |
| Rendered 3D: terrain, product, type in a world | `scene3d`: heightfields, extruded text, materials, lights, fog (`docs 3d`) |
| Glowing lines, dust, sparks | emissive instances or `particles` (`docs particles`) |
| Abstract animated backgrounds | your own GLSL matched to the palette (`docs shader`), not a random library shader |
| Grade, grain, bloom, vignette | `post` with values from `grade` and texture (`docs post`) |

## 4. Build each shot to match its breakdown, with the user's content

Start from `draft.json` (timing, transitions, camera, palette, tempo). For each scene, read its `notes` and your breakdown, then build layers that match **the same medium, type treatment, layout, light, grade and motif**, with the user's subject and words. Swap the nouns, not the style: if the reference shows a glowing route over dark terrain, the user's version shows a glowing route over *their* world, lit the same way. Keep shot lengths (±10%), transitions and beat alignment. Never import the reference's footage, logos, characters or copy. `anikit-video check s.json` must pass.

## 5. Compare side by side, and iterate until it reads as "same style"

```bash
anikit-video render s.json -o out.mp4 --scale 0.5
anikit-video analyze out.mp4 -o out.json --sheet out-sheet.jpg
# pair frames at the reference's key times
for t in $(jq -r '.shots[].key_time' ref.json); do
  ffmpeg -v error -ss $t -i ref.mp4 -frames:v 1 -vf scale=-2:360 r_$t.png
  ffmpeg -v error -ss $t -i out.mp4 -frames:v 1 -vf scale=-2:360 o_$t.png
done
```

Put each pair next to each other and look. For every pair ask: would a viewer say this is the same designer's work? Check typeface class, weight and size, colour and grade, light direction, motif, and density of detail. Also compare `global` and each shot's `grade`, `layout` and `motion_character` in `ref.json` vs `out.json`. Fix the worst pair, re-render it with `still --at`, and repeat. Check the rhythm against `TASTE.md` §6 (shot-length variation, a breath after the run, cuts on action) and run the taste pass (`anikit-video docs taste`). Then run `critique s.json --render --pack rv/` and `python3 docs/video/quality/pace.py out.mp4`, and check docs/video/quality/BAR.md (crisp readable text, motivated light, no forced 3D).

Tell the user what you took from the reference (structure, timing, type system, grade, motif) and what is theirs.

Worked example: `docs/video/skills/reverse-examples/` (`fernway-to-tideline/` is the good one, with a pair sheet).
