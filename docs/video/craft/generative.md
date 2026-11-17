# Generative imagery: shader work that looks authored, not "demo-scene"

Research for agents making shader-driven imagery in Anikit Video: what separates work that stops people from the generic noise-and-glow look, with numbers you can apply, and where Anikit stands. This is advice, not rules. Any look can work when it is chosen for the idea.

## Who to learn from, and what each one teaches

- **Inigo Quilez** (iquilezles.org, Shadertoy). Procedural imagery as *construction*: signed-distance modelling, domain warping (evaluate f(g(p)) instead of f(p), nested two deep for marble and smoke), cosine palettes (a + b·cos(2π(c·t + d))), soft shadows and AO built from the distance field. The lesson is that every iq piece is a *depiction of something* (a snail, a canyon, a selfie girl), lit like a photograph. The technique serves a subject. [warp](https://iquilezles.org/articles/warp), [palettes](https://iquilezles.org/articles/palettes/)
- **Tyler Hobbs** (Fidenza, QQL). Flow fields are a grid of angles with curves drawn through them. The art is in the *constraints*: where curves start, how they avoid each other, how thick they are, and colour chosen by **weighted probability per palette** rather than by spreading hues evenly. He warns against borrowing other people's palettes. [flow fields](https://tylerxhobbs.com/words/flow-fields), [colour](https://www.tylerxhobbs.com/words/working-with-color-in-generative-art), [colour arrangement](https://www.tylerxhobbs.com/words/color-arrangment-in-generative-art)
- **Zach Lieberman** (daily sketches, openFrameworks). Vivid, *specific* palettes, motion with bounce and personality, and simple systems iterated daily, each sketch a variation on yesterday's. Colour "unlocks" a piece and is chosen by feel, then committed to. [interview](https://layer.com/artists/zach-lieberman/interview), [lerandom](https://lerandom.art/editorial/zach-lieberman)
- **Refik Anadol** ("data paintings"). Data as pigment, fluid simulation as the brush: huge numbers of particles advected through a flow so the image *moves like matter*, and only *one* idea per piece, at architectural scale. [designboom](https://www.designboom.com/?p=972384), [Right Click Save](https://www.rightclicksave.com/article/the-interview-refik-anadol-moma)
- **Extraweg / Oliver Latta** (*Severance* titles, Emmy). Surreal *subject* first, story beat by beat, built from a long-kept reference library and many motion tests before any storyboard. Imagery is uncanny because it is physically plausible (soft bodies, real light), not because it glows. [Art of the Title](https://www.artofthetitle.com/title/severance/), [Motionographer](https://motionographer.com/2025/04/04/onlyon-extrawegs-severance-season-2-titlles-when-to-kill-your-brain-babies/)
- **Volumetric light** (Horizon Zero Dawn clouds, Maxime Heckel's write-ups). Beer–Lambert transmittance plus a "powder" term that darkens thin, sun-facing edges is what makes clouds read as clouds and not fog. Production quality wants ~64–128 view steps and ~6 light steps. [Heckel](https://blog.maximeheckel.com/posts/real-time-cloudscapes-with-volumetric-raymarching/), [optimisation paper](https://arxiv.org/pdf/1609.05344)

## Principles (what the good work has in common)

1. **A subject, not an effect.** Noise is a material (marble, smoke, ink, stone, cloud), never the subject. If a frame could be captioned "abstract background", it is a backdrop; the BAR's "soft blob" and "effect without subject" rejections are this failure.
2. **One idea per piece, pushed far.** Fidenza is only flow fields. *Severance* S1 is only a man inside his own head. Many effects at once read as a demo reel.
3. **Physical light.** Light at linear values above 1, a tone curve at the end (`ak_tonemap`), a clear key direction, shadows and occlusion. Glow should come only from things that are actually bright. Amateur work clamps to 0–1, adds bloom everywhere, and lights from nowhere.
4. **Deliberate palettes.** Three to five colours with *unequal weights* (one dominant, one support, one accent at 5–10 % of the area), mixed in a perceptual space (`ak_mix`, OKLab) so midpoints don't turn grey. Rainbow cosine palettes are the demo-scene tell. Pick the a, b, c, d of `ak_palette` from a reference image instead.
5. **Scale hierarchy.** Big structure, medium form and fine detail at roughly 1 : 1/4 : 1/16, with *most* contrast in the big structure. Domain warp shapes the big form; detail octaves should be quiet (gain 0.4–0.5, not 0.5–0.7).
6. **Motion with weight.** Slow base drift (0.02–0.1 of the frame per second) plus events: a surge, a fold, a reveal that lands on a beat (`iBeat`, timeline uniforms). Uniform constant-speed scrolling is what screensavers do.
7. **Finish like film.** Luma-weighted grain at 1–3 % (`ak_grain`), a gentle vignette, no banding (Anikit dithers 8-bit output), anti-aliased edges (on by default), and no clipped highlights unless chosen.
8. **Restraint in density, not in effort.** Many considered layers per frame (a foreground subject, a mid layer, atmosphere, type set into the world) beat one full-screen procedural fill.

## Techniques, with numbers

| Want | Do | Numbers |
|---|---|---|
| marble, smoke, ink | domain warp: `ak_fbm(ak_warp(p, a), 5–6)` | warp `a` 0.4–1.0; base frequency 1–3 per frame height |
| flowing lines / hair / currents | flow field: integrate along `ak_curl(p)` in a buffer | 20–60 steps; step length 0.5–1 px; fade 0.96–0.99 per frame |
| crisp cells, cracks, scales | `ak_voronoi(p)`: edges from `y − x` | edge width = `fwidth`-based (`ak_fill`) |
| organic growth | reaction–diffusion (Gray–Scott) in a buffer | feed 0.030–0.062, kill 0.057–0.065; 8–20 sub-steps per frame |
| clouds, god rays | volume march with Beer–Lambert + powder | 48–96 steps, 4–6 light steps, jittered start |
| glass, water, heat | refract the layers below by a height field's gradient | offset 5–40 px at 1080p; chromatic split 1–3 % |
| iridescence | thin film by view angle (`ak_iridescence`) | only on a real surface, 10–30 % mix |
| print and paper looks | style filters (`halftone`, `risograph`, `film` …) over the whole frame | grain 1–3 %, misregistration 1–4 px |

## What separates studio work from amateur work

| Amateur | Studio |
|---|---|
| a full-frame noise wallpaper | noise used as a *material* on a subject |
| rainbow or neon cosine palette | 3–5 colours, weighted, from a reference |
| everything glows | glow only from light sources, with the tone curve doing the work |
| constant-speed drift | rest, surge, settle; motion keyed to the edit |
| one layer | 3–6 layers with depth and atmosphere between them |
| clamped 8-bit look, banding | HDR maths, a filmic curve, dither, grain |
| demo shapes (torus, checker) | a depicted thing: a material sample, a landscape, an object with a use |

## Gap list against Anikit (and what this round adds)

- ✅ JIT GLSL at 1080p, buffers (simulations, trails), audio-reactive uniforms, default anti-aliasing, film-like dither.
- ✅ **Added: the `ak_` building blocks** (warp, curl, voronoi, OKLab mix, tonemap, grain …), so an agent writes about 5 lines instead of 60.
- ✅ Style filters (print, paper and film looks) for the whole frame.
- ✅ **Added in this round:** `glass` (refracts the layers below through fluted, ribbed or rippled glass), `clouds` (volumetric, lit with powder), `ink` (pigment billowing in water), `growth` (a real Gray–Scott simulation in buffers), `flow-lines` (Hobbs-style flow-field strokes). See `docs/video/shader.md`.
- ⏳ Gaps: no particle simulation with millions of points in a shader (the engine's `cloud` covers point clouds); no signed-distance text (type set *into* a shader world needs a text channel: use `{"layers": [{"text": …}]}` as a channel); no GPU-scale fluid (Anadol-style), only 2D advection in buffers.
