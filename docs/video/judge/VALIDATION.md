# Judge validation: the user's rejected work vs professional references

Run on 2026-11-17 (21:40–22:20 UTC) with `scripts/judge.py` (procedure: `RUBRIC.md`). Each row is 3 blind trials against 3 style-matched refs from `refs/`; the jury never knew which entry was ours. Every panel shown, the keys and the full answers are in `validation/results/<name>/`.

**Result: all 20 rejected pieces FAIL. In 60 of 60 trials the jury called them not professional.** They scored 1.75–4.38 (median 3.12) against references at 6.5–8.5. The 6 controls (held-out pro refs in the candidate's seat) were called professional in 17 of 18 trials and scored 5.38–8.12: 4 PASS, 2 PRO, 0 FAIL. The two ranges do not overlap. Offset and Substation, the two pieces TASTE's independent craft review singled out as strongest, are in the judge's top three.

## Rejected work (sorted by score)

| Piece | Verdict | Judged pro | Blind score | Refs mean | Δ | Full score (+critic) | Compared with |
|---|---|---|---|---|---|---|---|
| div-g-offset | FAIL | 0/3 | 4.38 | 7.54 | -3.16 | 4.78 | fitc-tokyo-2015, catch-me-if-you-can, semi-permanent-sydney-2014 |
| blind-swell | FAIL | 0/3 | 3.88 | 7.34 | -3.46 | 4.11 | game-of-thrones, cosmos-spacetime-odyssey, jpmorgan-payments |
| div-c-substation | FAIL | 0/3 | 3.88 | 7.29 | -3.41 | 4.0 | demo-festival, kurzgesagt-short, bbc-two-ident-vertical |
| div-b-railyard | FAIL | 0/3 | 3.62 | 7.79 | -4.17 | 4.06 | coinbase-system-update, jpmorgan-payments, game-of-thrones |
| sift | FAIL | 0/3 | 3.62 | 6.5 | -2.88 | 3.89 | osko-launch, offf-milano-2017, coinbase-system-update |
| launch-tern | FAIL | 0/3 | 3.5 | 7.25 | -3.75 | 3.67 | osko-launch, kurzgesagt-short, cosmos-spacetime-odyssey |
| div-a-halde | FAIL | 0/3 | 3.38 | 7.08 | -3.7 | 3.72 | hermes-buck, calm-sleep, new-yorker-festival |
| div-f-calibre9 | FAIL | 0/3 | 3.25 | 7.75 | -4.5 | 3.61 | osko-launch, the-crown, tiffany-holiday |
| fernway | FAIL | 0/3 | 3.25 | 7.79 | -4.54 | 3.67 | coinbase-system-update, thank-you-for-smoking, fitc-tokyo-2015 |
| div-e-arctic | FAIL | 0/3 | 3.12 | 7.33 | -4.21 | 3.39 | kurzgesagt-short, cosmos-spacetime-odyssey, kurzgesagt-climate |
| margin | FAIL | 0/3 | 3.12 | 7.58 | -4.46 | 3.28 | coinbase-system-update, thank-you-for-smoking, fitc-tokyo-2015 |
| rej-meridian | FAIL | 0/3 | 3 | 8.25 | -5.25 | – (stills) | game-of-thrones, fitc-tokyo-2015, the-crown |
| div-d-burrow | FAIL | 0/3 | 2.88 | 7.16 | -4.28 | 3.22 | osko-launch, tintin-titles, new-yorker-festival |
| rej-northern-silver | FAIL | 0/3 | 2.75 | 7.88 | -5.13 | – (stills) | game-of-thrones, jpmorgan-payments, bbc-two-ident |
| rej-tidewater | FAIL | 0/3 | 2.75 | 6.79 | -4.04 | – (stills) | osko-launch, wealthsimple-trade, jpmorgan-payments |
| rej-lighthouse | FAIL | 0/3 | 2.62 | 8.54 | -5.92 | – (stills) | game-of-thrones, the-crown, westworld |
| rej-films-drafts | FAIL | 0/3 | 2.5 | 7.87 | -5.37 | – (stills) | fitc-tokyo-2015, coinbase-system-update, catch-me-if-you-can |
| rej-spec-to-screen | FAIL | 0/3 | 2.25 | 7.79 | -5.54 | – (stills) | coinbase-system-update, thank-you-for-smoking, fitc-tokyo-2015 |
| rej-components | FAIL | 0/3 | 2.12 | 7.96 | -5.84 | – (stills) | game-of-thrones, fitc-tokyo-2015, new-yorker-festival |
| rej-launch-template | FAIL | 0/3 | 1.75 | 7.79 | -6.04 | – (stills) | coinbase-system-update, jpmorgan-payments, game-of-thrones |

## Controls (a professional ref judged in the candidate's seat)

| Ref | Verdict | Judged pro | Blind score | Refs mean | Δ |
|---|---|---|---|---|---|
| australian-open | PASS | 3/3 | 7.12 | 7.83 | -0.71 |
| bbc-two-ident | PASS | 3/3 | 7.62 | 7.5 | 0.12 |
| calm-sleep | PRO | 3/3 | 5.38 | 7.75 | -2.37 |
| coinbase-system-update | PRO | 2/3 | 5.38 | 8.25 | -2.87 |
| kurzgesagt-climate | PASS | 3/3 | 6.75 | 7.29 | -0.54 |
| stranger-things | PASS | 3/3 | 8.12 | 8.0 | 0.12 |

### Extra controls on REFS's benchmark set (merged at 22:10)

Run after merging REFS's benchmark set and adding automatic letterbox trimming (YouTube grabs have baked-in bars).

| Ref | Verdict | Judged pro | Blind score | Refs mean | Δ |
|---|---|---|---|---|---|
| channel4-glazer-idents-2015 | PRO | 3/3 | 7.0 | 8.04 | −1.04 |
| dissembargo-f1-2026-opening-titles | PASS | 3/3 | 7.5 | 6.79 | +0.71 |
| weirdcore-aphex-twin-t69-collapse-2018 | PASS | 3/3 | 5.88 | 6.71 | −0.83 |
| artcamp-moosajee-thom-yorke-last-i-heard-2019 (480p grabs) | FAIL | 1/3 | 4.38 | 6.58 | −2.2 |
| artcamp-moosajee-thom-yorke-last-i-heard-2019 (1280px frames) | **PASS** | 3/3 | 8.0 | 6.58 | +1.42 |

With four grainy 480p YouTube grabs, the Thom Yorke video was a false reject: 1 of 3 trials, scoring 4.38. The jury took the treated monochrome for generated imagery, even though its own notes said "every frame works as a poster". REFS then replaced YouTube thumbnails with YouTube's own 1280×720 frames and dropped designed thumbnails. Re-run on the new frames, it **PASSES: 3/3 trials, 8.0 against refs at 6.58**.

**Final control tally: 10 of 10 PASS or PRO, 0 false rejects. Rejected work: 20 of 20 FAIL, 0 of 60 trials judged professional.** Lesson: references must be real frames at a decent resolution. Low-resolution stills make austere or lo-fi work look generated, so `sync-bench` now uses `--no-thumbs` and 1280px frames.

## Do the gaps match the user's words?

The user said "boring", "AI slop typography", "generic", "not impressive". The table counts how often the jury's tells and gaps mention each theme, per piece, summed over all trials.

| Theme | Rejected | Controls |
|---|---|---|
| generic / template / AI | 14.7 | 4.3 |
| typography | 13.8 | 3.2 |
| boring: static / slide / repeated / empty | 11.1 | 2.3 |
| literal / no idea | 4.4 | 2.2 |
| flat light / material / clip-art | 4.8 | 2.7 |

Median per-dimension score:

| | idea | composition | typography | colour | light_texture | motion | craft | originality |
|---|---|---|---|---|---|---|---|---|
| Rejected | 3.0 | 3.0 | 3.0 | 4.0 | 3.0 | 3.0 | 4.0 | 2.0 |
| Controls | 6.5 | 8.0 | 5.0 | 8.0 | 6.5 | 7.0 | 8.0 | 6.5 |

## The jury's three biggest gaps, per piece (first trial, verbatim)

- **blind-swell**: Find one visual metaphor (for example the water parcel's orbit as the through-line) and build every frame from it, instead of switching between stock footage and line effects · Replace the default serif-plus-mono label pairing with a typographic system that has scale contrast and actually interacts with the waves · Commit to one material world with real light (simulated water, physical texture) rather than stock ocean plates mixed with flat vector lines
- **div-a-halde**: Find one ownable idea (e.g. the contour-line topography of the origin becoming the cup, the bean and the type) instead of a feature checklist · Commit to a typographic point of view: custom or characterful faces, extreme scale contrast, type that interacts with the illustration · Break the repeated slide grid; choreograph elements so they transform between frames and each frame works as a poster
- **div-b-railyard**: Commit to the railyard world with real material, scale and light (switching levers, signal lamps, wet steel) instead of flat line diagrams on a dark ground · Give the type an idea: integrate it with the tracks, vary its scale dramatically, and choose a face with character in place of the default grotesk headline-plus-caps-label pairing · Choreograph a journey with a different composition in each frame rather than repeating one diagram with a webhook card added each time
- **div-c-substation**: Find an idea specific to the decommissioned power station, such as real archive footage, real texture or actual electrical behaviour driving the type, instead of stock lightning and hazard tape · Break the repeated template: vary scale, cropping and layout from frame to frame and make the type physically interact with light and material · Replace the default condensed sans and mono-label combination with a typographic point of view, and introduce real texture and depth (grain, print, photographic material)
- **div-d-burrow**: Give the illustration an authored voice (texture, a distinctive palette, character design) instead of stock children's-app vector · Break the repeating caption-bar layout. Let the words become part of the image, as D's credits are part of the set · Find an idea beyond a feature demo: for example, a story that really comes alive through reading aloud, with choreography between scenes
- **div-e-arctic**: No idea beyond 'chart the data'. Find a physical or visual metaphor for the shrinking minimum (the ice itself breaking, retreating, being measured) so the data has a body · Replace the default serif/grotesk slide layout with a typographic system that has scale contrast and lives inside the image rather than beside it · Give it real light, material and choreography (camera, transformation between states) instead of the same chart re-rendered with a different year
- **div-f-calibre9**: Find a real idea: the hand-engraving process, time, or the wearer. Dramatise it the way D turns the crown's making into the show's theme, instead of listing specs · Replace the label-plus-serif slide template with typography that has scale contrast and sits inside the image, and break the repeated composition · Render real material: macro light raking across engraved metal, depth of field, dust, imperfection. The current dial is a shader pattern under flat light
- **div-g-offset**: Let the tile system do the talking: remove the numbered caption pills and the slide footer and build the information into the tiles and grout, the way D threads credits through its architecture · Add real material and light, such as glazed ceramic, crazing, uneven hand-painted cobalt, raking Lisbon sun, so the azulejo reference feels physical rather than a vector repeat · Choreograph a turn: the pattern-to-wordmark build is predictable, and two of six frames are a static centred lockup. The tiles should break, offset or reveal something surprising
- **fernway**: Find a specific idea (the physical experience of losing signal on a trail, real cartography, real place) instead of captions describing features · Commit to real material and light (photographed terrain, paper maps, lit sets) instead of a procedural grey mesh and a glow line · Break the slide structure: vary scale, crop and layout between frames, and let the type become the image as D's packaging does
- **launch-tern**: It needs a real visual idea beyond 'spectrogram + phone + map'. For example, make birdsong itself the typographic or graphic material, the way A makes cosmic and biological forms rhyme · The type should be designed, not set: scale contrast, a chosen face, type interacting with the sound visualisation instead of slide-placed text blocks · Light, texture and atmosphere (dawn, foliage, field grain) are needed so frames are posters rather than UI screens, with real choreography between shots instead of a repeated layout
- **margin**: Find a surprising idea, not the product name made literal. D turns the subject into the whole visual world · Break the template: vary scale, framing and structure from frame to frame, and let the type take over the frame instead of sitting like slide copy · Add material and a palette with character (real paper, ink, handwriting, a pen hitting the page) so it is not a flat cream background with one accent
- **rej-components**: Needs an actual idea: a metaphor or world for 'the machine' instead of mock UI widgets shown one at a time · Typography needs a point of view, with scale contrast, a distinctive face and integration with the image, instead of a default serif headline over grey caption text · Frames need density, material and real light (texture, depth, layered elements) and transitions that carry objects from one shot to the next, as B and D do
- **rej-films-drafts**: There is no unifying concept. Pick one subject and build a world around it, as A does with the chase · Replace placeholder rectangles, blobs and UI cards with real image-making: illustration, texture, material and light · Typography needs a point of view: custom or characterful faces, expressive scale and integration with image, instead of label-over-headline slide layouts
- **rej-launch-template**: There is no idea. It needs a concept and imagery instead of a feature list read off a slide · Type set in defaults with no scale contrast or relationship to the image; it needs typography with a point of view · There is no motion design, light or material. It needs actual choreography between distinct scenes and textured, lit frames
- **rej-lighthouse**: No idea beyond the literal subject: it needs a specific conceit (the keeper's ritual, the light as a clock, the beam revealing story fragments) that only this film could have · Typography needs a real typographic idea, such as type lit by or revealed by the sweeping beam, expressive scale, or a chosen face with character instead of a default serif and caps label · Composition and motion need variety: different shot scales, macro detail of lens, mechanism and texture, and choreography between frames instead of one locked-off layout
- **rej-meridian**: It has no idea: nothing ties the corridor to whatever 'Meridian' is. Like B, it needs a concept specific to the subject that would drive both the world and the camera · The type needs to become a designed element (scale, placement, interaction with the architecture) rather than a corner label · It needs cuts, changes of scale and events between frames, plus real material detail in the architecture, so the piece moves past a single static render
- **rej-northern-silver**: Find an actual idea. What is Northern Silver, and why should these objects behave this way? Give the forms a subject and a story arc · Build real material and light: an environment the spheres actually sit in, interaction with the water, varied scale and composition from frame to frame · Make the type part of the image, or make it a confident hero moment, instead of a tiny default label
- **rej-spec-to-screen**: There is no idea beyond 'product name + feature list'. Find a conceit that dramatises the product, as D turns credits into the subject's own packaging · Replace the centred default-sans slides with a typographic system that has a point of view: scale contrast, layout, and integration with the image · Replace placeholder UI and stock chrome/aurora effects with designed, textured imagery and choreography between shots
- **rej-tidewater**: No idea beyond 'show the app'; it needs a visual concept rooted in sailing and tide (water, weather, time) that only this product could own · No choreography: six beats of the same frame; it needs changes in scale, camera, environment and type behaviour like A's object-led sequence · Typography and palette are defaults; it needs a chosen face, expressive scale and integration of type with image and material
- **sift**: Find a non-literal metaphor for email overload that a viewer would remember · Choose or draw type with a point of view instead of bevelled default sans · Build a palette and lighting design specific to the brand, beyond black, red and chrome

## Limits

- References are editorially chosen stills, while ours are evenly spaced frames. That leans against us on purpose, and the controls show that real work still passes under it.
- Some references are famous, so the jury may recognise them. That only makes the bar harder.
- The critic stage for these runs predates the yes/no TASTE checks, which were added at 22:00.
- Cost: about $0.47–0.66 per piece (3 trials, plus the critic for MP4s); about 2–3 min wall time per piece.

## Motion stages (validated 2026-11-18 00:45–01:05, both kept OFF)

The parent asked for a motion-aware stage, because the jury sees only about 6 stills and motion barely registers. Two stages were built (`scripts/judge.py`) and validated the same way as the stills judge. **Neither discriminates well enough to gate on. Both are off by default** (`JUDGE_MOTION=1` and `JUDGE_STRIPS=1` enable them), and the stills verdict remains the gate.

**Reference clips.** The environment refused to download the references' YouTube or Vimeo videos, so only lawful direct files were usable (REFS's `clips.json`): three Blender Studio open movies (CC BY 4.0) and one NASA SVS visualisation (public domain). Each was cut to a 20 s excerpt into `.cache/judge-clips/`. None of the motion-design or kinetic-type references is lawfully downloadable.

**1. Blind motion compare.** Two strips per film, each 10 consecutive frames 1/10 s apart around the busiest moments, compared blind with 3 of the clips. 3 trials.

| Candidate | Motion | Judged pro | Refs mean |
|---|---|---|---|
| div-c (rejected) | 4 | 0/3 | 6.33 |
| div-f (rejected) | 6 | **2/3** | 6.33 |
| div-g (rejected) | 6 | **3/3** | 6.67 |
| r1 a-social | 4 | 0/3 | 6.67 |
| r1 g-titles | 4 | 0/3 | 6.33 |
| r1 k-sports | 5 | 0/3 | 6.33 |
| control: Charge | 8 | 3/3 | 5.67 |
| control: Spring | 7 | 3/3 | 6.0 |
| control: Sprite Fright | 8 | 3/3 | 6.0 |
| control: NASA CO₂ | 2 | **0/3** | 8.0 |

Two false accepts (div-g, div-f) and one false reject (NASA, a slow continuous field). The available clips are narrative 3D films, a poor reference for motion-design choreography, and NASA's near-static field drags the reference mean down.

**2. Critic with strips.** The candidate alone: three 10-frame strips plus frame pairs.

| Film | Motion | Craft |
|---|---|---|
| div-g (rejected) | 6 | 7 |
| div-f (rejected) | 6 | 6 |
| r1 k-sports | 5 | 6 |
| Charge | 7 | 8 |
| Sprite Fright | 8 | 8 |
| NASA CO₂ | 4 | 5 |

The separation is 1–2 points. Strips also nudged div-f's motion up from 5 (without strips) to 6. That is too weak to gate on, and it would inflate the "full" score compared with r1.

**What would fix it:** lawful clips of real motion-design and kinetic-type work. That means owner permission or press files, or the user approving clip downloads for evaluation only. Until then, motion is judged inside the stills stage (median 3 for rejected and r1 work, 7 for controls) and in the critic's frame pairs and pace check.
