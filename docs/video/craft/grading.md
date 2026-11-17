# Colour grading: what colourists actually do

Scope: 10–60 s pieces rendered by the engine: 2D vector and type, illustration, GLSL shaders, ray-traced 3D and particles. The post stage offers a grade with presets, bloom, grain, vignette and halation.
Tags: **[src]** means a cited source. **[conv]** means a widespread practitioner value for which I found no primary source; treat it as a start point.
Values are given on display-referred 0–1 (sRGB/Rec.709) unless noted.

## 1. Principles (why)

- **The best grade goes unnoticed.**
  - Tom Poole (Company 3): "The viewer shouldn't be too conscious of the grade"; the aim is that "it feels like a moving photographic print". [src: Televisual]
  - Dado Valentic's interview headline: "Der Look muss unsichtbar sein" ("The look must be invisible"). [src: Business Punk]
  - Sonnenfeld: "I try to keep my color grading fairly simple"; the palette and contrast were largely set in camera. [src: Creative Cow, Frankenstein]
  - Agent reading: build the look into art direction (palette, lighting, materials). Use the grade to unify, not to rescue.
- **Separation matters more than saturation.**
  - Cullen Kelly: "Many times we feel we need more saturation when what we're really after is more separation, or contrast between colors."
  - Fix 1–2 hues locally instead of raising global saturation.
  - "Cinematic" should cue "global adjustments, not local ones".
  - "Skin is the most important element of any frame" it occupies.
  - "The eye is more sensitive to contrast than color."
  [src: Frame.io, Kelly]
- **Colour as story, used structurally rather than decoratively.** Ian Vertovec on Fincher films: [src: Fincher Analyst, Vertovec's notes]
  - *The Social Network*: "avoid a neutral look": "interiors warmer (not pink though)… exteriors cooler".
  - *Dragon Tattoo*: "icy environments as oppressive as possible", against a "summertime flashback Kodachrome look".
  - *Gone Girl*: white balance starts neutral and *drifts* as the story twists.
  - Agent reading: tie each look to a story state and change it at structural cuts. Do not run one preset over the whole piece.
- **Look first, or balance first? Both approaches are valid.**
  - Look first: Poole builds a LUT per show and a loose "colour bible" in the first session. [src: Televisual] Kelly sets the look at timeline level and grades shots underneath it. [src: Frame.io grain article]
  - Balance first: other colourists balance first, because a look sitting on an unbalanced shot means each clip "will inherit a slightly different error". [src: ProVideo Coalition "Together or apart"]
  - For CG, where every shot is already balanced by construction, look-first is natural.
- **Contrast comes from how gently you roll off the ends.** Volpatto sets the main tone mapping at the look level and adds gain for contrast. How gently you push the whites and blacks decides whether the image reads soft or hard. [src: Lowepost Volpatto thread]

## 2. Techniques with numbers

### Print-film tone curve: the "film look" is mostly the curve
- A colour print film has a mid-scale gamma of about **3.0**, which is higher in contrast and density range than the negative. [src: SMPTE Journal 102/12]
- The *toe* of the print compresses highlights further, on top of the negative's shoulder. That double rolloff is why film highlights never clip harshly. [src: SMPTE Journal]
- Kodak 2383 has about 13 stops of contrast range (2393 Premier has about 18). Its higher D-max gives "improved black". Its matched toes give "more neutral highlights". [src: ASC Apr 2005; Kodak 2383 page]
- Emulation recipe [conv]:
  - an S-curve in log space;
  - toe: lift black to 0.01–0.03, with the toe spanning the lowest ~15% of input;
  - shoulder: begin rolling off around 0.75–0.8 output and reach 0.97–0.99 asymptotically, so pure white is rare;
  - mid-grey 0.18 linear maps to ~0.40–0.45 display;
  - slightly desaturate as luminance passes the shoulder, so highlights go toward white rather than to clipped primaries.
- For CG, the view transform *is* the print emulation. [src: ACES docs; Blender Artists AgX thread]
  - Per-channel curves (old ACES, Filmic) skew bright saturated colours, for example blue toward magenta ("Notorious Six" hue skews).
  - ACES replaced its blue-light LMT with Reference Gamut Compression.
  - ACES 2.0 tone-maps only the J (lightness) component, preserving hue.
  - Agent reading: tone-map luminance (or use a hue-preserving transform), then roll off chroma separately.

### Lift / gamma / gain and saturation ranges for mood [conv]
| Mood | Black point | Mid adjustment | Highlights | Saturation |
|---|---|---|---|---|
| Neutral / "commercial clean" | 0.0–0.02 | 1.0 | 0.95–1.0 | 1.0 |
| Warm nostalgic | 0.03–0.06, warm toward 40–60° hue | +0.03 | 0.92, cream | 0.8–0.9; highlights −20% |
| Cold clinical / thriller | 0.01–0.03, toward cyan 190–210° | −0.03 | cool white | 0.6–0.8; skin protected |
| High-energy pop | 0.0 | +0.02 | 1.0 | 1.1–1.25, but only on 1–2 hues |
| Melancholy / bleach-ish | 0.02 | −0.05 | 0.9 | 0.4–0.6; contrast +15% |

- Saturation by luminance band: most pro looks keep the most saturation in the midtones. Reduce it by about 20–40% in deep shadows (below 0.1) and in highlights (above 0.85). [conv; consistent with print behaviour per SMPTE]
- Split-toning:
  - push shadows and highlights in roughly opposite hues, at low chroma (a 2–6% shift);
  - classic pairs are cyan/teal shadows (~190°) with amber highlights (~45°);
  - also green-yellow shadows with magenta-neutral highlights for "sickly".
  - Leave skin (~20–50° hue) unmoved in hue. [conv]

### Skin and memory colours
- Van Hurkman's ranges: Caucasian skin around 35–40% saturation, on or about 2° off the skin-tone ("I") line. Darker skin is lower; sources conflict at 10–25%. [src: Larry Jordan, after the Color Correction Handbook]
- On a Rec.709 vectorscope the skin line sits at about 123°. [conv]
- Teal-orange became a cliché after the first fully digital-intermediate (DI) feature (*O Brother*, 2000). Overused, it "pushes skin tones so orange". [src: Todd Miro via DIYPhotography; Adobe]
- Valid alternative: complementary separation from the *set* (wardrobe, background) instead of from the grade.

### Specific looks
- **Bleach bypass / silver retention:** the bleach step is skipped, so silver stays with the dyes. The result has more contrast, less saturation, harsher grain and less latitude, and is usually paired with 1 stop of underexposure. [src: Wikipedia, Bleach bypass] Technicolor's ENR/OZ and Deluxe's ACE/CCE allow *partial* retention. [src: same]
  - Emulation: overlay a contrast-boosted monochrome copy at 30–60% (soft-light or multiply-luma), then desaturate 30–50%. [conv]
- **Cross-process (E-6 in C-41):** strong contrast, yellow-green casts, "often cooler shadows with warmer highlights", unpredictable. Emulate by multiplying or screening individual channel densities. [src: Photrio; UniquePhoto Q&A; Martin Evening tip]
- **Day for night:**
  - underexpose 1.5–2.5 stops;
  - pull highlights down first;
  - desaturate, especially greens and yellows;
  - shift toward blue/teal (not pure blue);
  - darken the sky with a gradient or key;
  - protect practical lights with a luma key so they stay warm and bright.
  [src: tryuncle Resolve tutorial; Mixing Light workflow; Wikipedia]
  "Blue alone isn't enough": without contrast shaping it reads as day with a blue filter. [src: Chapterz]
- **Wes Anderson pastel:** the pastels come from production design, a "wedding cake or ice cream parlor" palette. [src: KCRW, Stockhausen] Colourist Jill Bogdanowicz graded each time period differently (1930s "de-saturated or faded"): "if the color pulls viewers out of the story, I haven't done my job". [src: Sherwin-Williams STIR; Lowepost case study]
  - Agent reading: set palette in assets; the grade only sets the period.
- **Fincher look:** low saturation; warm/cool splits by location; a slow white-balance drift as story device. [src: Vertovec notes] The "green-yellow" label is a fan generalisation, not a documented spec. [src: howtofilmschool]
- **Kinescope / black halation:** Vertovec built a "black halation" for an East-German ORWO B&W look. [src: Vertovec notes]

### Halation, bloom, grain, vignette
- **Halation physics:**
  - light reflects off the film back and re-exposes mostly the *red* layer, which gives a red halo;
  - very bright sources also reach green, which shifts the halo's near edge toward orange;
  - the remjet anti-halation backing keeps halos small;
  - tungsten sources halate strongly and LEDs barely do.
  [src: Dehancer]
- **Halation settings:**
  - threshold only the brightest sources;
  - small radius: halos look larger on 16 mm and are tiny on 65 mm;
  - hue from red at the far edge to orange near the source;
  - "small halos… natural; large, obvious halos… a flaw";
  - balance it against bloom, which is usually more visible.
  [src: Dehancer]
  Starting values: threshold at linear ≥ 1.0 (scene white) [conv]; radius 0.2–0.6% of frame height for a 35 mm feel [conv]; tint around (1.0, 0.35, 0.1) [conv].
- **Grain:**
  - "Grain should be felt rather than seen";
  - apply at timeline level, *before* the look, and keep it uniform across shots as "a unifying element";
  - final opacity is "around 30%";
  - add a slight blur before the grain;
  - coarse grain is reserved for 16 mm and Super 8 looks;
  - Kelly does not use grain on every project;
  - grain on a flat digital-feeling image feels "slapped on".
  [src: Frame.io, Kelly]
  Control strength mainly through grain *size*. [src: Noam Kroll]
  Delivery compression can erase subtle grain. [src: same search; check the final encode]
- **Vignette** [conv; no primary number found]:
  - an invisible vignette is a 0.2–0.5 stop corner falloff with a radius larger than the frame and a very soft edge;
  - if the viewer notices it, it is an effect, not a grade.
  - Avoid on graphic and type-led frames, where it greys out flat fills.

## 3. Grading animation, CG and graphic design-led motion
- **CG:** the renderer already controls light, so the grade's job is the view transform (tone curve plus gamut handling) and unifying shots. Most "CG look" complaints come from a per-channel view transform that clips and skews hues, not from colour choice. [src: ACES RGC docs; AgX thread]
- **Graphic and design-led motion:** colour *fidelity* is the grade.
  - Brand and palette hexes should come out of the pipeline unchanged.
  - Use no tone curve, no split-tone and no vignette. At most, grain at ≤ 2–4% amplitude and a little bloom on true emissive elements. [conv]
  - Pick a contrasting valid approach per piece: flat and exact (Swiss/editorial), or "printed" (paper texture, slight misregistration, ink-density curve). Do not combine both.
- **Mixed pieces** (type over 3D): grade the 3D layer, then composite type *after* the grade, so white type stays white. [conv]

## 4. What separates studio work from amateur work
- **The look comes from art direction and the curve, not from a LUT preset.** [src: Poole; Sonnenfeld; Kelly]
- **Highlights roll off and blacks have density without being crushed.** [src: SMPTE; Kodak]
- **Separation of 2–3 controlled hues** rather than global saturation. [src: Kelly]
- **Skin and memory colours stay plausible inside stylised looks.** [src: Van Hurkman via Jordan]
- **Shot-to-shot consistency**, with grain and texture applied uniformly. [src: Kelly]
- **Looks change with story state** (Social Network, Gone Girl). [src: Vertovec]

## 5. Anti-patterns
- A LUT applied before balancing: it can clip shadows or highlights, and the lost detail does not come back. [src: C-I Studios; Creator Essentials]
- Crushed blacks used to fake contrast; clipped highlights; banding. [src: same]
- Teal-orange by default; orange skin. [src: Miro via DIYPhotography]
- Global saturation above 1.3, and neon highlights where the shoulder should desaturate them. [conv; SMPTE]
- Visible vignette on every shot; grain changing from shot to shot; grain that is noticeable at all. [src: Kelly]
- Large, obvious halation or bloom on everything (the "Dehancer default"). [src: Dehancer]
- A blue tint as the whole of day-for-night. [src: Chapterz]
- Grading brand colours or flat UI and type layers.
- One preset over the whole piece with no colour change at structural moments.

## 6. How an agent can check its own work (on rendered stills/strips)
1. **Range:** luminance percentiles per shot [conv]:
   - p0.5 should be ≥ 0.01 unless crushing is intended, and less than 0.5% of pixels at exactly 0;
   - p99.5 ≤ 0.98, and less than 0.3% of pixels at exactly 1.0 (except emissive or type);
   - flag banding: fewer than 40 distinct luma codes across a smooth gradient region.
2. **Saturation profile:** mean chroma in luma bands below 0.1, 0.1–0.85 and above 0.85. Midtones should carry the most chroma; flag if the highlight band exceeds the mid band.
3. **Separation:** build a hue histogram (36 bins, weighted by chroma). Good looks show 2–3 dominant peaks with clear gaps. A flat spread means mush; a single peak is a tint, which is valid only by intent.
4. **Skin** (when present): mean hue within ±5° of the ~123° vectorscope I-line, and saturation within the target range.
5. **Brand fidelity:** for design-led layers, ΔE2000 between the spec hex and the rendered pixel. Aim below 2 (imperceptible), and never above 5.
6. **Consistency:** mean luma and mean a*/b* per shot across a scene. Flag jumps above 0.05 luma or above 4 ΔE at cuts that are *not* meant as look changes.
7. **Story mapping:** list look changes against the structural cut list. There should be at least one deliberate shift in pieces longer than 20 s.
8. **Effects visibility:** compare a 100% crop with and without grain and halation. If the difference is obvious at normal viewing size, halve it. Check grain again after the final encode.
9. **A/B against "no grade":** if the ungraded render is preferred more than half the time, the grade is not earning its place.


## 7. Gaps against Anikit (TASTE)

- **Design-led pieces want no grade.** For graphic work, faithful colour *is* the grade: brand colours within ΔE2000 ≈ 2 of the spec, with no curve, split-tone or vignette, and type composited after the look. `post` presets are opt-in, which is right. The docs could say so explicitly, so agents don't add `cinematic` to flat design by habit.
- **The film look lives in the tone curve.** `scene3d` offers `neutral` and `aces` tonemaps. A hue-preserving filmic curve (AgX-like) with a soft toe and shoulder would give CG the double highlight rolloff of print film, without the magenta skew that per-channel curves put on bright blues.
- **Halation and grain have physics:**
  - halation is red at the far edge and orange near the source, and small;
  - grain should be felt, not seen (around 30% opacity), uniform across shots, and applied before the look.
  - `post.halation` and `post.grain` exist. Worth checking that the halation falloff is red-shifted and that grain sits before the grade in the pipeline (POST/SHADER owners).
- **Grade drift across shots** (a mean Lab jump between consecutive shots with no scripted change) is an editors' complaint about AI ads. The render review samples every cut, so it could flag drift as a tip.
- **Colour as a story device:** a grade that changes with the arc (warm interiors and cool exteriors; white balance drifting as the story turns) is possible per scene with `post` overrides. A recipe could show one.

## 8. Sources
- Poole: https://www.televisual.com/news/the-art-of-the-grade-tom-poole/
- Valentic: https://www.business-punk.com/?p=78615 ; https://mixinglight.com/color-tutorial/colorist-podcast-ep-022-dado-valentic
- Sonnenfeld: https://creativecow.net/frankenstein-graded-with-davinci-resolve-studio/ ; https://www.company3.com/?p=46487
- Kelly on separation/contrast/skin: https://blog.frame.io/?p=18573
- Kelly on grain: https://blog.frame.io/2023/08/14/how-a-pro-colorist-uses-film-grain/
- Balance vs look: https://www.provideocoalition.com/together-or-apart-organizing-the-fusion-of-style-and-balance/
- Volpatto: https://lowepost.com/insider/courses/hollywood-colorist-walter-csi-about-his-color-grading-process-r48/
- Vertovec / Fincher notes: https://thefincheranalyst.com/2019/03/18/colorist-ian-vertovecs-instagram-notes-on-his-work-for-david-fincher/ ; https://mixinglight.com/color-tutorial/colorist-podcast-ep-020-ian-vertovec/
- Fincher-look generalisation: https://howtofilmschool.com/dictionary/fincheresque/
- Peter Doyle (print-emulation LUTs in log): https://studiodaily.com/2006/10/color-supervisor-peter-doyle-on-harry-potter
- Kodak 2383: https://www.kodak.com/en/motion/product/post/print-films/vision-color-2383-3383/
- Print gamma ~3.0, toe compression: https://journal.smpte.org/periodicals/SMPTE%20Journal/102/12/6/07238748.pdf
- 2383 vs 2393 stops: https://theasc.com/magazine/april05/conundrum2/image6.html
- ACES RGC / blue light: https://acescentral.com/knowledge-base-2/crushed-colors-caused-by-brightly-colored-lights/ ; https://docs.acescentral.com/rgc/guides/rgc-user/ ; https://docs.acescentral.com/system-components/output-transforms/
- AgX / Filmic hue skew: https://blenderartists.org/t/agx-changes-hue/1527123?page=2 ; https://blenderartists.org/t/feedback-development-filmic-baby-step-to-a-v2/1361663?page=105
- Halation: https://blog.dehancer.com/articles/halation/ ; https://en.wikipedia.org/wiki/Remjet
- Grain: https://noamkroll.com/why-most-film-grain-looks-fake-and-how-to-achieve-better-results-in-post-production/
- Skin tones (Van Hurkman values): https://larryjordan.com/articles/the-secret-to-setting-skin-colors-accurately/
- Teal & orange: https://www.diyphotography.net/reasons-orange-teal-look-popular-movies/ ; https://www.adobe.com/creativecloud/video/hub/features/why-use-orange-teal-grading
- Bleach bypass / ENR: https://en.wikipedia.org/wiki/Bleach_bypass
- Cross-process: https://www.photrio.com/forum/threads/why-cross-process-slide-films-for-night-scenes.67713 ; https://www.uniquephoto.com/community/qa/what-does-cross-processing-slide-film-do-and-is-it-useful-for-conventional-photography ; https://www.artiestick.com/cm/art/artists/martinevening/tips/cross.html
- Day for night: https://tryuncle.com/learn/davinci-resolve/davinci-resolve-day-for-night-color-grading-tutorial ; https://mixinglight.com/color-tutorial/day-for-night/ ; https://en.wikipedia.org/wiki/Day_for_night ; https://chapterzmagazine.com/shooting-day-for-night-why-most-modern-filmmakers-still-get-it-wrong-1pnp
- Grand Budapest: https://www.sherwin-williams.com/architects-specifiers-designers/inspiration/color-conversation/SW-ART-STIR-WES-ANDERSON ; https://lowepost.com/color-grading/case-studies/grand-budapest-hotel-r19 ; https://kcrw.com/culture/shows/design-and-architecture/adam-stockhausen-on-creating-the-vibrant-world-of-the-grand-budapest-hotel
- Grading mistakes: https://c-istudios.com/common-color-correction-mistakes-and-how-to-avoid-them-in-tv-and-film ; https://www.creatoressentials.com/glossary/color-grading/
