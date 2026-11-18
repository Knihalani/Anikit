# e-data rebuild: Arctic sea ice, September minimum 1979–2026

Brief: a 30 s, 16:9 piece on the September minimum of Arctic sea ice, using NSIDC Sea Ice Index v4 (`N_09_extent_v4.0.csv`, in this folder).

**Score**, from `python3 scripts/judge.py brief e-data final.mp4`: the same three pinned references as r1 and r2, 3 trials plus the critic.

| Round | Score | Full score | Judged pro |
|---|---|---|---|
| r1 | 3.62 | 3.83 | 0/3 |
| r2 | 3.38 | 3.67 | 0/3 |
| rebuild v1 | 4.38 | 4.50 | 0/3 |
| rebuild v2 | 4.62 | 4.94 | 0/3 |
| rebuild v3 | 4.50 | 4.72 | 0/3 |
| **rebuild v4 (this file)** | **5.25** | **5.39** | **1/3** |

The full jury answer is in `score.json`.

## The device

The ice is the chart. `ice.glsl` is a pack-ice shader seen from straight above: Voronoi floes with mixed sizes, lit snow, melt ponds, a translucent rim, a submerged ice foot, cast shadows on the water, and a low sun.

- **Calibration.** Its `cover` is calibrated from measured pixels, so in the melt shot the white share of the frame equals the year's extent divided by the 9 million km² of ocean the frame stands for. The table is `CAL` in `build.py` and `CF` in the shader.
- **Painted years.** Each year is painted on the floes in survey-orange stencil (Big Shoulders Stencil, `multiply` plus `inkTexture`). A second, water-only pass of the same shader goes on top, so open water cuts the paint. As the ice retreats, the year drowns.

## The shots

1. **1979** at full pack.
2. **1980–2011**: the melt front rises, one year every 0.24 s, and eats the painted numeral.
3. **2012**: the record low, 3.57, painted across a cracked pack.
4. **48 Septembers**: the pack cut into 48 bars, one per year. 2012 is in paint orange, with the 1979 level marked.
5. **Dusk**: "Missing since 1979: a Greenland." The 2.24 million km² lost compares with Greenland's 2.17.

The light goes from high white day to low amber sun to blue twilight. Sound is wind ambience, with crunch and shatter on the cuts and a scored ambient bed.

## Build

```
python3 build.py      # writes spec.json from the CSV
ANIKIT_RENDER_CACHE=0 anikit-video render spec.json -o final.mp4
```

`scripts/defects.py final.mp4` reports OK.
