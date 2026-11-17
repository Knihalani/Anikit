# Swell — the life of an ocean wave in 15 seconds

Rendered with Anikit Video at 1920×1080, 30 fps: `anikit-video render swell.json -o swell.mp4`.

| Time | Shot | Medium |
|---|---|---|
| 0–2.5 s | Wind isobars drawn on, the gust as one coral line | SVG paths, `repeat`, draw-on |
| 2.5–5 s | Chaos sorts into swell: 56-line ridgeline plot of the sea | custom GLSL (`ridge.glsl`) |
| 4.7–7.5 s | The swell crosses an ocean | built-in `ocean` raymarch shader |
| 7.5–10 s | Under the wave: 80k suspended particles each tracing its real orbital circle, orbits shrinking with depth | ray-traced `scene3d`, `underwater` env, particle force field |
| 10–12.5 s | The shore stands it up: the ridgeline field rears into a crest on the bar‑5 hit | `ridge.glsl` (steep), flash + shake |
| 12–15 s | Title over dusk sea | `ocean` + `film` filter |

Score: generated orchestral cue in D minor at 96 BPM, cuts on the bar grid, riser and boom on the break.
The figures are real deep-water wave physics (L = 1.56·T², c = 1.56·T, breaking at depth ≈ 1.3 × height).
