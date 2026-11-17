# Analyze: reverse-engineer a reference video

```bash
anikit-video analyze ref.mp4 -o analysis.json --sheet sheet.jpg --spec draft.json
anikit-video analyze frames/ --fps 24      # a directory of PNG/JPG frames (or frame%04d.png); GIF, WebM, MOV work too
```

Needs ffmpeg (as video layers do). One decode pass at 640 px; a 30 s 1080p clip takes a few seconds.
Recreate **structure, pacing, style and techniques**, never the footage, logos, characters or music themselves.

**analysis.json**
- `global`: aspect, fps, duration, shot stats (median, longest, cuts/min), `pace` (the `intent.pace` scale), palette, transition and technique mix, grain/vignette/glow, `style_summary`.
- `shots[]`: start/end, `transition_in`, OKLab `palette` with shares, `luma`/`contrast`/`key`, `camera` (pan in frame widths, zoom, roll, shake, a label like `push in + pan left`), `motion` (4/s), `depth.dof` (shallow/deep/soft), `look` (flat-2d / rendered-3d / photographic), `text[]` boxes (`height` ×100 ≈ the vh size, `align`, `position`), `points` (particles/sparks), `technique` (best guess first: particles, kinetic-type, shader, scene3d, 2d-motion-graphics, footage-or-scene3d).
- `transitions[]`: cut, crossfade, fadeBlack, fadeWhite, flash, whip, zoom, wipe (+ direction). `flashes`: bright frames that are not cuts.
- `audio`: bpm, beats, downbeats, onsets, `hits` (biggest impacts), loudness (4/s), `cuts_on_beat`.

**sheet.jpg**: one key frame per shot: number, start, length, incoming transition (border colour: white cut, blue crossfade,
yellow flash, red whip, green wipe, purple zoom), camera label, technique guess, text boxes (cyan), palette strip;
a timeline strip at the bottom with beats and bars. **Look at it**: the numbers are heuristics.

**draft.json** (passes `check`): one scene per shot at the measured times and transitions, theme tokens from the palette,
camera moves, placeholder text in the detected boxes, `music` at the detected tempo with `hits`, `post` grain/vignette/bloom,
and a starting layer per technique (the `particle-ignite` / `chrome-hero` recipes, a library shader, a shape).

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
