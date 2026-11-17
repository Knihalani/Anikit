# Finish & life: texture, secondary motion, particle depth, frame density

Research notes for agents that author 1080p films from JSON. These are recommendations, not rules: each one reads "tends to read as X because Y".
Tags: **[src]** = stated in the linked source. **[book]** = Williams, *The Animator's Survival Kit* (2001), read from the OCR text.
**[heur]** = my own working number, built from the sources plus craft convention. Tune it by eye.

Jury critiques these notes target: "procedural-looking, uniform texture" (§1); "rigid puppet motion, the eye opens as a vector morph" (§2); "one layer of soft dots" (§3); "sparse frames" and "small, un-choreographed type" (§4).

---

## 1. Hand-made texture that doesn't read as procedural

### Principles
- **Texture is placed on purpose, by shape, not laid over the frame.**
  - At Klaus, compositors chose "how much texture each shape received"; skin got less than clothing.
  - The grain was so subtle you "almost have to squint", and it **moves with the character** [src: befores&afters].
- **Texture follows form and light.**
  - Spider-Verse halftones and hatching scale with the shading.
  - Ben Reilly's hatching was steered by hand-painted **flow maps** along the muscle curvature, because it "needed to react to both lighting and his movement and deformations" [src: Foundry ×2].
- **Lock texture to the object, or it "swims".**
  - Marco Recuay on rim-light halftones that didn't track Miles: "he'll appear to be swimming through a pattern" [src: Foundry].
  - Screen-locked grain over moving shapes is the single biggest procedural tell.
- **The exception: texture as light, not material.**
  - Spider-Verse loosened some halftones on purpose "so they feel like lights and not a texture". Flare halftones scaled with light intensity, and scattered dots suggested glow [src: Foundry].
  - Screen-space texture is fine for light, glow and atmosphere. It is wrong for surfaces.
- **The texture vocabulary carries meaning.**
  - In Wolfwalkers, the town got a "very uniform kind of woodblock line" and the forest and wolves were "more scratchy". Construction lines were kept on purpose: "It's very deliberate" (Tomm Moore) [src: befores&afters, Frame.io].
  - Two or three texture *families*, each tied to a story idea, tend to read as designed. One global texture reads as a filter.
- **Many layered sources, not one noise.**
  - Across the Spider-Verse ran up to **45 brush layers** through a 40+ node stroke system; Imageworks built **25+ comp tools** for the first film [src: Foundry].
  - Klaus used up to **8 lighting layers** (key, bounce, ambient, occlusion, rim, eye specular, SSS …), then texture as a separate pass [src: befores&afters, Cartoon Brew].
- **Lines are hand-shaky but confident.** Arcane background lines are freehand and "slightly shaky"; Arcane sharpens areas characters touch and softens the rest [src: 80.lv].
- **A small palette, mixed subtly.** Kazuo Oga (Ghibli) worked with about **21 poster colours**: "even with a small number of colors, you can create a wide range… through subtle mixing" [src: Animation Obsessive].

### Techniques with numbers
- **Density follows value.** Drive stroke, dot or hatch density from the shading (dark = denser or larger marks), not from noise; this is the halftone principle [src: Foundry]. [heur] Map luminance to coverage over about 15–85%, and keep the lightest lights free of texture.
- **Direction follows form.** Orient hatch and brush strokes along a per-shape flow direction (the tangent of the silhouette, or the axis of the limb). [heur] Isotropic fBm with no direction is the commonest tell; give every surface texture a dominant angle within about ±20°.
- **Vary scale.** [heur] Use at least 3 distinct grain scales in a frame: paper tooth about 1–2 px, brush or dry-brush about 6–20 px, a large wash or blotch at 10–30% of the shape size. Larger marks go on larger, nearer shapes.
- **Opacity.** Klaus-level subtlety means surface texture modulates value by only a few percent [src: "squint"]. [heur] Use 3–8% value modulation for "paper/grain" and 10–25% for intentional brush texture. Anything above 30% reads as a pattern overlay.
- **Boil (texture on twos).**
  - Hand-drawn work runs at 12 drawings/s ("on twos"). Spider-Verse animated mostly on twos for "that sort of crunchy frame rate of 2-D animation"; Across the Spider-Verse switched between ones and twos by character [src: TheWrap, Gizmodo].
  - [heur] Re-seed texture or line jitter every 2 frames (12 Hz), or every 3–4 frames for calm scenes. Hold the re-seed per object, so neighbouring objects don't boil in sync.
  - [heur] Keep the amplitude at 0.5–1.5 px for lines and 1–3 px for texture offset. A full-frame noise re-seed every frame (24 Hz) reads as video noise, not a drawing.
- **Print and riso misregistration.** Riso shops warn of **0–3 mm** of plate shift; digital imitations use **1–3 px** offsets, with screen angles of 15°/75°/0°/45° per ink to avoid moiré [src: riso guides]. [heur] Offset per colour layer, constant within a shot; vary it between shots, not per frame.
- **Breaking repetition.** Tiling and repeat patterns are the known tell, which is why stochastic texturing was invented [src: Unity, Jason Booth]. Use non-tiling sources and domain-warp the coordinates. [heur] Give every instance its own random offset and rotation.
- **"Accidents".** [heur] Add 1–3 deliberate irregularities per shot, where an artist's hand would leave them: a dry-brush skip, a blot, a stroke overshoot at a corner. Clustered imperfection reads as hand-made; uniform imperfection reads as a filter.

### Studio vs amateur
- **Studio:** texture amount set per shape and per material; locked to the object; follows light and form; several sources; restrained opacity; sharper where the action is.
- **Amateur:** one full-screen grain or paper overlay; the same fBm on every surface; equal strength everywhere; re-randomised every frame.

### Anti-patterns
Isotropic fBm with uniform scale on every surface; grain in screen space while shapes move; perfect tiling; texture in the highlights; the same seed on every instance; boil at 24 Hz across the whole frame; texture strength that ignores value.

### Self-checks on stills and strips
1. Crop four 128 px patches from different materials. If they look interchangeable, there is too little texture vocabulary.
2. Take the difference of two frames 2 apart on a panning or moving object. Texture should travel with the object; if the grain stays put, it is screen-locked.
3. Run an FFT or autocorrelation on a 256 px crop. Off-centre peaks mean tiling; a radially symmetric spectrum means isotropic texture with no directionality.
4. Plot local texture variance against luminance. A flat line means density ignores value.
5. Count the distinct grain scales visible at 100% zoom. Target at least 3.

---

## 2. Secondary & overlapping motion

### Principles
- **Nothing starts or stops at once.** "If we don't stretch and overlap and delay parts, things just look like a piece of pasteboard cut-out moving around" [book, p.~158]. This is exactly the "rigid silhouette" critique.
- **Successive breaking of joints.** Motion travels from the hips and chest through shoulder, elbow, wrist and fingers. Each joint breaks in turn, often bending against the lead direction (Art Babbitt via Williams) [book; animationapprentice].
- **Delay parts.** Williams's walkthrough of a simple step back: move the hips and stomach first, "still delay the head", then "throw in a blink — follows last". His conclusion: "simply by overlapping parts we've injected life into a pedestrian situation" [book].
- **Tex Avery takes** were "a series of compound actions delaying bits, often just **2 frames apart**". The head leads the anticipation and everything else follows [book].
- **Break twinning by delaying one side.** When arms or hands move together, offset one by a few frames or tilt another part [book].
- **Physics categories for 2D secondary motion.** A survey of hand-drawn films found three [src: Willett et al., UIST 2017]: sway (37 instances: scarves, hair, necklaces), jiggle (43: tufts, ears, tutus) and trailing (6: tails, tentacles).
- **Tuning by weight** [src: Willett et al.].
  - Heavy objects sway "slowly, stiffly"; light ones move freely, often exaggerated.
  - Sway uses **soft** springs and many handles. Jiggle uses **stiff** springs and **few** handles, which gives a more rigid deformation.
  - Trailing parts follow the path of the root and keep their length, with stretch only on accents.

### Numbers
- **Timing on twos and ones** [book]. Normal acting goes on twos (12 drawings/s); fast or very smooth moves, and far-apart spacing, go on ones. Never pan on ones under a character on twos, or you get "stroboscopic jitter".
- **Walk tempo, frames per step at 24 fps** [book]: 4 = very fast run; 6 = run; 8 = slow run or "cartoon" walk; 12 = brisk natural walk (2 steps/s); 16 = stroll; 20 = elderly or tired; 24 = slow; 32 = very slow.
- **Walk shape** [book; heur].
  - The down position comes just after contact and the up position at the passing position. The head usually sits about one stride-beat behind the hips.
  - [heur] Vertical bob is about 2–4% of character height for a normal walk and 6–10% for a cartoon walk. Williams also shows a walk with the head "floating level" while the pelvis bobs, as a character choice [book].
- **Overlap offsets** [heur, consistent with Williams's "2 frames apart"].
  - Each successive link in a chain lags its parent by **1–3 frames**: torso → head 2f; shoulder → elbow → wrist about 1–2f each; hair or tail tip 3–6f behind the root.
  - The settle after a stop is **2–3 overshoots**, each decaying about 50–70%. Light parts (hair, ribbons) overshoot more and settle slower, in 12–24f; heavy ones (a coat hem) overshoot less, in 6–10f.
- **Anticipation** [heur]. About 15–30% of the main action's distance, in the opposite direction, lasting about ⅓–½ of the main action's time: 4–8f for a normal action, 8–16f for a big take. A take is about **8 frames** long [book].
- **Blinks.**
  - Real lids **close fast and open slowly**. Measured at 300 fps, the opening converges asymptotically, and symmetric ease-in/out curves "do not accurately mimic" it [src: Trutoiu et al., Disney Research].
  - Measured blink rates were 6.6, 8.2 and 27.0 blinks/min across three actors, so the rate is character-specific [src].
  - Animator rules of thumb at 24 fps: about 2f closing, 1–2f fully closed, 3–4f opening, 6–8f in total. The lower lid trails the upper by about 1f, and the brow dips slightly with the lid [src: tutorials; Animation Mentor].
  - Blink **during a head turn or gaze shift**: "The eye, switching focus from one side to the other, will blink en route" (unless the character is frightened) [book].
  - "Move pupil down with lid as if it's heavy"; distend the pupil to show the form of the eye [book].
  - **Lid squash:** the lid is a shape on a sphere. The closing lid curve bows down over the eyeball, and the closed lid sits *below* centre. The eye shape itself squashes about 5–15% vertically in the closed frame, and the cheek pushes up slightly [heur]. A straight-line vector morph between open and closed shapes misses all of this.
- **Breathing** [heur]. About 12–20 breaths/min at rest, which is a 72–120f cycle at 24 fps; excited is about 30–40f. Chest and shoulder rise about 0.5–1.5% of height. The inhale is a little shorter than the exhale, about 40/60. Head and arms inherit the motion 2–4f late.
- **Moving holds.** A character never fully freezes. Keep about 1–2 px of drift and the breathing going, with a blink every 2–6 s at irregular intervals [heur, from the measured rates above].

### How 2D rigs fake it (Moho / Duik / Rubberhose / performed rigs)
- **Bone chains with lag.** Hair goes on its own layer with 2+ bones [src: Moho class]. Duik's "Wiggle" and "Swing" add noise or pendulums to secondary bones [src: Duik class].
- **Spring follow.** In Willett's Follow rig, the handles track points on the root's *trajectory*, fixed by a 1D rope simulation, with springs between neighbouring handles. It exposes two controls: "stretchiness" (spring stiffness) and "follow strength" [src].
- **Good procedural motion.**
  - Critically to under-damped springs, with damping ratio about 0.3–0.6 for light parts and 0.7–0.9 for heavy ones [heur]; stiffness scaled by part length.
  - Collisions respected: the sleeve rides up, the fish squashes on the bowl [src: Willett].
  - Noise added **only** when the character is idle, at amplitude below about 1–2 px.
- **Bad procedural motion:** the same spring constants on every part; noise wiggles independent of the root motion ("floaty"); infinite oscillation; secondary parts that move while the root is perfectly still; every chain link with identical lag.

### Studio vs amateur
- **Studio:** hips lead; parts delayed 1–3f each; asymmetric timing; twinning broken; blinks motivated by thought or a gaze shift; lids that wrap the eyeball; parts that settle and stop.
- **Amateur:** the whole silhouette translates and rotates as one; symmetric ease on everything; blinks on a timer; an eye drawn as a 2-shape morph.

### Anti-patterns
Rigid group transforms; linear or symmetric blink curves; both arms in sync; secondary parts with the same phase as the root; constant-amplitude sine "idle" motion on everything; overlap that never settles.

### Self-checks on frame strips (sample every 2f across an action)
1. Track the centroids of the head, torso and hand. Their velocity peaks should be staggered by at least 1–3 frames; peaks within 0f of each other mean a rigid puppet.
2. For an extremity (hair, tail tip), plot its angle against the root's angle. You should see lag plus 1–3 decaying overshoots after the root stops.
3. Blink strip: the closing should take fewer frames than the opening, and the lid contour should be curved in the half frames.
4. Silhouette IoU: if consecutive key poses differ only by an affine transform (the IoU recovers to 1 after an affine fit), the motion is rigid.
5. Twinning: mirror the left limbs onto the right. If they match in the same frame, offset one.

---

## 3. Depth layering of particles & FX

### Principles
- **Layer by depth, and composite characters between the layers.**
  - Disney's *Prep & Landing* snow pipeline placed **three snow layers (R, G and B passes) through the shot camera**, and characters were "sandwiched between the layers", with no complex holdouts. Snow types such as gentle, flurry and blizzard came from a pre-cached library [src: Coony et al., SIGGRAPH 2010].
  - The multiplane principle: the further from camera, the slower the motion [src: multiplane histories].
- **Particles are actors in the environment.**
  - Ghost of Tsushima's art director asked that particles *show the wind direction*. The team built two layers of procedural gusts, made particles rise over uphill terrain, and changed the particle *identity* per biome: pampas fluff in fields, leaves in forests, ash in burnt areas.
  - They used **up to about 60,000 leaf particles** in some cases [src: PlayStation Blog / 80.lv]. Hidden "vorticles" drive the visible particles, so motion is coherent, not random [src: Game Developer].
- **Light makes particles visible.**
  - Cinematographers agree that rain, dust, smoke and snow show up **backlit or side-lit against a dark background**. Without backlight, rain becomes "a hazy blur", and film rain must be torrential to register [src: RedShark, CML, DIYP].
  - Particles need to be lit by the key: bright on the light side and in the shafts, near-invisible in shadow.
- **Sizes are not uniform; they follow a heavy-tailed distribution.**
  - Raindrops follow Marshall–Palmer: **N(D) = N₀·e^(−ΛD)**, with N₀ = 8000 m⁻³mm⁻¹ and Λ = 4.1·R^−0.21 mm⁻¹. Snow aggregates follow the same exponential form (Gunn–Marshall) [src].
  - So: many small, few large. A uniform or Gaussian size reads as synthetic.

### Techniques with numbers
- **Three layers minimum.**
  - **Far / haze:** [heur] many (about 60%), tiny (1–2 px), low contrast, 20–40% opacity, slow; tinted toward the sky or fog colour; no blur, or merged into fog.
  - **Mid:** about 30–35%, 2–6 px, 50–90% opacity; lit by the key, sharp at the focus plane, the main read.
  - **Foreground / bokeh:** few (about 3–8%; often just 2–10 on screen), 20–120 px; defocused discs with 30–60% soft edge; 10–40% opacity; moving 3–6× faster than the mid layer, often partly out of frame. [src: the cinematic-snow lib uses front flakes at 1.4–2.0× with blur, and near-camera flakes grow and fade.]
- **Parallax and size by depth** [physics]. Screen speed and screen size both scale as 1/depth. If far : mid : near depths are 8 : 2 : 0.5, then speeds and sizes are 1 : 4 : 16. Use a log spread, not a linear one.
- **Size distribution** [heur, from Marshall–Palmer]. Sample the size as `s = s_min − ln(u)·s_mean`, which is exponential, or use a power law with α about 2–3. Clamp so about 1–3% of particles are "hero" sized, 3–5× the median.
- **Motion blur.** With a 180° shutter at 24 fps (1/48 s), a streak is **half the per-frame displacement** [src: shutter guides]. Rain at 180° can wash into a sheet: use shorter exposure, or fewer and brighter streaks [src: RedShark]. For snow and dust, blur only the near layer.
- **Spawn and density** [heur]. Don't spawn uniformly. Use density gradients toward the light source, wind gusts or the horizon, and clusters (Poisson-disc plus a low-frequency density mask). Vary density 2–4× across the frame, with gust pulses every 3–8 s.
- **Per-particle variation** [heur].
  - Vary size, speed (±30%), rotation, opacity flicker (sparkle on 2–6% of flakes when facing the light) and drift phase.
  - Embers: about 5–15% hot white cores, a colour gradient as they cool, and short 0.5–2 s lives with a fade-out.
- **Interaction.** Particles collide or settle, react to characters and part around them. P&L used separate kicked-up chunk sims, and even hand-painted snow interaction where that was faster [src].
- **Effects-animation design.** Gilland's *Elemental Magic* treats effects as designed shapes with rhythm and timing, not simulation [src: book listings]. Magic can "invent our own set of physics", but it stays internally consistent [src: O'Reilly ch.6 excerpt].

### Studio vs amateur
- **Studio:** 3+ depth passes with characters between them; size and speed tied to depth; exponential sizes; wind-coherent motion; particles lit by the key and visible mostly in light; particle type matched to the place.
- **Amateur:** one screen-space layer of identical soft dots; uniform random spawn; Gaussian sizes; independent random drift; the same brightness everywhere.

### Anti-patterns
One layer; every particle the same blur, opacity and colour; particles brighter in shadow than in light; linear parallax; spawn rate constant over time; particles passing *through* characters with no response; bokeh discs in the far layer; infinite lifetime with no fade.

### Self-checks
1. Histogram of particle screen sizes. It should be right-skewed (exponential or power law), and the size range should span at least 10×.
2. Speed against size scatter. It should be positively correlated (near = big = fast); no correlation means a single layer.
3. Bin luminance by local key-light intensity. Particle brightness should rise with the light.
4. Density map, 8×8 grid count. Max/min should be at least 2; a flat map means uniform spawn.
5. In a 12-frame strip, the near layer should cross more than 25% of the frame while the far layer moves less than 3%.

---

## 4. Frame density without clutter

### Principles
- **Detail goes where people look, and people look at faces and story.**
  - Gurney's eye tracking (16 subjects) found that faces and figures took the gaze within the first 5 seconds. Almost nobody looked at the sky, the upper palm fronds or the middle of a trunk.
  - Viewers "leap from one point of interest to another", alternating between context and detail. Anomalies in peripheral vision (lichen patches) drew about 100% of viewers; a detailed leaf patch drew about 60%.
  - "Abstract design gets trumped by human stories" [src: Gurney Journey].
  - Untouched viewers spend about **3–4 s per image** [src: Gurney], so a held shot must read in under 2 s.
- **Selectivity.** Sargent kept faces sharp and backgrounds soft [src: Gurney]; Arcane sharpens interaction areas and softens the rest [src: 80.lv]. "Your focal point only works if something else is quieter" [src: art-education].
- **Rest areas are necessary.** A frame with equal detail everywhere has no hierarchy; "every area has the same level of detail" is the symptom [src].
- **Shared light and colour logic holds density together.** Klaus had a colour bible for the whole film and a colour script for at least 1 in 4 shots [src]; Oga used about 21 colours [src].
- **Density is built in layers of read, not by scattering icons.** Illustrators block the main structure first, then layer smaller details and characters [src: Rod Hunt]. Kurzgesagt illustrates about **200 panels per video** in Illustrator, mostly from scratch [src: "1200 hours"].

### Techniques with numbers [heur unless marked]
- **Depth planes.** Use 4–6 distinct planes: foreground framing (often dark and partly cropped), subject plane, mid set, background, sky or haze, with particles between them. The foreground frame occupies about 5–15% of the frame and is low-detail, darker or blurred.
- **Three levels of read.** Primary: the subject (1). Secondary: 2–4 supporting elements that tell context. Tertiary: texture and small environmental detail. Each level is roughly 2–3× smaller in scale and lower in contrast than the one above.
- **Detail budget.**
  - About 15–30% of the frame area at the highest detail and contrast, concentrated within about ⅓ of the frame width of the focal point.
  - About 30–40% at medium detail.
  - About 30–50% as rest areas: flat or gradient fields, sky, shadow masses.
- **Contrast concentration.** The darkest dark and lightest light, and the hardest edges, touch the focal point [src: value-study advice]. Peripheral contrast is about 50% or less of focal contrast; vignette 10–25% at the corners.
- **Environmental storytelling.** Put at least 2–3 objects per establishing shot that imply history or use (wear, footprints, a lit window, a half-done task). These count as secondary reads.
- **Count discipline.** Treat repeated small elements (windows, leaves, stars) as **one** texture read, varied in size (power law) and clustered. Don't place them as evenly spaced instances.
- **Life in density.** Something should move at every depth: background drift, mid secondary motion, foreground particles. Static density reads as a poster; layered micro-motion reads as a world.
- **Type** (for the "small, low-contrast, un-choreographed" critique).
  - Title type at 1080p should have a cap height of roughly 5–12% of frame height (54–130 px) [heur], with contrast against the local background of at least about 4.5:1.
  - Type should enter as part of the choreography: on a beat, with overshoot and settle, and secondary words offset 2–4f each.
  - Hold long enough to read twice: about 1 s for a 1–3-letter word, 2–3 s for a short phrase [src: title guides, which warn against fixed sec/word rules].

### Studio vs amateur
- **Studio:** dense *around* the subject, quiet elsewhere; foreground framing; 4+ depth planes with atmospheric falloff; repetition clustered and varied; every element serves a read.
- **Amateur:** either sparse (a subject on an empty gradient) or cluttered (uniform detail edge to edge, equal-contrast icons scattered evenly).

### Anti-patterns
A subject centred on an empty gradient; detail and contrast evenly spread; evenly spaced repeated elements; no foreground plane; everything at the same saturation; small type floating with no relation to the composition.

### Self-checks on stills
1. Edge-density map (Sobel magnitude, 16×16 grid). The top-20% cells should be contiguous and contain or touch the focal point. More than 50% of cells above the median of the busiest cell means clutter; less than 10% of the frame with any fine detail means too sparse.
2. Blur at σ ≈ 8 px and squint. The focal point must still be the max-contrast region.
3. Depth planes: cluster the frame by luminance and blur level. You should get at least 4 distinguishable bands from front to back.
4. Rest-area check: at least about 30% of the frame should have low local variance, and it should not all be one rectangle.
5. Grey-scale thumbnail at 160×90. Can you tell the subject, the setting and the time of day? If not, the hierarchy failed.
6. Type: measure cap height in px and the contrast ratio against the pixels behind it, and check that the strip shows staggered entry, not one fade.

---


## Gaps against Anikit (TASTE, routed to the owners)

These come from the CEILING-2 judge critiques (`deliverables/ceiling2/*/GAPS.md`).

- **Object-locked texture that follows form** (SHADER, COMPONENTS):
  - grain and hatch in the object's own space, re-seeded every 2–4 frames per object;
  - hatch direction from a flow or gradient field, with density driven by value;
  - at least 3 grain scales rather than one isotropic fBm.
  - Today screen-locked `post.grain` and uniform shader noise are the defaults, and both read as procedural.
- **Secondary and overlapping motion** (COMPONENTS):
  - chains with 1–3 frames of lag per link;
  - settles that shrink 50–70% per overshoot;
  - a blink that closes in ~2 frames, holds 1–2 and opens in 3–4, with the lid wrapping the eye.
  - Today parts are keyed by hand or move as one rigid silhouette.
- **Depth-layered particles** (SIM):
  - one emitter split into ≥ 3 depth layers, with size and speed scaling by 1/depth;
  - a skewed size distribution (1–3% hero particles);
  - particles lit by the key, so backlit ones glint;
  - characters compositable between the layers.
- **Frame density:** no engine gap. It is direction: 4–6 depth planes, detail concentrated near the focal point, 30–50% rest areas. `critique --render` could measure an edge-density map against the focal point later.

## Sources
- **Texture.**
  - Klaus: https://beforesandafters.com/2019/11/14/heres-what-made-the-2d-animation-in-klaus-look-3d/ ; https://cartoonbrew.com/feature-film/how-klaus-draws-on-centuries-old-artistic-principles-to-push-2d-animation-forward-182325.html
  - Spider-Verse comp and brushes: https://www.foundry.com/insights/film-tv/graphic-look-in-comp-spiderman ; https://foundry.com/insights/film-tv/across-the-spider-verse-nuke-mari-katana
  - Spider-Verse linework: https://beforesandafters.com/2024/01/10/giving-spider-man-across-the-spider-verse-that-crucial-hand-made-quality/
  - Spider-Verse on twos: https://www.thewrap.com/spider-man-into-spider-verse-roadblock/ ; https://gizmodo.com/how-spider-man-into-the-spider-verses-technical-wizard-1831179866
  - Arcane: https://80.lv/articles/arcane-artists-show-how-they-combine-traditional-art-3d-for-backgrounds
  - Wolfwalkers: https://beforesandafters.com/2020/12/16/wolfwalkers-and-the-art-of-staging-shots/ ; https://blog.frame.io/?p=15484
  - Kazuo Oga: https://animationobsessive.substack.com/p/what-kazuo-oga-thinks-about-when
  - Riso: https://monikerpress.ca/printing ; https://jonathanstephens.us/play/riso/
  - Tiling: https://medium.com/@jasonbooth_86226/stochastic-texturing-3c2e58d76a14 ; https://unity.com/blog/engine-platform/procedural-stochastic-texturing-in-unity
- **Motion.**
  - Williams, *The Animator's Survival Kit* (OCR text): https://archive.org/details/taskav-01
  - Successive breaking of joints: https://animationapprentice.blogspot.com/2019/04/what-is-successive-breaking-of-joints.html
  - Trutoiu et al., blinks: https://la.disneyresearch.com/wp-content/uploads/Modeling-and-Animating-Eye-Blinks-Paper.pdf
  - Blink rules of thumb: https://www.animationmentor.com/blog/tutorial-animate-blinks-eye-movement/ ; https://www.bloopanimation.com/blinking-animation/
  - Willett et al., UIST 2017: https://pixl.cs.princeton.edu/pubs/Willett_2017_SMF/paper.pdf
- **Particles.**
  - Prep & Landing snow (SIGGRAPH 2010): https://media.disneyanimation.com/uploads/production/publication_asset/58/asset/prepEFX_xmasInJuly_sig2010.pdf
  - Ghost of Tsushima: https://blog.playstation.com/?p=345372 ; https://80.lv/articles/insights-into-building-vfx-for-ghost-of-tsushima/ ; https://gamedeveloper.com/design/using-vorticles-to-simulate-wind-in-i-ghost-of-tsushima-i-
  - Drop and snow size distributions: https://en.wikipedia.org/wiki/Raindrop_size_distribution ; https://www.sjsu.edu/faculty/watkins/raindrop.htm
  - Backlight: https://redsharknews.com/production/item/5204-everything-looks-better-with-backlight ; https://cinematography.net/edited-pages/Photographing_Rain.htm
  - Shutter: https://provideocoalition.com/the-180-shutter-angle-rule-is-broken
  - Layered snow reference implementation: https://gh.r-mj.com/lo-cafe/react-cinematic-snow
  - Multiplane: https://petapixel.com/2025/04/04/how-disneys-multiplane-camera-achieved-the-illusion-of-depth
  - Gilland, *Elemental Magic*: https://www.routledge.com/Elemental-Magic-Volume-I-The-Art-of-Special-Effects-Animation-1st-Edition/Gilland/p/book/9780240811635
- **Density.**
  - Gurney: https://gurneyjourney.blogspot.com/2009/09/eye-tracking-and-composition-part-3.html ; https://gurneyjourney.blogspot.com/2011/05/automated-selectivity.html
  - Resting space: https://www.milanartinstitute.com/blog/why-do-my-paintings-need-resting-space
  - Kurzgesagt: https://kurzgesagt.org/youtube/ ; https://rosetta.to/u/kurzgesagt/how-to-make-a-kurzgesagt-video-in-1200-hours
  - Rod Hunt: https://rodhunt.com/working-process-of-rod-hunt-illustrator-map-artist
  - Type read times: https://open-design.ai/blog/kinetic-typography-readability/

Gaps: I found no primary source for Fragapane or Giant Ant, Gilland's own particle numbers (403 errors), or BBC subtitle sizes (fetch blocked). Those numbers are marked [heur].
