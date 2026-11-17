# Launch-video skill: two end-to-end runs

`skills/anikit-launch-video/SKILL.md` was run on two fictional products, each with a different brief, on 2026-11-17. The machine was the 4-vCPU cloud container with no GPU, using the release build from this branch. Each folder holds the brief generator (`make.py`), the spec, the MP4 and a contact sheet (2 frames per second).

| | Sift | Fernway |
|---|---|---|
| Brief | Email triage app: "Your inbox, sorted before you wake." Features: overnight sorting, one-line summaries, focus on what needs you. No brand given. | Offline trail navigation: "Trail maps that never drop." Features: offline maps, turn-by-turn, share location without bars. No brand given. |
| Format | 16:9, 15 s, 128 BPM `house` | 9:16, 15 s, 128 BPM `cinematic-hybrid` |
| Concept (`concepts.md`) | #5 **Physical simulation**. Noun: letters; verb: sorts; before→after: a falling pile → a neat stack. **Motif:** one red tile among white ones. **Turn:** the pile blown apart in slow motion, cut to the sorted inbox on a glass pane. | #4 **Data-driven** (the terrain is the map). Noun: a contour map; verb: guides; before→after: lost in fog → a lit trail. **Motif:** an amber trail of glowing beads that draws itself in every 3D shot and on the UI. **Turn:** out of the whiteout to the device standing on the ridge. |
| Anikit capabilities | Ray-traced `scene3d` (dark studio, DOF); `physics` rain, pile with a slow-motion impulse, and dominoes; the UI composition as a pane texture; chrome extruded wordmark; generated music with hits on `@bar3`/`@bar7`; sfx | Ray-traced `heightfield` terrain with fog and a low key light; 220 instanced emissive beads drawn by an expression; the UI composition on a device; whiteout fog; generated music; sfx |
| Shots | 14 detected, median 0.93 s, longest 1.87 s | 12 detected, median 0.93 s, longest 1.90 s |
| `pace.py` on the final | **PASS** | **PASS** (failed twice in draft: a dark-to-dark cut went undetected; fixed by alternating light and dark shots) |
| Draft iterations (`--quality draft --scale 0.5`) | 3 renders, 40–50 s each | 3 renders, 44–45 s each |
| Final render (default quality, 1080p) | 68.1 s (7 fps) → 26.7 MB | 72.5 s (6 fps) → 46.0 MB |
| Repo copy | re-encoded with ffmpeg x264 CRF 22 → 9.8 MB | re-encoded with ffmpeg x264 CRF 25 → 3.9 MB |
| Wall time, first spec → final MP4 | ≈ 16 min | ≈ 13 min |
| Tool calls, first spec → final MP4 | ≈ 22 | ≈ 18 |

## BAR self-review (honest)

- **Sift passes:** a real subject in every shot (tiles, the inbox, the stack), real light and DOF, product copy from the brief only, one accent colour used only on hits, and a proof that is a demonstration (an unread counter running to 0), not a statistic.
- **Sift is weak in:**
  - the two tension slams ("Too much" / "mail.") are flat type on solid colour, which is close to BAR rule 1, though there is no gradient and they run 0.94 s each;
  - the end-card wordmark reads small at 16:9.
- **Fernway passes:** the motif is carried through every 3D shot and into the UI, the fog → ridge turn reads, and type sits in the vertical safe area.
- **Fernway is weak in:**
  - the terrain is low-contrast at draft resolution;
  - "Every trail *saved.*" touches the device's header in shot f1b;
  - the slams are flat colour fields, the same caveat as Sift.

## Friction found, and what was done

| Friction | Status |
|---|---|
| A `scene3d` with `physics` but no `objects` rendered the lookdev demo ("ANIKIT" chrome plus spheres) | **Fixed** in `src/scene3d/mod.rs`: physics-only scenes no longer get the demo objects |
| `text: "=expr"` is shown literally (`docs expressions` says it works) | Worked around with timed layers. **Reported** to the parent |
| A path layer is placed by its bounding box, so contour paths at `x: 0, y: 0` all stacked at the top | Documented in the skill and in `docs launch` |
| A `scene3d` `scale` expression must be `"=[s, s, s]"` | Documented |
| No bitrate/CRF option on `render`, so a 15 s 1080p file is 27–46 MB | Re-encoded with ffmpeg and documented. **Reported** to the parent |
| `pace.py` misses dark-to-dark cuts | Rule added to `beats.md` |
| `critique` flagged text on a backing rect inside a group at 1.1:1 against the scene background (a spec-level false positive; the render check didn't flag it) | **Reported** |
