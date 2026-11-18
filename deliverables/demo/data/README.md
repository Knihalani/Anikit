# e-data rebuild: Arctic sea ice, September minimum 1979–2026

Brief: a 30 s, 16:9 piece on the September minimum of Arctic sea ice, using NSIDC Sea Ice Index v4 (`N_09_extent_v4.0.csv`, in this folder).

## Scores

All scores come from `python3 scripts/judge.py brief e-data final.mp4`, against the three references pinned for r1 and r2.

| Version | Score | Full score | Judged pro | Trials |
|---|---|---|---|---|
| r1 | 3.62 | 3.83 | 0/3 | 3 |
| r2 | 3.38 | 3.67 | 0/3 | 3 |
| v1: ice shader, cover equals extent | 4.38 | 4.50 | 0/3 | 3 |
| v2 | 4.62 | 4.94 | 0/3 | 3 |
| v3 | 4.50 | 4.72 | 0/3 | 3 |
| v4: painted stencil years drown in the water | 5.25 | 5.39 | 1/3 | 3 |
| v7: real Greenland, calibration re-measured | 5.25 | 5.33 | 0/3 | 3 |
| v8 | 5.00 | 5.11 | 0/3 | 3 |
| v9 | 5.25 | 5.17 | 0/3 | 3 |
| **v9, decision run (this file)** | **5.38** | **5.44** | 0/6 | 6 |
| v4, decision run | 5.00 | 5.19 | 0/6 | 6 |

The scores plateaued from v4 on. `score.json` is the 6-trial jury and critic for v9; `score-3trials.json` is the 3-trial run.

## The device

The ice is the chart. `ice.glsl` is a pack-ice shader seen from straight above:

- floes of mixed sizes, with wind-blown snow and melt ponds;
- a translucent rim and a wet margin;
- a submerged ice foot, and shadows cast on the water;
- a sun that sinks through the film.

**Calibration.** In the melt shot, the white share of the frame equals the year's extent divided by the 9 million km² of ocean the frame stands for. The calibration was measured from rendered pixels (`CAL` in `build.py`, `CF` in the shader), and the error is within about 0.02.

**Painted years.** Each year is painted on the floes in survey-orange stencil (Big Shoulders Stencil, `multiply` plus `inkTexture`). A water-only pass of the same shader goes on top (`hl = -2`, `alpha: true`), so open water cuts through the paint. As the ice retreats, the year drowns.

## The shots

1. **1979** at full pack.
2. **1980–2011**: the melt front rises, one year every 0.24 s, and eats the painted numeral.
3. **2012**: the record low, 3.57, painted across a cracked pack. Camera shake on the cut.
4. **48 Septembers**: the pack cut into 48 bars. 2012 is painted orange, with the 1979 level marked and brash adrift below.
5. **Dusk**: the frame is 18 million km², with the 2026 ice on the left.
   - A floe shaped like Greenland, at true scale, melts away. The coastline is Natural Earth 1:50m in Lambert equal-area (`greenland.json`), 2.15 million km² as projected.
   - That leaves an orange coastline beside the line "Missing since 1979: a Greenland." The 2.24 million km² lost compares with Greenland's 2.17.

The light goes from high white day to low amber sun to blue twilight. Sound is wind ambience, with crunch and shatter on the cuts and an ambient bed.

## Build

```
python3 build.py      # writes spec.json from the CSV (reproduces this spec.json byte for byte)
ANIKIT_RENDER_CACHE=0 anikit-video render spec.json -o final.mp4
```

`scripts/defects.py final.mp4` reports OK.

## What the jury still asks for

- Photographic-grade ice material: refraction, depth and wetness.
- A fuller typographic system beyond the stencil numerals.
- A more staged chart beat.
