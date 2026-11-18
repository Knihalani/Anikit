# Typography for screen and motion

Research notes for Anikit Video. The audience is the agent writing a JSON spec, and the engineers deciding what the text engine should do next. Every number here is a starting point to adjust, not a rule.

**Why this exists.** A user called our output "super boring and AI-slop-like typography". The cause was easy to see. We shipped three faces (Inter, Instrument Serif, JetBrains Mono), and the defaults were centred, medium weight and one modest size step. Two of those three faces are now the clearest tells of AI-generated design. Adrian Krebs scored 1,590 Show HN landing pages for "AI design" patterns; the font patterns he flags are *Inter for everything, especially centred hero headlines*, *Space Grotesk / Instrument Serif / Geist combos*, and *a serif italic accent word in an otherwise-Inter hero* ([Krebs, "Design slop"](https://adriankrebs.ch/blog/design-slop/)). In the Google Fonts popularity ranking (metadata API, Nov 2026), Inter is #5, Space Grotesk #60 and Instrument Serif #75 out of 1,961 families ([fonts.google.com/metadata/fonts](https://fonts.google.com/metadata/fonts)). The faces are good. The problem is a default nobody chose.

Units used throughout:
- `vh` is a percentage of frame height, as in the engine; at 1080p, 1 vh = 10.8 px.
- `em` is relative to the font size.
- Cap height is about 0.70–0.73 em for most grotesques (Inter: 0.727) and 0.62–0.68 em for old-style serifs and didones.

Axis ranges for Google Fonts families were checked against the Google Fonts metadata API on 2026‑11‑17.

---

## 0. The ten-second version (read this if nothing else)

1. **Choose a face for a reason you can say in one sentence** ("compressed grotesk because this is sport and the frame is wide"). If you can't, you are about to make slop.
2. **One family, two weights, one accent** beats three families. Make contrast with *size* first, *weight* second and *face* third.
3. **When type leads, push size contrast hard.** In motion, a headline at 30–60 vh beside a caption at 2.2–3 vh (a 10:1 to 25:1 ratio) reads as designed. 9 vh over 3 vh (3:1) reads as a slide deck.
4. **Set it flush left on a grid** unless the content is truly symmetrical (a single word, a logo, a date). Centred multi-line text is the #1 amateur tell.
5. **Track against size.**
   - Large lowercase gets tighter: −0.01 to −0.05 em.
   - Caps and small text get looser: +0.05 to +0.12 em ([Butterick](https://practicaltypography.com/letterspacing.html)).
   - Loose-tracked lowercase body text tends to fall apart into letters, because word shapes are what we read.
6. **Lead against size.** Display: 0.85–1.0. Text: 1.25–1.45 ([Butterick](https://practicaltypography.com/line-spacing.html)).
7. **Let type touch the frame.** Cropping, bleeding and filling 60–90% of the width are studio moves. A medium-sized word floating in the middle is not.
8. **Hold long enough to read.** Allow 0.3–0.4 s per word plus 0.5 s, and anything held for less than 5/6 s tends to go unread ([Netflix](https://backlothelp.netflix.com/hc/en-us/articles/215758617)).
9. **Animate one property with intent** (position *or* tracking *or* weight), on an expo/quart ease-out, with staggers under 40 ms per character. Per-letter bounce on everything is a template tell.
10. **Decide whether type leads or supports.** In the REFS benchmark (79 world-class pieces), type is absent or supporting in 61. When it supports, set it small, precise and consistent, on the image's own grid and out of the picture's way (see §3.6). Big type is a choice for when the words *are* the image.
11. **Detail is what signals craft:** real quotes (“ ” ’), en/em dashes, tabular figures in counters, `case` forms in all-caps, no widows, optical kerning at display sizes.

---

## 1. Type as image: the title-design canon and the moves to steal

Title design is where typography in motion was invented. Each master below gets a summary of *the move*, which is what an agent can reuse, not the look.

### 1.1 The founders

| Designer / work | What they did with type | Reusable move |
|---|---|---|
| **Saul Bass**, *The Man with the Golden Arm* (1955), *Vertigo* (1958), *North by Northwest* (1959), *Anatomy of a Murder* (1959), *Psycho* (1960) | *North by Northwest*: white sans-serif title blocks rise and fall along diagonal and vertical blue lines on green, and the grid dissolves into a skyscraper facade. It is "often credited as being the first sequence to use kinetic type" ([Art of the Title](https://artofthetitle.com/title/north-by-northwest)). *Psycho*: credits split across horizontal bars that slide past each other, legible only when the bars align ([BFI](https://www.bfi.org.uk/lists/celebrating-saul-basss-centenary-10-essential-title-sequences)). *Anatomy*: cut-paper limbs, with hand-cut, irregular lettering revealed by tearing paper ([Art of the Title](https://www.artofthetitle.com/title/anatomy-of-a-murder)). | **Type rides a structure.** Put the text on the lines of a grid or object and move it with them. **Fracture to signify**: slice words into strips that misregister, then lock. **Hand-cut irregularity** in one display face over flat colour fields. |
| **Pablo Ferro**, *Dr. Strangelove* (1964), *The Thomas Crown Affair* (1968), *Bullitt* (1968) | Hand-drawn grease-pencil lettering on glass, overlaid on aerial footage. Letters are tall, gangly, "squat, long, and lean" in turn, at deliberately incongruous sizes: a giant "A" dwarfs "STANLEY KUBRICK PRODUCTION" ([Art of the Title](https://www.artofthetitle.com/title/dr-strangelove/), [AIGA](https://www.aiga.org/membership-community/aiga-awards/2009-aiga-medalist-pablo-ferro)). Kubrick kept the rough comp over a refined version. Also multi-screen split panels. | **Scale anarchy inside one line.** Let a single letter run 3–5× its neighbours. **Thin, condensed hand lettering over live image**, filling the frame edge to edge. **Split-screen grids** carrying type in some cells. |
| **Maurice Binder**, Bond titles from *Dr. No* (1962), 14–16 films ([Art of the Title](https://www.artofthetitle.com/designer/maurice-binder)) | The gun-barrel open, silhouettes, liquid light. Type stays small, centred and modest so the image leads. | **Type as a quiet guest in a loud image.** Small caps at 2–3 vh, generously tracked, in pauses between visual beats. |
| **Kyle Cooper** (R/GA → co-founded Imaginary Forces in 1996 → Prologue in 2003 ([Wikipedia](https://en.wikipedia.org/wiki/Kyle_Cooper), [Art of the Title](https://www.artofthetitle.com/studio/prologue))), *Se7en* (1995) | Type "hand-etched into black-surface scratchboard", "manipulated during the film transfer process to further smear and jitter it", the transfer "cut up and reassembled… to add a final layer of temporal distress". It mixes hand lettering with Helvetica and, he says, "probably breaks several guild legibility rules" ([Art of the Title](https://www.artofthetitle.com/title/se7en/)). It is edited staccato to a remix of "Closer". | **Type as evidence of a character.** The letterforms come from inside the story. **Frame-level instability**: 1–3 px jitter, a 1–2 frame double image, flash frames of 2–4 frames, and a clean grotesk interleaved with raw marks. |

### 1.2 Studios (1996 →)

| Studio | Typographic signature | Move to steal |
|---|---|---|
| **Imaginary Forces** | *Mad Men* (2007, Mark Gardner and Steve Fuller): a Bass-descended silhouette with clean, small type ([Art of the Title](https://www.artofthetitle.com/title/mad-men/)). *Stranger Things* (2016, CD Michelle Dougherty): built from ~15 Stephen King paperback covers, set in ITC Benguiat (1977). It is purely typographic: red glowing strokes, letters drifting together in extreme close-up, and tight kerning that only resolves at the end. Scanned film grain and deliberate inconsistencies keep it from looking digital ([Fonts In Use](https://fontsinuse.com/uses/13992/stranger-things), [Print](https://www.printmag.com/culturally-related-design/stranger-things-logo-design/)). | **Macro crop + slow convergence.** Fill the frame with a 300–800% crop of glyph details. Slide letters along x over 6–15 s on a near-linear ease, with the logo resolving only in the last 1–2 s. |
| **Prologue** (Cooper) | *Spider-Man* (2002), *Iron Man* (2008), *The Walking Dead* (2010). Type built into the world: engraved, projected, extruded. | **Diegetic type**: set it as if printed, etched or lit on a surface in the scene. |
| **Elastic** (with Patrick Clair / Antibody) | *Game of Thrones* (2011, dir. Angus Wall): a mechanical map with simple Trajan. Typeroom calls Trajan "the most lazy font of all" for epics ([Typeroom](https://www.typeroom.eu/article/turn-your-logo)). *True Detective* (2014, Emmy): double-exposure "living photographs" with small, restrained credits ([Art of the Title](https://www.artofthetitle.com/title/true-detective), [Motionographer](https://motionographer.com/2014/01/24/patrick-clair-elastic-hbos-true-detective)). *Westworld*. | **Let the image carry the drama and the type carry the facts.** Credits at 2–3.5 vh in one roman, placed in the negative space of the composition, never over the subject's face. |
| **Picture Mill** | *Succession* (2018): home video, New York and money. The type is restrained small caps in an Engravers-Gothic-like wide face, "perfectly at home in the rarefied world of the Roy family" ([Art of the Title](https://www.artofthetitle.com/title/succession), [Jamie Clarke Type](https://jamieclarketype.com/blog/fonts-for-TV)). | **Signal class through restraint.** Use wide, small, all-caps, heavily tracked (+0.15–0.25 em) engraver-style caps at 2–2.5 vh. Wealth whispers. |
| **Extraweg (Oliver Latta) + Teddy Blanks** | *Severance* (2022 Emmy): soupy, squirming CG bodies set against lean, elegant, custom Helvetica-ish type. Typography and type animation are by Blanks ([Typeroom](https://www.typeroom.eu/creative-arts-emmys-2022-severance-main-title-design-winner-apple-tv), [Art of the Title](https://www.artofthetitle.com/title/severance/)). Lumon's in-world faces are Manifold Extended + Forma DJR. | **Counterpoint.** Pair chaotic image with cold, perfect, small type. The tension is the concept. |
| ***The Bear*** (FX) | Episode title cards: a custom ultra-bold grotesque, white on flat red, full frame, cut hard ([Art of the Title](https://www.artofthetitle.com/title/the-bear-season-1-episode-7/)). | **The hard card.** One heavy word at 25–45 vh on a flat field, a 1–2 s hold, a straight cut with no animation. The cut *is* the motion. |
| **COLLINS** | Brand systems built on custom variable type. Freeform: the variable Hans Grotesk, with "letterforms that swivel and twist" ([COLLINS](https://wearecollins.com/work/freeform)). San Francisco Symphony: variable type that responds to music ([Typeroom](https://www.typeroom.eu/collins-san-francisco-symphony-variable-type)). Dropbox: Sharp Grotesk with a "motion signature" ([COLLINS](https://www.wearecollins.com/work/dropbox/)). | **Axis as voice.** Map a data or audio signal to `wght`/`wdth`, so the type *behaves*, not just moves. |
| **DixonBaxi** | Max, TNT Sports (a custom serif by F37 with a mapped motion system) ([Promax](https://oldbrief.promax.org/article/fans-come-first-in-dixon-baxis-bt-sport-rebrand-to-tnt-sports), [Creative Boom](https://www.creativeboom.com/work/dixon-baxi-hbo-max/)). | **The type system is the motion system.** Define 3–4 typographic "behaviours" (slam, glide, stack, ticker) and reuse them everywhere. |
| **Pentagram** | Paula Scher's Public Theater: wood-type grotesques in mixed weights, jammed, tilted and stacked to fill the poster. MotoGP (with F37): a variable face whose width flexes with speed ([F37](https://f37foundry.com/custom-fonts/moto-gp)). | **Density and collision.** Words abut and overlap, with mixed weights in one block, at 0.8–0.9 leading. |
| **ManvsMachine** | Nike Flyknit, Air Max Day: CG materiality. The Flyknit identity used a custom Objectiv Fly, which Dalton Maag developed into a full font ([It's Nice That](https://www.itsnicethat.com/articles/man-vs-machine-nike-flyknit-innovation-190416)). | **Type as object.** Extrude it, light it and give it a material; keep the face plain so the material reads. |
| **Buck** | Hand-made 2D/3D hybrids where the lettering is drawn into the animation, not laid on top. | **Lettering that is animated, not text that is moved**: stroke-draw reveals, frame-by-frame boil (2–3 drawings at 8–12 fps). |
| **Sagmeister (& Walsh)** | Lettering cut into skin (AIGA Detroit, 1999), and type made of physical things (bananas, coins, branches) ([Wikipedia](https://en.wikipedia.org/wiki/Stefan_Sagmeister)). | **Type made of the subject.** Particles, objects or footage arranged into letterforms. Use it once per film, as the hero shot. |

**Common thread.** Each of them chose *one* idea about what the type *is* (evidence, a structure, a whisper, an object, a voice), then executed it consistently. None of them used "a nice font, centred, fading up".

---

## 2. Kinetic type: techniques with numbers

All timings assume 24–60 fps. At 30 fps one frame is 33 ms.

### 2.1 Reading time (the constraint everything else serves)

| Source | Figure |
|---|---|
| Brysbaert 2019 meta-analysis, 190 studies | Adult silent reading in English: **238 wpm** non-fiction, 260 fiction, range 175–300 ([J. Memory & Language](https://doi.org/10.1016/j.jml.2019.104047)) |
| Ofcom / BBC subtitles | **160–180 wpm** (2.7–3 words/s); >200 wpm is hard for many viewers ([BBC R&D](https://www.bbc.co.uk/rd/blog/2015-09-how-fast-should-subtitles-be)) |
| Netflix timed text | Minimum event **5/6 s** (20 frames at 24 fps), maximum 7 s; adult reading speeds of 17–20 cps depending on language ([Netflix](https://backlothelp.netflix.com/hc/en-us/articles/215758617)) |

Use these to schedule holds:

- **Hold time ≈ 0.5 s + words ÷ 3** (≈ 180 wpm). For dense or unfamiliar words (names, numbers), use ÷ 2.5.
  - 1 word: 1.0 s minimum, 1.5–2 s comfortable.
  - 5 words: ≈ 2.2 s.
  - 12 words: ≈ 4.5 s. Past that, split the card.
- **The clock starts when the last word is legible**, not at the start of the reveal. A 1 s stagger on a 6-word line adds 1 s to the scene.
- **A number counts as two words.** A sequence of 3+ numbers (a stat grid) needs about 1 s per number.
- **Words revealed one at a time** (word-by-word captions, "spoken" type) should run at **2.5–3.5 words/s**, synced to voice where there is one. 4–5 words/s is the short-form "fast caption" ceiling. Use it only when the audio carries the meaning.
- **Exits:** start them no earlier than the hold ends. Make them 60–70% of the entrance duration.

### 2.2 Stagger, duration, easing

| Unit | Stagger per unit | Duration per unit | Notes |
|---|---|---|---|
| Characters, fluid | 15–30 ms | 350–600 ms | Reads as one gesture. Keep the total spread ≤ 0.6 s. |
| Characters, deliberate | 40–80 ms | 400–700 ms | Titles of 3–12 characters only. |
| Characters, dramatic / typewriter | 80–150 ms | 0–80 ms (a step) | Long strings get boring fast; cap at about 2 s total. |
| Words | 60–150 ms | 400–700 ms | 90 ms is a good default. |
| Lines | 80–200 ms | 500–900 ms | Mask-up per line, 120 ms. |
| Blocks / layout elements | 150–300 ms | 600–1000 ms | Headline, then rule, then caption. |

Guidelines:

- **Total reveal ≤ 1.2 s for anything over 8 characters.** Distribute the stagger over a fixed total (`"total": 0.6`) rather than a fixed step, so long strings don't crawl.
- **Easing defaults:**
  - Entrances: `outExpo`/`outQuart`, roughly `cubic-bezier(0.16, 1, 0.3, 1)`. Material 3's "emphasized decelerate" is `cubic-bezier(0.05, 0.7, 0.1, 1)` ([M3 tokens](https://m3.material.io/styles/motion/easing-and-duration/tokens-specs)).
  - Exits: `inQuart` or `inExpo`, at 0.6–0.7× the entrance duration.
  - Position-locked things (credits roll, ticker, typewriter): linear.
  - Overshoot (`outBack`) at s ≤ 1.2, and only for playful or retro briefs. At s = 1.7 (the default in most libraries) type looks rubbery.
- **Ease the stagger distribution as well.** Apply `inQuad` over the index so the first letters land fast and the tail settles. Use `from: center` for symmetric words (logos, single-word titles); on sentences it tends to scramble the reading order.
- **Travel distances:** rise and drop moves of 0.3–0.6 em (or 20–40% of line height). More than 1 em reads as a slide-in, not a type entrance. Blur-in: 8–20 px at 1080p, resolved fully before reading starts.
- **Don't animate what you want read.** Blur, wobble or noise on text the viewer is reading is a tell. Animate in, *then hold still* (a 2–5% push-in is fine).

### 2.3 Technique cookbook

| Technique | Settings that look professional | Common mistake |
|---|---|---|
| **Mask reveal (line-up)** | Each line slides up 100–110% of its line height inside a line-bounded clip. 600–900 ms `outExpo`, 100–140 ms line stagger. Leading 0.9–1.05 so masks don't clip descenders: pad the mask by 0.15 em. | A mask that clips descenders or accents; a fade added on top. |
| **Tracking converge** | +0.25 → 0 em over 1.5–3 s `outCubic` (elegant); or +1.0 → +0.12 em on all caps over 6–12 s near-linear (cinematic, Stranger Things-style). | Tracking lowercase out to +0.3 em and *holding* it there. |
| **Tracking expand** | −0.05 → +0.08 em with an opacity drop on exit, 400 ms. | Using it as the entrance and the exit and the loop. |
| **Weight morph** | Punch: `wght` 100 → 800 in 250–450 ms `outQuart` as the word lands. Breathing: ±80–120 around 400 at 0.5–1 Hz. Lock the width with `wdth` or use a tabular/mono face, or the line reflows. | Animating weight in a face whose advance widths change, so the neighbours jitter. Recursive keeps widths "consistent across the range" ([Recursive](https://www.recursive.design/)). |
| **Width morph** | Anybody `wdth` 50 → 150, Mona/Hubot 75 → 125, Science Gothic 50 → 200. Pin the line to its left edge. 500–900 ms. Fits a word exactly to a box at the end. | Centre-pinned width morphs (both edges move, so nothing anchors). |
| **Scale extreme** | Type cap height at **40–80% of frame height**, cropped 5–30% off one or two edges, slow push 100 → 106% over the hold. One word only. | Scaling a whole sentence up. |
| **Slam / smash cut** | 0–2 frames in, a 1-frame white flash optional, a 1–3 frame 2–4% scale settle. Pair with an audio transient. Hold 12–24 frames, hard cut. | Easing a slam, which kills the impact. |
| **Typewriter** | 12–20 cps with ±30% per-character jitter (seeded). Pause 150–300 ms after a comma and 400–700 ms after a full stop. Caret blink 530 ms on / 530 ms off; hide the caret while typing. Monospace or `tnum`. | A constant 20 cps on 200 characters (10 s of dead time). |
| **Scramble / decode** | Each character cycles 3–8 random glyphs at 1–2 frames each before resolving. Resolve left to right at a 20–40 ms stagger. Draw from the face's own charset (caps for caps, digits for digits). Monospace or tabular, or the width jitters. | Cycling through symbols and lowercase on an all-caps title. |
| **Split-flap** | Each cell flips 4–12 times at 60–90 ms per flip, with a 30–60 ms column stagger. Each half-flap is a 2-frame hinge with a darkening on the back. Add a clack per flip, lower in the mix. | Using a split-flap without cells (no gap or hinge line). |
| **Highlight / marker** | The bar wipes at 300–500 ms `inOutCubic` behind the word. Bar height 0.75–0.95 em. Offset −0.08 em from the baseline. Colour at 100% chroma *or* the text inverts. | Highlighting 5 words in one line. |
| **Word-swap** (one slot, cycling words) | Each word holds 0.8–1.2 s; out-up/in-up 300 ms; slot width animates to the new word's width over the same 300 ms. | A fixed slot width, which leaves ragged gaps. |
| **Credits roll** | Constant speed, a whole number of px per frame (at 1080p/24 fps, 2–4 px/frame = 48–96 px/s) to avoid judder. Names on a centre axis with the role right-aligned left of it, gutter 1–1.5 em. | Eased rolls; speeds that aren't whole pixels per frame. |
| **Per-glyph scatter → assemble** | Seeded random offsets of ±0.5–2 em and ±15–45°, assembled over 700–1100 ms `outExpo`, with a 10–25 ms random-order stagger. | An unseeded random (different every render); rotation over 90°. |
| **Text on path** | Path offset animates at 0.5–1.5 em/s. Use a face with low contrast and even colour (curves exaggerate stroke modulation). Letters 0.04 em looser than normal. | Didones on tight curves. |
| **Type as mask** | Footage shows only inside heavy letterforms: weight ≥ 800, size ≥ 30 vh, a counter-rich face. Slow scale 100 → 115% of the *footage* inside a static mask (or the reverse). The reveal ends by expanding the mask to full frame (1–1.5 s `inOutExpo`). | Thin faces as masks: the image vanishes. |

### 2.4 Rhythm across a sequence

- **Vary tempo deliberately.** Use 3–4 fast cards (0.6–1 s, slams), then one long hold (3–4 s). Equal durations everywhere are what make AI edits feel machine-made.
- **Lock entrances to the beat grid.** At 120 bpm a beat is 500 ms; land the *last* letter on the beat, not the first.
- **Re-use one motion idea across 80% of the cards** and break it once for the climax (Elastic's "start dark before it completely changes").

---

## 3. Scale, measure, leading, tracking, safe areas

### 3.1 Modular scales and when to abandon them

| Ratio | Name | Steps from 2.4 vh caption | Use |
|---|---|---|---|
| 1.25 | Major third | 2.4 → 3 → 3.75 → 4.7 → 5.9 | UI, explainers with lots of text levels |
| 1.333 | Perfect fourth | 2.4 → 3.2 → 4.3 → 5.7 → 7.6 | Editorial explainer, data stories |
| 1.5 | Perfect fifth | 2.4 → 3.6 → 5.4 → 8.1 → 12.2 | Brand film, product launch |
| 1.618 | Golden | 2.4 → 3.9 → 6.3 → 10.2 → 16.5 | Posters, title cards |
| ×3–×6 jumps | "Poster scale" | 2.4 → 12 → 48 | Hero numerals, single-word cards |

- **In motion, use fewer levels and more contrast.** A video frame shows 1–3 text levels at a time, not 6. Pick 2–3 steps and *skip* the middle ones.
- **Headline-to-caption ratio:**
  - 3:1 is "corporate deck".
  - 6–8:1 is "designed".
  - 12–25:1 is "poster or editorial", for one-idea frames, stat reveals and title cards. A 40 vh numeral over a 2.4 vh label is 17:1.
- **Weight contrast compensates for small ratios.** If the headline has to stay under 8 vh (dense content), contrast 300 against 800, not 500 against 600.
- **Avoid the "medium everything" trap.** The engine's old styles were 9/6/3.6/3 vh with weights 800/700/500/400: four levels that are almost the same. Collapse them to *huge / small / tiny*.

### 3.2 Legibility minimums at 1080p (scale ×2 for 4K, keep vh for other sizes)

| Role | Minimum | Comfortable | Notes |
|---|---|---|---|
| Any text meant to be read | cap height ≥ 18 px (≈ 2.4 vh font size) | 3–4 vh | Compression and phones crush thin strokes below this. |
| Body / subtitle-like text | 2.8–3.2 vh | 3.5–4.5 vh | BBC online subtitles use a line height of about 8% of the active height for 16:9 ([BBC](https://bbc.github.io/csun/how_big_should_subtitles_be/index.html)). |
| Labels / small caps | 1.8 vh, weight ≥ 500, +0.08–0.12 em | 2.0–2.4 vh | Avoid hairline weights (< 300) below 4 vh. |
| Lower third: name / role | 4–5 vh / 2.6–3.2 vh | | Plate or scrim when over busy footage. |
| 9:16 vertical for phones | 1.4× the 16:9 numbers in vh | | Viewed at arm's length on a 6″ screen. |

- **Thin strokes and fine serifs** (didones, hairline sans) need ≥ 5 vh. Below that, use the face's lower `opsz`, which thickens hairlines and opens spacing.
- **Contrast:** aim for ≥ 4.5:1 luminance contrast for readable text, and ≥ 7:1 over moving footage. Fix contrast with value (scrim, darker plate), not with outline or glow.

### 3.3 Safe areas

| Area | Margin per side | Retained frame | Source |
|---|---|---|---|
| Action safe | 3.5% | 93% | [EBU R95](https://tech.ebu.ch/docs/r/r095.pdf); SMPTE ST 2046‑1 |
| Graphics/title safe | 5% | 90% | [EBU R95](https://tech.ebu.ch/docs/r/r095.pdf), [Netflix](https://backlothelp.netflix.com/hc/en-us/articles/4406208331923-Title-Safe-and-Safe-Action-Best-Practices) |
| Social 9:16 (rule of thumb) | top ≈ 12–14%, bottom ≈ 20–25%, right ≈ 12% | | Platform UI overlays: username, captions, buttons |

- **Safe area is for *information*, not for *image*.** Display type that is *meant* to crop can and should break it. A 70 vh word bleeding off the left edge is fine; the small caption under it stays inside 90%.
- **Margins as a design element.** Swiss work uses a large, consistent outer margin: 6–8% of width, generous and visibly equal. Place the information column's left edge on it in every card.

### 3.4 Line length (measure)

- **Body text:** 45–90 characters per line ([Butterick](https://practicaltypography.com/line-length.html)). Bringhurst gives 45–75, with 66 as the ideal ([webtypography.net](https://webtypography.net/2.1.2)).
- **On screen, read in a few seconds:** **25–45 characters** and ≤ 3 lines per card.
- **Captions:** ≤ 42 characters per line, 2 lines (the broadcast subtitle norm).
- **Display:** 1–4 words per line. Break by *sense*, not by box width: "The future / of film" rather than "The future of / film".

### 3.5 Leading (line height as a multiple of size)

| Context | Leading |
|---|---|
| Body, 3–4 vh, lowercase | 1.25–1.45 ([Butterick](https://practicaltypography.com/line-spacing.html): 120–145%) |
| Subheads 5–8 vh | 1.05–1.2 |
| Display lowercase ≥ 10 vh | 0.88–1.0 |
| Display all caps (no descenders) | 0.78–0.9; stacked compressed caps can go to 0.75 |
| Mixed-size stack (huge numeral + label) | Set by **baseline distance**, not by leading: label baseline 0.6–1.2 cap heights under the numeral's baseline |

Tight leading is the single fastest way to make display type look designed. 1.2 on a 15 vh headline looks like a web page.

### 3.6 Tracking

| Context | Tracking |
|---|---|
| Body lowercase, 3–4 vh | 0 to +0.01 em |
| Small text < 2.4 vh | +0.01 to +0.03 em |
| All caps / small caps, any size | **+0.05 to +0.12 em** ([Butterick](https://practicaltypography.com/letterspacing.html)) |
| Luxury / "engraver" caps, small | +0.15 to +0.35 em (deliberate, sparse) |
| Display lowercase 6–15 vh | −0.01 to −0.025 em. Inter's dynamic metrics give ≈ −0.022 em at ≥ 32 px ([rsms.me/inter/dynmetrics](https://rsms.me/inter/dynmetrics/)). |
| Display lowercase > 15 vh | −0.025 to −0.05 em; check that letter pairs don't collide (rn → m) |
| Compressed heavy caps (sports) | 0 to +0.02 em; they are designed tight |
| Monospace | 0. Tracking mono tends to break its grid, because every cell is already the same width. |

Kerning at display sizes:
- Default font kerning is tuned for text sizes. At ≥ 20 vh, inspect pairs like *AV, To, Ty, LT, r., f'* and quotes against capitals, and adjust by −0.02 to −0.06 em.
- Optical kerning, which computes spacing from shapes, helps with mixed faces and with fonts that have poor kern tables.

### 3.7 Optical size (`opsz`)

- Set `opsz` to the rendered size in **points at the intended viewing size**. A practical mapping for 1080p video is opsz ≈ font size in px × 0.75 (px → pt), clamped to the axis.
- **Large display:**
  - Fraunces at 144: high contrast, tight spacing, small x-height ([Fraunces](https://github.com/undercasetype/Fraunces)).
  - Playfair goes to 1200, which is meant for poster-scale hairlines.
- **Small captions:**
  - Literata/Newsreader at 6–9: thicker hairlines, wider spacing, larger x-height.
- **Deliberate mismatch is a technique.** Setting a big headline at a *low* opsz gives a sturdy, warm, "bookish" display. Setting small text at a *high* opsz looks fragile; avoid it.

---

### 3.6 Type in support (the common case)

In the REFS benchmark, type is absent or supporting in 61 of 79 world-class pieces. Where it supports the picture, its job is to be precise and get out of the way:
- **Small and consistent.** One face, one or two sizes (2–3.5 vh at 1080p), one weight, used the same way every time. Severance's credits use a small, left-aligned grotesque under a thin overline rule, always in the same place, so the image does the talking.
- **On the image's grid.** Align to an edge or a line already in the picture (a horizon, a doorframe, the edge of a product), not to the frame centre. Hold 4–6% margins inside title-safe.
- **Out of the subject's way.** Put text in negative space; over faces or the focal point it tends to fight the picture for attention. Move it between shots rather than covering the picture.
- **Quiet motion.** A 200–400 ms fade or a 4–8 px rise, or a cut on the beat. Supporting type that animates harder than the picture competes with it.
- **Detail still counts.** Tabular figures for data, real quotes, `case` punctuation in caps, +0.04 to +0.08 em tracking on small caps labels, a hairline rule (1–2 px) for structure instead of boxes or pills.

## 4. Type pairing

### 4.1 Principles

1. **Pair across *structure*, not just category.** Contrast skeletons (humanist vs geometric vs grotesque vs didone), widths (compressed vs wide) or construction (stroke-modulated vs monoline). Google Fonts Knowledge calls this the "font matrix" ([Google Fonts Knowledge](https://fonts.google.com/knowledge/choosing_type/pairing_typefaces_based_on_their_construction_using_the_font_matrix)).
2. **Superfamilies first.** A family or superfamily "often gives enough contrast" without a second face ([Google Fonts Knowledge](https://fonts.google.com/knowledge/choosing_type/pairing_typefaces_within_a_family_superfamily)). Good OFL superfamilies:
   - IBM Plex (Sans/Serif/Mono/Condensed)
   - Roboto Flex + Roboto Serif + Roboto Mono
   - Recursive (sans ↔ mono ↔ casual in one file)
   - Mona Sans + Hubot Sans (same metrics, different personalities)
   - Big Shoulders (+ Stencil, Inline)
   - Noto
   - Source (Sans 3, Serif 4, Code Pro)
   - Saira (+ Stencil)
   - Bungee (Regular, Inline, Outline, Shade, Hairline, Spice)
3. **One-family systems** look the most professional in motion, because every card shares proportions. Get hierarchy from size, weight and case.
4. **Give each face a job.** Display = voice; text = information; mono = data or system. Two faces doing the same job tend to read as indecision rather than contrast.
5. **Match x-height or cap height when two faces share a line.** Scale the second face 0.9–1.15× so cap heights align. Inline italic accents from another family are the classic "AI hero" tell.
6. **At most two families on screen at once, three per film** (the third is usually mono for data).

### 4.2 Fifteen worked examples of pairings (all OFL, on Google Fonts)

Read these as worked examples of the *reasoning*, not as a menu. If every film picks from the same fifteen, outputs converge again; the TASTE A/B found the same tracked generic caps in 4/4 pieces. To find a pairing beyond this list:
- **Start from the brief's era, place and material.** A 1970s Italian rail timetable, a Lagos bus, a Victorian playbill or a lab instrument each implies a construction: grotesque or slab, condensed or wide, ink-trapped or stencilled. Search the library by genre and axis (`anikit-video fonts slab`, `fonts wdth`), or pull letterforms from `anikit-video board` references.
- **Contrast structure, not just style.** Pair different skeletons (didone + grotesque, humanist + geometric mono), or one superfamily in two widths or optical sizes.
- **Give each face one job** (voice, information, data), and say that job in one sentence.
- **Vary across a project.** Consecutive films in the same pairing read as a template. Change at least the display face per brief.


| # | Display (role) | Text / support | Why it works | Good for |
|---|---|---|---|---|
| 1 | **Bodoni Moda** (opsz 96, wght 400–500, italic accents) | **Hanken Grotesk** 400/600 | Hairline didone vs neutral grotesk; extreme stroke contrast vs monoline | Fashion, beauty, editorial covers |
| 2 | **Newsreader** (opsz 72, 500) | **Schibsted Grotesk** 500 (built for a newsroom) | Text-born serif with a display cut, plus a no-nonsense news grotesk | Journalism, explainers |
| 3 | **Big Shoulders** (opsz 72, 800–900) | **IBM Plex Mono** 400 | Civic, compressed, signage vs typewriter-engineered mono | Industrial, infrastructure, urban, sport stats |
| 4 | **Anybody** alone (wdth 50 ↔ 150, wght 100–900) | — | One family; contrast through width alone | Brutalist, music, event promos |
| 5 | **Mona Sans** (wdth 125, 800) | **Hubot Sans** 400 / **Martian Mono** for data | Superfamily: same skeleton, different temperature | Developer and product launches |
| 6 | **EB Garamond** (italic, 400) | **Archivo** (wdth 62–75, 600 caps labels) | Renaissance roman vs compressed American grotesk | Literary, documentary, cultural |
| 7 | **Young Serif** or **Gloock** | **Familjen Grotesk** 500 | Chunky soft serif vs quirky Scandinavian grotesk | Food, lifestyle, warm brand |
| 8 | **Libre Caslon Display** | **Libre Franklin** 400–700 | The Caslon + Franklin Gothic pairing of American newspapers and posters | Americana, politics, heritage |
| 9 | **Playfair** (2023 variable, opsz 1200 at display) | **Atkinson Hyperlegible Next** | Poster-scale hairlines vs a face designed for low-vision legibility | Accessible editorial |
| 10 | **Fraunces** (SOFT 100, WONK 1, wght 800) | **Recursive** (CASL 1, MONO 0) | Soft "Old Style" wonk vs casual brush sans | Playful, kids, food, 70s |
| 11 | **Cormorant Garamond** 300 at ≥ 15 vh | **Tenor Sans** caps, +0.2 em | High-contrast light display vs a calm wide sans | Luxury, perfume, hospitality |
| 12 | **Grenze Gotisch** 700–900 | **Barlow Condensed** 600 caps | Blackletter voice vs a neutral condensed workhorse | Metal, football club, beer, heritage sport |
| 13 | **Shippori Mincho** / **Zen Old Mincho** | **Zen Kaku Gothic New** + Latin **Instrument Sans** | Mincho vs gothic, the core pairing of Japanese typography | Japanese-influenced layouts, craft |
| 14 | **Bricolage Grotesque** (opsz 96, wdth 75, 800) | **Literata** (opsz 12, 400) | Ink-trapped expressive grotesk vs a sturdy reading serif | Culture, podcasts, indie brands |
| 15 | **Roboto Flex** (wdth 25–151, GRAD) | **Roboto Serif** | One design space: parametric sans plus a matching serif | Data-driven or responsive systems |
| 16 | **Saira** (wdth 50, 900, italic) | **Saira** (wdth 100, 500) | One family across 50 → 100 width | Sport, motorsport, esports |
| 17 | **Unbounded** 900 | **Onest** 400 | Wide geometric display vs a compact text grotesk | Bold tech or crypto *only if wanted*: Unbounded is already a crypto cliché |

### 4.3 Clichéd pairings: avoid unless the brief is the cliché

| Pairing / pattern | Why it reads as default |
|---|---|
| **Inter (everything) + Instrument Serif italic accent word** | The 2024–26 AI landing page ([Krebs](https://adriankrebs.ch/blog/design-slop/)) |
| Space Grotesk + Space Mono, or Space Grotesk + anything | Top-60 Google font; the "web3/startup" voice |
| Montserrat + Open Sans / Lato / Raleway | Template-era 2015–2020 defaults (Montserrat is GF #6) |
| Poppins headings (any pairing) | GF #8; the geometric default of Canva and Fiverr |
| Playfair Display + Lato / Source Sans / Raleway | The first "elegant" pairing in every pairing generator |
| Bebas Neue + Montserrat, all caps, +0.3 em | The YouTube "cinematic" title |
| Cinzel / Trajan caps for "epic" | Trajan is "the most lazy font" for epics ([Typeroom](https://www.typeroom.eu/article/turn-your-logo)) |
| Cormorant + Montserrat, cream background | Wedding and lifestyle template |
| Orbitron / Audiowide / Exo for "tech" or "sci-fi" | 2000s sci-fi pastiche |
| Pacifico / Lobster scripts for "fun" | Over-recognised; low craft signal |
| Geist + Geist Mono | Vercel-adjacent house style; reads as "an AI tool demo" |
| Fraunces or Instrument Serif headline + cream bg + sage accent | The "tasteful AI" house look |

A cliché is fine when you pick it on purpose and push it: Trajan can work as a *parody* of epic. The tell is a cliché that arrived by default.

---

## 5. Genres and styles

Each entry gives the faces (OFL first), the layout signature, colour, and motion character.

### 5.1 Swiss / International Typographic Style
- **Lineage:** Müller-Brockmann's Musica Viva posters for the Zürich Tonhalle (1950s–70s): Akzidenz-Grotesk, flush left / ragged right, objective sans, mathematical grid ([People's GD Archive](https://peoplesgdarchive.org/item/17122/musica-viva-posters-for-the-zurich-tonhalle), [Wikipedia](https://en.wikipedia.org/wiki/Josef_M%C3%BCller-Brockmann)). Theory: *Grid Systems in Graphic Design* (1981). For the whole vocabulary of grid, scale and hierarchy, see Lupton, [*Thinking with Type*](https://thinkingwithtype.com/).
- **Faces:**
  - Akzidenz/Helvetica stand-ins: **Archivo**, **Inter Tight** (fine when *chosen*: its tight spacing is right for display), **Hanken Grotesk**, **Schibsted Grotesk**, **Instrument Sans**, **Public Sans**, **Albert Sans**, **Familjen Grotesk**.
  - Univers-like: **Pathway Extreme**, **Radio Canada**.
- **Layout:**
  - 6–12 column grid with a 6–8% outer margin.
  - Everything flush left on 1–3 column edges; ragged right with a controlled rag.
  - One or two sizes at extreme contrast; numbers as graphic elements.
  - Text can rotate 90° as a column. Big asymmetric negative space (40–60% of the frame empty).
- **Colour:** black/white plus one primary (signal red #E30613-ish), or full-bleed flat colour with white type.
- **Motion:** linear translations along grid lines; elements slide on x or y only; hard cuts; no overshoot, no blur. Timing 400–700 ms `outQuart`. Rhythm comes from the grid, not from bounces.

### 5.2 Brutalist / anti-design
- **Faces:** **Anybody** (wdth extremes), **Archivo Black**, **Anton**, **Rubik Mono One**, **Space Mono** (when *raw* is the point), **Times-like Tinos** / **Libre Caslon Text** at default settings (deliberately "unstyled"), **Special Gothic Expanded One**.
- **Layout:** default-looking HTML energy. Overlaps, type touching the edges, rules and boxes, visible structure, mixed sizes clashing, justified with ugly gaps *on purpose*, underlines, all caps.
- **Colour:** pure RGB primaries, #0000FF links, grey #C0C0C0, or black/white with one acid accent.
- **Motion:** instant state changes, stepped motion at 12 fps, frame-held jumps, marquee scrolls, no easing at all. Cuts on every beat.

### 5.3 Editorial / magazine
- **Faces:**
  - Didones: **Bodoni Moda** (opsz 6–96), **Playfair** (opsz 5–1200), **Libre Bodoni**, **Gloock**, **DM Serif Display**, **Abril Fatface**.
  - Text: **Newsreader**, **Source Serif 4**, **Literata**, **Spectral**.
  - Support: **Hanken Grotesk**, **Schibsted Grotesk**.
- **Layout:**
  - Huge numerals (30–60 vh) for chapter numbers and stats, with old-style figures (`onum`) in running text and lining figures (`lnum`) in display.
  - Drop caps of 3–4 lines. Pull quotes with hanging quote marks.
  - Italic for emphasis inside a roman headline, *from the same family*.
  - Deck (standfirst) at 1/4 to 1/6 of the headline size; small-caps bylines (`smcp`, +0.06 em).
- **Colour:** paper (#F4F0E8-ish), ink black, and one spot colour (oxblood, cobalt).
- **Motion:** slow and assured: 800–1200 ms `outCubic` fades and masks, page-turn wipes, numerals that count up with `tnum`. Hold 3–5 s.

### 5.4 Vernacular / street / hand-painted
- **Faces:** **Bungee** family (signage), **Rubik Wet Paint**, **Permanent Marker**, **Sedgwick Ave Display** (graffiti), **Protest Strike / Revolution / Guerrilla**, **Danfo** (Nigerian signage, ELSH axis), **Ojuju**, **Agbalumo**, **Caprasimo**, **Shrikhand**.
- **Layout:** sign-painter logic: words fill panels, mixed faces per line, shadows and outlines, arcs, banners.
- **Colour:** saturated and local (bus livery, shop signs, market posters).
- **Motion:** stop-motion boil (2-frame redraw at 12 fps), wiggle ±0.5°, stamped-on with a 1-frame squash.

### 5.5 Tech / terminal / data
- **Faces:** **JetBrains Mono**, **IBM Plex Mono**, **Martian Mono** (wdth 75–112.5), **Geist Mono**, **Azeret Mono**, **Sometype Mono**, **Kode Mono** (ROND/wdth), **Datatype** (wdth 50–150), **VT323**, **Silkscreen**, **Doto** (dot matrix, ROND), **Geist Pixel**, **Workbench** / **Sixtyfour** (CRT `BLED`/`SCAN` axes).
- **Layout:** monospace grid (characters as cells), left-aligned, line numbers, prompts (`>`, `$`), brackets as framing, tabular numbers, ASCII rules.
- **Colour:** do *not* default to green-on-black. Use paper-white on near-black (#0B0B0C), amber (#FFB000) or one brand colour. Syntax colours sparingly.
- **Motion:**
  - Typewriter at 30–60 cps for code (faster than prose), with a cursor.
  - Scrambles; line-by-line appends at 40–80 ms per line.
  - Scrolling logs with a constant px/frame speed.
  - CRT effect via `SCAN` -53 → 100 on Workbench/Sixtyfour.

### 5.6 Luxury
- **Faces:** **Cormorant Garamond** (300–500 at ≥ 12 vh), **Bodoni Moda**, **Italiana**, **Bellefair**, **Gilda Display**, **Marcellus**, **Tenor Sans**; **Ysabeau** for refined sans.
- **Layout:** tiny type and enormous space (70–85% empty). Centring is acceptable here, because luxury is the genre where symmetry is native. Caps at 1.8–2.4 vh tracked +0.2–0.35 em. Monogram scale moves.
- **Colour:** black, ivory, champagne (#C9B37E), deep green, oxblood; no gradients except real metallic film.
- **Motion:** slow (1.2–2.5 s), `inOutSine`, crossfades, very slow push-ins (2–3% over 5 s), letter-by-letter fades at a 60–90 ms stagger. Bounce tends to undercut the register, because luxury reads as unhurried.

### 5.7 Sports
- **Faces:** compressed and italic: **Saira** (wdth 50, italic, 800–900), **Big Shoulders**, **Barlow Condensed**, **Sofia Sans Extra Condensed** (wght 1–1000), **Anton**, **League Gothic** (wdth 75–100), **Teko**, **Alumni Sans**, **Oswald** (overused; prefer the others), **Saira Stencil**.
- **Layout:**
  - Names in all caps at 20–60 vh, italic or skewed −8° to −12°.
  - Numbers huge, outlined behind the player; diagonal slashes and chevrons.
  - Stacked caps at leading 0.8. Stats in `tnum` grids.
- **Colour:** team colours at full saturation, black, a halftone or noise overlay.
- **Motion:** slams (0–2 frames), whip pans, speed lines, 2–4% camera shake on impact, skewed wipes at 150–250 ms. Text animation follows the impact cut.

### 5.8 Y2K / acid / rave
- **Faces:** **Climate Crisis** (YEAR), **Rubik Glitch**, **Monoton**, **Syncopate**, **Michroma**, **Krona One**, **Lexend Zetta–Giga** (ultra-wide), **Tilt Warp/Prism** (XROT/YROT), **Kalnia Glaze**, **Nabla**, **Honk** (COLRv1 colour fonts), **Sixtyfour**, **Bitcount**.
- **Layout:** chrome/inline effects, warped type on arcs, extended widths, stacked repeats, blurred duplicates, star and sparkle glyphs, tiny wide-tracked metadata in the corners.
- **Colour:** chrome gradients, acid green #C6FF00, holographic, hot pink on black. Here, gradients *are* the genre.
- **Motion:** loops, echo trails (5–8 copies, 2–3 frames apart, decaying opacity), stretch-and-squash, glitch slices, 3D tumble via XROT/YROT.

### 5.9 Retro 70s (soft serifs, groovy)
- **Faces:** Cooper Black stand-ins: **Fraunces** (SOFT 100, WONK 1, wght 700–900, opsz 144), **Young Serif**, **Caprasimo**, **Bagel Fat One**, **Shrikhand**. Also **Righteous**, **Corben**, **Gloock**.
- **Layout:** tight to touching (−0.03 to −0.06 em), stacked and overlapping, warped on arcs, stroked and offset-shadowed in two colours, round-cornered panels.
- **Colour:** mustard #E3A72F, burnt orange #C8501E, avocado #6B7B3A, brown #4A2C1A, cream #F3E6C8.
- **Motion:** bouncy (`outBack` s ≈ 1.2), wobble, stepped 12 fps. Echo trails in the palette colours ("Soul Train" stacked repeats).

### 5.10 Blackletter / metal
- **Faces:** **UnifrakturMaguntia**, **UnifrakturCook**, **Pirata One**, **Grenze Gotisch**, **Jacquard 24**, **Jacquarda Bastarda 9**, **Texturina** (opsz, softened blackletter for longer text), **Montenegrin Gothic One**.
- **Layout:** centred and symmetrical (it's heraldic); logo-like lockups; long text tends to become unreadable, because blackletter word shapes are unfamiliar to most viewers. Pair with a neutral condensed sans for information.
- **Colour:** black/white, blood red, gold; for metal, illegible white logos on black with mirrored symmetry.
- **Motion:** slow burn-in through noise and grain, flicker 2–3 frames, fire or smoke displacement on the edges; or brutal slams for hardcore.

### 5.11 Handwritten / personal
- **Faces:** **Caveat** (wght), **Shantell Sans** (BNCE/INFM axes for bounce and informality), **Playpen Sans**, **Kalam**, **Gochi Hand**, **Reenie Beanie**, **Homemade Apple**, **Playwrite** family (school-hand models), **Agrave Pro**.
- **Layout:** annotations around a primary sans (arrows, circles, underlines), rotated ±2–5°, on the image rather than in a box.
- **Motion:** stroke-draw (write-on) at 4–8 characters/s; boil (2–3 seeded variants alternating at 8–12 fps); `BNCE` axis jitter on Shantell Sans.

### 5.12 Japanese-influenced layouts
- **Faces:**
  - Mincho: **Shippori Mincho**, **Zen Old Mincho**, **Noto Serif JP**, **Hina Mincho**.
  - Gothic: **Zen Kaku Gothic New**, **Noto Sans JP**, **M PLUS 1**, **BIZ UDPGothic**.
  - Display: **Dela Gothic One**, **DotGothic16**, **Reggae One**, **Train One**.
  - Latin partners: **Instrument Sans**, **Inter Tight**, **Shippori Antique**.
- **Layout:**
  - Vertical text (tategaki, right to left) as a column beside horizontal Latin.
  - Large single kanji at 40–70 vh as an image; small Latin captions tracked +0.1 em.
  - Strict but asymmetric grids, generous *ma* (negative space).
  - Mixed scripts aligned on the centre line rather than the baseline. Red seal accents.
- **Colour:** off-white paper, sumi black, vermilion #E34234, indigo.
- **Motion:** vertical reveals top to bottom, brush write-on, calm 600–900 ms fades; or kinetic J-pop energy (fast-cut mixed scripts, bold gothic at 2–4 cards/s).

### 5.13 Constructivism
- **Faces:** heavy condensed grotesques and squared sans: **Anton**, **Oswald**, **Big Shoulders Stencil**, **Saira Stencil**, **Russo One**, **Dela Gothic One**; Cyrillic-capable **Rubik**, **Golos Text**, **Onest**.
- **Layout:** diagonals at 15–45°, type on angled bars, overlapping red/black planes, circles and wedges, photomontage, huge exclamation marks, words stacked to form shapes.
- **Colour:** red #D7261E, black, cream, occasional grey.
- **Motion:** diagonal slides on the 30–45° axis, rotations to lock to angles, hard cuts, propaganda tempo (march at 100–120 bpm).

---

## 6. Trends 2023–2026, the clichés, and what separates studio work from amateur work

### 6.1 Live directions (use with intent)

- **Variable type as behaviour:** axes driven by audio or data (COLLINS SF Symphony, Spotify Mix by Dinamo ([Dinamo](https://abcdinamo.com/newsletter/the-dinamo-update-new-spotify-custom-typeface)), MotoGP by F37/Pentagram ([F37](https://f37foundry.com/custom-fonts/moto-gp))). See also [Type Network on branding with variable fonts](https://typenetwork.com/articles/anything-you-can-do-branding-with-variable-fonts).
- **Ink traps and expressive grotesks** at display sizes: Bricolage Grotesque, Funnel Display, Special Gothic.
- **Extreme widths in one family:** ultra-condensed next to ultra-extended (Anybody, Mona Sans, Zalando Sans Expanded).
- **Soft and wonky serifs, the "new 70s":** Fraunces, Young Serif, Gloock, Kalnia, Redaction (MCKL, on Google Fonts 2026). These are trending and so already saturating; push them further or skip them.
- **Pixel, bitmap and dot-matrix revival:** Geist Pixel, Doto, Tiny5 (BLED/JITT/ROND axes), Bitcount, Jacquard (pixel blackletter), Workbench.
- **Colour fonts (COLRv1):** Nabla, Honk, Bungee Spice, Kalnia Glaze. These are toys; use one per film at most.
- **Imperfection:** hand-drawn, distressed, generative "boil", "raw and unpolished" letterforms ([Creative Bloq, 2026 trends](https://www.creativebloq.com/design/fonts-typography/breaking-rules-and-bringing-joy-top-typography-trends-for-2026)).
- **Big type, small type, nothing in between:** poster-scale contrast in UI and motion.

### 6.2 AI-slop tells (what the critic saw)

| Tell | Why it's a tell | Fix |
|---|---|---|
| Inter (or Roboto, Poppins, Montserrat) everywhere, centred | The training-data default; "Inter unchosen signals nobody made a typography decision" | Pick a face per genre (§5); flush left on a grid |
| One serif italic word in a sans headline | 2024–26 landing-page meme ([Krebs](https://adriankrebs.ch/blog/design-slop/)) | Emphasis via weight or colour *within* the family, or a full-line face change with a reason |
| Gradient-filled text (purple → blue → pink) | "VibeCode purple", "gradients everywhere" | Flat colour; gradients only in genres where they're native (Y2K, chrome) |
| Glassmorphism plates behind text | 2021 Big Sur aesthetic, now template | Solid plate, scrim or nothing |
| All caps, +0.3 em, thin geometric sans, slow fade = "cinematic" | The YouTube trailer preset | Real cinema titles are usually *small* caps in a classical face with modest tracking, placed off-centre in negative space |
| Every word pops in with a per-letter bounce | Template motion | One reveal idea per film; ≤ 30 ms stagger; ease-out with no overshoot |
| Glow, outline or drop shadow on everything | Rescuing contrast | Fix value contrast; outline as a deliberate style only |
| Medium weight everywhere | No hierarchy | Use the weight extremes (200–300 vs 800–900) |
| Same size steps (9/6/4/3 vh) on every card | Template scale | Collapse to 2–3 levels with 8:1+ contrast |
| Uppercase section labels on everything (+ grey) | "All-caps section labels" pattern | Labels only where they navigate |
| Straight quotes, hyphens as dashes, "..." | No typesetting pass | Smart quotes, en/em dashes, ellipsis character |
| "Netflix documentary" serif (light Didone or Trajan, centred, slow push) | Over-used doc template | Set doc titles in a grotesk or a text serif *flush left*, or use archival vernacular type |
| Space Grotesk / Syne / Unbounded for "creative tech" | Popular 2020–24 picks, now uniform | Bricolage, Familjen, Schibsted, Mona/Hubot, or a mono |

### 6.3 What studios do that amateurs don't

1. **Hierarchy you can squint at.** Blur your eyes: 1 dominant, 1 secondary, the rest texture.
2. **A grid, visibly obeyed.** All left edges land on 2–3 x-positions across the whole film. Baselines snap to a baseline grid, with the unit = body leading.
3. **Deliberate negative space.** 40–70% of the frame empty, and *asymmetric*.
4. **Cropping.** Type bleeds off the frame, sits behind objects, or is masked by the subject.
5. **Restraint.** One face, two weights; one motion idea; one accent colour.
6. **Typographic colour.** Even grey texture in text blocks: no loose lines, no rivers, no mixed tracking.
7. **Rag.** Ragged-right edges alternate long/short without a staircase. No line under ~60% of the measure except the last. No hyphenation at display sizes.
8. **No widows or orphans.** One word alone on the last line of a 2–4 line display block tends to look like a mistake, because the eye reads the block as a shape. Rebreak by sense or use a non-breaking space between the last two words.
9. **Optical alignment.**
   - Round letters (O, C, S) overshoot the cap line and baseline by 1–3% by design.
   - At display size, align the *visual* edge: pull rounds and diagonals (A, V, W, T) 0.02–0.05 em into the margin.
   - Hang punctuation (quotes, hyphens) outside the column edge ([Fonts.com Fontology](https://www.fonts.com/content/learning/fontology/level-4/fine-typography/hanging-punctuation), [CreativePro](https://creativepro.com/typetalk-hung-punctuation-optical-margin-alignment/)). A pull quote whose opening “ hangs fully in the margin is the classic editorial move.
10. **Optical kerning at display size.** Check AV, To, Wa, LT, and any "r." or "f)".
11. **Real punctuation:**
    - “curly quotes” and ’ apostrophes.
    - – en dash for ranges (2023–2026); — em dash, or a spaced en dash in British style, for breaks.
    - × not x for dimensions; − minus in numbers; … ellipsis.
    - Primes ′ ″ for feet and inches.
12. **Figure styles:**
    - `tnum lnum` for counters, scores, timers and tables (no width jitter while counting).
    - `onum` in running serif text; `pnum` in display numerals.
    - `zero` (slashed zero) for codes.
    - `frac` for ½ ¾.
13. **Case-sensitive forms:** `case` raises hyphens, parentheses and @ to cap height in all-caps settings.
14. **Real small caps rather than scaled-down capitals:** `smcp`/`c2sc` for acronyms in running text (NASA → nasa set in small caps), tracked +0.05 em.
15. **Consistent motion grammar** (§2.4).

---

## 7. Variable fonts in motion

All ranges below were read from the Google Fonts metadata API (`fonts.google.com/metadata/fonts`, 2026‑11‑17). Registered axes are `wght`, `wdth`, `opsz`, `slnt` and `ital`; uppercase tags are custom (parametric or design-specific) axes ([Google Fonts Knowledge: parametric axes](https://fonts.google.com/knowledge/using_type/introducing_parametric_axes), [optical size](https://fonts.google.com/knowledge/glossary/optical_size_axis)).

### 7.1 Axis catalogue (OFL, on Google Fonts)

| Family | Axes (min–max) | What's interesting in motion |
|---|---|---|
| **Roboto Flex** | wght 100–1000, wdth 25–151, opsz 8–144, slnt −10–0, GRAD −200–150, XOPQ 27–175, YOPQ 25–135, XTRA 323–603, YTUC 528–760, YTLC 416–570, YTAS 649–854, YTDE −305–−98, YTFI 560–788 | The most complete parametric design space. `GRAD` changes weight **without changing width** (no reflow); `XTRA` changes counter width; `YTUC`/`YTLC` stretch cap and x-height. |
| **Google Sans Flex** (2025) | wght 1–1000, wdth 25–151, opsz 1–144, slnt −10–0, ROND 0–100 | Very wide weight range plus roundness: geometric → friendly. |
| **Fraunces** | wght 100–900, opsz 9–144, SOFT 0–100, WONK 0–1 | `SOFT` sharp → super-soft terminals; `WONK` toggles leaning n/m/h and bulbous italic flags. It substitutes automatically by opsz ([Fraunces README](https://github.com/undercasetype/Fraunces)). Animate SOFT 0 → 100 for "melting". |
| **Recursive** | wght 300–1000, slnt −15–0, MONO 0–1, CASL 0–1, CRSV 0–1 | `MONO` morphs proportional ↔ monospace; `CASL` linear ↔ brush-casual. `CRSV` 0.5 = auto cursive when slnt < −14 ([recursive.design](https://www.recursive.design/)). The best "code becomes human" morph. |
| **Anybody** | wght 100–900, wdth 50–150 | 3:1 width range: the brutalist width morph. |
| **Science Gothic** | wght 100–900, wdth 50–200, slnt −10–0, CTRS 0–85 | 4:1 width range; `CTRS` is contrast. Extreme stretch titles. |
| **Mona Sans / Hubot Sans** | wght 200–900, wdth 75–125 | GitHub's superfamily; Hubot is the mechanical twin with the same metrics. Cross-fade between them for a "human → robot" transition. |
| **Archivo** | wght 100–900, wdth 62–125 | A workhorse grotesk with real compressed and expanded widths. |
| **Saira / Saira Stencil** | wght 100–900, wdth 50–125 | Sport width range; Stencil adds bridges (2026). |
| **Encode Sans, Zalando Sans, Mozilla Headline, Firjar, Special Gothic** | wdth 75–125 plus wght | Moderate width morphs; good for fit-to-box. |
| **Bricolage Grotesque** | wght 200–800, wdth 75–100, opsz 12–96 | Ink traps grow at low opsz; quirky at display. |
| **Big Shoulders** (+ Stencil, Inline) | wght 100–900, opsz 10–72 | Chicago civic compressed; opsz tightens for display. |
| **Instrument Sans** | wght 400–700, wdth 75–100 | Limited range; fine for subtle width fitting. |
| **Unbounded** | wght 200–900 | Wide; weight only. |
| **Bodoni Moda** (+ SC) | wght 400–900, opsz 6–96 | Didone hairlines that thicken at low opsz. Animate opsz with scale to keep contrast "correct" while zooming. |
| **Playfair** (2023, not Playfair Display) | wght 300–900, wdth 87.5–112.5, opsz 5–1200 | opsz up to 1200 is for billboard hairlines. |
| **Literata** | wght 200–900, opsz 7–72 | Book serif with real optical sizes. |
| **Newsreader** | wght 200–800, opsz 6–72 | News serif; display cuts are elegant. |
| **Source Serif 4** | wght 200–900, opsz 8–60 | |
| **Imbue** | wght 100–900, opsz 10–100 | Condensed didone; very high contrast at 100. |
| **Texturina** | wght 100–900, opsz 12–72 | Blackletter-ish with optical sizes. |
| **Roboto Serif** | wght 100–900, wdth 50–150, opsz 8–144, GRAD −50–100 | Serif with width plus grade. |
| **Merriweather** (v2) | wght 300–900, wdth 87–112, opsz 18–144 | |
| **Inter** | wght 100–900, **opsz 14–32** | Inter now has optical sizes (Display at 32). |
| **DM Sans** | wght 100–1000, opsz 9–40 | |
| **Nunito Sans** | wght 200–1000, wdth 75–125, opsz 6–12, YTLC 440–540 | x-height axis. |
| **Commissioner** | wght 100–900, slnt −12–0, FLAR 0–100, VOLM 0–100 | `FLAR` flares the stroke ends (sans → glyphic); `VOLM` adds volume. |
| **Geologica** | wght 100–900, slnt −12–0, CRSV 0–1, SHRP 0–100 | `SHRP` sharpens corners. |
| **Kumbh Sans** | wght 100–900, YOPQ 40–300 | Thin/thick horizontal strokes (contrast). |
| **Shantell Sans** | wght 300–800, BNCE −100–100, INFM 0–100, SPAC 0–100 | Built-in bounce and informality: hand-drawn jitter without randomness. |
| **Sofia Sans** (+ Condensed, Extra Condensed) | wght 1–1000 | Hairline → black. |
| **Ysabeau** | wght 1–1000 | |
| **Climate Crisis** | YEAR 1979–2050 | Weight melts with Arctic sea-ice data. Animate YEAR as a data story ([specimen](https://fonts.google.com/specimen/Climate+Crisis)). |
| **Nabla** | EDPT 0–200, EHLT 0–24 | COLRv1 isometric extrusion depth and highlight. |
| **Honk** | MORF 0–45, SHLN 0–100 | COLRv1 inflated 3D; MORF morphs the shape, SHLN is shadow length. |
| **Kablammo** | MORF 0–60 | Cartoon wobble morph. |
| **Workbench / Sixtyfour** | BLED 0–100, SCAN −53–100 | CRT bleed and scanlines as axes. Sixtyfour Convergence adds XELA/YELA (−100–100) for RGB misconvergence. |
| **Tilt Warp / Tilt Prism** | XROT −45–45, YROT −45–45 | Glyphs rotate in faux 3D. |
| **Tiny5 / Tiny5 Duo** | wght 100–900, wdth 50–200, BLED, JITT, ROND 0–100 | Pixel font with jitter and bleed. |
| **Doto** | wght 100–900, ROND 0–100 | Dot-matrix; round ↔ square dots. |
| **Bitcount** (+ Grid Double) | wght 100–900, slnt −8–0, CRSV, ELSH 0–100, ELXP 0–100 | Pixel element shape and expansion. |
| **Geist Pixel** | ELSH 0–100 | Pixel element shape. |
| **Handjet** | wght 100–900, ELGR 1–2, ELSH 0–16 | Element grid and shape: modular letters. |
| **Danfo** | ELSH 0–100 | Lagos bus-sign display; element shape. |
| **Linefont / Wavefont** | wdth 25–200, wght 4–1000 / ROND, YELA −100–100 | Data glyphs: bars and waveforms as type. Good for audio meters. |
| **Readex Pro** | wght 160–700, HEXP 0–100 | Hyper-expansion. |
| **Kode Mono** | wght 400–700, wdth 75–125, ROND 0–100 | Mono with roundness and width. |
| **Martian Mono** | wght 100–800, wdth 75–112.5 | Mono with width. |
| **Datatype** (2026) | wght 100–900, wdth 50–150 | Mono for data with width. |
| **Afacad Flux** | wght 100–1000, slnt −14–14 | Slant *both ways*: lean forward/back on accelerate/decelerate. |
| **Foldit** | wght 100–900 | Folded-ribbon display; weight reads as depth. |
| **Kalnia / Kalnia Glaze** | wght 100–700, wdth 100–125 | Glaze is COLR. |
| **Pathway Extreme** | wght 100–900, wdth 75–100, opsz 8–144 | |

**Not on Google Fonts:** Commit Mono (OFL, but distributed separately). Cooper Black, ITC Benguiat, Helvetica, Akzidenz and Trajan are proprietary; use the stand-ins in §5.

### 7.2 Axis animation rules

1. **Watch reflow.** `wght` and `wdth` change advance widths, so neighbours move. Either
   - pin the line edge (left for flush-left), or
   - use axes that keep widths: `GRAD` in Roboto Flex/Roboto Serif; Recursive's weight; monospace faces; or
   - animate per glyph with each glyph's origin fixed (the "breathing letters" look).
2. **Ranges that read:**
   - wght: a ≥ 300-unit delta (300 → 800) or it looks like a rendering glitch.
   - wdth: ≥ 30% of the range.
   - opsz: tie it to scale, not to time.
   - slnt: 0 → max in 200–300 ms on acceleration.
3. **Per-glyph phase.** Wave a parameter through the word: axis(i, t) = base + amp·sin(2π(f·t − i·k)), with k = 0.08–0.15 per glyph and f = 0.5–1.5 Hz. It reads as alive without moving position.
4. **Data and audio mapping.** Map RMS loudness to `wght` with a 50–80 ms attack and 200–400 ms release (follower smoothing), or the type flickers. Map a value 0–1 to YEAR or wdth for data stories.
5. **Instances snap; variables glide.** If a family is static (Instrument Serif, Bebas Neue, Anton), don't fake weight animation with stroke growth beyond 1–2%. Pick a variable face instead.
6. **COLRv1 axes** (Nabla EDPT, Honk SHLN) make a cheap "3D" extrusion: animate depth 0 → 200 in 400 ms for a pop-out.

---

## 8. Gap list against Anikit (ranked by impact)

Current engine (from `docs/video/spec-reference.md`):
- Text layer: `font`, `size`, `weight`, `italic`, `color` (incl. gradient), `align`, `lineHeight`, `letterSpacing`, `uppercase`, `maxLines`, `fit` (shrink only), `stroke` (under fill), `split` chars/words/lines, `kinetic` presets, `reveal`, `value`.
- Also: an `animator` with range selectors and seeds, `textPath`/`pathOffset`, `matte`, `repeat`, `echo`, inline bold/italic/colour spans.
- Bundled faces: Inter, Inter Display, JetBrains Mono (+ Instrument Serif in practice).

Score: **impact** on output quality (1–5) × **frequency** an agent would use it (1–5).

| Rank | Feature | Impact × freq | Status / note |
|---|---|---|---|
| 1 | **Font library (~100 OFL families) with genre metadata** (category, genre tags, axes, recommended roles, cap-height/x-height, language coverage) so an agent can *choose* | 5×5 | Planned. Ship with a `fonts.json` the agent reads: tags like `swiss`, `sports`, `luxury`, `terminal`, plus "cliché" flags. |
| 2 | **Variable axes as animatable properties**: `axes: {wght, wdth, opsz, slnt, GRAD, SOFT, …}`, per layer and per glyph via `animator` | 5×4 | Planned. Needs `fvar`/`gvar`/`HVAR` interpolation and a glyph cache keyed by the quantised axis tuple (step 1 unit wght, 0.5 wdth). |
| 3 | **Auto optical size** (`opsz` = rendered px × 0.75, clamped) on by default | 4×5 | Cheap and invisible; fixes didones and Fraunces automatically. |
| 4 | **Layout defaults that aren't slop**: `align` default `left` for multi-line; theme scale with 2–3 levels at 8:1+; display leading 0.9–1.0; size-dependent tracking (Inter dynmetrics-style formula per family) | 5×5 | Mostly defaults plus a tracking curve per font: `tracking(size) = a + b·e^(c·size)`. |
| 5 | **Fit-to-box, both ways** (`fit: "width" | "height" | "box"`, grow *and* shrink) with an **optical overshoot** option (fit cap height or ink bounds, not em box; allow 1–3% bleed for rounds) and width-axis fitting (adjust `wdth` before size) | 5×4 | `fit` only shrinks today. Ink-bounds fitting is key for poster-scale words. |
| 6 | **Typesetting pass**: smart quotes, apostrophes, en/em dashes, ellipsis, ×, −, primes, non-breaking spaces before the last word (widow control) | 4×5 | Pure text preprocessing; on by default with `"smartypants": false` to opt out. |
| 7 | **OpenType features**: `features: {tnum, lnum, onum, pnum, smcp, c2sc, case, zero, frac, liga, dlig, calt, ss01–ss20, salt, swsh, kern}` | 4×4 | Needs GSUB/GPOS shaping (rustybuzz is no_std-friendly). Auto `case` when `uppercase: true`; auto `tnum` on `value` counters. |
| 8 | **Rag control and line breaking**: balanced breaks for display (minimise raggedness, Knuth–Plass-lite), no single-word last lines, sense-based manual breaks preserved, no hyphenation above 6 vh | 4×4 | `text-wrap: balance`/`pretty` equivalents. |
| 9 | **Per-glyph transforms with seeds** (x, y, rotation, scale, axis values, colour per glyph; deterministic noise), building on `animator` | 4×4 | `animator` exists; extend to axis values and arbitrary per-glyph expressions (`"=sin(t*2 - i*0.12)"`). |
| 10 | **Text as mask / knockout** over imagery and video: a text layer as the matte of a footage layer, plus `knockout: true` to cut text out of a plate | 4×3 | `matte` exists. Add ergonomic sugar (`"fill": {"src": "clip.mp4"}`) and expanding-mask transitions. |
| 11 | **Stroke modes**: outside / inside / centre; **inline** (offset inner stroke), **multi-stroke** (several offset outlines for 70s and Y2K), **offset shadow** (hard, coloured, n px at an angle), **stencil** (bridges via a font or a computed gap) | 3×4 | `stroke` is drawn under the fill only. Requires glyph outline offsetting. |
| 12 | **Optical margin alignment / hanging punctuation** for flush-left and right display blocks (quotes 100%, hyphens 50–70%, A/V/T/W 3–8%) | 3×3 | Per-glyph protrusion table. |
| 13 | **Baseline grid and type-scale tokens**: `theme.grid: {columns, margin, gutter, baseline}`; layers snap `x` to columns and baselines to the grid | 4×3 | Turns "aligned to a grid" from advice into a default. |
| 14 | **Split units that respect final layout** (`split: lines` after wrapping, plus `graphemes`, `sentences`) with masks per unit for mask-reveals | 4×4 | Verify line split happens after fit and wrap. |
| 15 | **Kerning controls**: `kerning: "metric" | "optical" | "none"`, per-pair overrides at display sizes | 3×3 | Optical kerning = sidebearing from ink profile. |
| 16 | **Mixed-face runs and baseline alignment**: inline spans with a different `font`, `size` and baseline shift; cap-height matching | 3×3 | Inline spans exist for bold, italic and colour only. |
| 17 | **Typewriter, scramble and split-flap realism**: punctuation pauses, seeded cps jitter, a caret with 530 ms blink, charset-aware scramble, flap cells with a hinge | 3×3 | `typewriter` and `scramble` exist; add cadence options. |
| 18 | **Vertical text** (CJK tategaki, rotated Latin columns) | 2×2 | `writingMode: "vertical-rl"`. |
| 19 | **Colour fonts (COLRv1)** and palette selection | 2×2 | Nabla, Honk, Bungee Spice, Kalnia Glaze. |
| 20 | **Text on path** improvements: per-glyph orientation smoothing, start/end alignment, along closed paths | 2×2 | `textPath` exists. |

Make these lint warnings (the engine already warns on unknown keys):
- multi-line `align: center` with > 3 lines;
- lowercase tracked > +0.05 em;
- hold time below 0.5 s + words/3;
- more than 2 families in a scene;
- size ratio < 2:1 between adjacent levels;
- straight quotes in display text;
- body text cap height < 18 px at 1080p.

---

## 9. Recipes: eight distinct looks

Sizes are % of frame height (vh) at 16:9; for 9:16, multiply by about 0.6 and re-check the width. Tracking is in em. Timings are in seconds or ms. "Proposed" keys depend on §8 features.

### 9.1 "Zürich Bulletin": Swiss grid explainer
| Setting | Value |
|---|---|
| Faces | **Archivo** wdth 100 (display 800, text 400); **Archivo** wdth 62 for labels |
| Grid | 12 columns, outer margin 7% of width, gutter 1.5%; baseline unit 1.6 vh |
| Sizes | Hero number 42 vh (`tnum lnum`); headline 7 vh; body 3.2 vh; label 1.9 vh |
| Tracking | Hero −0.04; headline −0.02; body 0; labels (caps) +0.08 |
| Leading | Hero 0.8; headline 0.95; body 1.35 |
| Case / align | Sentence case; **flush left** on columns 1 and 7; ragged right; labels all caps |
| Colour | Background #F2F2EE, ink #111, one signal red #E3241B for the hero number only |
| Motion | Elements slide 0.4 em on x along the grid, 500 ms `outQuart`, 120 ms block stagger; number counts up over 1.2 s `outExpo`; hard cuts between cards; hold = 0.5 + words/3 s |

### 9.2 "Vogue Numeral": editorial didone
| Setting | Value |
|---|---|
| Faces | **Bodoni Moda** opsz 96 (display 400 + italic 400); **Hanken Grotesk** 500 for decks and labels |
| Sizes | Chapter numeral 58 vh, cropped 12% off the bottom; headline 9 vh; deck 2.8 vh; byline 1.9 vh (`smcp`) |
| Tracking | Numeral −0.03; headline −0.02; byline +0.10 |
| Leading | Headline 0.92; deck 1.3 |
| Case / align | Headline sentence case with *one italic phrase from Bodoni Moda Italic*; numeral right-aligned on the outer margin; text flush left in a 38-character column |
| Colour | Paper #F4EFE6, ink #141210, oxblood #6E1423 for the numeral |
| Motion | Numeral rises 0.25 em through a line mask, 1.1 s `outExpo`; headline mask-up by line, 900 ms, 140 ms stagger; deck fades 600 ms at +0.5 s; hold 4 s; exit is a 700 ms fade |

### 9.3 "Full Contact": compressed sports slam
| Setting | Value |
|---|---|
| Faces | **Saira** wdth 50, wght 900, italic (or a −10° skew); **Saira** wdth 100, 600 for stats (`tnum`) |
| Sizes | Name 64 vh, all caps, bleeding off the left and right edges by 6–10%; jersey number 90 vh outlined (stroke 0.6% of height, no fill) behind; stat line 4 vh |
| Tracking | Name 0; stats +0.04 |
| Leading | Stacked name lines 0.78 |
| Case / align | All caps; flush left at −6% (off-frame) |
| Colour | Team primary as the flat field, name in white, number outline in team secondary, a 25% halftone overlay |
| Motion | Name slams in 0–2 frames (no ease), a 2-frame 3% scale settle, 3% camera shake for 6 frames, cut on the audio hit; number slides 0.2 em on x over 2 s linear; stats wipe at a −12° skew in 200 ms; card hold 1.2 s, then hard cut |

### 9.4 "Build Log": terminal / developer
| Setting | Value |
|---|---|
| Faces | **Martian Mono** wdth 87.5, 400 (log) / 700 (prompt) |
| Grid | Character grid: 64 columns across 86% of the width; everything on cells |
| Sizes | Log 2.8 vh; result line 8 vh |
| Tracking / leading | 0 / 1.45 |
| Case / align | As typed; flush left; line numbers in 40% opacity, right-aligned in a 4-character gutter |
| Colour | #0B0B0C background, #E8E6E1 text, amber #FFB000 for the prompt and cursor, one green #3DDC84 for "✓" only |
| Motion | Typewriter at 45 cps ± 30% seeded jitter, a 250 ms pause on newlines; caret █ blinking 530 ms; finished lines push the stack up 1 line in 120 ms `outCubic`; the result line scrambles in (5 swaps × 2 frames, 30 ms stagger) |

### 9.5 "Sunday Soul": 70s soft serif
| Setting | Value |
|---|---|
| Faces | **Fraunces** opsz 144, wght 850, SOFT 100, WONK 1; **Recursive** CASL 1, wght 500 for supporting text |
| Sizes | Headline 26 vh on two lines; support 3.6 vh |
| Tracking | Headline −0.05 (letters kiss); support +0.01 |
| Leading | Headline 0.82 |
| Case / align | Lowercase headline, centred (a symmetric lockup is right here), warped on a gentle 8° arc (`textPath`) |
| Colour | Cream #F3E6C8 on burnt orange #C8501E; a 4-copy echo stack offset 0.04 em down-right in mustard #E3A72F, avocado #6B7B3A and brown #4A2C1A |
| Motion | Letters pop by char from the centre: 45 ms stagger, 500 ms `outBack` s = 1.2; echo copies trail at 3 frames each; idle loop SOFT 100 ↔ 60 and wght 850 ↔ 750 as a per-glyph sine (0.8 Hz, k = 0.12) |

### 9.6 "Concrete Width": brutalist width morph
| Setting | Value |
|---|---|
| Faces | **Anybody** (wdth 50 ↔ 150, wght 900); **Space Mono** 400 for metadata, *deliberately raw* |
| Sizes | One word per card at 30–45 vh; metadata 1.8 vh in all four corners |
| Tracking | Word −0.02; metadata 0 |
| Leading | 0.85 |
| Case / align | All caps; the word *fills the frame width edge to edge* (fit-to-width via the `wdth` axis, proposed); metadata flush to the 3.5% action-safe corners |
| Colour | #000 on #FFF, inverted every second card; one #0000FF word |
| Motion | wdth animates 50 → 150 over 700 ms `inOutQuart` as the word grows to fill; cuts on every beat (120 bpm = 500 ms); no fades anywhere; metadata updates as stepped counters at 12 fps |

### 9.7 "Maison": luxury whisper
| Setting | Value |
|---|---|
| Faces | **Cormorant Garamond** 300 (display), **Tenor Sans** 400 (caps labels) |
| Sizes | Display 14 vh; label 2.0 vh |
| Tracking | Display −0.01; label **+0.32** |
| Leading | Display 1.0 |
| Case / align | Display in title case with italic ampersand; label all caps; **centred**, with 75–85% of the frame empty; the display sits on the optical centre (≈ 45% from the top), not the geometric centre |
| Colour | Ivory #F5F1EA on black #0A0A0A, or champagne #C9B37E on deep green #0F2A22 |
| Motion | Letter-by-letter fade at 70 ms stagger, 1.4 s each, `inOutSine`; tracking converges +0.12 → −0.01 over 3 s `outCubic`; the frame pushes 100 → 103% over the 5 s hold; exit is a 1.2 s crossfade; no other motion |

### 9.8 "Monolith Mask": type as window onto footage
| Setting | Value |
|---|---|
| Faces | **Big Shoulders** opsz 72, wght 900 (mask word); **Instrument Sans** wdth 85, 500 for the caption |
| Sizes | Mask word 78 vh (cap height ≈ 55 vh), 1 word, cropping 4% off the top and bottom; caption 2.6 vh |
| Tracking | Mask −0.01 (counters stay open); caption +0.06, all caps |
| Leading | n/a (single line) |
| Case / align | All caps; mask centred horizontally but fit to 94% of the width by ink bounds (proposed `fit: "ink"`); caption flush left at the 5% title-safe margin, bottom left |
| Colour | Outside the letters: #0D0D0D. Inside: footage graded +10% contrast. Caption #EDEDED |
| Motion | Footage scales 115 → 100% inside a static mask over 4 s `outCubic`; at 3.2 s the mask expands (scale 1 → 12 about the counter of the middle letter) in 1.2 s `inOutExpo` to reveal the full frame; caption fades up 400 ms after the mask lands and holds 2.5 s |

How the eight differ:

- **Case:** sentence, italic-accent, caps, as-typed, lowercase, caps, title, caps.
- **Alignment:** grid-left, asymmetric, off-frame, cell grid, centred arc, full-bleed, sparse centred, ink-fit mask.
- **Size contrast:** 13:1, 30:1, 16:1, 3:1 (mono), 7:1, 1 word, 7:1, 30:1.
- **Tempo:** mechanical, slow, violent, typed, bouncy, beat-cut, glacial, slow reveal.
- **Colour:** paper + signal red, paper + oxblood, team colours, dark + amber, warm 70s, binary + blue, ivory/champagne, footage inside black.

There are no repeated faces, and Inter and Instrument Serif appear in none of them.

---

## Further reading

- Robert Bringhurst, *The Elements of Typographic Style*: measure, scale, figures, small caps ([summary of the measure rule](https://webtypography.net/2.1.2)).
- Josef Müller-Brockmann, *Grid Systems in Graphic Design* (1981).
- Ellen Lupton, [*Thinking with Type*](https://thinkingwithtype.com/): letter, text, grid.
- Matthew Butterick, [*Practical Typography*](https://practicaltypography.com/).
- [Art of the Title](https://www.artofthetitle.com/): sequence-by-sequence interviews (Se7en, North by Northwest, Dr. Strangelove, True Detective, Severance, Succession).
- [Fonts In Use](https://fontsinuse.com/): identified typefaces in real work, searchable by format "Film/TV".
- [Typewolf](https://www.typewolf.com/google-fonts): curated Google Fonts, including condensed, display serif and blackletter picks.
- [Google Fonts Knowledge](https://fonts.google.com/knowledge): pairing, variable fonts, optical size, parametric axes.
- [Krebs, "Design slop"](https://adriankrebs.ch/blog/design-slop/): measured AI-design tells.
