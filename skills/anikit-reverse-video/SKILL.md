---
name: anikit-reverse-video
description: Reverse-engineer a reference video (MP4, WebM, GIF or a frame sequence) and recreate its structure, pacing, style and techniques as an Anikit Video spec adapted to the user's own content. Use when the user says "make one like this", shares a reference clip, or asks how a video was made.
---

**Rule zero: recreate, don't copy.** Take the *structure, pacing, style and techniques* of the reference: shot rhythm, transitions, camera language, palette mood, type treatment and finish. Never reuse its footage, frames, logos, characters, trademarks, music or copy, and never trace its artwork. Use the user's own product, words and assets. If the user asks for a pixel copy of someone else's work, say no and offer the "in the style of" version.

**Setup:** `anikit-video` on PATH (see `skills/anikit-video/SKILL.md`), plus `ffmpeg`.

## 1. Measure

```bash
anikit-video analyze ref.mp4 -o ref.json --sheet ref-sheet.jpg --spec draft.json
```

Takes a few seconds (one decode pass at 640 px). Read `global.style_summary` first, then `global` (pace, median and longest shot, transition mix, palette, grain, vignette, glow) and `audio` (bpm, `cuts_on_beat`, `hits`). `anikit-video docs analyze` explains every field.

## 2. Look (the numbers are heuristics; your eyes decide)

Open `ref-sheet.jpg`. Each cell is one shot: number, start, length, incoming transition (the border colour), camera label, technique guess, text boxes (cyan) and palette. The bottom strip is the timeline with beats and bars. For any shot you can't read from the thumbnail, extract its key frame: `ffmpeg -ss <key_time> -i ref.mp4 -frames:v 1 k.png`, and look at it.

Then write a short breakdown, one line per shot or group of shots:
- **What it is and how it was made:** 3D render, live footage, flat vector, kinetic type, particles, shader backdrop, UI capture.
- **Camera and motion:** push in, whip, orbit, speed ramp, parallax, shake.
- **Cut and timing:** cut on beat, hold length, where the energy peaks (loudness and `motion` curves).
- **Finish:** grade (key and contrast), grain, bloom, vignette, depth of field, motion blur.

Correct the tool when your eyes disagree. `look` and `technique` are guesses; a dark photo can read as `rendered-3d`, and busy textures can look like text.

## 3. Map each technique to Anikit

| In the reference | In Anikit (`anikit-video docs <topic>`) |
|---|---|
| Hard cuts on the beat | `"music": {"bpm": …}` + scene `duration: "@bar1"` or `"@beat4"` (`docs music`, `docs pacing`) |
| Crossfade, dip to black or white, wipe, zoom | scene `transition` (`crossfade`, `fadeBlack`, `fadeWhite`, `wipe`, `zoom`) |
| Whip pan | `recipes whip-transition`, `motionBlur` (`docs camera`) |
| Push-in or drift | scene `camera.animate` zoom/x/y; parallax with layer `depth` |
| Kinetic headline | text `enter` kinetic styles (`docs animation`), `recipes kinetic-slam` |
| Chrome, glass or product 3D | `scene3d` (`docs 3d`, `recipes chrome-hero`) |
| Sparks, dust, particle logo | `docs particles`, `recipes particle-ignite` |
| Abstract animated backdrop | library shader (`anikit-video recipes`: aurora, mesh-gradient, liquid-chrome…) or your own GLSL (`docs shader`) |
| Grain, bloom, vignette, grade | `post` (`docs post`) |
| Live footage | the user's own clips as video layers, or rebuild the idea in 3D or 2D. Never the reference's footage. |

## 4. Recreate for the user's content

Start from `draft.json`. It already has one scene per shot at the measured times, the transitions, theme tokens from the palette, the camera moves, placeholder text in the detected boxes, music at the detected tempo, `post` and a starting layer per technique. Then:
1. Replace every placeholder with the user's nouns, product and copy (1–4 words per shot).
2. Upgrade each shot's starting layer to what step 2 says it really is.
3. Keep the rhythm: same shot count and lengths (±10%), the same transition types and the same beat alignment. Change the music style if the user wants, but keep the tempo band.
4. Keep the palette *roles* (dark or light base, one accent), shifted to the user's brand colours if they have them.

`anikit-video check s.json` must pass.

## 5. Verify side by side

```bash
anikit-video critique s.json --render --pack rv/            # look at rv/ (strip, cut frames, review.md)
python3 docs/video/quality/pace.py rv/render.mp4            # or your render's path
anikit-video analyze out.mp4 -o out.json --sheet out-sheet.jpg
```

Compare `ref-sheet.jpg` with `out-sheet.jpg`, and `ref.json` `global` with `out.json` `global`: shot count, median and longest shot, pace, transition mix, palette roles, `cuts_on_beat`. Fix the largest gap and repeat. Then tell the user what you took from the reference (structure, timing and techniques) and what is theirs.

Worked examples with sheets and renders: `docs/video/skills/reverse-examples/`.
