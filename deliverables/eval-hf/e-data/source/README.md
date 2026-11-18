# Arctic sea ice: the September minimum, 1979–2026

A 30 s, 16:9 HyperFrames film (`film/`, rendered with hyperframes 0.9.64).

- **Data:** NSIDC Sea Ice Index, Version 4 (G02135), `N_09_extent_v4.0.csv`. A copy is in `data/`. All 48 Septembers are used as published. The 1979–88 and 2017–26 means are 7.21 and 4.61 million km², a 36% drop and 2.6 million km² less ice. Greenland covers about 2.17 million km².
- **Map:** Natural Earth 50m (from the `world-atlas` npm package). It uses a Lambert azimuthal equal-area projection centred on the North Pole, at 0.19 px per km. `build/map.mjs` bakes it.
- **Ice:** an organic shape whose area equals each year's extent at the map's scale. Its outline is illustrative, and the end card says so.
- **Sound:** `build/score.py` synthesises the score from the same clock the picture uses: a cold pad, wind, a tick for each year, an ice crack for the 2012 record, and a riser and impact for the loss. It is levelled to −16 LUFS with a true peak of −2.5 dBTP. The levels were measured; nobody listened to the mix.

Build: `node build/map.mjs && node build/assemble.mjs` (writes `film/compositions/map.html`), `python3 build/score.py raw.wav` → gain → `film/assets/score.wav`, then `hyperframes render` in `film/`.

Warnings kept on purpose: the source lines are set at 22–26 px, under the 28 px floor; they are references, not reading text. The full source credit on the end card is held for about 3 s.
