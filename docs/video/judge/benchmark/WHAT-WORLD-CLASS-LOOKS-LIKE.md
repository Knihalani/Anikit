# What world-class looks like: patterns across the benchmark set

This page sums up the 87 pieces in [INDEX.md](INDEX.md): the best short-form motion work we could find in 14 categories, from title sequences and launch films to idents, explainers, generative art, data stories, music videos and social. It says what they have in common, with numbers, so a judge (or a maker) can hold one of our renders against it.

**How sure the numbers are.** Category counts, lengths and tags come from `bench.py stats` over the front matter. Shot lengths are *est.* (each piece file says how they were estimated) unless marked *measured*, which means `anikit-video analyze` ran on a clip we may legitimately hold (Creative Commons). The "N of 79" counts (taken before the last 8 pieces, social and two top-ups, were added) of qualities come from a keyword pass over each piece's **What makes it world-class** section. They are **lower bounds**: an analysis can show a quality without using the keyword.

## 1. The numbers

| Measure | Value across the set |
|---|---|
| Pieces / categories | 87 / 14 (5–7 per category); 86 URLs verified through oEmbed, 1 on the studio page |
| Year | 1987–2026; 76 of 87 from 2015 on (classics: Prince *Sign o' the Times* lyric video 1987, Lambie-Nairn BBC2 1991, Se7en 1995, Catch Me If You Can 2002, Honda Grrr 2004, Rosling 2010, Metamorphosis 2012, Bear & Hare 2013, True Detective 2014) |
| Length | median 86 s (p25 53 s, p75 162 s; 37 of 69 known lengths ≤ 90 s). Commercial work clusters at **30 s** (Devialet, B&O, Air Max 32 s, Oppo 31 s, Weetabix 30 s) and **38–68 s** (UI launches 38–57 s, sport trailers ~60 s). Art and music pieces run 4–8 min. |
| Median shot (est., 44 pieces with an estimate) | **2.5 s overall**; 1.0 s broadcast/sport, 1.5 s idents and UI launches, 2.0 s product launches, 2.5 s CG, 3.0 s event openers, 3.5 s explainers, illustration and title sequences, 4.0 s award shorts, 8+ s data stories |
| Median shot (measured) | Blender Studio *Charge* (2022, CC BY): **1.62 s**, 46 shots, 10.3 cuts/min, 36% of cuts on a beat at 120 BPM, mean luma 0.25 (low-key), vignette 0.6. Calibration (not in the set): Blender *Spring* (2019): 4.5 s median, 9.7 cuts/min, 24% on beat. |
| Declared pace | steady 41, fast 35, calm 9, frantic 2 (one is Aphex Twin *T69 Collapse*, which failed the Harding flash test) |
| Type's role | none 34, supporting 32, lead 18, type-only 3 |
| Audio | music-led 52, VO-led 17, sound-design-led 14, mixed 3, lyric 1 |
| Medium (multi-tag) | 3D/CG 35, kinetic type 22, mixed 21, 2D vector 19, 2D illustrated 18, live action 16, stylised 3D 13, generative 12, cel 11, photoreal CG 10, UI 10, sim 9, stop-motion 9, data viz 9 |
| Palette (multi-tag) | limited 43, saturated 42, dark 39, high-contrast 25, brand colour 22, mono 12, light 11, neon 5 |
| Aspect | 16:9 75, 9:16 5 (social: Duolingo, Spotify Wrapped, Nike, Apple), wide/other 7 |
| Techniques (multi-tag) | camera move 50, depth of field 24, rhythmic edit 23, kinetic type 22, macro 22, morph 21, hard cut 20, long take 17, beat sync 14, grain 13, instancing 12 |

### What the numbers say

1. **Fast is not the same as many cuts.** The set's estimated median shot is 2.5 s, longer than the "fast" bar in `quality/BAR.md` (median ≤ 1.5 s). Only sport, ident, UI-launch and social work cuts that fast. The rest hold longer but **never hold still**: the pace comes from something changing inside the shot. That might be a camera that always moves (50 of 79 tag `camera-move`), a morph, a sim settling, a loop cycling, or secondary motion. Kurzgesagt holds 3–4 s with everything bobbing; *Feud* has no cuts at all for 76 s; Factory Fifteen's BBC Tokyo is one 60 s camera move through six worlds. Judge **dead air**, not cut count, unless the brief is sport or social.
2. **Dark and limited is the norm, not the exception.** 40 of 79 are tagged `limited` and 37 `dark`; only 10 are light-ground pieces. A world-class palette is usually 2–4 colours plus one accent, held across the whole film (*Feud*'s three tuned colours, CW's three named colours, Stranger Things' single red on black, Nothing's white light on grey).
3. **Most world-class motion is not type-led.** In 61 of 79 pieces type is absent or supporting. Where type leads (title sequences, Demo festival, Framer), it is a single typeface set with restraint, or the type is made of the film's own material (*Se7en*'s scratched emulsion, *Catch Me If You Can*'s rubber stamps, the Honda topiary). It is never a big centred sans headline over a backdrop.
4. **3D and real materials dominate.** 34 of 79 use 3D/CG, and stop-motion, live action and photo add another ~25. The flat-vector pieces that make the cut (Kurzgesagt, Oddfellows, Giant Ant, Champ's TED-Ed) all add texture, a light source or layered depth: crayon fills, halftone, airbrushed gradients, a motivated light cone.

## 2. Ten patterns (with the pieces that prove them)

1. **One idea, one device, used in every shot.** (17+ of 79 name it explicitly; nearly all show it.) True Detective's silhouette-as-window, BBC One's lens, CW's container shape, TNT's viewfinder T, Air Max's "air" vignettes, the Paris 2024 pavé square, Devialet's clockwork, *Cash In Cash Out*'s zoetrope. Asking "what is the device?" should get an answer in one phrase. *Test:* can you name what every shot means?
2. **The technique is the message.** WWF *Up in Smoke* is made of smoke; *Catch Me If You Can* is made of forger's stamps; Morph Studio's XREAL film is the product's blue; *Burn the Witch* uses innocent children's TV to make mob violence disturbing. The medium is chosen for the idea, not as a look.
3. **Material truth at a 100% crop.** (38+ of 79.) Knit pores, refraction in sapphire, layer lines on 3D prints, ink voids, fur, crayon grain. Nothing looks like default CG plastic or a clean vector default. *Test:* crop to 100%: is there texture and micro-detail, and does it hold without aliasing or banding?
4. **Motivated light, falling off to black.** (31+ of 79.) There is a key light that comes from somewhere (Nothing's Glyph LEDs, the light cone in Giant Ant's frames, Hublot's strip highlights rolling over bevels, Charge's practical lights) and real falloff. There is no flat fill and no glow over nothing.
5. **Restraint.** (26+ of 79.) One subject per frame, generous margins, a palette of 2–4 colours, one typeface, one accent. Pieces read in under a second per shot even at 1.0 s cuts (Paris 2024: 58 shots in 60 s, "always legible because each shot has one clear action and silhouette").
6. **Sound and picture are written together.** (34+ of 79.) Netflix's double hit lands with the N; Air Max's score is built from footsteps and breath; Wannerstedt gives each contact its own sound on the exact frame; Raven Kwok maps the spectrum visibly; Weirdcore deforms on individual drum hits. *Measured:* *Charge* puts 36% of its cuts on a beat. *Test:* mute it; does the edit still have the music's shape?
7. **Transitions are designed moments, not effects.** Shapes become the next scene (*Feud*'s cigar ash into Oscars), the camera travels through the eye and the cloth (B&O), architecture carries the cut (BBC Tokyo), and the medium changes in motion (Coldplay's 15 styles, Blinkink's World Cup moving from knit to enamel to nail art). In these pieces crossfades are rare and stock wipes don't appear.
8. **Real content, never placeholders.** Every UI piece shows real copy, real states and real data: Linear's issue IDs, Raycast's agent steps naming Linear and GitHub, Notion's "13 sources found", Whisk's actual images. Every data piece computes its motion from the data (3Blue1Brown's exact epicycles, the NASA CO₂ field, Tulp's point-cloud globe).
9. **Scale contrast.** (19+ of 79.) Tiny humans on a phone-as-landscape (Nothing), macro serifs then the whole word (Stranger Things), desk toys at architectural scale (Wannerstedt), person to cosmos (Kurzgesagt). This contrast is what makes a frame read as "a world" instead of "a layout".
10. **An arc, even in 4 s.** Netflix's 4 s ident goes mark, dive, release. Severance runs slow, peak, decay to its score. Ordinary Folk's manifesto rises in complexity and dimensionality in a straight line. B&O withholds the product until the last beat. Lists of moments are rare; escalation is the norm.

## 3. What separates them from what the user rejected

Set against `quality/BAR.md`:

| BAR.md failure | What the set does instead |
|---|---|
| Centred headline on a gradient | Type is absent or supporting in 61 of 79. Where it leads, it is set small and consistently (Severance: one small left-aligned face with an overline rule) or built from the world's material. |
| Soft blob backdrop | Grounds are flat colour, charcoal or black, a real set, or lit material. Gradients appear only as light on a surface (Raycast, Champ). |
| Placeholder UI | Zero placeholders in the UI category; every string is plausible product data. |
| Small content in an empty frame | Either one dominant subject filling the frame (Air Max, Devialet) or bento/grid density (Material 3, Oddfellows' 15-symbol grid). |
| Flat 2D only | 2D pieces carry light and texture (crayon, halftone, airbrush, DOF from a hot source in Ordinary Folk's act 3). |
| Slide-deck structure | One designed world with continuous transitions; 17 of 79 tag `long-take` and 10 `transitions-in-camera`. |
| Stock-demo look | Every piece has a specific cultural or physical source: Opera Garnier red and gold, a Hungarian village ferry, British lidos, bucket hats and bobbleheads. |
| Effect without subject | Effects are causal: fabric driven by data (Panoply), raindrop transients trigger events (Order from Chaos), Glyph light is the key light. |
| Slow single dolly | Holds of 3–4 s are fine **if** they move and change inside the shot; single unbroken moves work when they travel through several worlds (Tokyo, T69, Wide Open). |

## 4. Gaps against Anikit (from the pieces' "What Anikit would need")

Features the set leans on most (counts of pieces): camera 59, audio 54, post 42, scene3d 39, text 38, motionBlur 26, shape 26, image 20, transitions 18, mask 12, shader 10.

Gaps named by more than one piece, grouped:

- **Simulation:** soft-body, cloth and inflatables (Severance, Panoply, Air Max), liquid/FLIP and droplets (XREAL, Baidu), smoke and fire (WWF, True Detective), granular contact (Oppo snow), growth and reaction-diffusion (Order from Chaos, Oppo crystals).
- **Hand-made surfaces:** on-twos or on-threes timing presets and line boil (Golden Wolf, Accents, *Last I Heard* at ~10 drawn frames per second), crayon/pencil and risograph fills (Giant Ant, Oddfellows), rubber-stamp ink voids (Catch Me), film damage, gate weave and emulsion scratches (Se7en, Stranger Things).
- **Type:** variable-font axis animation (Demo, Pentagram MotoGP), per-glyph outline pieces (Stranger Things), type set into the world's material and lighting.
- **Character:** cel and rigged character animation with secondary motion (Kurzgesagt, Honda, Giant Ant, Weetabix), and digital humans (B&O's eye, Wide Open).
- **Light and lens:** volumetric strands with bloom (Netflix), refraction in clear materials (B&O, Hublot), DOF on illustrated planes (Bear & Hare).

Per-piece detail is in each file's **What Anikit would need** section; `bench.py stats` prints the full GAP list.

## 5. How the JUDGE should use this

1. Pick 3 references with `bench.py match --spec … --analysis …`, then read their files.
2. Answer their **Judge's checklists** for our render. Each "no" is a finding.
3. Apply the five questions every piece in the set passes:
   - **Idea:** can you name the device in one phrase, and does every shot use it?
   - **Material:** at a 100% crop, is there texture, and light with falloff?
   - **Restraint:** is there one subject per frame, a palette of 2–4 colours plus an accent, and one typeface?
   - **Life:** does anything stay static for more than ~1 s without a reason, and is the median shot appropriate to the category (sport and social ≤ 1.5 s; launch ~2 s; explainer and titles ≤ 4 s *with* in-shot motion)?
   - **Sound:** do the hits land on picture events, and does the edit have the music's shape?
