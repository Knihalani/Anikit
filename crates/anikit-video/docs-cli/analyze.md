# Analyze: reverse-engineer a reference video

```bash
anikit-video analyze ref.mp4 -o analysis.json --sheet sheet.jpg --spec draft.json
anikit-video analyze frames/ --fps 24      # a directory of PNG/JPG frames (or frame%04d.png); GIF, WebM, MOV work too
```

Needs ffmpeg (as video layers do). One decode pass at 640 px; a 30 s 1080p clip takes a few seconds.
Recreate **structure, pacing, style and techniques**, never the footage, logos, characters or music themselves.

**analysis.json**
- `global`: aspect, fps, duration, shot stats (median, longest, cuts/min), `pace` (the `intent.pace` scale), palette, transition and technique mix, grain/vignette/glow, `style_summary`.
- `shots[]`: start/end, `transition_in`, OKLab `palette` with shares, `luma`/`contrast`/`key`, `grade` (black/mid/white points, warmth, tint, a label like "lifted blacks, soft highlights, warm, desaturated"), `layout` (symmetry, centre of weight), `camera` (pan, zoom, roll, shake, label), `motion` (4/s) and `motion_character` (snap-then-settle, builds, steady, bursts), `depth.dof`, `look` (flat-2d / rendered-3d / photographic / minimal), `text[]` (`height` ×100 ≈ vh, `align`, `position`, `ink` light-on-dark or dark-on-light, `weight`), grain/vignette/glow, `technique` guesses.
- `transitions[]`: cut, crossfade, fadeBlack, fadeWhite, flash, whip, zoom, wipe (+ direction). `flashes`: bright frames that are not cuts.
- `audio`: bpm, beats, downbeats, onsets, `hits` (biggest impacts), loudness (4/s), `cuts_on_beat`.

**sheet.jpg**: one key frame per shot: number, start, length, incoming transition (border colour: white cut, blue crossfade,
yellow flash, red whip, green wipe, purple zoom), camera label, technique guess, text boxes (cyan), palette strip;
a timeline strip at the bottom with beats and bars. **Look at it**: the numbers are heuristics.

**draft.json** (passes `check`) is a timed skeleton, not content: one scene per shot at the measured times and transitions, camera moves, theme tokens, `music` tempo (`fit: false`), `post`, and a `notes` string per scene describing the reference shot. `layers` are empty: look at the key frames and build them (skill below).
Live-action footage can't be rebuilt in a motion-graphics engine: recreate its cutting, framing, grade and type.

| Technique guess | Build it with |
|---|---|
| kinetic-type | `docs animation` (kinetic styles), `recipes kinetic-slam` |
| particles | `docs particles`, `recipes particle-ignite` |
| scene3d / rendered-3d | `docs 3d`, `recipes chrome-hero` |
| shader | `docs shader`, `recipes` (aurora, mesh-gradient, liquid-chrome…) |
| whip / zoom cuts | `docs camera`, `recipes whip-transition` |
| 2d-motion-graphics | `docs primitives`, `docs compositions` |

Loop: analyze → read the sheet → rewrite the draft for your content → `critique --render --pack` →
`analyze` your render too and compare the two sheets and `global` blocks side by side.
Skill: `skills/anikit-reverse-video/SKILL.md`.
