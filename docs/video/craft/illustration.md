# Illustration craft: from clip-art to studio-grade

Research notes on why illustrated motion work looks studio-made or looks like generic "AI slop", and what Anikit Video should build next. Read this with [`../taste/TASTE.md`](../taste/TASTE.md), which covers general taste. This file covers illustration and 2D or hybrid animation only.

Numbers marked **(rule of thumb)** come from common studio and teaching practice and are starting values, not laws. Other numbers are linked to their source.

---

## 0. The diagnosis in one paragraph

Generic illustration is what you get when nobody makes decisions. Every shape is the same weight, every colour is mid-saturation, the light comes from nowhere, every edge is a perfect bezier, and everything moves on ones with the same ease. Critics of Corporate Memphis say it plainly: it is flat, bright, solid colour, "cartoon-like figures with disproportionate features", and the figures "lack defining characteristics" ([Wikipedia](https://en.wikipedia.org/wiki/Corporate_Memphis), [Marketplace](https://www.marketplace.org/story/2021/04/15/a-primer-on-corporate-memphis-big-techs-favorite-design-trend)). It spread because stock libraries such as unDraw and Freepik let non-illustrators produce it cheaply ([Eye on Design](https://eyeondesign.aiga.org/what-the-think-pieces-about-corporate-memphis-tell-us-about-the-state-of-illustration/), [Jim Nielsen](https://notes.jim-nielsen.com/n/2021-05-28-1225/)). AI output fails the same way: when details are missing, a model "falls back on patterns that appear most frequently" ([getimg](https://getimg.ai/blog/what-is-ai-slop-why-its-getting-worse)), and slop is "what happens when nobody makes enough decisions" ([Creativepool](https://creativepool.com/bilder-studio/news/ai-slop-is-a-taste-problem-not-a-technology-problem.35112)). Flat style is not the problem. Flat art has a long, rich history ([Eye on Design](https://eyeondesign.aiga.org/what-the-think-pieces-about-corporate-memphis-tell-us-about-the-state-of-illustration/)). The problem is defaults. Studio work is a stack of deliberate, slightly wrong-looking, consistent decisions, and the engine's job is to make each of those decisions cheap to state in JSON.

---

## 1. What the studios actually do

| Studio / film | Signature decisions worth stealing |
|---|---|
| **Spider-Verse** (Sony Imageworks) | Animation is "stepped", mostly on twos. In the studio's words, "Animating on 2s doesn't mean mathematically every other frame… More often than not, it's 12 fps, but it's a fluid scale" ([VFX Voice / SideFX](https://www.sidefx.com/community/spider-man-into-the-spider-verse/)). Comp tools replace optical effects with print effects. A halftone tool lets artists set dot size, angle and spacing, and dot density tracks the shading. A hatch tool sets thickness from shading. **ChromaShifter** replaces depth-of-field blur with colour offset ([Foundry](https://www.foundry.com/insights/film-tv/graphic-look-in-comp-spiderman)). Halftone on rim lights is sometimes locked to the character and sometimes loose, or a swinging character seems to be "swimming through a pattern" (same source). In *Across*, rates run from ones up to 6s, and Spider-Punk's guitar, jacket and outline each run on different rates ([Yahoo/IndieWire](https://ca.news.yahoo.com/inside-5-mind-blowing-dimensions-130000004.html)). Ink lines are drawn on keyframes and interpolated, and **MaskToInk** renders a mask "as if made of wet ink" ([befores & afters](https://beforesandafters.com/2023/06/28/this-was-the-first-cg-animated-movie-ive-ever-heard-of-that-actually-had-a-dedicated-inking-team/), [SIGGRAPH 2023](https://history.siggraph.org/learning/linework-in-spider-man-across-the-spider-verse-by-grochola-maccari-lee-and-boulet-gilly)). |
| **Klaus** (SPA Studios) | 2D drawings are lit in **up to 8 layers**: key or shadow, ambient occlusion, SSS, rim, eye specular and bounce. Each layer is a set of drawn shapes that a line-tracking system pins to the animation. Then a separate texture pass adds a subtle grain that "travels with the character", and compositors set a different texture amount per material. Lines are kept "only where needed", so value replaces outline ([befores & afters](https://beforesandafters.com/2019/11/14/heres-what-made-the-2d-animation-in-klaus-look-3d/), [80.lv](https://80.lv/articles/klaus-look-development-simulating-3d-effects-in-2d/)). Light terms were cut down to key, bounce, ambient, occlusion and rim so the result would not look "too 3D" ([Cartoon Brew](https://cartoonbrew.com/feature-film/how-klaus-draws-on-centuries-old-artistic-principles-to-push-2d-animation-forward-182325.html)). A colour script covered about one shot in four. |
| **Arcane** (Fortiche) | 3D sets get painted matte textures projected onto them. Lines are freehand and "slightly uneven". Detail rises only where characters interact and is softened elsewhere ([80.lv](https://80.lv/articles/arcane-artists-show-how-they-combine-traditional-art-3d-for-backgrounds)). Highlights and shadows are painted into textures instead of lit, and smoke and fire are 2D FX ([VFX Voice](https://vfxvoice.com/riot-games-and-fortiche-get-revolutionary-with-arcane-season-2/), [RedShark](https://redsharknews.com/why-netflixs-arcane-looks-so-good-how-fortiche-ramped-up-the-animation-pipeline)). |
| **Cartoon Saloon** (*Wolfwalkers*, *Song of the Sea*) | **Line style carries meaning.** The town gets a rigid, uniform "woodblock" line. The forest and wolves get a scratchy pencil line, and the pencil and charcoal under-drawings stay in the final frames. Wolfvision is a third style ([Awards Radar](https://awardsradar.com/2021/03/04/interview-wolfwalkers-artists-on-creating-magic-between-the-lines/), [Frame.io](https://blog.frame.io/2021/05/03/made-in-frame-wolfwalkers/)). *Song of the Sea* uses loose watercolour backgrounds, Celtic swirls hidden in shells, clouds and rocks, and city backgrounds made "mainly of squares and triangles" ([Hot Press](https://hotpress.com/culture/song-of-the-sea-14592226), [Animation Obsessive](https://animationobsessive.substack.com/p/art-directing-song-of-the-sea)). |
| **Puss in Boots: The Last Wish** (DreamWorks) | The goal was a "fairy tale painting look". Frame rate is tied to story: Puss shifts to stepped twos when he is fantasising, and the wolf "brings him back to reality" ([Cartoon Brew](https://www.cartoonbrew.com/feature-film/puss-in-boots-joel-crawford-inbtwn-224324.html), [befores & afters](https://beforesandafters.com/2023/01/11/heres-how-dreamworks-lent-a-whole-new-brand-of-stylized-look-and-feel-to-puss-in-boots-the-last-wish)). |
| **TMNT: Mutant Mayhem** | "Imply teenage imperfection everywhere." Models were approved in asymmetric poses, and the asymmetry was put back after rigging. Tileable maps and bump maps were banned and materials painted instead. **Motion blur was replaced by scribbles drawn in comp**, defocus by scribble bokeh, and glows were removed as "too digital". Seven light channels each got their own treatment (scribble, cross-hatch, stepped). 2D FX were stepped ([befores & afters](https://beforesandafters.com/2023/09/07/we-wanted-to-imply-teenage-imperfection-everywhere/)). |
| **Mitchells vs the Machines** | The human world is made of "thousands of watercolour brushstrokes", with painterly outlines on CG. The 2D doodle layer ("Katie-vision") is meant to look amateurish. Painterly sims have 2D FX on top ([befores & afters](https://beforesandafters.com/2021/04/29/behind-the-early-2d-3d-test-for-the-mitchells-vs-the-machines/), [CGW](https://www.cgw.com/Publications/CGW/2021/July-August-September-2021/3D-with-a-Twist.aspx)). |
| **Disney / Eyvind Earle** (*Sleeping Beauty*) | Trees start as a gouache blob, then get "smaller and smaller leaves". Square trees sit against curvy ground cover, and the horizontal and vertical planes carry into the character design ([Gurney](https://gurneyjourney.blogspot.com/2019/02/eyvind-earle-video-bio.html), [SIGGRAPH poster](https://history.siggraph.org/wp-content/uploads/2022/12/2014-Poster-97-Murphy_Stylized-Trees-and-Landscapes.pdf)). |
| **Golden Wolf, Buck, Animade, Moross** (motion-design studios) | Cel and frame-by-frame animation (TVPaint for roughs and cleanup) over designed, rig-light shapes ([CGW](https://cgw.com/Publications/CGW/2021/Jan-Feb-March-2021/Between-Two-Worlds.aspx), [Toon Boom](https://www.toonboom.com/?p=18220), [Motionographer](https://motionographer.com/2025/05/12/studio-stories-12-years-of-golden-wolf/)). Buck made Facebook's *Alegria* guide, the source of Corporate Memphis ([Wikipedia](https://en.wikipedia.org/wiki/Corporate_Memphis)). That system worked inside Buck's own animation and became slop once it was copied without the animation, the timing or the concept. |
| **Gobelins** | Strong graphic and character design, plus character animation. Layout is often done in 3D and finished in 2D. Students are pushed toward different styles, not a house style ([WeTransfer](https://wepresent.wetransfer.com/story/pioneers-gobelins-school-france/), [Animation Career Review](https://www.animationcareerreview.com/articles/gobelins-l%E2%80%99ecole-de-l%E2%80%99images-eric-riewer-discusses-program%E2%80%99s-unique-pedagogy-and-success)). |

**The pattern.** Every one of these productions *removes* default CG or vector perfection: motion blur, soft defocus, uniform lines, smooth 24 fps motion, symmetric models and tileable texture. Then each puts back a **specific, consistent, motivated** imperfection: halftone, scribble blur, pencil line, stepped rate, painted light shapes.

### Editorial and print references

- **Malika Favre**: she fills the page, then removes material until only the essential lines remain. Negative space carries the second reading. Colour comes last and serves the story ([Wallpaper](https://wallpaper.com/design/hide-and-seek-by-malika-favre-at-kemistry-gallery-london), [CC Magazine](https://ccmagazine.es/en/interview-with-the-illustrator-malika-favre/)). In practice that is 2–4 flat colours with no outlines, and figure and ground share edges.
- **Tom Haugomat**: a cool, pastel, mid-century palette, tiny figures in large fields, and light as the subject ([Handsome Frank](https://handsomefrank.com/illustrators/tom-haugomat)).
- **Owen Davey**: a "fairly limited colour palette" per piece, geometric shapes and texture. He sketches on paper and builds in Photoshop ([Friday Illustrated](https://www.fridayillustrated.com/every-artist-self-taught-way-think-interview-illustrator-owen-davey/), [Grain Edit](https://grainedit.com/?p=10410)).
- **Jean Jullien / Andrew Footit**: a single confident black brush line with flat fills. The idea is the image: humour and "visual efficiency" ([It's Nice That](https://www.itsnicethat.com/features/review-of-the-year-2015-illustration-jean-jullien), [Eye on Design](https://eyeondesign.aiga.org/how-illustrator-jean-jullien-fights-creative-fatigue-makes-the-most-of-his-limited-skills/)). Line variation comes from the brush itself, not from added effects.
- **Risograph**: translucent spot inks overprint into new colours (pink over blue gives purple). Registration shifts **0–3 mm** between drums. Large dark blocks print blotchy. "Grain Touch" is a random diffusion dither, and halftones run around **71–106 lpi** ([Oxford Green Print](https://oxfordgreenprint.com/home/design-for-riso/), [CCS](https://campus.collegeforcreativestudies.edu/imaging-center/2021/06/22/registration-trapping/), [Kenji Liu](https://atlasofmaking.kenjiliu.com/techniques/risograph)). Shops stock different drum colours, including fluorescent pink, yellow, aqua, federal blue, mint and violet ([Utah Book Arts](https://lib.utah.edu/collections/book-arts/risograph.php), [Moniker Press](https://monikerpress.ca/colours)).
- **Screen print / offset**: screens sit at 0°, 15°, 45° and 75°. The darkest ink goes at 45°, and inks that overprint as tints should be at least 15° apart ([PrintWiki](https://printwiki.org/Traditional_Color_Angles), [FESPA](https://fespa.com/en/news-media/screen-separations-in-printing)). Garment screens run 45–55 lpi, so the dots are visible on purpose. In trapping, the darker ink overprints the spread of the lighter one.
- **Lino / woodcut**: relief printing, so carved-away areas stay paper. Lino gives smooth, fluid gouge marks. Woodcut shows wood grain. The look is strong black and white, outline cuts plus hatch and stipple gouges ([Bachelorprint](https://www.bachelorprint.com/printing-guide/printmaking/woodcut), [Hickman](https://hickmandesign.co.uk/blog/history/what-is-lino-printing)). Gouge marks *follow the form*, unlike screen-space hatching.

---

## 2. Principles, with numbers

### Palette and value

- **Palette size.** Editorial vector work uses 3–6 inks plus paper (rule of thumb). Riso and screen print use 2–4 spot inks, and overprints give 1–3 more for free. A "limited palette" exercise is 2–3 colours ([Creative Bloq](https://creativebloq.com/illustration/create-illustration-limited-colour-palette-5148613)). **Stock and AI illustration typically shows 8–15 unrelated mid-saturation hues. That alone reads as clip-art.**
- **Proportion.** About 60 / 30 / 10 dominant / secondary / accent. In greyscale this means 60% light, 30% mid and 10% dark, and the 10% must differ from both in hue, value *or* saturation ([Made Good Designs](https://madegooddesigns.com/60-30-10-color-rule/), [Studio Matrx](https://www.studiomatrx.org/students/interior-design-foundations/colour-hue-value-saturation-interiors)).
- **Value first.** Design in 3 values (notan: light, mid, dark), squint, then add hue. A frame should read in greyscale at thumbnail size. Keep the darkest dark and lightest light, about 5–10% of area, at the focal point (rule of thumb). Klaus and Arcane use a colour script, roughly one keyframe per 4 shots ([befores & afters](https://beforesandafters.com/2019/11/14/heres-what-made-the-2d-animation-in-klaus-look-3d/)).
- **Shift hue, not only value.** Shadows go cooler and more saturated, and highlights go warmer (or the reverse under cold light). Never shade by multiplying with black or adding a grey gradient. A shadow is the local colour rotated about 10–30° in hue toward the ambient colour and dropped 15–30% in lightness (rule of thumb, in OKLCH).
- **Paper is a colour.** Off-white stocks (cotton, kraft, newsprint) are part of the palette. Pure `#fff` with pure black reads as digital.

### Shape language

- Circles read as friendly or soft, squares as stable or stubborn, triangles as dynamic or threatening. Mix them per character. **Straights against curves**: every curved contour should meet a straighter one on the opposite side (classic character-design teaching, see [Creative Bloq](https://creativebloq.com/how-to/simplfy-figures-for-animation)).
- **Big, medium, small.** Proportions of about 60 / 30 / 10 again, or 1 : ⅓ : ⅑ in scale steps (rule of thumb). Corporate Memphis breaks this with uniform rubbery limbs and a tiny head that is neither big nor small, just odd.
- **Silhouette test.** Fill the character black. It must read as that character, in that pose, with the limbs separated from the body.
- **Asymmetry on purpose.** Tilt eyes 2–5°, offset one shoulder, never mirror a face perfectly (Mutant Mayhem's rule above).
- **Tangents are bugs.** Edges that just touch, such as a hand on the horizon or a head on a frame edge, flatten depth. Either overlap or separate clearly.

### Line

- **Three weights.** Thin (interior detail), medium (contours) and thick (outer silhouette, overlaps, shadow-side accents) ([Cursa](https://cursa.app/en/page/digital-drawing-on-a-tablet-clean-line-art-techniques-line-weight-tapers-and-corners), [Proko](https://proko.com/course-lesson/how-to-draw-with-line-weight/notes)). A workable ratio is about **1 : 1.6–2 : 3** (rule of thumb). The heaviest weight goes at the focal point, and weight drops with distance.
- **Thick and thin within one stroke.** Swell on the curve and taper at both ends. Line is heavier on the side away from the light, and heavier where forms overlap.
- **Line colour.** A pure-black outline over colour reads as a colouring book. Studios colour the line, darker and more saturated than the fill it bounds, or drop it entirely where value separates the shapes (Klaus).
- **Lines don't always close.** Leave 1–3 px gaps where light hits, and let lines overshoot a little at corners (rule of thumb from inking).

### Texture amount

- Grain and paper sit at **3–8% luminance amplitude** at final size (rule of thumb). Raise it to 10–15% only for a deliberate print look. The engine's paper stocks use 3–6% (`illus_fill.rs`), which is in range.
- **Texture travels with the object** (Klaus), unless the medium is the paper. Screen-locked grain says "filter". Object-locked grain says "painted".
- **Set texture per material.** Skin gets less than cloth or rock (Klaus compositors).
- Grain size should be **1–2 px at 1080p**, or the H.264 encoder smears it to mush. Check after encoding.

### Timing

- **Ones, twos, threes.** At 24 fps, twos is 12 drawings a second and threes is 8. Spider-Verse used twos for most character action, ones for fast camera-relative motion, and up to 6s for snappy pose-to-pose ([Yahoo](https://ca.news.yahoo.com/inside-5-mind-blowing-dimensions-130000004.html)). **Rule:** camera moves and pans stay on ones, even when characters step, or the background strobes.
- **Mixed rates per part** (Spider-Punk). Body on 3s with the outline boiling on 2s reads as hand-made. A whole frame on one rate reads as a "low-fps filter".
- **Boil.** Redraw jitter at **4–12 fps**, about 8 fps typical ([PremiumBeat](https://www.premiumbeat.com/blog/how-to-create-line-boil-animations)). A cycle of 3–4 redraws is enough ([Medium](https://medium.com/the-inspired-animator/how-to-apply-a-hand-crafted-look-to-your-animations-in-after-effects-2da1c5dbd282)). Amplitude is 0.5–1.5 px at 1080p for "alive", and 2–4 px for "Squigglevision".
- **Holds.** A pose needs about 6–8 frames (¼–⅓ s) to read and 12+ to land a beat. A *moving hold* drifts 2–5% toward a more extreme version of the pose instead of freezing ([Animation Mentor](https://animationmentor.com/blog/why-all-animators-need-to-master-the-moving-hold), [AnimSchool](https://blog.animschool.edu/tag/moving-holds/)). In stepped work, hold the last drawing 2–4 frames longer than the timing suggests.
- **Smears and multiples.** Distort the subject across **1–2 frames** between keys, or duplicate it, as in Looney Tunes multiples. These replace motion blur ([Wikipedia](https://en.wikipedia.org/wiki/Smear_frame), [AnimSchool](https://blog.animschool.edu/?p=608)). Mutant Mayhem used scribbles in place of blur.
- **Squash and stretch.** Keep area constant: `sx·sy = 1`. At impact, squash to about 0.7–0.8 by 1.25–1.4 for 1–2 frames, then overshoot to 1.1 and settle (rule of thumb, Williams-style).
- **Anticipation and overshoot.** Anticipation takes about ⅓ of the main action's duration, in the opposite direction, at 10–20% of the amplitude. Overshoot is 5–15%.
- **Secondary motion.** Hair, cloth, ears and leaves lag the primary by 2–4 frames and settle 4–8 frames later. If the secondary action is more noticeable than the primary, it is wrong ([Prolific](https://prolificstudio.co/blog/secondary-action-in-animation/)).

---

## 3. Techniques by subject

### Characters
- Build from 3–5 primitive masses (big, medium, small), then cut negative space (Favre). Give each figure **one** exaggerated trait, not a generic stretch of every limb.
- Light in layers, Klaus-lite: base fill, **one** hard-edged shadow shape (local colour hue-shifted), **one** rim shape on the side away from the key, an optional bounce from the ground colour, and a tiny cheek or nose warm-up at the terminator.
- Faces: specular dots in the eyes and lids that overlap the iris. Mouths are shapes, not lines.
- Line follows meaning: clean for "civilised", scratchy for "wild" (Wolfwalkers).

### Environments
- 3–5 depth planes. Atmospheric perspective: each plane back moves toward the sky colour, loses about 20–30% contrast and saturation, and loses line weight (rule of thumb).
- Detail falls off from the focal point. Arcane softens everywhere characters don't interact.
- Geometric stylisation should be consistent: Earle's square trees, *Song of the Sea*'s squares and triangles in the city, Celtic swirls hidden in natural forms.

### Plants
- Earle method: a big silhouette blob, then smaller clusters, then a few individual leaves on the lit edge only. Never draw every leaf at the same size.
- Plants sway as secondary motion. Phase-offset each cluster by its distance from the root. Bend from the base with a 1–3° rotation, a 2–4 s period, and noise on top.
- Use 3 greens at most (light, mid, dark) plus a warm leaf accent.

### Clouds and sky
- Clouds are a few designed shapes: a flat bottom, a lit top made of 3–7 bumps of different sizes, a shadow shape on the underside in the sky's complementary colour, and a rim on the edge toward the sun.
- They drift as parallax planes, never as blur. For a painted sky, use 2–3 colour bands with dithered or grained transitions. A smooth vector gradient gives the "stock gradient" tell.

### Water
- Water is designed highlights, not a gradient. Use 2–3 values, highlight shapes such as lozenges, zig-zags or scallops that cycle at 6–8 fps, and reflections as broken, stepped horizontal slices. Gilland's *Elemental Magic* treats splashes, ripples and waves as hand-drawn shape families ([Routledge](https://www.routledge.com/Elemental-Magic-Volume-I-The-Art-of-Special-Effects-Animation-1st-Edition/Gilland/p/book/9780240811635)).
- Splashes are drawn on twos, and droplets break into **shapes with lines of action**, not particles.

### Light
- **Rim light on flat shapes.** Use a thin offset copy of the silhouette (inset 2–6 px toward the light), clipped to the shape, in a warm tint. Halftone the rim when it should read as light rather than texture (Spider-Verse).
- **Cast shadows are shapes.** Draw a hard-edged polygon in the ground's hue-shifted colour at 60–85% strength (multiply), then add contact darkening where the object meets the ground. A blurred drop shadow is the strongest single "UI, not illustration" tell.
- **Gradient plus grain.** A two-stop gradient no longer than about 30% of the shape, dithered or stippled so it never bands, with grain on top. A pure vector gradient reads as 2014-era flat design.
- **God rays and glows** are drawn shapes at low opacity with an additive or screen blend, sometimes halftoned. Soft bloom reads digital (Mutant Mayhem removed glows).

### Texture
- **Grain overlay**: multiply or overlay, 3–8%, object-locked, with a different seed per object.
- **Halftone**: dots whose size follows the shading, at angle 15° or 45°. Coarse (8–20 px cells at 1080p) for pop style, fine for print style.
- **Misregistration**: each ink plane offset 1–4 px in an independent direction (riso shifts 0–3 mm on paper), optionally wobbling by a pixel per boil frame. Overprints multiply.
- **Edge breakup**: rough the vector edges with 0.5–2 px noise at a scale of 3–10 px, so they look like ink on paper, especially on large shapes.

---

## 4. What makes vector work hand-made, not clip-art

| Clip-art / AI default | Hand-made equivalent |
|---|---|
| Perfect bezier edges, uniform stroke | Edge breakup, pressure swell and taper, overshoots, small gaps |
| 8–15 unrelated hues, pure white ground | 3–6 inks plus a paper colour, overprints, a 60/30/10 split |
| Gaussian drop shadow, glow, blur | Hard cast-shadow shapes, drawn rim, halftone or scribble for blur and defocus |
| Linear gradient fills | Hard-edged value steps, or dithered, stippled or hatched gradients |
| Symmetric faces and poses | Deliberate asymmetry, a line of action through every pose |
| Every element at full detail | Detail falls off from the focal point. Silhouettes simplify with distance |
| Generic people with stretched limbs | Specific characters with one exaggerated trait. Shapes chosen from personality |
| Texture as one screen-space overlay | Texture per object and per material, moving with it |
| Smooth 60 fps tween, same ease everywhere | Stepped poses, holds, smears, mixed rates, eases chosen per action |
| Decoration fills the empty space (blobs, leaves, sparkles) | Negative space is used. Each element earns its place |
| Perfect registration of line and colour | Small offsets between line and fill. Colour sometimes misses the line |

---

## 5. Motion techniques specific to illustration

1. **Stepped timing** per layer (twos or threes) and per part, with the camera on ones.
2. **Line boil**: re-seed the jitter at 4–12 fps with a 3–4 drawing cycle. Boil the *fill edges and textures* too, not only strokes.
3. **Shape morphs** between designed poses (cut-paper or flat style). Match vertex counts, ease between them, and put the morph *on twos*. A tweened smooth morph reads as vector software, and a stepped one reads as drawn.
4. **Smears** in place of motion blur: stretch along the velocity for 1–2 frames, add multiples for repeated actions, and use speed lines or scribbles.
5. **Squash and stretch** that preserves area, triggered by velocity and impact.
6. **Secondary and overlapping action**: a chain of lag springs on child parts such as hair, ears, leaves and scarves.
7. **Parallax and multiplane**: Disney's multiplane camera used up to 7 glass planes, the far ones moving slowest ([Wikipedia](https://en.wikipedia.org/wiki/Multiplane_camera)). A practical speed ratio is far 0.1–0.3×, mid 0.5×, focal plane 1× and foreground 1.5–2.5× (rule of thumb). Blur the foreground with a *graphic* treatment (flat dark silhouette or halftone), not Gaussian.
8. **2.5D camera**: flat cards in 3D, with a slow push of 2–5% scale over a shot, or a truck move. Painted projection (Arcane) gives a painting depth on a camera move. Characters stay stepped while the camera moves on ones.
9. **Cut-paper / cut-out animation**: rotate pieces at their pivots, no deformation, stepped on twos or threes. Drop shadows lift and offset when a piece "lifts".
10. **Stroke-on reveals** in drawing order (outline, then fill, then detail), not all strokes at once.
11. **Holds with life**: boil plus a moving hold, so a held frame is never dead.
12. **Frame rate as storytelling**: switch a character from twos to ones at the emotional peak (Puss in Boots, Miles).

---

## 6. Gap list for Anikit Video, ranked by impact

What exists today: brush strokes (`ink/dry/marker/pencil`, pressure presets, stroke-on, `boil`), `sketch`, hatch, stipple and scribble fills, paper stocks (`cotton kraft canvas watercolor newsprint`), `cutout` (torn and cut paper with lift), shape layers with gradients, `shadow`, `glow` and `blur`, post `grain` and `aberration`, `steps(n)` easing, and shader style filters (`halftone risograph linocut watercolour crosshatch screenprint print-offset dither`) that work on the **whole composite**.

| # | Gap | Why it matters | One-line implementation idea |
|---|---|---|---|
| 1 | **Per-layer frame rate ("on twos")** | The biggest single tell of studio 2D. Today everything moves on ones, or `steps()` quantises one tween, not the layer clock | `"fps": 12` or `"on": 2` on any layer or group: sample its animation time at `floor(t·fps)/fps`, camera excluded. Plus `"rateAt"` keyframes to switch rate at story beats |
| 2 | **Lit flat shapes (shadow, rim and cast-shadow shapes)** | Turns flat fill into Klaus or Favre lighting without 3D | `"light": {"dir": 135, "shadow": 0.25, "rim": "#ffd9a0"}` on shape groups: offset-and-clip the silhouette for the shadow and rim, hue-shift in OKLCH, plus a `castShadow` shape projected onto a ground line |
| 3 | **Palette and ink system with overprint** | Fixes the 15-hue stock look in one place | `"inks": ["#ff48b0", "#0078bf", "#ffe800"]` at video level: every fill snaps to the nearest ink or ink overprint (multiply), and a validator warns above 6 inks |
| 4 | **Per-layer print look (halftone, misregistration, grain locked to the object)** | Today's filters are global screen-space. Studios lock texture to the object and vary it per material | `"print": {"halftone": 10, "angle": 15, "misregister": 2, "grain": 0.05}` per layer, rendered in layer space before compositing. Plates come from the ink split in #3 |
| 5 | **Edge breakup on vector fills** | Perfect beziers are the clip-art tell. Brushes fix strokes but not fills | `"edge": {"rough": 1.2, "scale": 6, "boil": 8}`: displace flattened fill outlines with seeded noise, re-seeded at the boil rate |
| 6 | **Dithered or textured gradients** | Smooth gradients band in H.264 and read as stock | `"gradient": {..., "texture": "stipple"/"grain"/"bayer"/"hatch"}`: threshold the gradient ramp against a noise or pattern field |
| 7 | **Smears and multiples from velocity** | Replaces motion blur, which reads as CG | `"smear": {"stretch": 1.5, "multiples": 3}` when speed exceeds a threshold: scale along velocity for 1–2 stepped frames, or draw ghost copies with falling opacity |
| 8 | **Squash and stretch plus secondary lag** | Gives life without hand keyframing | `"squash": true` (area-preserving scale from acceleration) and `"follow": {"parent": "body", "lag": 3, "spring": "wobbly"}` for child parts |
| 9 | **Pose morph between shapes** | Cut-paper and flat character animation | `"morph": ["d1", "d2", "d3"]` with vertex resampling by arc length, played on the layer's stepped rate |
| 10 | **Parallax planes and a 2.5D camera preset** | Depth with no 3D modelling | `"plane": 0.3` on layers (camera offset × plane), with a `"multiplane"` camera preset that pushes 3% and pans |
| 11 | **Graphic defocus and blur** (halftone, scribble, chroma-shift) | Spider-Verse and Mutant Mayhem replaced optical blur | `"defocus": {"style": "chroma"|"halftone"|"scribble", "amount": 0.6}` per depth plane in place of Gaussian `blur` |
| 12 | **Hatch and stipple driven by light** | Today hatch is a fill. It should follow shading | Make `hatch` and `stipple` density a function of the shading from #2 (dark side dense, lit side empty), with direction from the shape's long axis |
| 13 | **Line styles by world and line colour** | Cartoon Saloon meaning-through-line. Black outlines read as a colouring book | `"lineStyle"` theme tokens (`clean`, `woodblock`, `pencil`) applied to every shape outline. Line colour defaults to the fill darkened in OKLCH, not `#000` |
| 14 | **Linocut and woodcut gouge fill that follows form** | The current linocut filter is screen-space | A gouge-stroke fill along the shape's medial axis or flow field, with tapered white cuts and an optional wood-grain displacement |
| 15 | **Designed water, cloud and foliage generators** | Environments are where generic shows most | Components `cloud` (bumps, shadow, rim), `foliage` (Earle blob plus leaf clusters plus lit-edge leaves) and `water-glints` (shape cycles at 6–8 fps), all palette-aware |
| 16 | **Texture per material and object-locked boil of paper and texture** | Klaus "grain travels with the character" | Give each layer a texture seed and a UV origin that follows its transform. Optional `"texture": {"amount": 0.04}` per layer |
| 17 | **Illustration lint (`still --lint illus`)** | Agents need feedback on taste, not only syntax | Count hues, check greyscale value spread and contrast at the focal point, flag Gaussian drop shadows, pure black lines, unused negative space and uniform stroke width |

---

## 7. How agents should use this today

- Pick a reference first (riso zine, Favre poster, Cartoon Saloon forest, Spider-Verse comic), then pick 3–5 inks and a paper. Write them down in `theme.colors`.
- Use brushes with `taper` or `swell` pressure for every outline. Use `boil` 8 and amount about 1 on everything hand-drawn, including still frames.
- Replace `shadow: true` on illustrated objects with a hand-placed darker shape. Replace `blur` with a halftone or risograph shader filter on that group.
- Use `steps(n)` eases on character tweens until per-layer frame rate exists. Keep camera moves smooth.
- Finish with one global print filter at low strength *or* paper grain, not both stacked high.

---

## Checklist for a studio-grade illustrated shot

1. One named reference style, and one sentence saying why this style fits this story.
2. 3–6 inks plus a paper colour. 60/30/10 split. Reads in 3 values in greyscale at thumbnail size.
3. Silhouettes read in solid black. Big, medium and small shapes, straights against curves, one deliberate asymmetry.
4. Lines in 3 weights with pressure taper, coloured, not pure black. Gaps where light hits.
5. Light is designed: one shadow shape, one rim, hard cast shadows. No Gaussian drop shadows or glows.
6. Texture is 3–8%, locked to objects, varied by material. Gradients are dithered or stippled, never smooth.
7. Characters on twos or threes, camera on ones. At least one rate change or hold timed to the story.
8. Smears or scribbles instead of motion blur. Area-preserving squash. Secondary parts lag 2–4 frames.
9. Something is always alive: boil at 4–12 fps, moving holds, swaying plants. No frame is fully static.
10. Depth from 3–5 parallax planes with atmospheric fade. Detail falls off from the focal point, and negative space is used.
