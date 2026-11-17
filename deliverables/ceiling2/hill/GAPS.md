# The Hill That Woke: what held the engine back

**What it is:** an 11.9 s, 16:9, 24 fps illustrated film. A wanderer with a lantern crosses a snowfield under a moonlit hill. Up close, an eye the size of a house opens. In the wide shot, the hill lifts its head against the moon: its pines are its fur and antlers. The look is hand-painted night (single moon key, a warm lantern against cold light, airbrushed grain re-drawn 12× a second, line boil). Score: a celesta lullaby, a timpani hit on the eye, and a D-major choir and horn bloom on the wake, plus synthesized wind and snow-crunch footsteps.

**Files:** `hill.mp4`, `contact-sheet.jpg` (2 fps), `compare.jpg` (ours on the left, references on the right, full size), `hill.json` (generated), `build.py` + `glsl/` (the real source), `refs/`.

**How it's built:** four full-frame GLSL shader layers (one per shot), about 330–430 ms/frame each at 1080p. They share a 200-line toolkit: SDF characters, saw-tier spruces, spray grain, boil, lantern light. Above that sit one text layer and a hand-written score. Final render: 137 s for 286 frames on 4 cores.

## References (what makes them great)
| Ref | URL | What makes it work |
|---|---|---|
| Giant Ant, *Hilda & the Mountain King* titles | https://www.stashmedia.tv/?p=84388 | One motivated light source. True-black masses against a brilliant lit plane. Light is drawn as *shapes* (a cast wedge of light), not glows. Coarse stipple grain inside gradients. Huge scale contrast. |
| Giant Ant, *The Last White Rhino* | https://www.giantant.ca/the-last-white-rhino | Backlit silhouettes. Foreground grass in extreme motion-blur DOF. A warm haze that eats the horizon. |
| Giant Ant, *Tako Faito!* | https://motionographer.com/2015/08/19/tako-faito/ | Real dry-brush and watercolour texture. Variable-weight ink line. A 3-colour palette. |
| Chromosphere, *Forms in Nature* | https://chromosphere-la.com/case-study/forms/ | Value steps per depth plane (atmosphere). Geometric simplification. Every element locked to a grid. A palette per scene. |

## Honest verdict next to the references
The idea and the staging hold up: the scale contrast in the eye shot, the reveal in the wake shot, and the continuity of the trees becoming fur. The surface craft does not, yet. Hilda's frames have a wider value range (pure blacks next to lit planes), light drawn as designed shapes, and a hand-drawn line inside the silhouettes. Ours sits in a narrower mid-violet band, with light as radial falloffs. The character is a rigid SDF puppet, not drawn animation. Nothing in the frame is a real brush mark.

### Iteration 2 (after the first comparison)
Light is now drawn as a shape: the lantern throws a hard-edged, stippled wedge of warm light up the creature's face, Hilda-style, instead of a radial glow. The blacks are deeper, and silhouettes get a dry-brush edge (`brushEdge`: bristle streaks along a direction, re-drawn on twos). The eye shot gained most. Hand-drawn interior line and real character animation are still missing (gaps 1–2).

### Blind judge (scripts/judge.py, -v-judge branch)
Run 1: **FAIL, 3.62** (refs: Mad Men, Tintin and Spider-Verse titles, mean 8.71; judged pro 0/3). The critic's defects:
- a blank flat frame at about 60–70%;
- the light wedge is a hard polygon with no volumetric falloff;
- the eye opens as a vector morph with no lid squash;
- the walk is stiff and moves as one rigid silhouette;
- the hatch texture is uniform and procedural;
- the snow is one layer of soft dots;
- the title is a generic italic serif that competes with the reveal.

**The blank frames were real, and an engine bug** (gap 0 below).

Run 2, after the clean render: **FAIL, 3.75** (ref mean 8.50). Removing the defect barely moved the score, so the gap is craft and idea execution, not the blank frames.

### Iteration 3: rebuilt on main d9c9b60 (font library, glyph fallback); judge **FAIL 4.0** (from 3.75; ref mean 8.33)
| Dimension | Before (run 2) | After |
|---|---|---|
| idea | 4 | 5 |
| light_texture | 4 | 5 |
| originality | 3 | 4 |
| motion | 4.0 | 3.5 |
| sound | 4 | 3 |

What changed:
- **Hand-made texture:** `furTex` replaces the repeating dash grid with long, domain-warped strands gathered into clumps, re-drawn on twos.
- **Layered particles:** `snowfall` has 4 depths. Far flakes are fine and dense; near flakes are big, soft and smeared along their velocity, with a gust; flakes near the lantern turn warm.
- **Secondary motion on the wanderer:** the hood tip, hem, pack, bedroll and cup each lag the step by a different phase.
- **The eye:** the lower lid leads and the upper lid peels from the outer corner. There's a fleshy lid band, a wet bead of light along the lower lid, and the brow fold lifts as it opens.
- **Choreographed title:** Fraunces from the new library, each word landing on its own beat with `axes.SOFT` 0→100 and `wght` 300→560 as it settles, then a tracking-out. A Caveat hand-written subtitle ("a story told by the snow"). Font loading by library name worked first time.

What still fails, according to the judge, and why it's the engine ceiling, not polish:
- the walk still reads as a sliding cut-out;
- the eye "pops" between 1/6 s samples;
- the hill's wake has no anticipation, mass or settle (no trees swaying, no snow shedding);
- the cut-out tree layers read as clip art;
- the sound is thin, with no low-end rumble or foley on the reveal.

These map directly onto gaps 1 (no painted primitive), 2 (no rig: a weighted walk with overlapping action is hand-keyed maths in GLSL) and 8 (no foley). New gap: **12. Secondary motion can't be authored on shader-drawn characters.** MOGRAPH's inertia and smear work on layers, but a character drawn inside a shader gets none of it. Every lag is a hand-written phase offset.

## Gaps, ranked (most limiting first)

0. **P0 render-cache bug: stale or blank chunks are spliced into renders.** The final had 3 flat background-colour frames (8.12 s, 8.92 s, 8.96 s). `still` at those exact times is correct, and so is `render --from 7.8 --to 9.4`. A full re-render reported `render cache: reused 16/16 chunks` and reproduced the blank frames byte for byte. With `ANIKIT_RENDER_CACHE=0`, the render is clean, and exactly those 3 frames differ. So some earlier render produced blank frames, possibly a transient or race in shader evaluation, and the chunk cache then preserved them indefinitely. *Workaround:* `ANIKIT_RENDER_CACHE=0` for finals. *Want:* the cache key covers shader file contents, the cache validates chunks (no flat or NaN frames), and the render-time blank frame itself is found and fixed.

1. **No painterly / drawn rendering primitive for shader-built worlds.** `brush`/`sketch`/`hatch` are separate vector components, so they can't follow a shader's shapes or be used *inside* GLSL. Every surface texture had to be faked with hash noise (stipple and fur dashes), and it reads as "procedural", not painted.
   *Workaround:* `spray()` (12 fps re-seeded stipple) and a tangential-dash fur pattern.
   *Want:* GLSL built-ins for brush-textured fills and strokes (`akBrush(sdf, dir)`, `akPaper(uv)`, dry-brush edge erosion), or a filter that paints any frame with a stroke field (oriented strokes following image gradients, Hertzmann-style).
2. **No character animation path.** There is no rig, no bone or puppet warp, and no frame-by-frame drawing import (only Lottie). The wanderer is a hand-coded SDF with a sine walk cycle; poses are limited to what you can express in maths.
   *Want:* a 2D puppet/mesh-warp component (pins + skeleton on an image or SVG), or an SVG-sequence player with per-frame boil.
3. **No `#include`/shared code between shader layers.** Four shots share a 200-line toolkit.
   *Workaround:* `build.py` concatenates `common.glsl` into each shot file.
   *Want:* a top-level `"glsl": {"common": "common.glsl"}` prepended to every shader, or `#include "file"`.
4. **Fonts: the `fonts` map key is ignored, and unknown fonts fall back silently.** `"fonts": {"Fraunces Italic": "x.ttf"}` registers the file under its *internal* family name (`Fraunces 72pt SuperSoft Wonky`). `"font": "Fraunces Italic"` then renders in Inter with **no warning**. It took 4 renders to find.
   *Want:* register the key as an alias, warn on an unknown family ("did you mean …"), and list loaded families in `preview`.
5. **A NaN in a shader blacked out the whole frame with no diagnostic.** `sdEll` with a zero radius on one frame range made ~1 s of black frames in the draft.
   *Want:* `still`/`render` should detect NaN/Inf output (count the pixels, report the layer and time), and ideally write NaN pixels as magenta in draft.
6. **The default scene transition is a 0.5 s crossfade.** The film came out 1.5 s short (10.4 vs 11.9 s), and every cut was a mush, until `"transition": "cut"` was set. For film work, a cut is the neutral default.
7. **The critique can't see shader content.** On this piece it said "violet on near-black generic", "5 of 5 shots centred and symmetric: reads as slides", "centred headline on a soft backdrop at 7.5 s" (that frame is the eye shot, which has no headline), and "first words at 8.8 s, viewers decide in 2 s" (this is a wordless film). All are false positives, because the critique reasons from the layer list, not from pixels.
   *Want:* image-based composition and colour analysis for shader/3D layers, and an intent flag for wordless/illustrated work.
8. **No foley or ambience in the sfx library.** The library holds only whooshes, risers and UI sounds: no wind, footsteps in snow, creature breath, rumble or fire.
   *Workaround:* custom noise synths (band-passed noise "wind" with an LFO; 90 ms noise bursts as snow crunch; a detuned saw through a ladder filter as the growl).
   *Want:* a small natural-foley set (wind, rain, footsteps on several surfaces, cloth, breath, creature, fire, water).
9. **Note-language papercuts:** rests need a slash (`r/4`, not `r4`), accents go after the duration (`D2/16?`, not `D2?/16`), and a hold is a separate token (`C3 _`). Each mistake costs a round trip. Errors are clear, though.
10. **Draft is barely faster for shader-heavy specs.** `--quality draft` (960×540) ran at 4 fps against 2 fps at full quality: about 2×, not the 4× fewer pixels.
11. **The default encode is huge for grainy shader work:** 120 MB for 12 s at the default settings (the GitHub file limit is 100 MB). Re-encoded with x264 CRF 21: 12 MB with no visible loss. *Want:* a bitrate-aware default, or a `--target-size`.
12. **Shader cost.** 330–430 ms/frame at 1080p with supersampling for 2D SDF illustration (no raymarching). Most of that is fbm and per-pixel loops over tree cells. That's acceptable for a 12 s piece, but iterating on full-res stills (~1 s each) is the bottleneck. The `--watch` flow helps.

## What worked well
- Shaders as a painting medium: full control of light, parallax per depth plane, 12 fps boil, and all animated from the timeline with `animate` on uniforms (`eye`, `wake`, `walk`, camera).
- Keyframes with per-key easing (`[[t, v, "outCubic"], …]`) made the eye's false-start flutter and the lid overshoot easy.
- The hand-written score with automation landed every hit exactly, and the mix came out at −14.2 LUFS.
