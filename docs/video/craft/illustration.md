# Illustration craft: from clip-art to studio-grade

Research notes on why illustrated motion work looks studio-made or looks like generic "AI slop", and what Anikit Video should build next. Read this with [`../taste/TASTE.md`](../taste/TASTE.md), which covers general taste. This file covers illustration and 2D or hybrid animation only.

**How to read the advice.** Nothing here is a law. Each tendency says what usually happens and why, and when to choose otherwise. Every "tell" listed below is a valid choice in some style. A held still frame is Saul Bass. A smooth gradient is Haugomat's sky. A mirrored face is a Byzantine icon or a Wes Anderson tableau. The aim is that every choice is *made*, not defaulted. Numbers marked **(rule of thumb)** are starting values from common studio and teaching practice. Numbers marked **(observed)** are my reading of published work, not the artist's statement. Everything else is linked.

Tools named below: available now are `brush`, `sketch`, `hatch`, `paper` and `cutout` (`docs/video/components.md`), shape and `draw` layers, post `grain`/`aberration`, `steps(n)` easing, and the shader style filters (`halftone risograph linocut watercolour crosshatch screenprint print-offset dither`). **Planned, not yet built:** the `illo` vector toolkit (lit flat shapes, inks, grain) and the `landscape`, `plants`, `clouds` and `water` generators. Keys shown for planned tools are proposals.

---

## 0. The diagnosis in one paragraph

Generic illustration is what you get when nobody makes decisions. Every shape is the same weight, every colour is mid-saturation, the light comes from nowhere, every edge is a perfect bezier, and everything moves on ones with the same ease. Critics of Corporate Memphis say it plainly: it is flat, bright, solid colour, "cartoon-like figures with disproportionate features", and the figures "lack defining characteristics" ([Wikipedia](https://en.wikipedia.org/wiki/Corporate_Memphis), [Marketplace](https://www.marketplace.org/story/2021/04/15/a-primer-on-corporate-memphis-big-techs-favorite-design-trend)). It spread because stock libraries let non-illustrators produce it cheaply ([Eye on Design](https://eyeondesign.aiga.org/what-the-think-pieces-about-corporate-memphis-tell-us-about-the-state-of-illustration/), [Jim Nielsen](https://notes.jim-nielsen.com/n/2021-05-28-1225/)). AI output fails the same way: when details are missing, a model "falls back on patterns that appear most frequently" ([getimg](https://getimg.ai/blog/what-is-ai-slop-why-its-getting-worse)), and slop is "what happens when nobody makes enough decisions" ([Creativepool](https://creativepool.com/bilder-studio/news/ai-slop-is-a-taste-problem-not-a-technology-problem.35112)). Flat style is not the problem. Flat art has a long, rich history ([Eye on Design](https://eyeondesign.aiga.org/what-the-think-pieces-about-corporate-memphis-tell-us-about-the-state-of-illustration/)). The problem is defaults. Studio work is a stack of deliberate, consistent decisions, and the engine's job is to make each decision cheap to state in JSON.

---

## 1. What the studios actually do

| Studio / film | Signature decisions worth stealing |
|---|---|
| **Spider-Verse** (Sony Imageworks) | Animation is "stepped", mostly on twos. In the studio's words, "Animating on 2s doesn't mean mathematically every other frame… More often than not, it's 12 fps, but it's a fluid scale" ([SideFX](https://www.sidefx.com/community/spider-man-into-the-spider-verse/)). Comp tools replace optical effects with print effects. A halftone tool lets artists set dot size, angle and spacing, and dot density tracks the shading. A hatch tool sets thickness from shading. **ChromaShifter** replaces depth-of-field blur with colour offset ([Foundry](https://www.foundry.com/insights/film-tv/graphic-look-in-comp-spiderman)). In *Across*, rates run from ones up to 6s, and Spider-Punk's guitar, jacket and outline each run on different rates ([Yahoo/IndieWire](https://ca.news.yahoo.com/inside-5-mind-blowing-dimensions-130000004.html)). |
| **Klaus** (SPA Studios) | 2D drawings are lit in **up to 8 layers**: key or shadow, ambient occlusion, SSS, rim, eye specular and bounce. Each layer is a set of drawn shapes that a line-tracking system pins to the animation. A separate texture pass adds grain that "travels with the character", with a different amount per material. Lines are kept "only where needed", so value replaces outline ([befores & afters](https://beforesandafters.com/2019/11/14/heres-what-made-the-2d-animation-in-klaus-look-3d/)). A colour script covered about one shot in four. |
| **Arcane** (Fortiche) | 3D sets get painted matte textures projected onto them. Lines are freehand and "slightly uneven". Detail rises only where characters interact ([80.lv](https://80.lv/articles/arcane-artists-show-how-they-combine-traditional-art-3d-for-backgrounds)). Light is painted into textures, and smoke and fire are 2D FX ([VFX Voice](https://vfxvoice.com/riot-games-and-fortiche-get-revolutionary-with-arcane-season-2/)). |
| **Cartoon Saloon** | **Line style carries meaning**: a rigid "woodblock" line for the town, a scratchy pencil line for the forest, and wolfvision as a third style ([Awards Radar](https://awardsradar.com/2021/03/04/interview-wolfwalkers-artists-on-creating-magic-between-the-lines/), [Frame.io](https://blog.frame.io/2021/05/03/made-in-frame-wolfwalkers/)). |
| **Puss in Boots: The Last Wish** | The goal was a "fairy tale painting look". Frame rate is tied to story: stepped twos when Puss is fantasising ([Cartoon Brew](https://www.cartoonbrew.com/feature-film/puss-in-boots-joel-crawford-inbtwn-224324.html), [befores & afters](https://beforesandafters.com/2023/01/11/heres-how-dreamworks-lent-a-whole-new-brand-of-stylized-look-and-feel-to-puss-in-boots-the-last-wish)). |
| **TMNT: Mutant Mayhem** | "Imply teenage imperfection everywhere." Models were approved asymmetric, painted materials replaced tileable maps, **scribbles replaced motion blur**, scribble bokeh replaced defocus, and glows were removed as "too digital". Seven light channels each got their own treatment ([befores & afters](https://beforesandafters.com/2023/09/07/we-wanted-to-imply-teenage-imperfection-everywhere/)). |
| **Mitchells vs the Machines** | "Thousands of watercolour brushstrokes", painterly outlines on CG, and a deliberately amateur doodle layer ([befores & afters](https://beforesandafters.com/2021/04/29/behind-the-early-2d-3d-test-for-the-mitchells-vs-the-machines/), [CGW](https://www.cgw.com/Publications/CGW/2021/July-August-September-2021/3D-with-a-Twist.aspx)). |
| **Eyvind Earle** (*Sleeping Beauty*) | Trees start as a gouache blob and get "smaller and smaller leaves". Square trees sit against curvy ground cover ([Gurney](https://gurneyjourney.blogspot.com/2019/02/eyvind-earle-video-bio.html), [SIGGRAPH poster](https://history.siggraph.org/wp-content/uploads/2022/12/2014-Poster-97-Murphy_Stylized-Trees-and-Landscapes.pdf)). |
| **Golden Wolf, Buck, Animade, Moross** | Cel and frame-by-frame animation over designed, rig-light shapes ([CGW](https://cgw.com/Publications/CGW/2021/Jan-Feb-March-2021/Between-Two-Worlds.aspx), [Toon Boom](https://www.toonboom.com/?p=18220), [Motionographer](https://motionographer.com/2025/05/12/studio-stories-12-years-of-golden-wolf/)). Buck made Facebook's *Alegria* guide, the source of Corporate Memphis. The system worked inside Buck's own animation and became slop when it was copied without the timing or the concept. |
| **Gobelins** | Strong graphic and character design. Layout is often done in 3D and finished in 2D, and there is no house style ([WeTransfer](https://wepresent.wetransfer.com/story/pioneers-gobelins-school-france/)). |

**The pattern.** These productions tend to *remove* default CG or vector perfection, such as motion blur, soft defocus, uniform lines and smooth 24 fps motion. Then each puts back a **specific, consistent, motivated** imperfection.

### Print processes

- **Risograph.** Translucent spot inks overprint into new colours. Registration shifts **0–3 mm** between drums, large dark blocks print blotchy, "Grain Touch" is a random diffusion dither, and halftones run around **71–106 lpi** ([Oxford Green Print](https://oxfordgreenprint.com/home/design-for-riso/), [CCS](https://campus.collegeforcreativestudies.edu/imaging-center/2021/06/22/registration-trapping/), [Kenji Liu](https://atlasofmaking.kenjiliu.com/techniques/risograph)). Drum colours vary by shop: fluorescent pink, yellow, aqua, federal blue, mint and others ([Utah Book Arts](https://lib.utah.edu/collections/book-arts/risograph.php)).
- **Screen print and offset.** Screens sit at 0°, 15°, 45° and 75°, with the darkest ink at 45°, and overprinting tints go at least 15° apart ([PrintWiki](https://printwiki.org/Traditional_Color_Angles), [FESPA](https://fespa.com/en/news-media/screen-separations-in-printing)). Garment screens run 45–55 lpi, so the dots are visible. In trapping, the darker ink overprints the lighter one.
- **Lino and woodcut.** Relief printing, so carved-away areas stay paper. The look is strong black and white, and the gouge marks *follow the form* ([Bachelorprint](https://www.bachelorprint.com/printing-guide/printmaking/woodcut), [Hickman](https://hickmandesign.co.uk/blog/history/what-is-lino-printing)).

---

## 2. Principles, with numbers

### Palette and value: several starting points

No single palette formula has authority. "60/30/10" comes from interior-design folklore and has no traceable origin ([Made Good Designs](https://madegooddesigns.com/60-30-10-color-rule/) repeats it without a source). Treat it as one option among these:

- **Notan / value first.** Design the frame in 2–3 values, light, mid and dark, before any hue. Value carries the read, and hue is decoration on top ([Studio Matrx](https://www.studiomatrx.org/students/interior-design-foundations/colour-hue-value-saturation-interiors)). Test: a greyscale thumbnail about 200 px wide. If the focal point does not pop there, hue will not save it.
- **Gamut masking** (James Gurney, after Walter Sargent, 1920s). Draw a shape, usually a triangle, on the colour wheel and use only colours inside it. Where you place the shape sets the mood ([Gurney Journey](https://gurneyjourney.blogspot.com/2011/09/part-3-gamut-masking-method.html), [Krita docs](https://docs.krita.org/user_manual/gamut_masks.html)). This is the best fit for an engine: a mask is data, and fills can be validated or snapped against it.
- **Monochrome plus one accent.** One hue family in 3–5 values, plus one saturated accent at the focal point, for example Hanuka's neon pink below.
- **Analogous.** 3 neighbouring hues within about 60° of each other, with value doing the contrast. This is Haugomat's and Ghibli's quiet register.
- **Spot inks.** 2–4 print inks plus their overprints plus paper (riso, screen print).
- **Proportion (60/30/10 and similar).** One dominant field, one secondary and a small accent is a useful *default* for giving one colour the room. Choose otherwise when the image depends on two equal fields: Favre's figure and ground, or Bass's split posters.

Further tendencies:
- **Palette size.** Editorial vector work tends to use 3–6 colours plus paper (observed). Stock and AI illustration tends to show 8–15 unrelated mid-saturation hues, because nothing constrains them. Choose more when the subject is variety itself, as in Ngai's dense pattern work, where a strong value structure holds the many hues together.
- **Shadows tend to rotate hue.** Shadows shifted 10–30° in hue toward the ambient colour and 15–30% darker in OKLCH read as light (rule of thumb). Multiplying by grey reads as a filter because real light has colour. Choose a grey or black shadow for noir, newsprint or one-ink work, where that limit is the look.
- **Paper tends to be a colour.** Off-white stock reads as a made object, and `#fff` with pure black reads as screen. Choose pure white for Swiss-style graphic work or Favre-like crispness.

### Shape language

- Circles tend to read as friendly, squares as stable and triangles as dynamic. Straights tend to be set against curves ([Creative Bloq](https://creativebloq.com/how-to/simplfy-figures-for-animation)).
- **Big, medium, small.** Scale steps of about 1 : ⅓ : ⅑ give hierarchy (rule of thumb). Corporate Memphis tends to use uniformly rubbery limbs, so nothing leads.
- **Silhouette test.** A solid-black fill tends to reveal whether a pose reads. Choose an overlapping, ambiguous silhouette when the ambiguity is the idea, as in Favre's double readings.
- **Asymmetry.** Small asymmetries such as a 2–5° eye tilt or an offset shoulder tend to read as alive, because real faces are asymmetric (Mutant Mayhem). Choose mirrored symmetry for icons, masks, heraldry, Anderson-style frontal staging and folk patterns, where formality is the point.
- **Tangents.** Edges that just touch tend to flatten depth because the eye can't tell which is in front. Overlap or separate them, unless you want a deliberate graphic lock-up.

### Line

- **Three weights.** Thin for interior, medium for contour and thick for silhouette, overlaps and the shadow side ([Proko](https://proko.com/course-lesson/how-to-draw-with-line-weight/notes), [Cursa](https://cursa.app/en/page/digital-drawing-on-a-tablet-clean-line-art-techniques-line-weight-tapers-and-corners)). The ratio is about **1 : 1.6–2 : 3** (rule of thumb). Choose a monoline for the woodblock, ligne claire or Jullien register, where uniform weight is the style.
- **Line colour.** A coloured line, darker than its fill, tends to sit inside the image. A black line tends to read as a colouring book, unless black is the only ink and the image is about the line (Jullien).
- **Open lines.** Gaps of 1–3 px where light hits and slight overshoot at corners tend to read as inked by hand (rule of thumb).

### Texture amount

- Grain at **3–8% luminance amplitude** tends to read as material, not filter (rule of thumb). The current paper stocks use 3–6%. Go to 10–15% for a deliberate print look.
- **Object-locked texture** tends to read as painted, and screen-locked texture as a filter (Klaus: grain "travels with the character"). Choose screen-locked when the medium is the paper or film itself.
- Grain of **1–2 px at 1080p** tends to survive H.264. Finer grain smears, so check after encoding.

### Timing

- **Ones, twos and threes.** At 24 fps, twos is 12 drawings a second. Characters on twos with camera moves on ones tends to avoid background strobing. Choose stepped camera moves for a deliberately handmade or stop-motion register.
- **Mixed rates per part** (Spider-Punk) tends to read as hand-made. One rate for the whole frame tends to read as a "low-fps filter".
- **Boil.** Re-seeding at **4–12 fps**, about 8 typical ([PremiumBeat](https://www.premiumbeat.com/blog/how-to-create-line-boil-animations)), with a 3–4 drawing cycle and 0.5–1.5 px amplitude at 1080p for "alive", or 2–4 px for Squigglevision.
- **Holds.** A pose tends to need 6–8 frames to read and 12+ to land. A *moving hold* drifts 2–5% toward a stronger pose ([Animation Mentor](https://animationmentor.com/blog/why-all-animators-need-to-master-the-moving-hold)). **A fully still frame is a valid choice**: Saul Bass titles, Steinberg-like editorial gags and Anderson tableaux hold dead still, because stillness makes the composition the event. Choose it when the image is the joke or the statement.
- **Smears and multiples.** Distort across **1–2 frames** in place of blur ([Wikipedia](https://en.wikipedia.org/wiki/Smear_frame), [AnimSchool](https://blog.animschool.edu/?p=608)).
- **Squash and stretch.** Keep `sx·sy ≈ 1`. Impact at 0.7–0.8 × 1.25–1.4 for 1–2 frames, then overshoot to 1.1 (rule of thumb). Choose rigid bodies for cut-paper or mechanical subjects.
- **Anticipation.** About ⅓ of the action's duration, at 10–20% of its amplitude. Overshoot 5–15%.
- **Secondary motion.** Lag the primary by 2–4 frames and settle 4–8 frames later. Secondary that out-shouts the primary tends to pull focus ([Prolific](https://prolificstudio.co/blog/secondary-action-in-animation/)).

---

## 3. Reference hands

Ten hands analysed for what makes them recognisable and how to get there with Anikit. Numbers are (observed) unless linked.

### Christoph Niemann: the idea is the image
- **Recognisable by** the "Sunday Sketch": a real object on paper plus a few ink lines that turn it into something else, "an exercise in seeing" ([NPR](https://www.npr.org/2016/10/31/499807738/how-does-christoph-niemann-make-art-look-effortless-with-a-lot-of-work)). He aims for the "perfect level of abstraction", which he puts at Charlie Brown, "a circle with dots", because more abstract becomes obscure ([Creative Independent](https://thecreativeindependent.com/people/Illustrator-christoph-niemann-on-meeting-the-needs-of-your-client/)). He starts from a punch line and works backwards. Formally: 1 ink line weight, 0–2 flat colours, a lot of white, and the gag lands in one read.
- **With Anikit:** an `image` layer (the object) plus a `sketch` with 3–8 paths in `ink` texture, `pressure: "even"`, `draw` 0.6 s per path, `boil` 0 (his lines are still) and `paper` `"cotton"`. Hold the finished frame 1.5–2 s. The hold is the payoff.

### Malika Favre: subtraction and negative space
- **Recognisable by** filling the page, then removing until only the essential lines remain ([Wallpaper](https://wallpaper.com/design/hide-and-seek-by-malika-favre-at-kemistry-gallery-london)). Negative space carries a second reading, and she first used it as a device in *Alphabunnies* ([Big Cartel](https://blog.bigcartel.com/malika-favre-interview)). Colour comes last ([CC Magazine](https://ccmagazine.es/en/interview-with-the-illustrator-malika-favre/)). Formally: 2–4 flat colours, often one near-black plus one saturated plus a pale, no outlines, shared edges between figure and ground, and repeated op-art stripes.
- **With Anikit:** shape layers with `path` and no stroke. Figure and ground share a single path edge (draw the ground as the figure's complement). A smooth fill is right here, so no grain or at most 2%. Motion: slow mask reveals that flip the figure-ground reading. Planned `illo` inks would enforce the 3-colour set.

### Jean Jullien: one confident brush line
- **Recognisable by** a black brush line, often a single gesture, and humour from observation with "visual efficiency" ([It's Nice That](https://www.itsnicethat.com/features/review-of-the-year-2015-illustration-jean-jullien), [Eye on Design](https://eyeondesign.aiga.org/how-illustrator-jean-jullien-fights-creative-fatigue-makes-the-most-of-his-limited-skills/)). Line weight varies 1 : 2–3 inside one stroke from brush pressure. Fills are 1–3 flat colours that sometimes miss the line.
- **With Anikit:** `brush` with `texture: "ink"`, `pressure: "swell"`, size about 10–14 px at 1080p and `taper: [0.15, 0.3]`. Put the flat fill under it, offset 3–6 px from the line. Boil at 8 fps, 0.8 px. Stroke-on in drawing order over 0.4–0.8 s.

### Tomer Hanuka: sharp ink, dramatic colour, one neon
- **Recognisable by** sharp inking, dynamic shading and limited palettes with "a dash of neon pink" ([Creative Bloq](https://creativebloq.com/computer-arts/tomer-hanuka-8108799), [Spectator](https://stuyspec.com/article/tomer-hanukas-polychromatic-fantasy)). He picks a palette per drawing rather than forcing one, and adds small soft highlights digitally at the end. Formally: 1 dominant hue family across 4–6 values plus a fluorescent accent under about 5% of the area, thin precise contours, and large dark masses with hard shadow shapes.
- **With Anikit:** `brush` `ink` at 3–5 px, `pressure: "taper"`. Hard shadow shapes as separate shape layers in the dominant hue at −30% L. One accent shape in `#ff2e88`-like neon. A few soft radial highlights (shape layer with `blur` 6–12 px), which are valid here because the artist uses them.

### Brecht Vandenbroucke: gouache, vivid colour, dark jokes
- **Recognisable by** paintbrush and gouache with "vivid colours", surreal and absurdist humour with dark undertones, and ATAK as his influence ([Pictoplasma](https://conference.pictoplasma.com/?p=2684), [It's Nice That](https://www.itsnicethat.com/articles/brecht-vandenbroucke)). Formally: high-chroma complementaries (red and green, blue and orange), opaque flat gouache areas with visible brush edges, wonky perspective and no line hierarchy.
- **With Anikit:** shapes filled flat, then a `brush` with `texture: "dry"` along each shape edge in the same colour, so the edge looks painted. `paper` `"cotton"` at tooth 0.4. 6–8 saturated colours are valid here because the value structure stays simple. The planned `illo` gouache fill would add opaque brush streaks inside shapes.

### Victo Ngai: line first, pattern everywhere
- **Recognisable by** line drawing as the base. She builds by layering over initial lines, starting from acrylic ink on stiff card for texture, using diagonal marks for rock and ovals for soft moss ([Apple masterclass](https://apps.apple.com/us/story/id1470268360)). She studies colour in context ([Creative Bloq](https://www.creativebloq.com/art/digital-art/inside-the-work-of-award-winning-illustrator-victo-ngai-philosophy-process-and-the-tools-behind-it-all)). Formally: dense pattern fills, many hues held together by a clear warm/cool split, and a fine monoline.
- **With Anikit:** `sketch` of the whole drawing in `pencil` or fine `ink`, then a `hatch` per region with `style` (`hatch` for rock at 45°, `stipple` for moss) and `toneFrom`/`toneTo` set by `light`. Colour fills go *under* the line layer. Gap: pattern fills (waves, scales, clouds) as a fill `style` are not built yet.

### Cartoon Saloon (*Wolfwalkers*, *Song of the Sea*, *Kells*)
- **Recognisable by** line style that carries meaning: a rigid woodblock line for the town and scratchy pencil and charcoal for the forest ([Awards Radar](https://awardsradar.com/2021/03/04/interview-wolfwalkers-artists-on-creating-magic-between-the-lines/)). Flattened, decorative perspective. Celtic swirls hidden in shells, clouds and rocks, watercolour backgrounds, and city backgrounds made "mainly of squares and triangles" ([Hot Press](https://hotpress.com/culture/song-of-the-sea-14592226), [Animation Obsessive](https://animationobsessive.substack.com/p/art-directing-song-of-the-sea)).
- **With Anikit:** two `sketch` styles in one film. Town: `ink`, `pressure: "even"`, `boil` 0. Forest: `pencil`, `boil` 12, 1.5 px. `paper` `"watercolor"`. Spirals come from `path` arcs. Planned `clouds` with a spiral-bump option and `plants` on a circle grammar would fit.

### Spider-Verse line work
- **Recognisable by** ink lines drawn on keyframes and interpolated, so lines follow form, plus **MaskToInk** turning masks into wet-ink shapes ([befores & afters](https://beforesandafters.com/2023/06/28/this-was-the-first-cg-animated-movie-ive-ever-heard-of-that-actually-had-a-dedicated-inking-team/), [SIGGRAPH](https://history.siggraph.org/learning/linework-in-spider-man-across-the-spider-verse-by-grochola-maccari-lee-and-boulet-gilly)). Halftone on shading, chroma offset for defocus, mixed frame rates per part ([Foundry](https://www.foundry.com/insights/film-tv/graphic-look-in-comp-spiderman)).
- **With Anikit:** `brush` `ink` lines with `pressure: "flick"` for accent lines on the shadow side only. The `halftone` shader filter on the shadow group alone (not the frame). `aberration` for defocused planes. `steps(12)`-style easing on characters until per-layer fps exists.

### Klaus: lighting on 2D
- **Recognisable by** drawn light shapes in layers (key or shadow, rim, bounce, occlusion, eye specular), a warm transition band at the terminator, minimal outlines, and grain that travels with the character ([befores & afters](https://beforesandafters.com/2019/11/14/heres-what-made-the-2d-animation-in-klaus-look-3d/), [Cartoon Brew](https://cartoonbrew.com/feature-film/how-klaus-draws-on-centuries-old-artistic-principles-to-push-2d-animation-forward-182325.html)).
- **With Anikit:** today, stack per character a base fill, a shadow shape clipped to it (hue +15°, L −20%), a rim shape 3–5 px wide on the edge away from the key, and a warm 4–8 px band along the terminator at 30% opacity. Planned `illo` lit flat shapes would generate these from one `light` direction.

### Studio Ghibli backgrounds (Kazuo Oga)
- **Recognisable by** Nicker poster colour, which works like opaque watercolour. Oga used about 21 colours at the time ([Animation Obsessive](https://animationobsessive.substack.com/p/what-kazuo-oga-thinks-about-when), [Gurney](https://gurneyjourney.blogspot.com/2018/05/painting-in-ghibli-style.html)). A wet base coat (*ji-nuri*) goes on before the paper dries, so cloud edges are soft. Greens are layered dark to light, with leaf clusters catching warm light on top. Formally: soft-edged sky gradients (valid here), hard-edged foliage clusters, and 4–6 greens from blue-green shadow to yellow-green light.
- **With Anikit:** sky as a smooth 2–3 stop gradient plus `paper` `"watercolor"` at tooth 0.3. Foliage as `cutout`-free shape clusters, dark first and lit last, with `brush` `dry` accents. Planned `landscape`, `plants` and `clouds` generators should default to this layering order.

---

## 4. Techniques by subject

### Characters
- Build from 3–5 primitive masses, then cut negative space. One exaggerated trait tends to beat a generic stretch of every limb.
- Light in layers, Klaus-lite: base, one hard shadow shape, one rim, an optional ground bounce, and a warm terminator band.
- Line can follow meaning: clean for "civilised", scratchy for "wild".

### Environments
- 3–5 depth planes. Each plane back moves toward the sky colour and loses 20–30% contrast and line weight (rule of thumb).
- Detail falls off from the focal point (Arcane). Geometric stylisation stays consistent (Earle's square trees).

### Plants
- Earle and Oga method: a silhouette blob, then smaller clusters, then a few leaves on the lit edge. Uniform leaf size tends to read as clip-art.
- Sway as secondary motion: 1–3° rotation at the base, a 2–4 s period, and phase offset by distance from the root.

### Clouds and sky
- Clouds as designed shapes: a flat bottom, 3–7 top bumps of different sizes, an underside shadow shape and a sun-side rim. Oga's soft wet-edge clouds are the other register, so pick one per film.
- Drift as parallax planes. Skies are 2–3 colour bands with dithered or grained transitions, or a smooth gradient on watercolour paper.

### Water
- Designed highlights in 2–3 values, as glint shapes cycling at 6–8 fps, with reflections as broken stepped slices. Gilland's *Elemental Magic* treats splashes, ripples and waves as shape families ([Routledge](https://www.routledge.com/Elemental-Magic-Volume-I-The-Art-of-Special-Effects-Animation-1st-Edition/Gilland/p/book/9780240811635)).

### Light
- **Rim** on flat shapes: an inset offset copy clipped to the silhouette, 2–6 px, warm. Halftone it to read as light.
- **Cast shadows** as hard polygons at 60–85% multiply, with contact darkening. Blurred drop shadows tend to read as UI. Choose them for a deliberately soft, paper-cutout-under-glass look.
- **Gradient plus grain**: a short two-stop gradient (≤ 30% of the shape) with dither or grain.

### Texture
- **Grain** at 3–8%, object-locked, with a seed per object. **Halftone** with dot size from shading, at 15° or 45°, in 8–20 px cells for pop. **Misregistration** of 1–4 px per ink plane. **Edge breakup** of 0.5–2 px noise at a 3–10 px scale.

---

## 5. Worked transformations: clip-art → studio

### An apple
**Clip-art version.** A red `#e53935` circle with a heart notch, a linear gradient from light red to dark red, a white elliptical highlight at top left, a green leaf with a centre vein, a 2 px black outline, a Gaussian drop shadow and a pure white ground. Tells: a gradient with no light source, a generic highlight shape, uniform line, a floating shadow, and 5+ unrelated colours.

**Studio decisions:**
1. **Reference and palette.** Riso two-ink: fluorescent pink `#ff48b0` and federal blue `#3255a4`, with overprint `≈ #5a2f7a` and cream paper `#f3eee2`. That is 3 colours plus paper, from a gamut mask over the red–blue arc.
2. **Shape.** The apple is not a circle. It is wider at the shoulders (w : h ≈ 1.1 : 1) and asymmetric, with the stalk 8° off vertical and the bottom flattened where it sits. Straight on the shadow side, curved on the lit side.
3. **Value.** Notan in 3 values: paper (light), pink (mid), overprint (dark). The shadow is a hard crescent on the right taking 30% of the apple, in overprint.
4. **Light.** A rim on the far left edge: a 4 px sliver of paper showing through. A cast shadow as a flat blue ellipse, 1.6 × the apple's width, offset 20° away from the light, with a darker contact line 3 px tall.
5. **Line.** No outline on the body. The stalk is a `brush` `ink` stroke, `pressure: "taper"`, 6→1 px. The leaf is one `cutout`-style shape with a `dry` brush vein.
6. **Texture.** Pink and blue planes misregistered by (2, −1) px and (−1, 2) px. Grain 6%, plus `halftone` on the shadow crescent only, 10 px cells at 45°.
7. **Motion.** The apple drops in on twos and squashes to 0.82 × 1.22 for 2 frames on landing. The leaf lags 3 frames and settles in 6. Boil 8 fps, 0.8 px. Hold 1.5 s with a moving hold of 2% leaf rotation.

### A fox
**Clip-art version.** An orange `#ff9800` fox from rounded rectangles: a symmetric head, two triangle ears, a white muzzle, black dot eyes with white glints, a smooth-gradient bushy tail, every line 3 px black, running in a 60 fps tween with ease-in-out.

**Studio decisions:**
1. **Concept.** One trait: a *sly* fox, so a long pointed snout (head length 1.4 × the clip-art's) and half-lidded eyes. Shape language: triangles for the head and tail, one big curve for the back.
2. **Palette.** Monochrome plus accent: rust in 4 values (`#e2703a`, `#b84f26`, `#7d3318`, `#3e1a0e`) on a blue-grey snow ground `#c9d3dc`. The accent is the cold blue in the shadows, which picks up the ground colour. 5 colours in total.
3. **Silhouette.** Readable in solid black. The tail is 45% of body length (a big shape) and the ears are the small shapes. Tilt the head 6° and set one ear back 10°.
4. **Light.** Low winter sun from behind left. The shadow side is cooled 25° in hue toward blue, and a warm 3 px rim runs along the back and tail. A hard cast shadow, stretched 2.5× long, falls in blue on the snow.
5. **Line.** Three weights: silhouette 4 px, interior 2 px, whisker and fur flicks 1 px, all `ink` with `taper`. Line colour is `#3e1a0e`, not black. Fur at the tail tip and chest is 6–10 `pressure: "flick"` strokes, with gaps on the lit edge.
6. **Texture.** `hatch` `style: "hatch"` at 30° on the shadow side of the body, `toneFrom` 0.2 to `toneTo` 0.7. `paper` `"cotton"`.
7. **Motion.** A trot on twos with a 10-drawing cycle (5 poses), the tail on threes lagging 3 frames. On the turn to camera, a 1-frame smear stretches the snout 1.5×. The camera pans on ones. The final pose is a moving hold with the eyes narrowing over 12 frames.

### A city skyline
**Clip-art version.** 12 rectangles of random heights in 8 pastel colours, window grids of identical yellow squares, a sky gradient from blue to purple, a sun circle with a glow, a uniform 2 px outline, and every building at the same detail.

**Studio decisions:**
1. **Reference.** A Haugomat-like dusk. Analogous palette from one gamut-mask triangle covering orange to magenta to violet: sky `#f2a36b` to `#c25a7a`, three building planes `#7a3d6b`, `#4f2a5e`, `#2b1a3f`, and window accent `#ffd27a`. 6 colours.
2. **Planes and value.** 3 depth planes. The far plane is closest to the sky colour (contrast −60%). The near plane is the darkest. The value steps between planes are even, about 18 L each in OKLCH.
3. **Shape.** Building widths vary 1 : 3 : 9 (big, medium, small), and heights are composed, not random: one tall landmark at the 1/3 line with a gap beside it for negative space. Roofs get 2–3 distinct silhouettes (a water tower, a stepped crown, an antenna). No tangent where a roof meets the horizon or the frame edge.
4. **Detail falloff.** Windows only on the near plane, about 15% lit, clustered rather than gridded. The far plane is pure silhouette.
5. **Light.** The sun sets behind the landmark, so a 3 px rim of `#ffd27a` runs along the landmark's edge only. God rays are 3 flat wedge shapes at 8% screen blend. Choose a smooth sky gradient here (valid for this reference) on `paper` `"watercolor"`, with 4% grain.
6. **Texture.** Edge breakup is not wanted for crisp Haugomat buildings. For a riso reading instead: 2 inks, a 2 px misregistration and a halftone sky.
7. **Motion.** A parallax truck-left: far plane 0.2×, mid 0.5×, near 1×, and a foreground lamppost 2× as a flat dark silhouette. Window lights flick on one at a time on 4-frame steps. A slow push of 3% scale across 6 s. The last 1.5 s holds dead still for the title.

---

## 6. What makes vector work hand-made, not clip-art

| Clip-art / AI default | Hand-made equivalent (when the style calls for it) |
|---|---|
| Perfect bezier edges, uniform stroke | Edge breakup, pressure swell and taper, overshoots, small gaps |
| 8–15 unrelated hues, pure white ground | A gamut mask or 3–6 inks plus paper, value first |
| Gaussian drop shadow, glow, blur | Hard cast-shadow shapes, drawn rim, halftone or scribble for blur |
| Linear gradient fills, unmotivated | Value steps, dithered or stippled gradients, or a deliberate smooth gradient on textured paper |
| Default symmetry | Chosen asymmetry, or chosen formal symmetry |
| Everything at full detail | Detail falls off from the focal point |
| Generic people with stretched limbs | Specific characters with one exaggerated trait |
| One screen-space texture overlay | Texture per object and per material, moving with it |
| Smooth tween, same ease everywhere | Stepped poses, holds (still or moving), smears, mixed rates |
| Decoration fills the empty space | Negative space used on purpose |

---

## 7. Motion techniques specific to illustration

1. **Stepped timing** per layer and per part. The camera tends to stay on ones.
2. **Line boil** at 4–12 fps with a 3–4 drawing cycle, on fill edges and textures too. Choose 0 for still editorial lines (Niemann).
3. **Shape morphs** between designed poses with matched vertex counts, played stepped.
4. **Smears and multiples** for 1–2 frames, plus speed lines or scribbles.
5. **Squash and stretch** that preserves area.
6. **Secondary action**: lag springs on hair, ears, leaves and tails.
7. **Parallax and multiplane**: Disney used up to 7 planes ([Wikipedia](https://en.wikipedia.org/wiki/Multiplane_camera)). Speed ratios of far 0.1–0.3×, mid 0.5×, focal 1× and foreground 1.5–2.5× (rule of thumb).
8. **2.5D camera**: flat cards in depth, a 2–5% push per shot, painted projection (Arcane).
9. **Cut-paper animation**: pivots, no deformation, on twos or threes, with the shadow lifting on a lift.
10. **Stroke-on reveals** in drawing order.
11. **Holds**: still (Bass, editorial) or moving (character work), chosen per shot.
12. **Frame rate as storytelling**: switch rate at the emotional peak.

---

## 8. Gap list for Anikit Video, ranked by impact

Existing: `brush` (`ink dry marker pencil`, pressure `swell taper even flick`, stroke-on, `boil`), `sketch`, `hatch` (`hatch crosshatch stipple scribble`, `toneFrom`/`toneTo` from a `light` direction), `paper` (`cotton kraft canvas watercolor newsprint`), `cutout` (torn or cut, lift shadow), shapes with gradients, `shadow`/`glow`/`blur`, post `grain` and `aberration`, `steps(n)` easing, and whole-composite shader style filters. Planned: `illo`, `landscape`, `plants`, `clouds`, `water`.

| # | Gap | Why | One-line implementation idea |
|---|---|---|---|
| 1 | **Per-layer frame rate** | The biggest single tell of studio 2D | `"fps": 12` or `"on": 2` on any layer or group: sample time at `floor(t·fps)/fps`, camera excluded, plus `rateAt` keyframes |
| 2 | **Lit flat shapes** (`illo`) | Klaus- or Favre-grade light without 3D | `"light": {"dir": 135, "shadow": 0.25, "rim": "#ffd9a0"}`: offset-and-clip silhouettes, hue-shift in OKLCH, `castShadow` onto a ground line |
| 3 | **Inks and gamut mask** (`illo`) | Fixes the 15-hue look in one place | `"inks": [...]` or `"gamut": {"triangle": [h1, h2, h3]}` at video level: snap fills, multiply overprints, warn when outside |
| 4 | **Per-layer print look** | Global filters can't lock texture to objects | `"print": {"halftone": 10, "angle": 15, "misregister": 2, "grain": 0.05}` rendered in layer space |
| 5 | **Edge breakup on vector fills** | Perfect beziers tend to read as clip-art | `"edge": {"rough": 1.2, "scale": 6, "boil": 8}` displaces flattened outlines with seeded noise |
| 6 | **Textured gradients** | Smooth gradients band in H.264 | `"gradient": {..., "texture": "stipple"/"grain"/"bayer"}`: threshold against a noise field |
| 7 | **Smears and multiples** | Replace CG motion blur | `"smear": {"stretch": 1.5, "multiples": 3}` above a speed threshold, for 1–2 stepped frames |
| 8 | **Squash and stretch plus follow-through** | Life without hand keys | `"squash": true` (area-preserving, from acceleration) and `"follow": {"parent", "lag": 3, "spring"}` |
| 9 | **Pose morphs** | Cut-paper and flat character animation | `"morph": ["d1", "d2"]`, resampled by arc length, played on the layer's rate |
| 10 | **Parallax planes and a 2.5D camera** | Depth without modelling | `"plane": 0.3` per layer, plus a `multiplane` camera preset |
| 11 | **Graphic defocus** | Spider-Verse and Mutant Mayhem | `"defocus": {"style": "chroma"/"halftone"/"scribble"}` per plane |
| 12 | **Hatch tied to lit shapes** | `hatch` takes a light direction but not the shape's shading | Feed `illo` shading into `hatch` density and orient strokes along the shape's long axis |
| 13 | **Line-style tokens and line colour** | Cartoon Saloon meaning-through-line | Theme `lineStyle` (`woodblock`, `pencil`, `clean`). Line colour defaults to the fill at −35% L |
| 14 | **Pattern fills** (waves, scales, Celtic spirals, clouds) | Ngai and Saloon density | New `hatch` styles that tile a motif along a flow field, clipped to the shape |
| 15 | **Environment generators** | Environments are where generic shows most | `landscape`, `plants` (Earle blob, clusters, lit leaves), `clouds` (bump or wet-edge), `water` (glint cycles), all ink-aware |
| 16 | **Object-locked texture per material** | "Grain travels with the character" | Per-layer texture seed and UV origin that follow the transform, with an amount per layer |
| 17 | **Illustration lint** | Agents need feedback on taste | `still --lint illus`: hue count versus mask, greyscale focal contrast, Gaussian shadows, uniform strokes. It warns and never blocks |

---

## Checklist for a studio-grade illustrated shot

1. Is there one named reference hand and one sentence on why it fits this story?
2. Did the palette come from a method (notan, gamut mask, inks, monochrome plus accent)? Does it read in greyscale at thumbnail size?
3. Do the silhouettes read? Are there big, medium and small shapes? Are symmetry and asymmetry both chosen?
4. Do lines have chosen weights and colour (a hierarchy or a deliberate monoline)?
5. Is the light designed: shadow shape, rim, cast shadow? Is any soft shadow or glow there on purpose?
6. Is texture 3–8% and object-locked, unless the medium is the paper? Is every smooth gradient a choice?
7. Is the frame rate chosen per layer, with at least one rate change or hold timed to the story?
8. Does fast motion use smears or scribbles, does squash keep area, and do secondary parts lag?
9. Is every hold either alive (boil, moving hold) or still on purpose?
10. Does depth come from planes and atmospheric fade? Does detail fall off from the focal point, and is the negative space used?
