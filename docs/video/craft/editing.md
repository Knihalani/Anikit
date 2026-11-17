# Editing and rhythm at a professional level

Scope: 10–60 s pieces at 1080p, built from JSON specs: cuts, transitions, camera moves and generated music.
Tags: **[src]** means a cited source. **[conv]** means a widespread craft convention for which I found no primary source; treat it as a starting value, not a fact.
Frame arithmetic used throughout: 1 frame lasts 41.7 ms at 24 fps, 33.3 ms at 30 fps and 16.7 ms at 60 fps. One beat lasts `fps*60/BPM` frames, so 120 BPM gives 12 f at 24 fps and 15 f at 30 fps.

## 1. Principles (why cuts work)

- **Emotion outranks everything.** Murch ranks what a cut must preserve: emotion 51%, story 23%, rhythm 10%, eye-trace 7%, the 2D screen plane 5% and 3D space 4%. He says emotion "is worth more than all five of the things underneath it", and that the numbers are "slightly tongue-in-cheek… but not completely". When something has to give, drop it from the bottom of the list first: "Let go of three-dimensional continuity first and then work your way up". Among the top three, give up rhythm first. [src: Elwyn's Murch-opedia]
  - Agent reading: a cut that breaks screen direction but lands the feeling beats a "correct" cut that is dull.
- **A cut is a blink.** Murch, after John Huston: people blink when a thought ends, and a cut works when it falls where the viewer's blink would. The blink "helps an internal separation of thought… or is an involuntary reflex accompanying" one. Practical rule: cut when the idea of a shot has been delivered, not when the shot's motion runs out. [src: PremiumBeat; Web of Stories]
- **"One frame in one direction or another can make such a difference."** (Murch) Cut points are frame decisions, not second decisions. [src: Murch-opedia]
- **Rhythm means cycles of tension and release, shaped by an editor acting as choreographer.** Pearlman separates three rhythms that run at once:
  - *physical rhythm*: the movement of images and sound;
  - *emotional rhythm*: which shot, where in the sequence, and for how long;
  - *event rhythm*: the movement of plot and structure.
  [src: Senses of Cinema review of Cutting Rhythms; Elsevier listing]
  Her "trajectory phrasing" treats every movement as an arc with an attitude (Laban efforts), and the editor shapes where that arc is cut. A boring video usually has one flat physical rhythm and no emotional or event rhythm at all.
- **Cut for a reason.** Dmytryk's first rule: never make a cut without a positive reason. He warns that arbitrary rules ("keep every shot under N s") produce bad cutting, and he advises cutting long rather than short when unsure. [src: ProVideo Coalition, Dmytryk's rules; NYFA]
- **Eisenstein's five methods are five levels of control,** each building on the one before. [src: New School summary; Chicago Media Works notes]
  - *Metric*: shot lengths follow a formula whatever the content (e.g. 12-12-6-6-3-3 frames). Tension comes from shortening the pieces while keeping their proportions.
  - *Rhythmic*: movement inside the frame decides the length.
  - *Tonal*: the emotional "dominant" of each piece (light, sharpness, colour) drives the cut.
  - *Overtonal*: the sum of the three levels above.
  - *Intellectual*: two images collide to produce a concept that neither holds alone.
  Motion graphics mostly lives in metric and rhythmic montage. Studio work adds tonal montage (colour and value shifts as the beat) and intellectual montage (shape A + shape B = idea).
- **Kuleshov.** Meaning sits in the juxtaposition. Placing a neutral shot after a charged one transfers the charge, so the order of shots matters as much as which shots are used. [conv: classic film theory]
- **Shot length depends on how much there is to read.** Cutting found that each additional character in a shot added about 1.5 s to its average length: a dense shot needs time and a single subject can be cut fast. [src: cinemetrics.uchicago.edu article 590235ff; Stephen Follows]
- **Shot-length rhythms that grip are 1/f, not uniform.** Cutting, DeLong and Nothelfer measured 150 films from 1935–2005:
  - post-1980 films approach a 1/f ("pink") pattern of shot lengths, which mirrors how attention fluctuates;
  - action films come closest to it.
  [src: Psychological Science 2010 via EurekAlert / APS]
  Shot lengths within a film are roughly lognormal. [src: Cinemetrics response to Salt]
  Agent reading: uniform shot lengths (every shot 2.0 s) are a strong statistical signature of "boring".

## 2. Techniques with numbers

### Pace benchmarks
| Context | Average shot length (ASL) | Source |
|---|---|---|
| Hollywood 1930–60 | 8–11 s | Bordwell, "Intensified Continuity" [src: mediafactory summary; Artforum] |
| Mainstream films now | 3–5 s | Bordwell [src] |
| English-language films, 1930 → today | ~12 s → ~2.5 s | Cutting, via Cinemetrics [src] |
| Fury Road | ~2,700 shots, centre-framed, ASL ≈ 2.6 s | [src: Softpedia; NoFilmSchool] |

For 10–60 s pieces, a useful spread is a median of 1.2–2.5 s with a long tail: one or two holds of 3–6 s ("breath" shots) and bursts of 4–8 frame cuts at the peak. Never make all shots equal. [conv, consistent with the lognormal/1/f data]

### Cutting on action
- Cut *during* movement, not before or after it. The movement carries the eye across the cut. [src: Videomaker "Cutting on Action"; Frame.io on Fallout]
- Exit shot A about 1/3 of the way into the move, and enter shot B with the move already under way. Avoid both repeated action and missing action. [src: Videomaker]
- Overlap of **1–3 frames** of repeated action reads as continuous, and makes hits feel heavier. Removing 1–3 frames (an ellipsis) makes the action feel faster. [src: Reallusion forum, anecdotal; Frame.io notes on dropped frames in Fallout; Sixel: "increasing speed ramps and shaving off frames", ProVideo Coalition]
- Impacts: cut 0–2 frames after the contact frame, so the contact frame itself lands as the first or second frame of B or the last of A. Never cut 5+ frames after contact, because the energy has already drained. [conv]

### Eye-trace and centre framing
- Fury Road: George Miller kept ordering "Put the cross hairs on her nose!", so that "every new shot… must occupy the same space as the previous shot". That let Sixel "cut as fast as possible with the confident knowledge that the visual information would be understood". [src: Eugene Wei; Softpedia]
- Rule of thumb [conv]:
  - if the point of interest jumps more than ~25% of frame width across a cut, the cut needs ≥ 8–12 frames of settle time afterwards;
  - for cuts under 8 frames, keep the point of interest within ~10% of where it was.
- Deliberately breaking this (a jump left to right on the downbeat) is a valid accent. Use it at most once or twice per piece.

### Music
- Woollen (trailers): music is "eighty percent" of what gives a trailer its pulse: "When we figure out the music, we often figure out the trailer".
  - He builds rhythm from diegetic sound: chalkboard slams in A Serious Man, a train in Little Children.
  - He repeated a 1.5 s shot as a "rhythm heartbeat", against trailer convention.
  [src: NPR Fresh Air 2015; Yahoo/IndieWire profile]
- Editors "favor the co-occurrence of music and video events using strategies such as **anticipation**". How much co-occurrence depends on genre. (Large-scale analysis of official music videos.) [src: arXiv 2108.00970]
- Cutting on every beat is "very predictable and obvious", and slavish beat-cutting makes "the music composer the editor". [src: Creative Cow forum] On-beat and off-beat cutting produce measurably different attention patterns. [src: Wenberg & Carlgren, Projections 15(1) 2021, abstract]
- Suggested scheme [conv]:
  - Downbeats of phrases (every 4 or 8 bars) get the *structural* cuts: scene changes, colour changes.
  - Inside a phrase, cut on half-notes or on the snare/backbeat rather than the kick.
  - Accelerate to 8ths or 16ths only at the climax, for ≤ 1 bar.
  - Cut **1–2 frames before** the transient (anticipation): the eye registers the new image as the sound hits.
  - Syncopation means landing some cuts on the "and". Use it for 20–40% of cuts in the middle of the piece.
- AV sync tolerance (ITU-R BT.1359): sound leading picture becomes detectable at about 45 ms (1 frame at 24 fps), and sound lagging at about 125 ms. Sound arriving slightly late is natural and sound arriving early is not. A visual hit should never come *after* its sound by more than a frame. [src: ITU-R BT.1359]

### Sound across cuts (J/L cuts, sound bridges, smash cuts)
- J-cut: the incoming sound starts 6–24 frames before the picture cut, which pulls the viewer forward. L-cut: the outgoing sound carries 6–24 frames past the cut, which lets a moment linger. [conv]
- The sound bridge is the main way a 30 s piece gets continuity without matched action.
- Murch assembles scenes "as a silent movie" first. Sixel and Miller "cut silently for a long time". Baxter cut The Killer's tension scenes "dry" and relied on "abrupt sound editing". [src: Murch-opedia; ProVideo Coalition; Boris FX Art of the Cut]
  - Agent reading: a sequence should work muted. Sound then adds lift, not structure.
- Smash cut: a loud/fast sequence cuts on a frame to near silence or stillness (or the reverse). It needs ≥ 1 s of build in the outgoing shot. Leave 2–6 frames of silence or a held frame before the hit to sharpen it (a "pre-hit gap"). [conv]

### Speed ramps
- Sixel ramped exits from set pieces and shot lava at 1000 fps. Miller's rule is "staying on the wave". [src: ProVideo Coalition]
- A typical ramp [conv]:
  - 100% → 20–30% over 6–10 frames, eased in;
  - hold the slow section through the key pose for 12–24 frames;
  - snap back to 100–150% over 3–6 frames, easing *out* sharply.
  Keep the ramp curve monotonic in each direction. Put the fast segment *after* the peak pose, not before it.

### Tension and release
- Baxter: "'marinating in the stretch' is building tension… We're not in a rush here until we are". [src: Boris FX, The Killer]
- Hamilton: "if it can go, it should go". Sometimes "adjusting an edit slightly can be enough to make a beat hit better". He cut ~70 h of helicopter footage down to a 7.5 min sequence through passes of 12 h → 4 h → 2.5 h → 1 h. [src: Frame.io on Fallout]
- Pattern for a short piece [conv]:
  - establish a pulse in the first 3 s;
  - lengthen shots slightly (by 20–30%) before the drop;
  - drop to the fastest cuts for 1–2 s;
  - then hold one long shot (2–4× the median) for release, ideally with motion slowing inside it.
- Pace (cuts per second) and tempo (felt urgency from motion, music and content) can be set separately. Long shots with fast inner motion feel urgent; fast cuts of still images feel nervous rather than energetic.

### Trailer and commercial macro-structure
- Act 1 sets up. Act 2 complicates. Act 3 is a rapid montage, then the title, then a "button" (a final gag, scare or twist) after the title. [src: Eugene Wei on Woollen; roughcut blog]
- For 15–30 s spots, run 3 movements in ratios of about 30 / 50 / 20, with the button in the last 1–2 s. [conv]

### Editing designed frames (motion graphics)
- A graphic match picks one anchor (shape, colour, motion vector or position) and hides the change inside it. Do not match every property. [src: StudioBinder match cuts; cursa.app match-move guide]
- Common moves:
  - a colour panel wipes across and becomes the next background;
  - a rotating element keeps rotating but becomes a different graphic;
  - a logo becomes a line, which becomes a number.
  [src: School of Motion; cursa.app]
- Test hard cuts at several neighbouring frames and keep the one that best preserves motion and position. [src: cursa.app] Carry the *motion vector* across the cut: if A exits right at speed v, B should enter moving right at about v. [conv]

## 3. What separates studio work from amateur work
- **Variance.** Studio shot lengths are lognormal with clear phrasing; amateur lengths are uniform. [src: Cinemetrics; Cutting 2010]
- **Every cut has a reason.** Studio work cuts on thought completion or action; amateur work cuts on a timer. [src: Dmytryk; Murch]
- **Subtraction.** "If it can go, it should go." Studio editors remove frames inside shots, not just whole shots. [src: Hamilton; Sixel]
- **Hierarchy of rhythm.** Studio work has phrase-level structure (4/8-bar sections, each with its own density). Amateur work has beat-level sameness. [conv]
- **Sound leads.** J/L cuts and sound bridges in studio work; hard audio cuts on every picture cut in amateur work. [conv]
- **Selection standard.** Hamilton: "You can't put average shots in Top Gun". Studio work keeps only the best few frames. [src: Slashfilm]
- **Taste in repetition.** One repeated motif used deliberately, like Woollen's 1.5 s heartbeat shot, against amateur random variety. [src: IndieWire/Yahoo profile]

## 4. Anti-patterns
- Every shot the same length. Every cut on the downbeat. Same transition everywhere (crossfades especially).
- Transitions used to hide weak cuts. Studio work is roughly 90%+ hard cuts, with designed transitions saved for structural moments. [conv]
- Cutting on a still frame between two still frames: there is no motion to carry the eye. Cut on movement. [src: Videomaker]
- The point of interest jumping across the frame on a fast cut. [src: Fury Road analysis]
- Cutting 5+ frames after an impact or peak pose, so the energy dies.
- A speed ramp that slows down *before* the interesting pose and then holds after it.
- No release: three or more minutes at maximum density in a long piece, or more than ~6 s at maximum density in a 30 s piece. Without contrast, fast stops feeling fast. [conv]
- Visual hit late against its sound by more than 1 frame. [src: ITU-R BT.1359]
- Text cut away before it can be read [conv]:
  - minimum on-screen time ≈ 0.4 s plus 0.25 s per word (≈ 3–4 words/s for short display text);
  - never cut a 6-word line in under 1.5 s.

## 5. How an agent can check its own work
1. **Shot-length stats** from the spec:
   - compute the median, coefficient of variation (CV) and min/max of shot durations;
   - target CV ≥ 0.45, at least one shot ≥ 2.5× the median, at least one burst of cuts ≤ 0.5× the median;
   - fail if CV < 0.25.
2. **Beat alignment:**
   - for each cut, the offset in frames to the nearest beat and the nearest downbeat;
   - target: structural cuts within −2…0 f of a downbeat;
   - 20–40% of other cuts off the main beat (syncopated);
   - no visual impact more than +1 f after its audio transient.
3. **Phrase density:** cuts per bar for each 4-bar phrase. There should be at least 2 distinct density levels and a peak in the final third.
4. **Eye-trace delta:** render the last frame of A and the first frame of B. Compute a saliency or brightness centroid for each, and their distance as a % of frame width. Flag cuts under 8 f where the distance is over 15%.
5. **Contact sheet:** render 1 frame per shot (mid-shot) as a strip. By eye, adjacent tiles should contrast in at least one of scale, value, colour or direction. Six similar tiles in a row means a flat rhythm.
6. **Mute test:** play the cut without audio. If it is dull, the visuals carry no rhythm. [src: LBB "Finely Sliced" editors]
7. **Hard-cut ratio:** count hard cuts against designed transitions. Flag if designed transitions are more than 20% of cuts.
8. **Read time:** for each text element, check duration ≥ 0.4 + 0.25 × words (seconds).


## 6. Gaps against Anikit (TASTE)

- **Anticipating the beat is already possible.** Markers take offsets (`"@beat9-0.033"` is one frame early at 30 fps), so cuts can lead the beat the way pro music-video editors do. The pacing docs and skills only show on-beat cuts.
- **Eye-trace across fast cuts is unchecked.** On cuts under ~8 frames, a point of interest that jumps more than ~15% of the frame width is the Fury Road lesson. `critique --render` already samples the frames before and after each cut, so a salient-centroid jump check would be cheap there.
- **Busy frames need longer holds.** Holds aren't measured against frame density. Pairing per-shot visual complexity (edge density) with shot length in the render review would flag dense frames that cut too fast and empty frames that hold too long.
- **Cut silent first.** The skills could say: judge the strip muted before adding the score. `render --no-audio` exists.
- **Rhythm threshold.** The spec-level `rhythm` tip fires at CV < 0.25. Research puts healthy edited film at roughly ≥ 0.45 (lognormal, 1/f), so the tip text quotes 60–120% as typical without demanding it.

## 7. Sources
- Elwyn, "The Walter Murch-opedia": https://jonnyelwyn.co.uk/film-and-video-editing/the-walter-murch-opedia/
- Murch blink passage: https://www.premiumbeat.com/blog/when-and-where-to-make-the-cut-inspired-by-walter-murchs-in-the-blink-of-an-eye/ ; https://www.webofstories.com/play/56382
- Murch Rule of Six: https://nofilmschool.com/2016/11/6-rules-good-cutting-according-oscar-winning-editor-walter-murch
- Pearlman, Cutting Rhythms (review): https://www.sensesofcinema.com/2026/book-reviews/feeling-the-screen-storys-pulse-karen-peralmans-cutting-rhythms-creative-film-editing/ ; https://shop.elsevier.com/books/cutting-rhythms/pearlman/978-0-240-81014-0
- Dmytryk's rules: https://www.provideocoalition.com/seven_rules_for_film_and_video_editors/ ; https://www.nyfa.edu/student-resources/what-you-can-learn-from-edward-dmytryks-7-rules-of-cutting/
- Eisenstein methods: https://portfolio.newschool.edu/lune/2016/11/03/montage-methods-in-sergei-eisensteins-article ; https://www.chicagomediaworks.com/2instructworks/3editing_doc/Eisenstein.pdf
- Fury Road centre framing: https://www.eugenewei.com/blog/2015/5/31/creating-order-out-of-the-chaos-of-mad-max-fury-road ; https://news.softpedia.com/news/How-Editor-Brought-Order-into-the-Chaos-of-Mad-Max-Fury-Road-with-Center-Framing-Video-483084.shtml ; https://nofilmschool.com/2015/05/how-framing-mad-max-fury-road-keep-viewer-oriented-world-gone-mad
- Sixel, Art of the Cut: https://www.provideocoalition.com/art-cut-margaret-sixel-editor-mad-max-fury-road/
- Baxter on The Killer: https://borisfx.com/blog/aotc/art-of-the-cut-the-killer ; Gone Girl: https://www.provideocoalition.com/kirk-baxter-leads-edit-of-gone-girl-latest-box-office-hit-for-david-fincher/
- Hamilton on Fallout: https://blog.frame.io/2018/07/30/inside-mission-impossible-fallout/ ; Top Gun: https://slashfilm.com/1095254/top-gun-maverick-editor-eddie-hamilton-felt-the-weight-of-expectations-exclusive-interview
- Woollen: https://www.npr.org/2015/02/23/388481464/how-the-man-behind-the-trailers-sparks-an-urge-to-see-a-movie ; https://www.yahoo.com/entertainment/schindler-list-hidden-life-unsung-190015220.html ; https://www.eugenewei.com/blog/2014/11/10/mark-woollen
- Schoonmaker on rhythm: https://nofilmschool.com/2014/04/editor-thelma-schoonmaker-breaks-down-raging-bull-tribeca-film-festival
- Cutting, DeLong & Nothelfer 2010: https://www.eurekalert.org/pub_releases/2010-02/afps-tso022310.php ; https://www.psychologicalscience.org/observer/the-science-of-hollywood-blockbusters-2
- Cinemetrics (lognormal, characters per shot): https://cinemetrics.uchicago.edu/article/590235ff-85aa-4714-9553-71d4c44fd61c ; https://stephenfollows.com/many-shots-average-movie/
- Bordwell, Intensified Continuity: https://www.mediafactory.org.au/2016-cut-v-shot/?p=258 ; https://www.artforum.com/features/david-bordwell-2-195241/
- Music-video co-occurrence: https://arxiv.org/abs/2108.00970 ; Wenberg & Carlgren: https://katalog.lib.cas.cz/KNAV/EdsRecord/edb,149872035
- Beat cutting discussion: https://creativecow.net/?p=1239950 ; https://www.eoshd.com/comments/topic/46696-editing/
- Cutting on action: https://www.videomaker.com/article/c10/13536-cutting-on-action/ ; https://forum.reallusion.com/PrintTopic371393.aspx
- Match cuts / motion design: https://www.studiobinder.com/blog/match-cuts/ ; https://cursa.app/en/page/transitions-and-scene-building-cuts-slides-wipes-and-match-moves ; https://www.schoolofmotion.com/blog/sports-motion-design-inspiration
- Commercial editors on rhythm: https://lbbonline.com/news/finely-sliced-feeling-the-rhythm-of-the-edit-with-zach-pridding
- AV sync tolerance: https://www.itu.int/rec/R-REC-BT.1359
