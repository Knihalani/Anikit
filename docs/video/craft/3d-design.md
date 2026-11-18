# 3D design craft: how studios light, dress and shoot a shot

Who this is for: agents writing `scene3d` specs for product films and motion graphics. The complaint we are answering is that our 3D reads as "AI slop": technically clean, but designed by nobody. This page collects what product photographers, cinematographers and 3D motion studios do on purpose. It ends with rules for agents and a list of what the engine still lacks.

Two caveats on the sources:
- Studios such as ManvsMachine, Buck and Hornet rarely publish their lighting numbers. Where a number below comes from photography or cinematography practice, the link says so.
- Where a number is our own house recommendation, it is marked **(house rule)**.

---

## 0. What the best studios have in common

- **They start from something real.**
  - ManvsMachine scans the real Nike trainer "for almost every Nike project" so that the CG shoe "looks exactly like the real shoe". The stylised motion is then built on a low-poly copy of the scan ([Maxon](https://maxon.net/en/article/man-vs-machine-fusing-creativity-with-technology-with-the-help-of-cinema-4d)).
  - Apple's MacBook Neo launch film looks like CG but was largely practical. Behind-the-scenes footage shows LED strip lights, confetti cannons, stop-motion and handmade props ([PetaPixel](https://petapixel.com/2026/04/24/how-apple-filmed-its-flashy-macbook-neo-video-using-handmade-props/)).
- **They decide what each surface must say before adding detail.** Buck's look-dev lead Alex Dingfelder works "big to small, asking myself what is important about the image every step of the way". He warns against "adding realism for the sake of realism", and builds one shared material base per project so that choices such as visible tool marks carry across every asset ([Motionographer](https://motionographer.com/2017/01/18/insight-into-the-world-of-look-development-from-one-of-our-industrys-best/)).
- **Every light has a job and a name.** Blender Studio's *Sprite Fright* lighters keep the rig simple at first and add complexity later. They name lights by function (key, fill, rim, soft key, warm fill), squint at a desaturated value study to see where the eye goes, and use hidden shadow-casters to darken whatever pulls focus ([Blender Studio](https://studio.blender.org/blog/sprite-fright-workflow-lighting-set-dressing)). Lightmap's product advice is the same: "a few well-considered lights", each doing "a very specific job", with each one soloed to check it ([Lightmap](https://lightmap.co.uk/blog/perfect-product-render)).
- **They remove things.** School of Motion's lesson on cinematic shots:
  - "show as little as possible";
  - many classic film stills are dominated by "three colors or fewer";
  - avoid evenly lit frames, because light should reveal *and* conceal ([School of Motion](https://schoolofmotion.com/blog/what-is-a-cinematic-shot-a-lesson-for-motion-designers)).

Slop is the opposite on every count: no reference, no hierarchy, every light on, every object visible, and everything moving at once.

---

## 1. Lighting design

### 1.1 Ratios: the single most important number

A lighting ratio compares the key side of the subject with the fill side. Each stop doubles the light ([Wikipedia](https://en.wikipedia.org/wiki/Lighting_ratio), [StudioBinder](https://www.studiobinder.com/blog/lighting-ratios/)).

| Ratio | Stops | Reads as | Use for |
|---|---|---|---|
| 1:1–1.5:1 | 0–0.5 | Flat, catalogue, "default render" | Almost never. It is the main tell of cheap CG. |
| 2:1 | 1 | Bright, friendly, high-key | Lifestyle, beauty, white-cyc product |
| 3:1–4:1 | 1.6–2 | Modelled, premium | The default for hero product and type |
| 8:1 | 3 | Dramatic, low-key | Night, launch "reveal" shots, mystery |
| 16:1+ | 4+ | Silhouette, edge-lit only | Opening teasers: the product as a line of light |

The 3dRender.com guide puts the fill at most at half the key (2:1), and at about 1/8 of it (8:1) for "shadowy" scenes. Overlapping fills together must never compete with the key ([3dRender](https://3drender.com/light/3point.html)). Note the trap: when the fill also lands on the key side, the *additive* ratio of a 2:1 intensity setup is really 3:1 ([Wikipedia](https://en.wikipedia.org/wiki/Lighting_ratio)).

### 1.2 Placement in degrees

The angles below are measured around the subject, with 0° at the camera:

| Light | Horizontal | Vertical | Notes |
|---|---|---|---|
| Key | 15–45° off the camera axis | 15–45° above the lens | Do a key-only render first. It should already read well ([3dRender](https://3drender.com/light/3point.html)). |
| Fill | The opposite side, lower than the key | Near lens height | Never symmetrical with the key. Tint it from the environment colour ([3dRender](https://3drender.com/light/3point.html)). |
| Rim / back | 150–180° (behind) | Above the subject | "Can be as bright as necessary" for the glint. Light-link it to the subject only ([3dRender](https://3drender.com/light/3point.html)). |
| Kicker | About 110–135°, three-quarter back | Near subject height | A backlight pushed to one side, so it rakes one edge ([CineD](https://cined.com/the-power-of-backlight-in-film-what-it-reveals-and-conceals), [PremiumBeat](https://www.premiumbeat.com/blog/basic-light-placements/)). The degree range is our reading of "three-quarter back". |
| Product strips | ±100–120°, tall and narrow (about 1:8–1:12) | 0–20° | Long, continuous edge highlights on metal and glass |
| Top key (product) | 0–30° | 60–80° | A large overhead softbox: the classic tabletop key |

3dRender itself calls three-point lighting "a simple starting-point", not "an excuse to light by formula". First find "the direction from which the light is brightest" in the story (a window, the sun, a screen), and put the key there ([3dRender](https://3drender.com/light/3point.html)). That is what *motivated* means.

### 1.3 Soft boxes vs strips: lighting things that reflect

On glossy products you do not see the light; you see its *reflection*. The shape of the reflection *is* the lighting design.
- A hard-edged reflection reads as gloss, and a blurred one as satin ([Lightmap](https://lightmap.co.uk/blog/perfect-product-render)).
- "Wrap light around your product so highlights are long and continuous … the shape of the highlight will define the shape of the product" ([SketchUcation](https://community.sketchucation.com/post/1082893)).
- Linear and strip lights are the standard tool for "long soft highlights on automotive, furniture, or consumer-product surfaces" ([Novedge/Rhino](https://novedge.com/blogs/design-news/rhino-3d-tip-professional-rhino-lighting-setup-for-clearer-more-realistic-renders)).
- Make strips long and narrow, but not so thin that they add noise ([Novedge/V-Ray](https://novedge.com/blogs/design-news/v-ray-tip-mastering-v-ray-rectangular-lights-for-realistic-illumination-and-reflections)).
- Place lights so that their reflections *follow* the product's curves, not cut across them ([Lightmap](https://lightmap.co.uk/blog/perfect-product-render)).

The Apple-style tabletop recipe, as reconstructed by Manfrotto ([Manfrotto](https://www.manfrotto.com/global-en/stories/how-to-shoot-cinematic-tech-videos-like-apple/)):
1. **Reveal:** three backlights aligned with the product's edges. They "highlighted the edges but not the details", so the product stays a mystery.
2. **Sweep:** a light on a linear slide is moved *across* the product to draw lines of light along its edges.
3. **Resolve:** a fill fades in at the end to reveal the whole product.
4. **The "Apple 45" screen reflection:** a light through diffusion, cut diagonally at 45° with black card.

The lesson for us is that **light changes over time**. Moving lights are half of the look.

### 1.4 Taking light away: negative fill and flags

Negative fill (black cloth or board on the shadow side) "brings the shadows down, increasing contrast". With digital's wide dynamic range, taking light away "has become just as important as adding it" ([Neil Oseman](https://neiloseman.com/what-is-negative-fill-and-how-do-you-use-it)). Moving a flag closer deepens the effect ([Videvo](https://www.videvo.net/blog/?p=40509)). For chrome and glass, black flags are what draw the dark lines that make the edges read. A chrome object on a white cyc with no flags looks grey and shapeless.

### 1.5 Gobos and cookies

A gobo or cookie is a pattern between the light and the subject: blinds, a window or foliage. It "can imply an unseen window", so it motivates the light and adds an off-screen world for free ([Videomaker](https://www.videomaker.com/how-to/lighting/what-is-a-cucoloris-and-how-is-it-used-on-professional-film-sets/), [Neil Oseman](https://neiloseman.com/?p=6680)).
- A crisp pattern needs a hard source.
- Making the pattern too legible "calls attention to the lighting instead of the subject".
- Perfectly parallel slats are a tell ([Melies](https://melies.co/cinematic-techniques/lighting/gobo-lighting)).
- Throw the pattern on the *background or floor* more than on the hero.

### 1.6 Haze and volumetrics

Haze only shows where light crosses it at an angle. Backlight makes the particles glow and separates the subject from the background ([ASC](https://theasc.com/articles/shot-craft-smoke-and-fog), [Rosco](https://spectrum.rosco.com/index.php/2017/07/four-haze-techniques-to-enhance-your-filmmaking)).
- Haze accentuates beams and keeps contrast; fog hides the background.
- Too much "will overwhelm the scene".
- There is no standard density: add it until the beams and the background layers read, then stop before the subject loses contrast.
- In `scene3d`, that is fog density 0.02–0.08 with shafts at night, and `haze` around 0.0015 in a studio (see `../scene3d.md`).

### 1.7 Colour of light

- **Warm key, cool fill**, or the reverse for night/tech. Tint the fill from the environment ([3dRender](https://3drender.com/light/3point.html)).
- **One complementary accent.** Peter Tarka's commercial renders use "clear edge lighting that separates objects from the background" and add warm orange or yellow accents into cool purple/blue palettes ([besthub analysis](https://www.besthub.dev/articles/design-inspiration-peter-tarka-s-2025-2026-explorations-reveal-future-aesthetics-7bf0519d3a45)).
- **CG shadows are often hue-less.** Real shadows shift hue toward the skylight or bounce colour ([Fox Renderfarm](https://www.foxrenderfarm.com/share/5-tips-to-make-rendering-more-realistic)).
- **Practicals** (lamps, LEDs, screens visible in frame) motivate colour and give the eye "tiny points of warm light" in a dark frame ([School of Motion](https://schoolofmotion.com/blog/what-is-a-cinematic-shot-a-lesson-for-motion-designers)).

### 1.8 High-key vs low-key

- **High-key:** 2:1, a white sweep, soft top key. The danger is flatness, so it needs strips or flags to keep edges.
- **Low-key:** 8:1 or more, black void, edge light only, haze. The danger is mud: keep one clean highlight and one readable silhouette.
- Apple-style product films use both. Analyses describe plain black or white backgrounds, with all the colour on the product or its UI and the surrounding space "empty on purpose" ([Manfrotto](https://www.manfrotto.com/au-en/how-to/how-to-shoot-cinematic-tech-videos-like-apple/), [DesignRush](https://www.designrush.com/best-designs/video/new-macbook-pro-with-proled-xdr-display-the-first-ever-mac-with-touch-video-design)).

---

## 2. Look development: materials that read as premium

| Do | Why / source |
|---|---|
| **Bevel every edge.** Hard-surface edge radius of about 0.5–2% of the object size; keep widths consistent | "Overly perfect geometry is a dead giveaway." Bevels catch light and make the edge highlight ([ProEdu](https://proedu.com/blogs/news/how-to-make-3d-renders-look-real), [Novedge/C4D](https://novedge.com/blogs/design-news/cinema-4d-tip-beveling-best-practices-for-realistic-hard-surface-modeling)). Octane's popularity is partly its "small highlights on bevels" ([Blender Artists](https://blenderartists.org/t/spectral-cycles-vs-octane/1460308?page=2)). For CAD or imported meshes, use a round-edges shader ([Will Gibbons](https://www.willgibbons.com/realistic-rendering/)). |
| **Vary roughness, not colour.** Use a roughness range (e.g. 0.18–0.32 on satin plastic), not one value | Purely procedural surfaces look "mathematically perfect but lack the micro-imperfections — fingerprints, dust, grain" ([Studio Matrx](https://www.studiomatrx.org/students/architectural-visualization-rendering/anatomy-of-a-great-render)). |
| **Add imperfections at the right scale**: smudges on glass and screens, micro-scratches that show only in the highlight, dust on horizontal faces | Real surfaces have "dust, scratches, or wear" ([Toolfarm](https://www.toolfarm.com/?p=32693)). Keep them subtle: Apple's crews *clean* the hero with glass cleaner and microfibre, then remove dust in post ([Manfrotto](https://www.manfrotto.com/global-en/stories/how-to-shoot-cinematic-tech-videos-like-apple/)). Premium means *almost* clean. |
| **Avoid 0 and 1.** Albedo 0.03–0.85 (pure black and white do not exist), roughness 0.05 minimum except true mirrors, metallic only 0 or 1 (it is a switch, not a slider) | Standard PBR practice. Pure values read as "plastic". |
| **Layer materials**: clearcoat over paint, glass over print, anodised over bead blast | Real products are built in layers. Dingfelder layers "slightly different color passes that fade into each other to create depth" ([Motionographer](https://motionographer.com/2017/01/18/insight-into-the-world-of-look-development-from-one-of-our-industrys-best/)). |
| **Mask noise with noise.** Never show a single procedural pattern | "Mask one noise with another to remove the obvious procedural look" ([Motionographer](https://motionographer.com/2017/01/18/insight-into-the-world-of-look-development-from-one-of-our-industrys-best/)). |
| **Test materials under several environments**, including zebra-stripe HDRIs to find bad surfaces | ([Lightmap](https://lightmap.co.uk/blog/perfect-product-render)) |
| **Keep the palette small.** Three colours or fewer, plus neutrals; the accent goes on the product | ([School of Motion](https://schoolofmotion.com/blog/what-is-a-cinematic-shot-a-lesson-for-motion-designers)) |

---

## 3. Camera language

### 3.1 Lenses (full-frame equivalent)

| Lens | Use | Notes |
|---|---|---|
| 14–24 mm | Scale, environments, flying through abstract fields | It distorts. CG wide shots look "stretchy" at the corners ([School of Motion](https://www.schoolofmotion.com/blog/lessons-motion-designers-learn-from-hollywood-lenses)). Keep the hero away from the frame edges. |
| 35 mm | Product in context, hand-held "documentary" | Neutral-wide |
| 50 mm | Neutral; the start of the product range | Under 50 mm, products start to distort ([Practical Ecommerce](https://www.practicalecommerce.com/product-photography-part-6-selecting-a-lens), [Cotinga](https://cotinga.io/blog/best-focal-length-for-product-photography)) |
| 85–100 mm | **The hero product lens**: flattering compression, separation from the background | 85/100 mm "isolate products … without distortion". 100 mm macro is the product workhorse ([Cotinga](https://cotinga.io/blog/best-focal-length-for-product-photography)). |
| 100 mm macro | Details: ports, textures, edges, logos | Real macro gives razor-thin focus. Apple-style detail shots often stop down to *f/32* to keep the whole product sharp ([Manfrotto](https://www.manfrotto.com/global-en/stories/how-to-shoot-cinematic-tech-videos-like-apple/)). Decide which you want. |
| 135–200 mm | Isolation, stacked compression, "tension" | A long lens "compresses the Z depth, bringing the foreground and background together" ([School of Motion](https://www.schoolofmotion.com/blog/lessons-motion-designers-learn-from-hollywood-lenses)). |

Compression comes from camera *distance*, not the lens itself. A long lens only compresses because you have to stand further back ([School of Motion](https://www.schoolofmotion.com/blog/lessons-motion-designers-learn-from-hollywood-lenses)).

### 3.2 Aperture and focus

- **Shallow focus (f/1.4–f/2.8) makes CG read as photographic.** David Ariew's tunnel render "immediately looks more photographic" once DOF is added ([School of Motion](https://schoolofmotion.com/blog/how-to-mimic-photographic-effects)).
- **Do not blur everything, though.** At product scale, f/2.8 at 100 mm leaves only millimetres in focus. Use f/4–f/8 for the hero so the whole object is sharp, and save shallow focus for macro details and for foreground and background layers **(house rule)**.
- **Rack focus:** a pull of 0.6–1.2 s, eased in and out, motivated by something new entering or lighting up. Hold 0.5 s before and after the pull **(house rule)**.

### 3.3 Movement

- **Slow is premium.** The Apple style is "the art of the slow show": slider moves, the product rolling toward the lens, starting on a detail and pulling back to reveal ([Manfrotto](https://www.manfrotto.com/global-en/stories/how-to-shoot-cinematic-tech-videos-like-apple/)).
- **House numbers (house rule):**
  - hero orbit: 6–15°/s;
  - push-in: 2–5% of the subject distance per second;
  - an orbit should cover no more than 30–60° in a 4 s shot (a full 360° turntable reads as a catalogue);
  - whip pans are 6–10 frames of blur between two held frames, always a transition and never a showcase.
- **Easing:** ease the start and land of every move. Ease-out only on a "settle" into the final frame. Linear motion belongs only to constant-speed flybys and must never start or stop on screen.
- **Motivation:** before a flashy move, ask whether it "reveals the dramatic point of view or just follows the action" ([School of Motion](https://schoolofmotion.com/blog/what-is-a-cinematic-shot-a-lesson-for-motion-designers)).
- **Motion blur:** use a 180° shutter, i.e. 1/48 s at 24 fps ([Neil Oseman](https://neiloseman.com/understanding-shutter-angles), [DIYPhotography](https://www.diyphotography.net/?p=309010)).
  - 45–90° gives a crisp, "hyper-real" staccato for impacts or particle hits.
  - 270–360° gives a dreamy smear.
  - Still CG without blur "doesn't feel fast" ([School of Motion](https://schoolofmotion.com/blog/how-to-mimic-photographic-effects)).
- **Frame rate:** 24 fps for film, 30 or 60 fps for UI-heavy social pieces. "Frame rate is a stylistic choice, not a default" ([School of Motion](https://schoolofmotion.com/blog/what-is-a-cinematic-shot-a-lesson-for-motion-designers)).

### 3.4 Handheld and camera noise

- **Rotation sells handheld more than position.** "Handheld camera operators tend to shake more rotationally than positionally" ([Unity Cinemachine](https://docs.unity3d.com/Packages/com.unity.cinemachine@3.1/manual/CinemachineNoiseProfiles.html)).
- **Layer three bands per axis:**
  - low 0.1–0.5 Hz (breathing and drift);
  - mid 0.8–1.5 Hz (the handheld body);
  - high 3–4 Hz (jitter).
- **Scale the amplitude down for long lenses**, because the narrower FOV amplifies the shake ([Cinemachine](https://docs.unity3d.com/Packages/com.unity.cinemachine@3.1/manual/CinemachineNoiseProfiles.html)).
- **Screen-space reference values**, as a fraction of the frame's short side ([ComfyUI camera-shake node](https://comfy.icu/node/WASVideoCameraShake)):

  | Amplitude | Reads as |
  |---|---|
  | 0.004 | Steady hand |
  | 0.01 | Handheld |
  | 0.03 | Running |

  Roll for handheld is about 0.3°.
- **For product films**, a "tripod breath" of about 0.002 at 0.2–0.4 Hz removes the dead-CG stillness without anyone noticing it **(house rule)**.

---

## 4. Set design, scale and composition

- **Give the hero a place to stand.** Plinths, podiums, stepped blocks, a sweep (cyc) or a single slab. Abstract "geometry fields" (repeated columns, arches, tiles, ribbons) suggest a world without a location. Contact matters: a soft contact shadow under every grounded object ([Studio Matrx](https://www.studiomatrx.org/students/architectural-visualization-rendering/anatomy-of-a-great-render)).
- **Negative space is a material.** Leave 50–70% of the frame empty in hero shots **(house rule)**. Apple-style frames keep the space around the product "empty on purpose" ([DesignRush](https://www.designrush.com/best-designs/video/new-macbook-pro-with-proled-xdr-display-the-first-ever-mac-with-touch-video-design)).
- **Scale cues:** a known object (a hand, a coin, a person, a doorway), atmospheric perspective (haze grows with distance) and lens choice. A 24 mm lens low to the ground makes a 10 cm object monumental. A 135 mm lens from far away makes a building a model.
- **Composition:**
  - put the subject on thirds or on a deliberate one-point symmetry, which reads as "cold and overpowering" ([School of Motion](https://schoolofmotion.com/blog/what-is-a-cinematic-shot-a-lesson-for-motion-designers));
  - put the horizon on the upper or lower third, never at mid-frame;
  - use leading lines (strip reflections, floor seams, cable runs) that point to the hero.
- **Depth layers:** every establishing shot needs a foreground element (out of focus, dark, partly in frame), the hero in the midground, and a background with lower contrast and haze. A wide lens deepens the Z depth and a long lens stacks the layers ([School of Motion](https://www.schoolofmotion.com/blog/lessons-motion-designers-learn-from-hollywood-lenses)).
- **Check like a lighter:**
  - view the frame at thumbnail size;
  - flip it horizontally to test the balance ([Lightmap](https://lightmap.co.uk/blog/perfect-product-render));
  - desaturate it and squint to find where the eye lands ([Blender Studio](https://studio.blender.org/blog/sprite-fright-workflow-lighting-set-dressing)).

---

## 5. Abstract 3D vs product 3D

| | Product 3D | Abstract / brand 3D |
|---|---|---|
| Truth | The real object: scans, CAD, accurate materials ([Maxon/ManvsMachine](https://maxon.net/en/article/man-vs-machine-fusing-creativity-with-technology-with-the-help-of-cinema-4d)) | An *idea*. Materials only need to "feel the same" as the design, not look the same ([Motionographer](https://motionographer.com/2017/01/18/insight-into-the-world-of-look-development-from-one-of-our-industrys-best/)). |
| Light | Strips, flags and big soft keys, designed through *reflections*. Ratio 3:1–4:1 or edge-only. | A motivated single key plus colour accents. Gobos, haze, practicals. Ratio 4:1–8:1. |
| Camera | 85–100 mm, slow eased slides and orbits ≤ 15°/s, macro inserts | Wider lenses (24–50 mm), flythroughs, scale play, more motion but one idea per shot |
| Set | A sweep, plinth or void. The product is the only colour. | Geometry fields, repetition, simulation, physical materials (cloth, clay, liquid) |
| Structure | Reveal (edges only), then details (macro), then the hero, then context ([Manfrotto](https://www.manfrotto.com/global-en/stories/how-to-shoot-cinematic-tech-videos-like-apple/)) | A visual metaphor that transforms. ManvsMachine's Squarespace films were "a tactile take on the online experience" ([Escape](https://www.escape-technology.com/news/how-manvsmachine-created-squarespace-s-new-advertising-campaign)). |
| Failure mode | Catalogue turntable | "Beeple-era daily": an iridescent chrome blob, glowing pillars and a fog plane, with no idea behind it |

Why "daily" renders read as cliché:
- Beeple's *Everydays* are, in his own words, sketches: one image a day for over fifteen years ([M+](https://www.mplus.org.hk/en/magazine/critique-on-digital-artist-beeple-nft-artwork-everydays-and-video-sculpture-human-one/), [Le Random](https://www.lerandom.art/editorial/beeple-on-infinite-creation-machines)).
- The *imitation* of that cadence produced a shared toolkit: an Octane default studio, chrome and iridescent spheres, emissive neon tubes, a big fog plane, heavy bloom and centred symmetry. Thousands of portfolios carry the same tags ([Dribbble listings](https://dribbble.com/jaykats)).
- That look is fast to make, so it is everywhere, so it signals "no one decided anything". This is our reading; we found no formal critique.
- An agent making the same choices by default reproduces exactly that look.

---

## 6. What makes CG look cheap: tells to check before render

1. **Perfect materials:** uniform roughness, no smudges, no dust, pure 0/1 values ([Toolfarm](https://www.toolfarm.com/?p=32693)).
2. **Razor edges:** no bevels, so there are no edge highlights ([ProEdu](https://proedu.com/blogs/news/how-to-make-3d-renders-look-real)).
3. **Flat, even light:** a ratio under 2:1, light from everywhere, no shadow side ([School of Motion](https://schoolofmotion.com/blog/what-is-a-cinematic-shot-a-lesson-for-motion-designers)).
4. **The default HDRI or studio**, used unchanged, with no flags. Chrome reflects grey mush.
5. **Hue-less shadows** and hard CG light edges ([Fox Renderfarm](https://www.foxrenderfarm.com/share/5-tips-to-make-rendering-more-realistic)).
6. **Everything centred:** horizon at mid-frame, subject at frame centre, no negative space.
7. **Uniform-speed camera:** linear orbits, a 360° turntable, moves that start and stop on screen.
8. **Over-saturation and too many hues.** More than three colours compete.
9. **Post as a crutch:** bloom added to "save the day" ([SketchUcation](https://community.sketchucation.com/post/1118022)), chromatic aberration and vignetting overdone ([D5](https://www.d5render.com/post/postprocessing-vignette-chromatic-aberration-for-photorealistic-3d-rendering)). An over-graded image looks "as fake as un-graded ones" ([Studio Matrx](https://www.studiomatrx.org/students/architectural-visualization-rendering/anatomy-of-a-great-render)).
10. **The plastic look:** a dielectric with specular 0.5, roughness 0.3 and a saturated albedo everywhere.
11. **Floating objects:** no contact shadow or occlusion.
12. **No atmosphere:** infinite clarity, with the background as crisp as the hero.
13. **No depth layering:** no foreground, a flat background.
14. **Too many objects and too many ideas per shot.**
15. **Dead-still camera** or, the opposite, sine-wave "shake" with no rotation.
16. **Motion without blur**, or blur on a static hero.

---

## 7. Principles for Anikit agents

1. **Write the shot intent first,** in one sentence: subject, emotion, and what the light reveals. Then pick the lens, the ratio and the set. Never start from objects.
2. **Ratio, always:** 3:1–4:1 for hero product, 2:1 only for bright lifestyle, 8:1 or more for reveals. Never even light.
3. **Key at 15–45° off-axis and 15–45° up, from a motivated direction.** Fill on the other side, lower. Rim or kicker at 120–180°. Never symmetrical.
4. **Light metal and glass with strips and flags.** Use two tall strips at about ±110° behind the product and black cards at about ±75°, aligned with the edges so that highlights run *along* the form. On a white cyc, chrome always gets a black card.
5. **Animate light, not just the camera.** Reveal with edge lights only (16:1), sweep a strip across the hero over 1–2 s, then fade in fill to resolve.
6. **Use one colour idea:** a neutral set, one accent on the product, a warm key with a cool fill (or the reverse). No more than three hues in a frame.
7. **Use hero lenses at 85–100 mm and f/4–f/8.** Macro inserts at 100 mm with focus on a single feature. 24 mm only for scale and worlds. Keep the subject away from the edges of wide frames.
8. **Keep it slow and eased:** orbit ≤ 15°/s and ≤ 60° per shot, push-ins of 2–5% of the distance per second. Ease every start and stop. No 360° turntables.
9. **One move per shot:** push *or* orbit *or* crane, plus at most one rack focus (0.6–1.2 s, with a 0.5 s hold either side).
10. **Use a 180° shutter by default,** 90° for impacts, and add a subtle rotational "breath" (amplitude about 0.002 of the frame, about 0.3 Hz) to every locked shot.
11. **Ground everything:** contact shadow, a plinth or sweep, haze that grows with distance. Lift glass 1 mm and never float the hero unintentionally.
12. **Build three depth layers:** a soft dark foreground element, the hero, and a hazy lower-contrast background.
13. **Use imperfect materials:** bevels on every edge, roughness varied by ±0.05–0.1, smudges on glass and screens, no pure 0/1 albedo.
14. **Keep post restrained:** bloom only on true emissives (threshold ≥ 1.0 linear), chromatic aberration ≤ 1 px at 1080p, grain about 0.016, vignette ≤ 0.2. Grade with lift/gamma/gain, not saturation.
15. **Leave negative space:** 50–70% empty in hero frames; horizon on a third; the subject on thirds unless symmetry is the point. Check each still at thumbnail size, flipped and desaturated.

---

## 8. Gap list against Anikit

**Status (overnight, 3D DESIGN):**
- **Built:**
  - lighting rigs with ratios and gobos (1, 2), including `pinspot` and `stage` with real local spots;
  - animated light: rig `animate: sweep|reveal` and local lights whose pos, target and intensity take expressions (3);
  - the handheld/breath noise model (5);
  - focus pulls onto named objects (6);
  - lens presets (7);
  - set builders: plinth, podium, steps, sweep, field, arches, terrain, landscape (9);
  - surface imperfections, plus machined patterns with anisotropy and live engraving (10);
  - height fog and drifting fog (14);
  - the look linter in `check`, spec-level (15, partly);
  - per-key camera easing, including `settle` (16);
  - practicals as one object (17);
  - decals for type on surfaces.
- **Open:**
  - light linking;
  - strip gradient profiles (4);
  - anamorphic, blades and cat-eye bokeh (7, 8);
  - a round-edge shader for meshes (12);
  - LUTs (13);
  - render-based linting: a value study and a ratio meter (15).

See `docs/video/scene3d.md` and the proofs in `docs/video/shots/3d-*`.


What `scene3d` already has is listed in `../scene3d.md`. Each gap below is mapped to the engine. "In progress" means code exists in the working tree but is not documented or committed.

| # | Gap | Why it matters (section) | Engine mapping / proposal | Priority |
|---|---|---|---|---|
| 1 | **Named lighting rigs with ratios** | §1.1–1.2: the most common slop tell is a missing ratio | **Built:** `crates/anikit-video/src/scene3d/rig.rs` has `rembrandt`, `loop`, `clamshell`, `product`/`edge`, `high-key`, `low-key`, `split`, `rim`, `golden`, `window`, `top` and `butterfly`, with `ratio`, `rim`, `angle` and `warmth`. To do: document it in `scene3d.md`, add light linking (rim on the hero only), and make `product` the default when chrome or glass is present. | P0 |
| 2 | **Gobos and cookies** | §1.5 | **Built** in `rig.rs` (`gobo`: blinds, window, leaves; `goboScale`, `softness`). To do: allow a gobo on any spot or `lights[]` entry (not only the rig key), irregular slats, and a projected texture (`#comp`) as a gobo. | P1 |
| 3 | **Animated lights** (`keys` on `lights[]`: dir, pos, intensity, colour) | §1.3: the light sweep, the edge-only reveal, the fill fade-in | Light specs accept no `keys` and no `=expr` (`spec.rs` light `check_keys`). Add the same key and expression support that objects have, plus rig-level `reveal` (edge only, then fill) and `sweep` presets. | **P0** |
| 4 | **Strip-light shapes with a gradient profile** | §1.3: real strips have soft-edged gradient reflections | `lights[].size` makes uniform boxes. Add a `profile` (gradient falloff across the width, or a texture) so chrome highlights get the premium soft-edge-with-hot-core look. | P1 |
| 5 | **Handheld and camera-noise model** | §3.4 | `camera.shake` exists, but it is a positional sum of sines in metres (`spec.rs` around line 993), with no rotation, no roll, fixed band ratios and no lens scaling. Replace it with rotational layered noise (three bands, low/mid/high Hz), amplitude as a fraction of the frame (independent of focal length), roll, and presets `breath`, `handheld`, `shoulder` and `run`. | P0 |
| 6 | **Focus-pull presets** | §3.2 | Keys have `focus`, but agents must hand-key distances. Add `focus: "objectName"` per key (focus tracks an object) and `rack: { from, to, at, duration }` with an eased pull and an optional slight focus breathing. | P1 |
| 7 | **Lens presets** | §3.1 | `focal` and `fstop` exist. Add a `lens` field: `wide24`, `normal50`, `portrait85`, `macro100` (with a close minimum focus and an auto-matched working distance), `tele135`; plus aperture `blades`, cat-eye bokeh toward the edges, and optical vignetting. | P1 |
| 8 | **Anamorphic** | §3.1, §6 | The 2D `lensFlare` has `lens: anamorphic` and `lightLeak` has streaks. scene3d DOF lacks oval bokeh (2× squeeze) and horizontal highlight streaks tied to *3D* emissives. | P2 |
| 9 | **Set builders** | §4 | Add types or scripts for `plinth` (cylinder or box with bevel and a reveal step), `steps`, `arch`, `sweep` (a cyc of any colour and size, not only the env preset), `field` (a column/tile/ribbon grid with jittered heights by expression), and `terrain` (a heightfield with fBm noise). All are possible today with `count` and expressions, but agents don't build them. | P1 |
| 10 | **Surface imperfections** | §2 | Materials take one scalar `roughness` (`MAT_KEYS`). Add procedural `roughnessVariation`, `smudge` (fingerprint/wipe masks on glass and screens), `scratches` (anisotropic micro-scratches visible only in the highlight), `dust` (on up-facing surfaces) and `edgeWear`, plus `anisotropy` for brushed metal and `clearcoatRoughness`. Clay already has fingerprints (`trace.rs`). | **P0** |
| 11 | **HDRI and PBR texture maps** | §1, §2 | **Done** (ASSETS): `env.hdri`, `material.texture`, textured glTF, see `docs/video/craft/assets.md`. Still to do: zebra-stripe and strip-studio HDR panels for lookdev (§2). | done |
| 12 | **Round-edge shader for imported meshes** | §2 | `extrude`, `text` and `box` have bevel/radius (box defaults to 6% of the smallest side). glTF/CAD `model`s have hard edges: add a ray-traced rounded-edge normal (`roundEdges: 0.002`). | P1 |
| 13 | **Colour grading LUTs** | §6 | Post already has CDL lift/gamma/gain, white balance, halation, chromatic aberration, grain and presets (`render/post.rs`). Missing: `.cube` LUT import, hue-vs-sat/hue-vs-hue secondaries, and named film looks with a split tone (warm highlights, cool shadows). | P2 |
| 14 | **Atmosphere with structure** | §1.6, §4 | Fog is uniform density. Add height fog (dense near the floor), noise-modulated haze that drifts, and distance-based atmospheric perspective on `cyc`/`studio`. | P1 |
| 15 | **Look linter / critic** | §4 check, §6 tells | A `--lint-look` pass that renders a draft and warns: ratio < 2:1, subject centred and horizon at mid-frame, linear camera starting or stopping on screen, orbit > 20°/s, more than 3 hue clusters, bloom over non-emissives, a floating object with no contact, chrome with no dark reflector. It also outputs a thumbnail, a flipped and desaturated value study, and a false-colour key/fill meter. This is the highest-leverage feature for *agents*, who cannot squint. | **P0** |
| 16 | **Camera-move speed units and easing curves** | §3.3 | Presets are eased (`smooth`), but keys have a single interpolation. Add per-key `ease` (bezier or named), `degreesPerSecond` for orbit, and a "settle" (ease-out overshoot ≤ 1%) for landing frames. | P2 |
| 17 | **Practical-light helpers** | §1.7 | Point lights exist. Add `practical` objects (a lamp, LED strip or screen whose emissive geometry *and* light match in colour and intensity) so motivated sources are one line. | P2 |

### The five to build first

1. Animated lights (#3).
2. The look linter (#15).
3. Surface imperfections (#10).
4. A rotational camera-noise model (#5).
5. Finishing and documenting the rig and gobo work already in `rig.rs` (#1–2).

Together they cover the tells in §6 that agents fall into most: flat light, perfect surfaces, dead cameras and the absence of a designed reveal.
