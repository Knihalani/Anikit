# Launch-video skill: two end-to-end runs

**These are two possible films, not the format.** They were made with an earlier version of the skill that
prescribed a fixed structure (hook → tension → reveal → three features → proof → logo at 128 BPM); that
structure has since been removed. A new launch film should invent its own shape, tempo and look from its product's
story (see `skills/anikit-launch-video/SKILL.md`). Read these for technique (3D type in the set, UI as a texture,
physics, terrain, timing sfx to a score), not for structure.

`skills/anikit-launch-video/SKILL.md` was run on two fictional products, each with a different brief, on 2026-11-17. The machine was the 4-vCPU cloud container with no GPU, using the release build from this branch. Each folder holds the brief generator (`make.py`), the spec, the MP4 and a contact sheet (2 frames per second).

| | Sift | Fernway |
|---|---|---|
| Brief | Email triage app: "Your inbox, sorted before you wake." Features: overnight sorting, one-line summaries, focus on what needs you. No brand given. | Offline trail navigation: "Trail maps that never drop." Features: offline maps, turn-by-turn, share location without bars. No brand given. |
| Format | 16:9, 15 s, 128 BPM `house` | 9:16, 15 s, 128 BPM `cinematic-hybrid` |
| Concept | **Physical simulation**. Noun: letters; verb: sorts; before→after: a falling pile → a neat stack. **Motif:** one red tile among white ones. **Turn:** the pile blown apart in slow motion, cut to the sorted inbox on a glass pane. | **Data-driven** (the terrain is the map). Noun: a contour map; verb: guides; before→after: lost in fog → a lit trail. **Motif:** an amber trail of glowing beads that draws itself in every 3D shot and on the UI. **Turn:** out of the whiteout to the device standing on the ridge. |
| Anikit capabilities | Ray-traced `scene3d` (dark studio, DOF); `physics` rain, pile with a slow-motion impulse, and dominoes; the UI composition as a pane texture; chrome extruded wordmark; generated music with hits on `@bar3`/`@bar7`; sfx | Ray-traced `heightfield` terrain with fog and a low key light; 220 instanced emissive beads drawn by an expression; the UI composition on a device; whiteout fog; generated music; sfx |
| Shots | 18 detected, median 0.93 s, longest 1.87 s | 12 detected, median 0.93 s, longest 1.90 s |
| `pace.py` on the final | **PASS** | **PASS** (failed twice in draft: a dark-to-dark cut went undetected; fixed by alternating light and dark shots) |
| Draft iterations (`--quality draft --scale 0.5`) | 3 renders, 40–50 s each | 3 renders, 44–45 s each |
| Final render (default quality, 1080p) | 74.8 s (6 fps) → 29.9 MB | 106.4 s (4 fps; type shots trace at full resolution) → 52.5 MB |
| Repo copy | re-encoded with ffmpeg x264 CRF 22 → 10.4 MB | re-encoded with ffmpeg x264 CRF 25 → 3.7 MB |
| `critique --render` errors | 0 | 1: the known contrast false positive on the backed "Works offline." label |
| Wall time, first spec → final MP4 | ≈ 16 min | ≈ 13 min |
| Tool calls, first spec → final MP4 | ≈ 22 | ≈ 18 |

## BAR self-review (honest)

The first pass had flat type-on-solid slams (BAR rule 1). The parent asked for a second pass. Every word shot now
puts its type inside the world:
- **Sift:**
  - "Too much" is extruded paper-white 3D type in the storm of falling tiles;
  - "mail." is red lacquered 3D type on a bright cyc, with the tiles still falling;
  - the proof counter is 3D numerals on a red cyc.
- **Fernway:**
  - "No signal." is dark 3D serif on the ridge in pale fog;
  - "No map." and the proof "0" are emissive amber type that lights the terrain;
  - "without bars." now sits on the product's own map;
  - the logo slam is 3D in fog.
- **Fernway terrain contrast:** a grazing key light (intensity 22, 11° elevation), a lighter base and exposure 1.3.

**Third pass on Sift (the mechanical type rule).** Every word is now 3D type in its set: "Sorted" beside the
dominoes, "overnight." among them, "Every thread," / "one line." beside and in front of the inbox pane, and
"Only what" / "needs you." in front of the focus pane. "Too much" now has emissive white type, with the tiles kept
behind it. The end wordmark fills about 55% of the frame width. The proof's 2D label and the end tagline were
removed. The self-test in SKILL.md now lists only `sift.mail` as 2D text.

Remaining weak spots:
- Fernway was not re-done for the mechanical rule. It still has 2D type ("Now what?", "Every switchback.", the
  map shots' words) and the "Works offline." pill, which is BAR #3.
- In Sift, "overnight." partly intersects the dominoes.
- "Every trail *saved.*" still touches the device header in Fernway's f1b.

## Friction found, and what was done

| Friction | Status |
|---|---|
| A `scene3d` with `physics` but no `objects` rendered the lookdev demo ("ANIKIT" chrome plus spheres) | **Fixed** in `src/scene3d/mod.rs`: physics-only scenes no longer get the demo objects |
| `text: "=expr"` was shown literally | Worked around with timed layers; since **fixed** by SPEC (e6ab9cf) |
| A path layer is placed by its bounding box, so contour paths at `x: 0, y: 0` all stacked at the top | Documented in the skill and in `docs launch` |
| A `scene3d` `scale` expression must be `"=[s, s, s]"` | Documented |
| No bitrate option on `render` (27–46 MB for 15 s at 1080p) | The repo copies were re-encoded with ffmpeg; `render --crf N` has since been added |
| `pace.py` misses dark-to-dark cuts | Rule added to `beats.md` |
| `critique` flagged text on a backing rect inside a group at 1.1:1 against the scene background (a spec-level false positive; the render check didn't flag it) | **Reported** |
