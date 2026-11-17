# AI-made video and design tells (2024–2026), and what to do instead

Scope: the cues viewers, editors and designers use to flag work as AI-made or template-made, and how an agent that renders procedurally (vector, type, shaders, ray tracing, particles, generated music) can avoid them. Treat these as advice, not rules. Most tells are about *defaults*, not about the tool used.

## 1. Principles

1. **Viewers can't reliably spot AI, but they punish what they believe is AI.** In tests, people detect synthetic media at about chance: 51.2% across 1,276 participants [R1], and about 52% for Veo/Sora clips in a small 2025 study [R2]. Yet when people *believe* an ad is AI-made, they rate it 17% less premium, 19% less inspiring, 16% more artificial and 11% less trustworthy (Raptive, 2025) [R3]. AI display ads that were *perceived as human* got significantly higher CTR [R4]. So the goal isn't to hide the tool. The goal is to remove the cues that make work *read* as default.
2. **"Not ugly, generic."** Designers describe slop as "design with no author" [R5]. The tells are "not ugly, they are generic" [R6]. Every tell below is a mode of the training distribution: the statistically average font, gradient, light, camera, cadence or phrase.
3. **Editors judge editing, not pixels.** Six professional editors (5–13 years' experience) critiqued 70 AI cinematic ads. Their 870 critique sentences clustered into six dimensions: narrative progression, audiovisual coordination & sound design, visual composition & graphics, shot-to-shot continuity, message & brand coherence, and temporal rhythm & pacing [A1]. Continuity (99 sentences across 39 videos) and message/brand (97 across 38) drew the most criticism [A1].
4. **The tells keep moving.** In 2024 the UI tell was Inter on an indigo gradient [R6]. By 2026, Wired and Creative Bloq were reporting that AI brands had moved to *serifs on warm paper* to look human, and critics call it "tasteslop" [R7][R8]. So any fixed "safe" style becomes the next tell. Avoid defaults, not specific styles.

## 2. Tells by medium, each with what to do instead

### 2a. Generated-video artifacts (Sora / Veo / Runway / Kling / Seedance)
| Tell | Evidence | Do instead (procedural pipeline) |
|---|---|---|
| Morphing hands, merging objects, things passing through each other | Hands blending or bending impossibly; a hand passing through a box [R9][R10] | Use real collision. Keep topology fixed per shot. Never cross-fade two solids as a "transition" |
| Cause and effect broken (eating, pouring, impacts) | Physics-education guides: track one object over several frames and ask whether its motion matches the forces [R9] | Simulate the interaction (springs, rigid body, fluid). Add anticipation, contact frames and follow-through |
| Backwards motion, wrong-facing body | Editors: "the car is driving backwards"; "the lady's head is backwards while she is walking forwards" [A1] | Derive orientation from the velocity vector. Assert facing·velocity > 0 |
| Floaty, linear, jitter-free camera ("flight simulator") | [R11] | Give the camera a physical rig: mass, damped spring, a little handheld noise (1/f, sub-pixel). Hold some shots locked |
| Same flat light in every shot ("pleasant cloudy afternoon"), no hard shadows | [R11] | Pick a key-light motivation per scene (sun, practical, window). Use hard shadows and contrast ratios that differ between scenes |
| Oversaturated reds, teals and yellows; orange skin; "plastic" smooth surfaces | [R11][R12] | Grade with intent: limited palette, lower chroma in midtones, real roughness maps, micro-variation and grain |
| Inconsistent grade across shots | Editor: two sunset shots "feel filmed on different cameras" [A1] | Apply one LUT or grade per film, then vary only by an intended, scripted amount |
| Gibberish in-scene text that changes per frame | [R11] | Use real type only (we render vectors). Never draw pseudo-text texture |
| Background extras moving oddly; tiled textures | [R11] | Use instanced crowds with distinct phases. Break tiling with noise and stochastic texturing |
| Stitched 8-second clip structure | Veo 3 outputs 8-second clips, so long videos made of stitched clips are suspicious [R10] | Vary shot lengths (see 2e). Avoid equal-duration blocks |
| Lip and audio drift, lips not closing on p/b/m, breathless TTS, "TTS plateau" cadence | [R10][R11] | If VO is used, keep breaths every 5–10 s and vary the prosody. Prefer no VO over flat VO |

The 2024 Coca-Cola "Holidays Are Coming" AI ad shows several of these in one piece. Truck wheels "glide rather than rotate", wheels flicker, lighting is awkward, and Santa has a "dead-eyed stare" and "rubber skin" [R13][R14]. It still tested well with general consumers (System1) [R13]. The backlash came from craft audiences and from social media. Wheels that don't rotate are a *kinematics* tell, and they are trivially fixed when you compute motion rather than hallucinate it.

### 2b. Motion-graphics and template tells (Canva / CapCut / AE presets / "AI explainer")
- **Preset in/out on every element.** The same slide-up and fade or scale-pop with the same ease, staggered at a fixed interval. Template kinetic type is built for the first 0.5–1.5 s of the scroll: "oversized typography, kinetic motion, high-contrast backgrounds" [R15]. Commentators flag "homogeneity, as more brands… rely on prebuilt motion logic" [R16]. *Instead:* give each element a behaviour tied to meaning (Caption with Intention maps type to intonation [award-level S7]). Use 2–3 distinct easing families with overshoot or drag where the physics calls for it. Use non-uniform stagger based on reading order or a beat grid.
- **Effects that feel "applied" rather than "composed"** (even a vendor makes this contrast [R17]). *Instead:* make the composition generative. One camera axis or grid drives everything (DIA's "logic engine" [award-level S12]).
- **The "AI explainer" look:** a dark hero, a blue-to-purple gradient, three rounded cards with thin-line icons, glassmorphism, glowing orbs [R6][R18]. *Instead:* use a flat background with one strong accent colour, and composition with real negative space and asymmetry.
- **Wrong transitions.** Editors called a slide wipe "jarring" and disliked a dissolve. A logo cut to black was "abrupt". A better fix was to "fade the logo into the sky plate" [A1]. *Instead:* motivate transitions with match cuts, motion carry-through or an L-cut sound. Cut on action (52% of human cinematic ads use it [A1]).

### 2c. Typography tells
- **Default faces:** Inter, Roboto, Arial, Open Sans, system-ui. One guide calls the typeface "the single strongest tell" [R18][R19]. Also the Tailwind-style size ladder 12/14/16/18/24/30/36/48/60/72 instead of a modular ratio [R18].
- **The 2025–26 "humane" swing:** serif display on a warm, brownish "book page" background (Claude, Perplexity, Runway, Manus), now criticised as "tasteslop" [R7][R8]. The "elegant serif on cream" look is now *the* AI-brand look.
- **Gradient text**, the indigo→purple gradient ("bg-indigo-500") [R6], and centred stacks of headline / subhead / button repeated across unrelated products [R6].
- **"Over-polished" type that "feels off on closer inspection"**: inconsistent spacing, no optical kerning, uniform tracking at all sizes [R20].
- **Captions with poor legibility:** low-contrast captions, text sitting on the subject's face, text over busy detail ("you can't quite see the text") [A1].
- *Instead:* choose type for a *reason* in the story. Severance used a custom, slightly "off" grotesk because the company is slightly off [award-level S1], and DEMO used Graphik Wide at extreme sizes [award-level S11]. Tighten tracking as size grows. Left-align or set on a grid. Use one family at 2–3 weights with strong scale contrast (≥3:1). Never use gradient fills on text. Keep captions in safe areas and away from faces, with a contrast ratio ≥4.5:1.

### 2d. Copy tells in on-screen text
- **Vocabulary**, from Wikipedia's maintained catalogue [R21]. 2023–24: delve, tapestry, intricate, testament, pivotal, crucial, vibrant, meticulously, boasts. 2024–25: align with, enhance, fostering, showcasing, highlighting, underscore. Mid-2026: leverage, multifaceted, dependable, prioritize, "quietly", "steady". Marketing variants: Unlock, Elevate, Seamless, Empower, Reimagine, "Build faster. Ship smarter." [R6].
- **Constructions** [R21]: negative parallelism ("not just X, but Y", "it's not…, it's…"), the rule of three, trailing "-ing" clauses, "serves as" instead of "is", vague attribution ("experts say"), and heavy em-dash use.
- **"Weightless headlines" and interchangeable feature text** that doesn't say who it is for [R6].
- *Instead:* write on-screen copy as concrete nouns and numbers ("56 puppets, 42 real voices" is the register of the Telstra case [award-level S4]). One idea per card. Avoid aphorisms and triplets. Prefer specific verbs. Every line should fail the swap test (it shouldn't fit any other brand).

### 2e. Edit, pacing and sound tells
- **Flat energy.** "Stayed at roughly the same energy for all 14 seconds, with no crescendo before the logo" [A1].
- **Music cut off abruptly** ("more so a mistake than an artistic choice") and silent openings that make viewers check their volume [A1].
- **Missing sound for visible action** (a basketball shot without a swoosh felt "incomplete") [A1].
- **Repetitive framing:** "Shots from second 4–7 are all framed the exact same way"; four cuts in three seconds, all with the same framing [A1]. Jump cuts where the camera barely moves read as a stutter [A1].
- **Narrative order errors:** drinks delivered before they're made, a key turned before approaching the car [A1].
- **Pacing that ignores the subject's register:** fast cuts for a mattress or yoga brand ("you want this ad to feel relaxing") [A1].
- **Reference numbers (human cinematic ads, n=99):** 17.9 s average, 15.2 shots, 1.2 s mean shot, 44.4% use text overlays [A1].
- **AI music tells:** spectral "shimmer" and hiss from diffusion denoising [R22]. Reverb-like smear on vocals, and vocals that bleed into the backing because the track is one stream [R22][R23]. Harshness at 3–6 kHz [R22]. Monotone vocal inflection [R24]. "Too glossy, too perfect" [R25]. Rigid grid timing with zero velocity variation [R22]. Even energy with no arrangement arc (generic stock bed).
- *Instead:* compose arrangement first: intro, build, drop or peak at 60–80%, then the out. Add velocity and micro-timing humanisation (±5–15 ms, velocity ±10%). Give the visible impacts transient-aligned hits. Use deliberate silence (with ambience so viewers don't check their volume). End on a resolved cadence, never a truncated bar.

### 2f. The corporate-AI aesthetic, in one picture
Dark gradient hero, glowing orb or particle swirl, glassmorphic cards, Inter or a soft serif on cream, an "Unlock the future of X" headline, a slow push-in on a floaty camera, everything lit identically, an uplifting stock pad with a riser, and a logo cut to black. Each of these is the mode of its distribution [R6][R11][R18]. The award-level antidote is in `award-level.md`: one idea, one owned material, one invariant, an energy arc, and sound as a co-author.

## 3. Studio vs amateur, seen through the tells
- Studios grade shots to match. AI and amateur work changes colour temperature from shot to shot [A1][R11].
- Studios motivate every transition. Templates use wipes and dissolves as decoration [A1].
- Studios draw or choose type per project (Severance, Clarkson, Graphik Wide). Defaults show the toolchain's fonts [R18][R19].
- Studios score to the picture. Stock beds keep the same energy throughout [A1][R24].

## 4. Anti-pattern checklist (flag if any is true)
1. The font is Inter, Roboto, Arial, Open Sans, Poppins or Montserrat, or a generic display serif on a cream/beige (#F5F0E8-ish) background, with no reason tied to the concept.
2. Gradient-filled text, or an indigo→violet / teal→purple gradient anywhere.
3. A centred headline-subhead stack appears in more than one scene.
4. Every element enters with the same ease and duration, with a constant stagger.
5. Camera motion is constant-velocity or has a single global ease, with no hold.
6. Shot lengths are all within ±10% of each other, or come in blocks of ~8 s.
7. All scenes share the same light direction and softness.
8. Mean saturation is above about 0.6 in HSV across the frame, or skin hue is shifted toward orange.
9. The copy contains: unlock, elevate, seamless, empower, reimagine, delve, tapestry, testament, journey, "not just…but", em-dash triplets, three-adjective lists.
10. The music has no section change, or ends with a cut rather than a cadence.
11. A visible impact has no sound, or audio peaks don't line up with visual events.
12. Particles or glows don't obey the scene's physics (no light falloff, no occlusion, no motion blur).
13. Logo cut to black at the end with no motivated transition.
14. A text caption overlaps a face or the product, or has contrast below 4.5:1, or sits outside the safe area.

## 5. How the agent can check its own output (automatable)
- **Font audit:** parse the spec and fail on fonts in the default list unless the spec carries a `reason` field. Measure the type-scale ratio (ratios near 1.125–1.25 between all steps look like a default ladder; use ≥1.5 contrasts for display).
- **Easing entropy:** histogram of (ease, duration) pairs over all tweens. If one pair covers more than 50% of tweens, it reads as a template.
- **Stagger regularity:** coefficient of variation of the inter-element delays. Below 0.1 is mechanical.
- **Shot-length stats:** compare against 1.2 s mean and ~15 shots per 18 s for cinematic social [A1]. For calmer subjects, deliberately longer. Check that variance is above zero.
- **Energy curve:** per-second optical-flow magnitude plus audio RMS. Require a peak in the 55–85% window and a lower opening.
- **AV sync:** detect onsets in the audio and motion peaks in the video. More than 50% of visual impacts should fall within ±1 frame of an onset.
- **Grade consistency:** per-shot mean Lab and white point. ΔE between consecutive shots should be small unless the spec scripts a change.
- **Saturation and skin:** frame-wise HSV saturation histogram; flag a 95th-percentile S above 0.85 over large areas.
- **Legibility:** contrast ratio of each caption against its local background (≥4.5:1). Check overlap with salient regions (faces, product bbox) and the 90% safe area [A1].
- **Copy lint:** regex for the vocabulary and constructions in 2d [R21]. Count em dashes (at most one per film).
- **Swap test (LLM self-critique):** "Would this frame or line work unchanged for a different brand?" If yes, it's generic.
- **Five-frame poster test** and a **one-sentence idea test** (see award-level.md §6).


## 6. Gaps against Anikit (TASTE)

Already checked by `critique`:
- the house palette and default type (`sameness`);
- one entrance everywhere (`sameness`);
- equal shot lengths (`rhythm`);
- energy arc (`arc`), abrupt ending (`ending`), flicker;
- contrast and safe areas;
- the stock corporate bed (`music`).

Cheap to add as tips:
- **Easing entropy:** one (ease, duration) pair covering more than 50% of tweens.
- **Stagger regularity:** the CV of delays is under 0.1 across more than 6 elements.
- **Copy lint:** unlock, elevate, seamless, empower, reimagine, delve, tapestry, testament, journey, "not just … but", and more than one em dash per film.
- **Logo cut to black** with no motivated transition.
- **Grade drift:** mean Lab between consecutive shots jumps with no scripted change. This one needs `--render`.
- **Visible impact with no sound:** a camera `shake` with no `sfx` within ±2 frames.

The tells move: Inter and indigo in 2024, serif on warm paper in 2026 ("tasteslop"). The checks therefore target *unchosen defaults and uniformity*, not particular fonts or colours.

## Sources
- [A1] Law, Li, Narechania, "How Do Professional Editors Evaluate the Editing Quality of AI-Generated Cinematic Video Ads?", arXiv 2608.24329 (2026): https://arxiv.org/abs/2608.24329 ; full text: https://arxiv.org/html/2608.24329
- [R1] "As Good As A Coin Toss", arXiv 2403.16760: https://arxiv.org/abs/2403.16760v1
- [R2] Maastricht Univ. (Kuenn), Veo/Sora detection, 2025–26: https://excellence-edlab.maastrichtuniversity.nl/sites/excellenceedlab/files/2026-05/2025-2026%2C%20Media-Steffen%20Kuenn.pdf
- [R3] Raptive study via PPC Land: https://ppc.land/raptive-study-shows-ai-content-cuts-reader-trust-by-half/
- [R4] "AI in Disguise" working paper (MSI/TUM): https://www.msi.org/working-paper/ai-in-disguise-quasi-experimental-analysis-of-a-large-scale-deployment-of-ai-generated-display-ads/
- [R5] Kosta Canatselis, "Spot the slop": https://world.hey.com/kostac/spot-the-slop-a-ui-designer-s-guide-to-fixing-ai-defaults-4c448c9c
- [R6] 925 Studios, AI slop fonts and gradients: https://www.925studios.co/blog/ai-slop-design-tells
- [R7] Wired, "AI Has Come for Serif Fonts" (mirror): https://wirednews.pages.dev/story/ai-has-come-for-serif-fonts/ ; Keya Vadgama: https://keyavadgama.substack.com/p/the-serif-renaissance-in-ai-branding ; Xataka on "tasteslop": https://www.xataka.com/robotica-e-ia/ultimo-truco-empresas-ia-para-parecer-artificiales-usar-tipografias-clasicas-llegada-tasteslop
- [R8] Creative Bloq, "Why AI brands are obsessed with serif fonts": https://www.creativebloq.com/design/fonts-typography/why-ai-brands-are-obsessed-with-serif-fonts
- [R9] Popular Science, spotting AI video: https://www.popsci.com/diy/how-to-spot-ai-generated-video/ ; physics-error guide: https://geekchamp.com/how-to-spot-physics-errors-in-ai-generated-videos/
- [R10] TechRadar, Sora 2 tips: https://www.techradar.com/ai-platforms-assistants/openai/can-we-really-tell-whats-made-by-sora-2-11-tips-to-help-spot-ai-generated-video
- [R11] Opus, "The AI Slop Aesthetic: 12 Tells" (vendor): https://www.opus.pro/blog/ai-slop-aesthetic-12-tells
- [R12] Hailuo, neutralising AI saturation (vendor): https://hailuoai.video/knowledge/neutralize-ai-video-saturation-natural-cinematic-tones
- [R13] RetailBoss on Coca-Cola backlash: https://retailboss.co/coca-colas-ai-generated-holiday-campaign-and-the-backlash-that-went-viral/
- [R14] TechRadar on Coca-Cola: https://www.techradar.com/ai-platforms-assistants/coca-cola-faces-backlash-again-for-missing-the-spirit-of-the-holidays-with-another-ai-slop-ad ; Campaign Asia: https://www.campaignasia.com/video/coca-cola-faces-backlash-for-ugly-ai-generated-christmas-spot/499379
- [R15] CapCut kinetic-type templates: https://www.capcut.com/explore/kinetic-typography-motion-graphics
- [R16] TechRaisal on Template-by-CapCut homogeneity: https://www.techraisal.com/blog/template-by-capcut-the-invisible-engine-behind-every-viral-edit_1759743072
- [R17] AutoAE vs CapCut (vendor): https://autoae.online/blog/autoae-vs-capcut-pro-motion-not-basic-edits
- [R18] Pixelslop AI slop pattern catalog: https://cdn.jsdelivr.net/npm/pixelslop@0.3.8/dist/skill/resources/ai-slop-patterns.md ; Impeccable: https://www.mintlify.com/pbakaus/impeccable/concepts/anti-patterns
- [R19] UX-skill wiki, detecting AI slop: https://github.com/Laith0003/ux-skill/wiki/How-to-detect-AI-slop-in-your-design
- [R20] Venngage, AI slop in design: https://venngage.com/blog/ai-slop-in-design/
- [R21] Wikipedia: Signs of AI writing: https://en.wikipedia.org/wiki/Wikipedia:Signs_of_AI_writing
- [R22] Shimmer/hiss and stem harshness: https://jackrighteous.com/blogs/guides-using-suno-ai-music-creation/reduce-ai-music-noise-fix-shimmer-hiss-in-suno-ai ; https://www.roexaudio.com/blog/suno-stems-sound-bad-here-is-why-and-how-to-fix-them
- [R23] Newgrounds AI-audio moderation guide: https://www.newgrounds.com/wiki/help-information/site-moderation/how-to-detect-ai-audio
- [R24] Engadget, telltale signs of AI music: https://engadget.com/2308062/ai-music-difficult-to-detect-telltale-signs
- [R25] Start-Track, how to spot AI music: https://start-track.com/start-tracks-guide-on-how-to-spot-ai-music/
