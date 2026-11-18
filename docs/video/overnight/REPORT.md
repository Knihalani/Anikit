# Overnight report: 2026-11-17 21:30 → 2026-11-18 07:00 UTC

## The honest headline

- **Every category got better, but nothing reaches professional level yet.**
- The bar is a blind panel that compares each film against three style-matched professional pieces (87 references, 14 categories). On that panel our best films now score about 4–5. The professional references score 6.5–8.7, and our evening work scored around 3.2–3.4.
- Over 13 categories, the best film per category rose by +0.6 to +1.5 points. No film passes. A handful won individual "professional" votes: data story 1/12, lyric video 3/12, illustration 1/12, event promo 2/12.
- The judge is validated: all 20 rejected pieces fail and all 10 professional controls pass. It also has noise of about ±0.5, so any gap under 0.75 is a tie.

## Demo set (one film per category)

See `docs/video/overnight/DEMOS.md` for all paths. Pooled blind scores:

| | Data story | Illustration | Event/brand | Launch | Lyric video | Titles | Sports | Explainer | Event 9:16 | Social 9:16 | 3D product | Generative | Ident |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| best | 5.1 | 4.9 | 4.6 | 4.4 | 4.4 | 4.3 | 4.2 | 4.0 | 3.9 | 3.9 | 3.6 | 3.6 | 3.4 |
| first attempt | 3.6 | 3.4 | 4.0 | 3.75 | 3.1 | 3.6 | 4.5* | 2.75 | 2.5 | 3.25 | 3.5 | 3.1 | 2.75 |

\*The sports opener re-judged at 3.75 today, which shows the judge's drift.

## Versus HyperFrames (same briefs, same judge)

| | Anikit r1 | Anikit r3 | HyperFrames |
|---|---|---|---|
| Score on the three shared briefs | 3.75 / 3.62 / 3.62 | — | 2.88 / 2.75 / 3.38 |
| Agent cost per film | $2.03 | $2.74 (art-director reviewer on) | $2.84 |
| Wall time, median | 12.3 min (incl. a 2.5–6 min cold build, since removed) | 9.3 min | 10.9 min mean |
| Render | 16–109 s typical | 53–85 s typical | n/a (browser capture) |
| Output defects | none found | none found | stutter: frozen bursts of 7–19 frames |

- Anikit beat HyperFrames on all three shared briefs.
- Cost and time are now roughly at parity: we traded the cost lead for quality (the reviewer step).

## What moved quality (measured; details in `docs/video/craft/WHAT-WORKED.md`)

**Moved the score:**
- **Rebuilding structure against the judge's notes** was worth about +1 to +2:
  - one idea from the subject, escalated every beat;
  - no repeated layout;
  - type designed into the image.
- **An independent art-director reviewer** before the final render coincided with r3's +0.68 over r2 (3/6 briefs beyond noise). n=6, so the reviewer can't be credited alone.
- **Sound** was the one critic dimension that rose (4 → 5.5).

**Didn't move the score:**
- New fonts on their own: +0.
- More features, and menus of features: agents used them more, but scores didn't change.
- Engine fidelity: lit brick, live 3D textures and so on improved the critic's scores, not the blind comparison.
- Prose self-review questions were a rubber stamp: 2/12 agents changed anything substantive.

**Still the gap, in every critique:** a subject-specific idea, type with a point of view, and physical or photographic material and light.

## What was built tonight (by domain; all merged on main, tests and clippy green)

- **Type:**
  - 131-family OFL library;
  - variable axes, animated per glyph;
  - name resolution, glyph fallback, wrapping, balanced breaks, typesetting;
  - type as a matte.
- **Graphic design:**
  - halftone/CMYK, riso, print presets, brush, cutout and patterns;
  - grid and grid motion;
  - palettes;
  - editorial charts.
- **Motion design:**
  - inertia, smear, drag and per-layer motion blur;
  - idea transitions (zoomThrough, carry);
  - dolly, timeRemap, velocity expressions;
  - tasteful defaults (overshoot cut from 39% to 2–8%).
- **3D:**
  - lighting rigs, lens and rack focus, sets;
  - imperfections, anisotropic machined finishes (guilloché, sunburst), decals, practicals;
  - fog;
  - local lights on textures; shader compositions as 3D textures;
  - glass fixes.
- **Assets:** 218 pinned CC0 HDRIs, PBR materials and models, plus 72 Kenney models, film LUTs and photo backplates.
- **Illustration:** the `illo` toolkit (one light, a hand, a rig), landscape, plants, water.
- **Shader:** `ak_` building blocks, multi-pass in one file, growth/flow/ink/soap/sky, drawn-look blocks; faithful previews.
- **Simulation:** fluid detail (MacCormack, turbulence), trails, particle layers and glints, physarum, cloth, sprites, liquid.
- **Sound:**
  - 75+ procedural sounds, beds and foley;
  - `"sound": "auto"` from picture and from 3D mechanisms;
  - a −14 LUFS master.
- **Music:** real instrument packs (bass, guitar, drums, strings), trailer grammar, stereo, alt mixes, music→picture binding (`onNotes`).
- **Spec and agents:**
  - arrays in expressions, relative layout, `frames`;
  - forgiving syntax and helpful errors;
  - `docs key`;
  - a capability menu and the reviewer step in the skill;
  - prebuilt install (setup cut from 2.5–6 min to about 5 s).

## Speed, memory, determinism and size

**Speed:**
- **Post:** 3.6× faster, bit-exact (290 → 80 ms/frame per thread).
- **Glass:** faster 3D glass (multi-hit walk).
- **Mesh traversal:** SBVH, about 1.8×.
- **3D quality for cost:** edges at 9 samples (+7–17% CPU, flicker clearly down).

**Memory:** memory kept after renders cut 5–20×, and r2's 5 GB peaks are gone (0.5–1.4 GB in r3).

**Determinism and correctness fixes:**
- a work-stealing race;
- the render cache (file-content keys, no storing after errors, `--no-cache`);
- a shader-job eviction that caused random blank frames, which needs PERF's repeat-render confirmation;
- a stale-layer bug under a moving camera;
- glyph-atlas drops;
- several crashes.

**Encoder:**
- B-frames, hex motion search, 8×8 transform, IDR lookahead and a bitrate ceiling: 4–30% smaller on test specs, and a 30 s film went from 149 to 98 MB.
- **Honest gap:** it's still about 2.8× x264's size on paper texture and 1.6× on grain.

## Known gaps and next steps

1. **Concept and art direction are the binding constraint, not features.** Next: an idea-generation step that leaves the obvious family, judged before building.
2. **Motion is judged from stills.** Blind motion scoring needs real reference clips, and downloading them was blocked here.
3. **The encoder is behind x264 on texture.** Next: a full macroblock tree, RD partitions, psy-rd.
4. **Real-world slow renders are shader-bound** (e.g. 1–3 fps on custom GLSL). Next: JIT and AA cost.
5. **Session message delays** of 1–3 h hit three teams tonight (MOGRAPH, 3D, PERF), so two rebuilds started late.

## One decision for you

Should JUDGE download the professional reference clips, for evaluation only, so the panel can judge motion blind? It's switched off until you say yes.
