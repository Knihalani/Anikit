# Taste: the canon

What separates work a studio would post from generic, templated or "AI slop" video, distilled from practitioners and research (every claim is sourced in [`sources.md`](sources.md)). It exists because the user rejected work as "AI slop", "boring", "generic", "slow", "text hard to read and looks low resolution", "too much synthesizer", "lighting is off", "don't force 3D", "the same 3D visuals every time" (`docs/video/quality/BAR.md`).

**This is not a style.** Every section gives *ranges, reasons and contrasting valid choices*. The only universal rule: every choice is made for this subject, on purpose, and you can say why. A default you didn't choose is the tell (prg.sh, 925Studios); a choice that would fit any product is wrong (swap test, §8). If two of your pieces look alike, at least one of them is wrong: **range is the goal**. Two briefs should come out as different in medium, world, palette, type, motion and sound as two studios' work would (taste pass step 1, *Diverge*).

Each section runs **principles → rules (numbers from practice) → tells → checks**. Numbers marked *(heuristic)* are our proposals built on sources; tune them by eye. `anikit-video critique` implements the checks marked ⚙.

---

## 1. Typography

**Principles.**
- Structure beats typeface. Vignelli did a career with ~4 faces: "it is not the type but what you do with it that counts".
- Contrast of scale is the main tool, with few sizes. Vignelli: "one or two type sizes at the most", the large "usually twice as big". Bringhurst: "don't compose without a scale".
- Louder is not better. Making everything bigger and bolder is Vignelli's "intellectual vulgarity"; "in a world where everybody screams, silence is noticeable. White space provides the silence."
- Type becomes the image only when the concept demands it. Bass: titles carry "the prime underlying core of the film's story… in some metaphorical way". Cooper's scratched *Se7en* type was "from the mind of the killer". Elastic's *True Detective* double exposures mean "internally divided". It is never a default effect.

**Rules.**
- **Scale: pick one ratio per piece and reuse 2–4 sizes.**
  - 2:1 or more, with two sizes only, for statements and kinetic cards.
  - 1.618 for three dramatic levels.
  - 1.25–1.333 only for dense information: data, lower thirds, UI.
  - Adjacent sizes under ~1.2× don't read as different; they read as mistakes ⚙.
  - Where a hero exists, it should be ≥ 3:1 against the next size.
- **Size floor (1080p).**
  - BBC subtitles set lines at 7–8% of frame height (76–86 px). That is the comfortable reading size.
  - ~4% (≈ 43 px line, ≈ 36 px type) is the floor for any secondary text that must be read *(heuristic)*.
  - Under ~28 px, or thin weights (≤ 300) under ~40 px, H.264 and phone downscaling make text look "low resolution".
- **Legibility comes from luma, not hue.** MP4 stores colour at half resolution (4:2:0), so red-on-blue or any equal-luma pair smears even when the PNG looked fine.
  - Contrast: ≥ 4.5:1 for text and 3:1 for large text (WCAG). Or APCA Lc ≥ 75 for body and ≥ 45 for large headlines.
  - Measure against the *worst* frame of a moving background, not the average ⚙.
- **Read time.** Netflix allows ≤ 20 chars/s, ≥ 5/6 s and ≤ 7 s per event. BBC uses 160–180 wpm, ≈ 0.3 s per word.
  - Hold ≥ max(0.83 s, chars/15, words × 0.3 s) **after** the text settles; motion time doesn't count ⚙.
  - Exception: Apple's "Don't Blink" shows one-word cards for 5–7 frames, but only single short words, cut to a percussive score.
- **Measure.** Body copy runs 45–75 characters (66 ideal). On screen, keep lines ≤ 42 chars (Netflix) or ≤ 68% of the frame width (BBC). Past ~40 chars it is reading matter, not display: time it like a subtitle or cut it.
- **Tracking runs opposite to size.**
  - Tighten display type ~2% (Inter: −0.022 em at ≥ 32 px).
  - Track all-caps and labels +5–10%.
  - Never letterspace lowercase without a reason.
- **Leading.** Text +10–15%; display 0.9–1.1; all-caps stacks 0.8–0.95.
- **Pairing.** The default is one family varied by weight and size, with two families at most. If you pair, make the contrast clear: serif display with grotesk text, or mono labels with a humanist head. Never pair two similar grotesks.
- **Safe area (EBU R95).** Graphics-safe is 5% (96 px and 54 px at 1080p); action-safe is 3.5%. Bleed type off-frame only deliberately, as image.

**Tells.**
- Centred heavy grotesk in Title Case with wide tracking and a gradient fill, on every card.
- Many sizes with no scale.
- Everything big and bold.
- Inter or Roboto with no stated reason (the font is fine; the unchosen default is the tell).
- Glow, outline or drop shadow used to rescue a low-contrast pair instead of fixing value.
- Blur, defocus, motion blur or bloom on words while they are meant to be read ⚙.
- Per-letter bounce-pop or typewriter applied to everything.
- Scratched or 3D-extruded type with no story reason.

**Checks.**
- Downscale the still to 480×270: every word you meant to be read is still legible.
- Squint: one dominant level, 1–2 secondary, everything else quiet.
- Count the distinct sizes per scene and across the piece ⚙.
- Check luma contrast of the text against the 5th–95th percentile of the background behind it.

## 2. Colour

**Principles.**
- Colour is relative. Albers: "the most relative medium in art". A swatch only means something at its final area, next to its real neighbours.
- **Value before hue.** Gurney designs the light–dark structure as a separate step and warns against "middle-value mumbling", where everything sits in mid-grey. The notan (light/dark pattern) is the skeleton.
- Restraint is a chosen gamut: a shape cut from the colour wheel (Gurney's gamut masking). Many forms are valid; Vignelli: "times for strong primary colors… for subtle pastel colors… for just black and white… Appropriateness is one of the rules."
- Colour has a script over time. Pixar colour scripts (Eggleston, Romano) map emotion scene by scene: the palette changes with the arc rather than staying one look throughout.

**Rules.**
- **Values first.** Plan 3–4 value groups. The strongest light–dark difference in the frame sits at the focal point. Hierarchy must survive greyscale.
- **Palette.** One dominant family, one support and one accent, plus neutrals. Put the accent on ≲ 10% of the area, where the meaning is. More than ~4 saturated hue families means no colour means anything ⚙.
  - 60-30-10 is interior-design folklore with no traceable source; it is a starting ratio, not a law.
  - Valid and contrasting systems:
    - monochrome plus one accent (Vignelli's black, white and red);
    - analogous (one gamut mask);
    - a complementary pair at unequal saturation;
    - flat primaries as signifiers;
    - earth tones from a material.
- **Source the palette** from the subject: a photograph, a material, an era, a painting, or the brand's real colours. Never from a colour-wheel generator or a tool default.
- **Saturation budget.** Keep most of the area low to mid chroma. Spend full saturation on the accent and the climax beat *(heuristic: flag > ~25% of the area at high chroma)*.
- **Levels (EBU R103).** 8-bit video runs 16–235 nominal, 5–246 preferred. Out-of-range pixels matter beyond 1% of the image. Lift the deepest blacks slightly unless the piece is deliberately graphic, and keep large areas off clip; clipped white is for speculars and small type.
- **Grading order.** Set contrast (black and white points) first, then neutral balance, then the creative look (lift/gamma/gain). Skin sits near the 123° I-line on a vectorscope.

**Tells.**
- Indigo or violet → blue gradients: "the single loudest AI tell", traced to Tailwind's `indigo-500` default.
- Mesh or aurora blob washes; glassmorphism as house style.
- Neon purple/cyan "synthwave" by default.
- Teal-and-orange by default: physically implausible light, every song in one key.
- Rainbow hue rotation; everything at full saturation.
- Hierarchy carried by hue alone.
- Mid-value mumble: frame luma p2 > 20% and p98 < 80%, with no look declared.
- One fixed look for the whole video, with no colour arc.

**Checks.**
- Greyscale plus blur plus posterise to 3–5 levels: is there a clear notan, with the focal point at maximum local contrast?
- Count hue families ⚙.
- Luma histogram: clip and crush under 1% unless intended.
- Plot the mean colour per shot over time: a flat line means no script.

## 3. Composition and framing

**Principles.**
- **Contrast versus affinity** (Bruce Block). Space, line, shape, tone, colour, movement and rhythm each sit somewhere between contrast and affinity. Contrast is intensity.
  - "Boring" is every component at the same middling level in every shot.
  - Plan an intensity curve: affinity in calm beats, contrast at the climax.
- **Centring is not the error; centring by default is.** Research finds the rule of thirds plays "a minor, if any, role" in judged quality (Amirshahi 2014), and viewers can prefer a centred single object.
  - Symmetry works as a sustained system: Kubrick's one-point perspective, which reads as control and unease, or Anderson's, which is storybook and poised.
  - One centred title on an empty field is a slide.
- **Balance is weight × distance from centre** (Arnheim; McManus 2011 confirms it as a loose tendency). A small, bright, saturated accent far off-centre can balance a big dull mass.
- **Negative space must be shaped.** It can be one-sided (lead room and direction), textured, or a system repeated across shots. Even margins around a small cluster read as a keynote. Müller-Brockmann: white space in an asymmetric grid is active.
- **Space is a choice** (Block): deep, flat, limited or ambiguous. Flat graphic space is as valid as deep 3D space. Mixing them by accident is the tell.

**Rules.**
- **Occupancy.** A hero subject's bounding box spans ~15–60% of the frame; statement headlines have a cap height of ~8–20% of frame height. A box under ~5% with nothing else designed is "small content in an empty frame" *(heuristic)*.
- **Scale contrast.** Hero to secondary is ≥ 3:1, up to 10:1 for posters and kinetic type. Equal sizes give affinity: use that only for calm beats.
- **Placement.**
  - Off-centre: the subject's centroid at ~0.30–0.40 or 0.60–0.70 of the width, with lead room on the open side.
  - Centred: commit fully, with symmetric architecture, a vanishing point and a rhyme across cuts.
  - Denying lead room creates unease. Use it on purpose.
- **Lens feel** (35 mm equivalent, also for CG cameras).
  - 14–24 mm: immersive, exaggerated near–far (keep logos and faces off the edges).
  - 35–50 mm: neutral, documentary.
  - 85–200 mm: compressed, graphic stacked layers, which suit type in 3D.
  - Rotate lens and shot size through a sequence: wide → medium → close → insert.
- **Depth staging.** When the brief wants deep space, use at least 2 of foreground, midground and background, with the background 1–2 stops darker, cooler or desaturated (aerial perspective). A flat shot commits to being flat.

**Tells.**
- A centred headline plus a centred sub-line on a two-stop gradient, with nothing at the edges and no grid.
- Every shot at the same scale, placement and distance.
- Objects floating in a void with no ground, contact shadow or overlap ("ambiguous" by accident).
- A default eye-level, straight-on ~50 mm camera, or the same orbiting turntable, in every 3D shot.
- Thirds placement applied mechanically while the brightest thing sits elsewhere.

**Checks.**
- Thumbnail at 200 px: name the focal point in under 1 s.
- Mirror the frame: if the balance collapses, it was accidental.
- Share of text scenes that centre every word ⚙.
- Across shots: spread of subject size and centroid x. Near zero means monotony.

## 4. Light

**Principles.**
- **Every light needs a reason.** Birn: ask what light "is motivated", meaning what would really be there. Deakins' sources trace to a window, a lamp or a fire. Even abstract graphics need one implied source that highlights, glows and shadows agree with.
- **Three-point lighting is a start, not a look.** Birn: "don't think of three-point lighting as an excuse to light by formula, or to make every scene look the same." Name lights by their job: key, fill, rim, kicker, bounce, practical.
- **Placement and restraint.** Deakins: "It's easy to put up a light, it's figuring out where you put it that's the real problem"; and "I had just enough light to expose the action."
- **Darkness is a tool.** Willis kept Brando's eyes in shadow, "and there was a reason". Storaro: drama is light against shadow and warm against cold.
- **Light reveals material.** Specular highlights are images of the light sources: sharp when rough is low, broad and dim when rough is high. Fake specular dots that reflect nothing read as CG.

**Rules.**
- **Key:fill ratio sets the mood. Choose it; don't inherit it.**
  - 1:1–2:1 is high-key: open, commercial.
  - 3:1–4:1 is the natural dramatic default.
  - 8:1 or more is low-key: noir, tension.
  - Birn: fill at ½ to ⅛ of the key; the fills together never compete with it.
- **Key position.** 15–45° off the camera axis and 15–45° above it is the start. Then move it: 90° side light for texture, top light, or a back/edge key with the camera on the shadow side for shape.
- **Colour temperature.** One dominant temperature per shot. A warm practical (~2700–3200 K) against a cool ambient (~5600 K+), or the inverse for night, is a choice, not a default.
- **Exposure (Rec.709).**
  - Middle grey sits at ~41–48 IRE; clip is at ~100.
  - Keep large areas out of the top ~5% unless they are meant to read as a light.
  - The subject is ≥ 1 stop brighter than the background, or a deliberate silhouette.
- **Tone mapping.** Use a filmic/AgX-style roll-off so emissives and suns go to white instead of flat clipped blobs.
- **HDRI.** Use it for ambient light and reflections, add area lights for key and shape, and never use it as the visible backdrop ("off-the-shelf HDRIs can be too generic").

**Tells.**
- Flat sourceless 1:1 light with no cast shadows.
- The studio cyc: grey sweep, top-left softbox, stock HDRI and a chrome hero, the same in every piece.
- Rim light on everything from no source; purple-cyan neon rims by default.
- Clipped highlight blobs; crushed blacks in a non-noir piece; grey haze with no true black or white anchor.
- A background as bright as the subject.
- Identical light across beats of different mood.
- Forcing 3D light onto content that is graphic by nature (Block: flat space is valid).

**Checks.**
- Point to the source: every highlight, shadow and glow must agree with it.
- Luma histogram:
  - flag Y ≥ 0.98 above 0.5–1% of the frame (excluding intended lights);
  - flag Y ≤ 0.02 above 2–5% (non-noir);
  - flag a p1–p99 spread under 0.5.
- Mean subject luma over background ≥ 2× linear.
- Log key direction, ratio and temperature per shot: if they never change across moods, flag it.

## 5. Motion

**Principles** (Lasseter, SIGGRAPH '87, after Thomas & Johnston).
- Timing *is* meaning: it "reflects the weight and size of an object, and can even carry emotional meaning".
- Give each phase "enough time (but no more)". Too long and "attention will wander"; too short and the move "is finished before the audience notices it". This one rule covers both "slow" and "glitchy".
- **Weight comes from acceleration.** A heavy body is slow to start and stop. Constant speed is "mechanical" (Material); symmetric ease-in-out on everything is floaty.
- **Staging:** "only one idea seen… at a time". In a busy scene the eye goes to what is *still*, so stillness is a tool.
- **Anticipation** lets fast action read: a 3–4-frame zip needs its wind-up held.
- **Follow-through and overlap:** nothing stops dead all at once, and the next action overlaps the last.
- **Strobing:** when an object's positions in successive frames don't overlap, the eye sees separate copies. Blur or stretch fixes it.
- **Registers** (IBM Carbon): *productive* motion is efficient; *expressive* motion is for "significant moments". A piece needs both.

**Rules.**
- **Duration from meaning: the Disney ladder.** Same poses, different in-betweens: 0 is "hit by a tremendous force", 4 is "crisp order", 5 is "friendly", 10 is "stretches a sore muscle". For screen work:
  - snappy or productive moves take 150–300 ms (Material standard `cubic-bezier(0.2,0,0,1)`);
  - an expressive hero move takes 500–1000 ms (emphasized decelerate `(0.05,0.7,0.1,1)`);
  - Material's old UI guidance calls transitions over 400 ms "too slow"; a hero move in a film may take longer.
- **Asymmetry.** Fast attack, long settle. Exits are shorter than entrances (Material: 225 ms in, 195 ms out) and accelerate away.
- **Springs** (Apple WWDC23: duration + bounce).
  - Bounce 0 is the critically damped default.
  - ~0.15 is "barely bouncy, more brisk" (≈ 0.6% overshoot).
  - ~0.3 is noticeable (≈ 4.6%).
  - Above 0.4 is "use with caution" (≥ 10%, cartoon).
  - Tasteful overshoot is 1–5% of the travel. Opacity and colour never overshoot.
- **Overlap and stagger.**
  - Parts of one object: 1–3 frames apart.
  - Separate items: 20–80 ms apart, with the total sweep ≲ 500 ms unless the sweep is the hero *(heuristic)*.
- **One leads.** At each beat one element carries most of the motion; the rest follow 0.1–0.4 s later or stay still. Text arrives, then **stops and stays sharp** while it is read.
- **Motion blur.** A 180° shutter gives a streak of ≈ 0.5 × the per-frame displacement. 90–180° is the valid range; a narrow shutter gives deliberate staccato. Anything moving more than its own width per frame needs blur or stretch.
- **Contrasting valid grammars:**
  - heavy and mechanical (slow starts, hard stops, no bounce);
  - elastic and playful (springs at 0.2–0.35);
  - editorial (fast cuts, near-static frames, type that simply appears);
  - fluid continuous (one long camera move, nothing cuts).
  - Pick one per piece from the subject.

**Tells.**
- One curve and one duration everywhere (everything `inOutCubic` 0.6 s).
- Linear spatial moves.
- Fade-and-rise-from-below, or scale-from-0 pop-in, on every element: the "modern web" signifier (Bushell).
- Everything animating at once, or animating every element so nothing reads as important (Paxton).
- A uniform 100 ms stagger across 20 items, making a 2 s domino.
- Idle wobble or "breathing" on held elements.
- A floaty, over-smooth flight-sim camera.
- Big bounce on serious content.
- Fast moves at 30 fps with no blur.

**Checks.**
- Frame strip at every third frame over a 1 s move. Spacing should bunch at the end. Evenly spaced copies mean linear; copies bunched symmetrically at both ends mean the generic ease.
- Count what moves at once at each beat start.
- Histogram of entrance presets: flag when one entrance covers ≥ 90% of text, or fade-up ≥ 60% ⚙ (`sameness`).

## 6. Editing and rhythm

**Principles.**
- **Murch's Rule of Six.** Emotion 51%, story 23%, rhythm 10%, eye-trace 7%, 2D plane 5%, 3D space 4%. Sacrifice from the bottom. A cut that is *only* on the beat is worth ≤ 10%.
- **Cut at comprehension.** Murch cuts where the thought completes, "the blink". In graphics, that is when the text has been read or the hero action has resolved, not on a fixed clock.
- **Pace has a shape** (Cutting, 160 films).
  - Shots lengthen and motion drops in the setup.
  - Shots shorten and motion rises into the climax.
  - Shots **lengthen again at the end of the climax**.
  - So: a fast run, then a held shot. Everything fast with no release is monotony.
- **Shot length and motion are coupled** (r = −.46). Short shots carry one decisive event; long shots need internal development. "The same composition for 7 seconds" is a long shot with nothing happening.
- **Rhythm comes in waves, not a tempo.** Shot lengths are skewed (lognormal-like) and trend toward 1/f: runs of short shots alternate with runs of long ones. Constant lengths and random lengths are both wrong.
- **Music guides cutting; it does not dictate it.** Save beat-synced cuts for key moments. Cutting slightly before or after an accent can hit harder. Cutting every kick is "a metronome with pictures". Change scene on phrase boundaries (4 or 8 bars).

**Rules.**
- **Reference average shot lengths (ASL).**
  - Hollywood now: < 4 s (1930s: ~10 s).
  - TV ads: < 2 s.
  - Music video: ~2 s.
  - Cinematic social ads: ~1.2 s, with a range of 0.5–7.5 s.
- **Spread matters as much as the mean.** Shot-length CV (sd/mean) of 0.6–1.2 is healthy; < 0.4 is metronomic ⚙; > 1.5 means one shot dominates *(heuristic on lognormal)*. Aim for max/median ≥ 3.
- **Fast showcase** (20–60 s):
  - median shot 1–2 s;
  - bursts of 2–4 shots at 0.3–0.5 s, never as the constant;
  - at least one 3–6 s breath after the densest run or under the title.
  - The repo's fast-intent limits (≤ 2.5 s per shot, ≤ 1.5 s median; `docs pacing`) are this user's showcase taste. Calm and steady pieces hold longer, but every hold develops.
- **Form.** Cold open (hook) → introduce → escalate → climax without resolution → title → **button** (one last beat after the title). The quiet middle is what lets the loud end land.
- **Cut types.**
  - Hard cut by default.
  - Cut on action; match cuts that carry a shape, colour or motion vector across.
  - J/L cuts (the next shot's sound leads in, or the last shot's sound trails over).
  - Slides, spins and decorative dissolves "felt like this was edited by a teen in iMovie" (pro editors reviewing AI ads, 2026).
- **Eye-trace.** Put the incoming focal point near where the eye was (continuity), or far away on purpose (a jolt).

**Tells.**
- Equal shot lengths.
- Every cut on every beat.
- Flat energy with no crescendo or drop.
- No breath after the peak.
- Long static holds.
- Near-identical consecutive framings (a stutter).
- Environment and palette jumping with no motive ("a compilation of unrelated stock footage").
- Music ending by truncation.
- Visible impacts with no sound.
- A logo fading out with no button.

**Checks.**
- From the cut list: ASL, median, CV and max/median ⚙ (render: `pace`, `rhythm`, `arc`).
- Plot shot length over time: expect a slower open, the densest cuts at ~60–85% of the way through, then lengthening.
- Look at the cut pairs in `critique --pack`: is the focal point placed by choice, does the motion carry over, and is the framing different enough?

## 7. Sound and music

**Principles.**
- **Sound changes what the picture means.** Chion calls this "added value". *Synchresis* is the irresistible weld of a sound and an image that happen at the same instant. A sync point is a choice of what to stress.
- **Presets ignore context, and that is the "too much synthesizer" problem.**
  - Zimmer: "I'll never use one of the presets"; a synth string patch "will sound the same and it won't be right."
  - Reznor calls endless cheap plugin synths "lifeless", "sterile".
  - Synths are a deliberate colour (Göransson's one distorted synth in *Oppenheimer*), not the bed.
- **A specific instrument becomes the film's identity:** Göransson's bass recorder for *The Mandalorian*, "warm and intimate".
- **Silence is structure.** Wallfisch plans silence while spotting a film. Burwell: "do more with less". Wall-to-wall sound leaves the loud moments "nothing to contrast against."
- **Spot before you score.** Decide where each cue starts and stops and what it hits.
- **Don't mirror every motion.** Mickey-mousing ("most vulgar", Cocteau) only restates the picture; use it only in play and comedy.
- **Signature sounds wear out.** The *Inception* BRAAAM became a cliché. Risers, whooshes and sub-drops on everything are next.
- **Stock "corporate uplifting" is written to fit anything,** so it fits nothing. Music supervisors look for "something other people don't know about".

**Rules.**
- **Instrumentation comes from the concept.** Choose one signature voice and name why. Examples:
  - felt piano for intimacy;
  - pizzicato or marimba for precision and data;
  - a brass chorale for civic weight;
  - nylon guitar for warmth and handmade;
  - bowed vibraphone or glass for technology without synth;
  - solo cello for gravity;
  - a field drum for momentum;
  - clock ticks or tuned percussion for time.
- **Synths only when the concept is electronic.** Even then, give the synth one job (pad *or* bass *or* lead), and keep it ≲ 30% of the voices otherwise *(heuristic)* ⚙ (`music` tip).
- **Vary between pieces.** Change at least 2 of: lead family, tempo (±15 BPM), mode and groove.
- **Arc.** Start sparse with 1–2 voices, build to 3–5, peak, then drop to near silence before the final hit or name. Plan at least one silence of 0.5–2 s, or a bar's rest, and land the reveal right after it.
- **Sound effects.** At most 1 riser per 30 s and 0–3 impacts per minute; never a whoosh on every cut. Match the material: paper, glass, wood or a click beat generic air. A single real sound can be the logo sting (Apple's doorbell).
- **Sync.** Viewers detect audio up to 45 ms early or 125 ms late (ITU-R BT.1359). Put hits on the frame or one frame late, never early. Pick 3–6 hit points per minute rather than chasing every cut.
- **Loudness.**
  - Web delivery: −14 to −16 LUFS integrated, true peak ≤ −1 dBTP (≤ −2 if louder than −14). Spotify and YouTube normalise to ≈ −14 and only turn audio *down*.
  - Broadcast (EBU R128): −23 LUFS ±0.5, ≤ −1 dBTP.
  - Voice-over: music ≥ 10 LU under speech (Torcoli, JAES 2019), with the duck recovering over 0.3–1 s.
  - Short-term loudness should range ≥ 6 LU: the quiet section has to exist.

**Tells.**
- The four-on-the-floor "uplifting corporate" bed: claps, ukulele or whistle, I–V–vi–IV, and a pad from frame 1 to the end.
- All-synth stock patches (supersaw, big-reverb pad, plucked arp) with the same timbres every video.
- No silence and a flat loudness curve.
- A whoosh or riser on every cut.
- A BRAAAM on non-epic content.
- A master louder than −14 LUFS.
- Text-to-speech voice with no breaths and one cadence for every sentence.

**Checks.**
- Mute test: do the pictures alone tell it?
- Audio-only test: does the music alone suggest *this* subject? If it fits any product, it fails.
- Loudness gate ⚙ (`audio`).
- Hit frames against onset frames: within 1 frame, never early.

## 8. Concept, originality and taste

**Principles.**
- **One idea that only fits this subject: the swap test.** Put a competitor's logo or name on it. If it still works, it fails (Brandautopsy; Neumeier: "no other company should be able to improve its icon by using part of yours").
- **Concept first, then style.** A style copied without its concept goes "from Buck to… yuck" (Honig). Corporate Memphis began as Buck's commissioned system for Facebook, and became a template once it was copied without the reason.
- **Every detail carries meaning.** Vignelli is against "stripes and swash of color splashed across pages for no reason", and against fashion. Rams: "Less, but better"; honest; "thorough down to the last detail"; innovation "can never be an end in itself."
- **Paul Graham, "Taste for Makers".** Good design is simple, suggestive, often slightly funny, strange and daring, and it is redesign: "rare to get things right the first time". "If you don't know where your ideas are coming from, you're probably imitating an imitator." Draw from nature and the real source material, not from other people's output.
- **The gap** (Ira Glass). Taste outruns ability, and the gap closes through volume. For an agent: make several variants, judge them against strong references, then cut.
- **Real craft is felt.** Apple's 2025 TV intro was shot with real glass ("craft should be felt, not faked"). Linear's launch films keep the real UI full screen. "Don't Blink" animates what each word *means* and leaves the 2D plane exactly once, as a joke.
- **Defaults are averages.** A model's output is the median of its training data; constraints, references and named bans fix that (prg.sh).

**Rules.**
- **Write the idea as one sentence** with a metaphor taken from the subject's own data, mechanism, history or physical material. Swap-test it.
- **Include at least 3 subject-only elements:** the real UI, real data, real names and numbers, the product's own shapes and sounds.
- **The medium follows the idea.** Set type, illustration and paths, real UI, photography-style compositing, shader worlds and ray-traced 3D are equal options. Use 3D only where depth carries meaning, for one or two moments or for the whole piece if the idea *is* spatial. Never use the same environment, material or motif as a previous piece.
- **One motion idea per piece.** Animate the signified: a word that builds is built.
- **Redesign pass.** Make 2–3 concept variants and pick one. Then cut 20–30% of the elements or the duration, and swap-test again.
- **Series variety.** Within any 5 pieces, no two share a palette, a typeface, a lead instrument, a 3D motif or an opening move.
- **References vs copying.** Borrow the *grammar* of a reference (its pacing curve, framing logic, how type meets image), never its shots, palette or exact moves (`skills/anikit-reverse-video`). Recipes are techniques, not pieces (critique flags a spec that is mostly a recipe).

## 9. Catalogue of tells (avoid unless the concept truly demands it, and say why)

**Visual.**
- Indigo/violet → blue gradients.
- A centred hero headline on a mesh, aurora or blob wash.
- Glassmorphism.
- Three identical rounded cards with thin-line icons and 0.1-opacity shadows.
- Placeholder grey bars instead of real content.
- Default Inter/Roboto.
- Sparkle or emoji bullets.
- Corporate Memphis figures.
- Blanded geometric-sans wordmarks.
- Primitive spheres, cubes and tori on checker or cyc floors.
- Chrome or glass blobs, repeated from piece to piece.
- Teal and orange.
- Lens flares and particles over nothing.
- Gibberish or low-resolution text.

**The "tasteful AI" look** (the user's fourth rejection, Margin: "complete AI slop"):
- a cream/off-white "paper" ground, an editorial serif (Instrument Serif and its kin) and one red or terracotta accent;
- aphorism copy, and small-caps tracked labels over a serif line;
- sparse, near-static frames where a slow drift is the only motion;
- a Notion/Linear/Arc-style landing page turned into a video.
- Its documented tells (Jim Nielsen's "The AI Aesthetic" and its HN thread; capitalandcompute; claudecodehq):
  - a cream ground around #F4F1EA, Instrument Serif or Fraunces, and a terracotta accent near #D97757;
  - tracked all-caps or mono micro-labels;
  - 01/02/03 numbering on things that aren't a sequence;
  - one accented word per headline.
- The wider 2026 tired list:
  - bento grids ("cards all asking for the same amount of attention");
  - mesh gradients ("until every site looked like Stripe's");
  - the dark "Linear look" of gradient, glass and bold sans;
  - "lazy minimalism: minimalism without soul" (Creative Boom);
  - count-up numbers;
  - floaty camera drift, which AI-video vendors recommend *because* it hides artifacts.

Restraint swapped for 3D clichés is still a cliché. Restraint must be rich (§11), never empty.

**Motion.**
- Fade-and-rise or scale-0 entrances on everything.
- One ease and one duration everywhere.
- A floaty flight-sim camera.
- Everything moving at once.
- Idle wobble.
- Constant-tempo cuts.
- Decorative slide or spin transitions.
- The same slow 6–7 s dolly in every shot (rejected: Meridian, Lighthouse).

**Sound.**
- An uplifting corporate bed.
- All-synth presets.
- No silence.
- Whooshes on every cut.
- A BRAAAM.
- Text-to-speech with no breaths.

**Copy.**
- Weightless lines ("Build faster. Ship smarter.").
- Seamless, revolutionary, unlock, leverage, "We're excited".
- Claims with no number or demonstration.

## 10. The taste pass (run before any final render)

Step 1 happens before designing; the rest happen on the `critique --pack` strip, its cut pairs and three full-size stills, listening once muted and once audio-only.

1. **Diverge.** From the brief, write 3 genuinely different directions, each with a medium, a world, a palette, a type choice, a motion character and a sound. Make them differ on every axis, not just in colour. Pick the one most specific to this brief. List what earlier pieces in this project used (medium, world, motif, palette, type, light, music) and make sure the chosen direction differs from all of them.
2. **Swap test.** With a competitor's name it would stop working. The idea fits in one sentence, and there are ≥ 3 subject-only elements (real UI, data, names, the product's own shapes or sounds).
3. **Medium.** It follows from the idea: type, 2D, UI, photo, shader or 3D. 3D only where depth means something.
4. **Focal point.** Each still names its subject in < 1 s at thumbnail size, and the eye knows where to go next.
5. **Composition.** Off-centre with lead room, or committed symmetry. Negative space is shaped. Shot size varies.
6. **Scale.** 2–4 type sizes on one ratio. The hero is ≥ 3:1 against the next size. Nothing shouts that doesn't need to.
7. **Read.** Every word is crisp at 100% (no blur, defocus or bloom), ≥ 4.5:1 in luma, ≥ ~36 px, and held max(0.83 s, chars/15, words × 0.3) after it settles.
8. **Values.** Hierarchy survives greyscale. No mid-grey mumble, and no clipped or crushed areas unless intended.
9. **Palette.** From the subject: one base, one accent, ≤ 4 hue families. It changes with the arc.
10. **Light.** You can point to the key. The ratio fits the mood, the subject separates from the ground, and highlights roll off.
11. **Motion.** Fast attack and long settle. One element leads. Entrances vary by role. Fast moves are blurred. Overshoot ≤ 5% unless the piece is playful.
12. **Rhythm and arc.** Pace matches the declared intent. Shot lengths vary (CV ≳ 0.6), with a breath after the densest run. Cuts land on action or comprehension, with beats saved for the key hits. Hook by 1 s, peak at about ⅔, then land on a held frame or a button.
13. **Sound.** Instruments come from the concept (synths only if it's electronic). At least one silence. Hits on the frame. Sfx are rare and match the material. −14 to −16 LUFS, ≤ −1 dBTP.
14. **The bar.** Would a senior motion designer scroll past this on Vimeo? Name the trend it looks like; if you can, it fails. No placeholder, no filler copy, and no §9 tell without a stated reason.
15. **Cut 20%.** Remove the weakest shot, word or effect. If it got better, cut again.

## 11. Ambition: amazing, not merely correct

Correct is not enough. The fourth rejection (Margin) passed every rule above and was still "complete AI slop", because it was empty and recognisable as a trend. Vimeo's Staff Pick tenets are concept, craft, execution and originality, with one question: "Does it look and sound amazing?" Award work can be restrained in palette, but never in idea, world logic, detail per frame or choreography.

**Principles.**
- **Idea first, polish last.** Clair (Elastic): the idea is the core of the apple; style is only the skin. Holmedal (ManvsMachine): don't get "distracted with execution (garnish) until the foundation is in place", and "make sure everything is motivated and has purpose."
- **A world with its own logic.**
  - *Game of Thrones* (Elastic) is one mechanical map, whose burning sun is also the key light. Gears grow the cities, the materials are wood, metal and leather, and the camera is motion control rather than a "magic camera".
  - Apple's M1 Max film (Shane Griffin): "I didn't want any piece to just turn on from nowhere"; everything "had to come from something".
- **Density of considered detail.** *Stranger Things* is purely typographic, yet it carries scanned 35 mm grain, light jitter, red colour shifts and "a few too many inconsistencies on purpose". Cooper etched *Se7en*'s type by hand. Buck calls the handmade "earned"; ManvsMachine 3D-scans every Nike shoe. Restraint chooses *what* is in the frame and never leaves it empty.
- **Transitions are ideas.** Match cuts keep the subject in the same screen position, match similar shapes, and carry momentum: cut mid-move, then resume (School of Motion). Contrast tempi, and allow one deliberately disorienting cut.
- **Surprise.** Elastic's Semi-Permanent film "start[s] dark before it completely changes". Golden Wolf: "there's always space for the weird".
- **Sound from the start.** Antfood joins "from concept and pre-production" and explores "30, 50 or 100 ideas"; sound "won't save mediocre direction".
- **Iterate past "working".** Apple's crystal-to-chip system was refined over days, and *Game of Thrones* went through four variants. Watch at full speed and in chunks, and review contact sheets.

**With this engine.**
- **Real imagery, made:**
  - paths drawn and repeated procedurally (`repeat` with `$i`, `script` loops: terrain, a field of glyphs, a data landscape);
  - your own GLSL worlds;
  - particles and rigid-body physics with a cause on screen;
  - ray-traced objects in a motivated key;
  - compositions used as textures.
- Gradients never stand in for imagery.
- **Texture at the edges:** grain, halation, slight jitter or misregistration chosen for the world, kept off words while they are read.
- **Choreography:**
  - one element's exit is the next one's entrance;
  - a camera move continues through a cut;
  - a shape becomes the next shot's subject.

**Checks.**
- Pause on a random frame. It holds up as a still: a hero, a supporting structure, texture, and ≥ 3 layers of intentional detail you can name with their jobs.
- Every entrance has a cause already on screen.
- At least one transition is an idea (a match, a morph or a continuous move), and you can name the surprise.
- Mute it, then listen with your eyes closed: each half still shows the structure.
- Would a senior motion designer stop scrolling on Vimeo? Name the trend it resembles; if you can, change the direction (§10, step 1).
