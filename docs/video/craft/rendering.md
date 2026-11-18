# Craft: the 2D rendering finish

What separates a studio-finished 2D frame from an amateur one is rarely the design. It is a stack of small technical defects that the eye reads as "cheap": bands in dark gradients, text that looks soft or thin, edges that crawl, glows that pulse in steps, motion that strobes. This page lists the techniques, with numbers, and where Anikit stands on each.

## Principles

1. **Never quantise early.** Every 8-bit rounding before the final output is a chance for banding. Keep gradients, glows, blurs and blends in float (or ≥ 12-bit fixed point) and round once, with dither. ([Gjøl, *Banding in Games*, 2016](https://loopit.dk/banding_in_games.pdf))
2. **Dither at the final quantisation, with a fixed pattern.** Triangular (TPDF) noise of ±1 LSB before rounding removes bands and noise modulation. The pattern must be anchored to screen pixels and identical every frame, or a video codec spends bits on it and still blocks it.
3. **Filter in linear light; blend where the look is defined.** Resampling and blurring in gamma space darkens fine detail and shifts colours ([Brasseur, *Gamma error in picture scaling*](http://www.ericbrasseur.org/gamma.html); [GPU Gems 3, ch. 24, *The Importance of Being Linear*](https://developer.nvidia.com/gpugems/gpugems3/part-iv-image-effects/chapter-24-importance-being-linear)). Compositing semantics (CSS/After Effects default) are sRGB-space, so a linear-light blend is an option, not a silent change.
4. **Text is the most scrutinised pixel.** Unhinted outlines at exact sub-pixel positions (no phase snapping), coverage shaped by the text colour's luminance (light-on-dark looks thin and dark-on-light looks heavy in sRGB blending), optional stem darkening at small sizes ([FreeType, *text rendering in general*](https://freetype.org/freetype2/docs/hinting/text-rendering-general.html)), and layout at fractional advances ([Levien, *Text layout is a loose hierarchy of segmentation*](https://raphlinus.github.io/text/2020/10/26/text-layout.html)).
5. **Resample with a real filter.** Enlarging images with bilinear interpolation produces visible creases and blur; Mitchell–Netravali (B = C = ⅓) is the standard sharp/smooth compromise, and B-spline for very smooth fields ([Mitchell & Netravali, 1988](https://www.cs.utexas.edu/~fussell/courses/cs384g-fall2013/lectures/mitchell/Mitchell.pdf)). Downscaling needs a prefilter, or it aliases.
6. **Motion blur is a shutter, not a filter.** A 180° shutter (exposure = ½ frame) integrates content over time; for fast motion cap the streak rather than strobe. Velocity-buffer post blur breaks at occlusion boundaries ([McGuire et al., *A Reconstruction Filter for Plausible Motion Blur*, 2012](https://casual-effects.com/research/McGuire2012Blur/index.html)), so sub-frame accumulation is the reference.

## Techniques and numbers

| Defect | Threshold where it shows | Technique |
|---|---|---|
| Gradient banding | Any 8-bit gradient changing < ~1 level per 3 px, worst in darks (≤ 20% luminance) | Float evaluation + TPDF dither ±½–1 LSB at the final rounding |
| Thin/heavy text | Light text on dark at < 24 px looks 10–15% thinner | Coverage gamma by text luminance (exponent ~1.2 for dark text → ~0.74 for white) |
| Text "swimming" | Glyph positions snapped to ¼ px step visibly when text moves < 1 px/frame | Exact sub-pixel phase |
| Crawling hairlines | Lines < 1 px wide alias as they move | Coverage AA, minimum 1 px coverage with alpha = width (no sub-pixel disappearance) |
| Strobing motion | Moves > ~10 px/frame without blur read as steps | 180° shutter, samples ≤ 1.25 px apart, streak cap ~40 px at 1080p |
| Soft upscales | Bilinear ≥ 2× | Mitchell (< 12×), B-spline (fields, ≥ 12×), premultiplied with colour ≤ alpha clamp |
| Codec blocking of noise | Per-frame random dither/grain at normal QPs | Fixed dither pattern; grain flagged to the encoder |

## Where Anikit stands (2D renderer)

Done:
- Float, dithered **smooth backdrops** (leading gradients and glows) and **cubic upscaling** (Mitchell/B-spline, dithered) of fields and images.
- **Dithered gradients** for panels and cards (float + fixed TPDF dither; `docs/video/render.md`, proof stills in `docs/video/proof/`).
- **Text**: exact sub-pixel phase, coverage gamma by luminance, cached runs (also scaled/rotated).
- **Motion blur**: true shutter accumulation in linear light, adaptive samples, slide smear, ≤ 6× plain cost on the showcase.
- **Post** in linear float with a fixed dither; film grain blended in a fast path.
- **Determinism** independent of cache history and timing.

Gaps (in order of visibility):
1. Frame-sized *animated* gradient washes (light leaks over content) still use vello's 8-bit gradient: a dithered path costs more than vello's own gradient today. Fix: a dither in vello's gradient painter (vendor patch), or compositing those ops outside vello.
2. No stem darkening option for small text (< 14 px) on light backgrounds.
3. Compositing is sRGB-space only; no linear-light blend option for glows and light (additive light should mix in linear light).
4. Hairlines thinner than 1 px fade by coverage only (fine), but strokes < 0.5 px can vanish; no minimum-coverage option.
5. Motion blur of clip wipes and scene cuts still re-renders regions per sample (20–35× on those frames).
