# The Planet Breathes: what held the engine back

**What it is:** a 12.4 s, 16:9, 30 fps data explainer built on real data: NOAA GML Mauna Loa monthly mean CO₂, Mar 1958 – Sep 2026, 823 months (`data/co2_mm_mlo.csv`, downloaded 2026-11-17). It has three beats and one continuous object:
1. **One breath.** The last 12 months (Oct 2025 – Sep 2026) are drawn as a magnified loop on a month clock, with the peak (May 2026, 432.3) and trough labelled. Copy: "Every year, the planet takes a breath … CO₂ falls by about 6 ppm" (432.34 → 426.07, May → Sep).
2. **68 breaths.** That year shrinks into place while all 68 years spiral out from 1958. A live year and ppm readout shows exact monthly values, and the colour ramps cool → vermilion by year.
3. **The unroll.** The spiral peels off from its outer end and uncoils like a spring into the Keeling curve. On the curve: per-decade trend rates of the deseasonalised series (+0.8 ppm/yr in the 1960s → +2.6 in the 2020s, 3.2×), a breathing "now" dot at 426.1 ppm (Sep 2026), and the 315.7 (Mar 1958) start. Copy: "The breath hasn't changed. The air has."

Every number on screen is computed from the CSV in `build.py`. Typefaces: Libre Franklin Light (editorial headlines) and IBM Plex Mono (data). Score: piano, strings, cello and celesta, −13.9 LUFS. Render: 372 frames in 9 s (41 fps); re-renders hit the chunk cache in 2.5 s.

**Files:** `breath.mp4`, `contact-sheet.jpg`, `compare.jpg` (ours on the left, Fragapane on the right), `build.py` (the real source; it generates `breath.json`, 27 MB), `data/`, `fonts/` (OFL), `refs/`.

## References
| Ref | URL | What makes it great |
|---|---|---|
| Federica Fragapane, *Words of War* (La Lettura) | https://informationisbeautifulawards.com/showcase/1515-words-of-war-svetlana-alexievich-s-vocabulary | Custom glyphs encode 3–4 variables each. Hairline connectors, dense but calm. A two-hue palette on warm grey. Serif annotation set *inside* the graphic. |
| Federica Fragapane, *One Year of Visual Narratives* | https://www.informationisbeautifulawards.com/showcase/3043-one-year-of-visual-narratives-visualizing-cultural-current-and-social-topics-for-la-lettura | Every chart is a new form invented for its data (nautilus fans, mirrored bells, half-dial small multiples). Labels sit everywhere, and nothing collides. |
| Randal Olson, *CO₂ piles up 3× as fast* (2026) | https://www.randalolson.com/2026/06/05/co2-rising-faster-every-decade/ | The same data, re-zeroed per decade. A clear single claim in the title, direct labels and no legend. (Our rate check matches his: 0.88 vs our 0.81 for the 1960s; methods differ.) |
| Ed Hawkins, *Warming stripes* | https://showyourstripes.info/ | One encoding, no axes, instantly legible; the colour *is* the data. |

## Honest verdict
The unroll is the strongest moment of the three pieces. The spiral-to-curve morph with the data intact is something a designer would ask about. Type and data are crisp at 100%, and every number is real. Next to Fragapane it is much sparser: one series and a handful of labels, against her dozens of encoded glyphs and annotations in a frame. It is a good explainer, but not yet a dense editorial graphic.

### Blind judge
Run 1: **FAIL, 4.0** (refs: Kurzgesagt ×2, Cosmos; mean 7.21). Part of that was **my bug**. Per-frame layer windows were rounded so every 3rd frame fell into a gap, which blanked the spiral and the unroll in a third of the frames. That is why the critic saw "the spiral never visibly unwinds" and "static". Fixed by centring each window on its frame (`start = t − ¼ frame`, `end = t + ¾ frame`). I also filled the empty opening frames. The rest of the critique stands:
- small, low-contrast labels (6–8 px at judge size);
- no type choreography;
- hairline strokes with no weight hierarchy;
- a single camera move;
- no material or texture.

Run 2, after the fix: **FAIL, 4.0** (ref mean 7.42). The fix removed a real defect, but the judge still ranks it last of four. The gap is density, material and type choreography.

## Gaps, ranked
1. **No data-driven geometry or path interpolation.** There is nothing for "polyline from CSV, morph between two layouts of the same points, draw-on by data index". *Workaround:* `build.py` bakes one `draw` layer **per frame** (673 layers, a 27 MB spec, ~200k points) for the draw-on, the year shrink and the unroll, plus per-frame text layers for the exact year and ppm readout. It works and renders at 41 fps, but the spec is unreadable and unauthorable by hand. *Want:* a `data` layer: `{ "data": "co2.csv", "x": "=…", "y": "=…" }`, layout keyframes (`polar` ↔ `cartesian`, per-point stagger), `reveal` by row index, and text bound to `row(i).field`.
2. **No glyph fallback.** Libre Franklin has no `₂`, and the engine drew tofu (□) with no warning. *Workaround:* wrote "carbon dioxide" in headlines; CO₂ appears only in Plex Mono. *Want:* per-glyph fallback through a font stack (`"font": ["Libre Franklin", "Inter"]`), plus a warning listing missing glyphs.
3. **Multi-line text defaults to centred alignment,** even for `anchor: topLeft`. Every editorial block came out centred until `"align": "left"` was set. Left alignment is the neutral default for paragraph text.
4. **No label-collision avoidance.** Three collisions (rate label vs big number, start label vs rate label, month initial vs value label) were found only by looking at frames. *Want:* a `labels` helper with leader lines that avoids overlaps, or at least a `check` warning for overlapping text boxes.
5. **Per-frame layers need exact frame windows (this caused a real defect: 1/3 of the frames went blank).** `end = start + 1/fps` made two frames visible at once (double year readout, double head dot). *Want:* a `frame: n` / `frames: [a, b)` key, or `"hold": "frame"`.
6. **Dark gradients with fine grain blocked up at default CRF.** Visible macroblocks in near-black. *Workaround:* `--crf 14` (28 MB for 12 s). The default should protect dark grain, or warn.
7. **There's no `maxWidth`/text box width for wrapping;** I wrapped by hand with `\n`. (`maxWidth` gave "unknown key".)
8. **The `fonts` map key is ignored** (see hill/GAPS.md #4). It worked here only because the keys matched the internal family names.

## What worked
- `draw` items with per-point `colors` render 5,000-point gradient polylines crisply at 146 fps on re-render.
- The incremental chunk cache made a label fix re-render in 2.5 s instead of 9 s.
- `"=…"` expressions and `{value:,}` formatting covered the counters that didn't need exact data.
