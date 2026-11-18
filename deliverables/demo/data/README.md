# e-data rebuild: Arctic sea ice, September minimum 1979–2026

Brief: a 30 s, 16:9 piece on the September minimum of Arctic sea ice, using NSIDC Sea Ice Index v4 (`N_09_extent_v4.0.csv`, in this folder).

**`final.mp4` is v11.** `v9.mp4` is kept for the record. v9 scored a little higher, but its captions contradicted each other: the opening caption said the frame holds 9 million km², and the ending said 18. v11 removes both statements, because a data piece must not carry a factual contradiction. The score difference is inside the judge's noise of about ±0.4.

## Scores

All scores come from `python3 scripts/judge.py brief e-data final.mp4`, against the three references pinned for r1 and r2.

| Version | Score | Full score | Judged pro | Trials |
|---|---|---|---|---|
| r1 | 3.62 | 3.83 | 0/3 | 3 |
| r2 | 3.38 | 3.67 | 0/3 | 3 |
| v1: ice shader, cover equals extent | 4.38 | 4.50 | 0/3 | 3 |
| v4: painted stencil years drown in the water | 5.25 | 5.39 | 1/3 | 3 |
| v4, decision runs (mine / JUDGE's) | 5.00 / 4.88 | 5.19 / 5.25 | 0/6, 0/6 | 6 + 6 |
| v9: real Greenland at true scale (`v9.mp4`) | 5.38 / 5.44 | 5.44 / 5.53 | 0/6, 1/6 | 6 + 6 |
| v10: scale change stated on screen | 4.75 | 5.06 | 0/6 | 6 |
| **v11: no frame-scale statements (`final.mp4`)** | **5.19** | **5.31** | 0/6 | 6 |

`score.json` is the v11 jury and critic; `score-v9.json` is v9's.

## Every number on screen, checked

I checked these by script against `N_09_extent_v4.0.csv`, using the extent column in million km² for September of each year.

| On screen | Value | Source |
|---|---|---|
| September 1979 | 7.05 | CSV row 1979 (NSIDC-0051) |
| Melt-shot captions, 1980–2011 | each year's value | CSV rows 1980–2011; all 32 match |
| Record low, September 2012 | 3.57 | CSV row 2012, the minimum of all 48 rows |
| "About half of 1979" | 3.57 ÷ 7.05 = 0.506 | CSV |
| Latest year, September 2026 | 4.81 | CSV row 2026 (NSIDC-0803; NSIDC moved 2025–26 to the successor dataset) |
| Lost since 1979 | 7.05 − 4.81 = 2.24 | CSV |
| 48 bars, "48 Septembers" | 1979–2026 | The shader's `D[48]` array equals the CSV extent column |
| Greenland | 2.17 million km² | 2,166,086 km², the standard figure for Greenland's total area. The drawn shape is the Natural Earth 1:50m outline in Lambert azimuthal equal-area; its generalised coastline measures 2.15. |

The source lines on screen are "Data: NSIDC Sea Ice Index v4. Coastline: Natural Earth." The data file is NOAA/NSIDC G02135, `north/monthly/data/N_09_extent_v4.0.csv`.

## The device

The ice is the chart. `ice.glsl` is a pack-ice shader seen from straight above:

- floes of mixed sizes, with wind-blown snow and melt ponds;
- a translucent rim and a wet margin;
- a submerged ice foot, and shadows cast on the water;
- a sun that sinks through the film.

**Calibration.** In the melt shot, the white share of the frame tracks each year's extent: white share = extent ÷ 9. The calibration was measured from rendered pixels (`CAL` in `build.py`, `CF` in the shader), and the error is within about 0.02. In the last shot the camera pulls back so that the 2026 ice and a Greenland-sized floe are at the same scale; Greenland's area is 0.12 of that frame.

**Painted years.** Each year is painted on the floes in survey-orange stencil (Big Shoulders Stencil, `multiply` plus `inkTexture`). A water-only pass of the same shader goes on top (`hl = -2`, `alpha: true`), so open water cuts through the paint. As the ice retreats, the year drowns.

## The shots

1. **1979** at full pack.
2. **1980–2011**: the melt front rises one year every 0.24 s and eats the painted numeral.
3. **2012**: the record low, 3.57, painted across a cracked pack. Camera shake on the cut.
4. **48 Septembers**: the pack cut into 48 bars. 2012 is painted orange, with the 1979 level marked and brash adrift below.
5. **Dusk**: the camera pulls back and a Greenland-shaped floe melts away beside the line "Missing since 1979: a Greenland."

## Build

```
python3 build.py      # writes spec.json from the CSV (reproduces this spec.json byte for byte)
ANIKIT_RENDER_CACHE=0 anikit-video render spec.json -o final.mp4
```

`scripts/defects.py final.mp4` reports OK. One render of this spec had two blank frames at the 13.0 s cut. A re-render was clean, and `preview` renders those frames correctly, so this looks like a nondeterministic renderer glitch. It has been reported, and the shipped file is the clean render.

## What the jury still asks for

- Photographic-grade ice material: refraction, depth and wetness.
- A designed type system, with annotations that have scale and hierarchy.
- A denser chart beat.
