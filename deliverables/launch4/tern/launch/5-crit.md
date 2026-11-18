# 5 · Critiques

## Crit 1 (pass 1): blind judge (`crit1/`, practice, 2 trials, tags product-launch + kinetic-type)
- **Overall 4.06** (references' mean 8.1, delta −4.04). Ranked last in both trials, judged pro 0/2.
- Scores: idea 5.5 · composition 4.0 · typography 4.0 · colour 4.5 · light/texture 2.5 · motion 3.5 · craft 4.0 · originality 4.5.
- Gaps, verbatim:
  1. "Make the document physical: real carbonless paper, pen pressure, ink bleed, a hand and real light, instead of a flat pink rectangle with a vignette" (**WORLD**)
  2. "Use true hand-lettering with variation, and give the type a point of view with expressive scale and moments where the writing breaks the grid" (**CRAFT**)
  3. "Vary the image across the piece by cutting to what the notes describe (the tin roof, the blackbird at 5:12am, the thunder) so the frames stop repeating one layout" (**WORLD**)
  4. "Make the object real: shoot or build actual paper, actual ballpoint with pressure variation and ink skips, and real light and shadow, so the conceit feels lived-in" (**WORLD**)
  5. "Add visual events beyond pan and zoom over one sheet (hands, the recorder, cut-ins of the sounds being logged) so frames change as much as A's and B's do" (**WORLD**)
  6. "Develop a typographic and colour idea beyond default form type and a pink field: B's palette and type do the storytelling, while here only the copy does" (**CRAFT**)
- Critic stage: motion 5 · sound 6 · craft 6. Defects: a script font, not a hand; stamped identical ticks; flat vector ink; a washed-out flash frame; a yellow sliver at the bottom of a wide frame; shots too long (up to 8.8 s); circles clipping text; sound not specific to the subject.

## Fake-look critic 1 (fresh subagent, 6 stills)
1. **Simulation / template / AI?** "It doesn't read as AI imagery or a game render. It does read as a typeset mock-up of something handmade, and the handwriting is what gives it away … The ticks are one copy-pasted glyph. The paper is flat pink with a smooth digital vignette … In frames 3 and 6 the strikethrough stops before the final 'e' … Frame 5 crops the subject column."
2. **What does it do?** "A recorder whose takes don't fail … 'clipped' to 32-bit float, 'batt died' to battery life, 'wasn't rolling' to instant record, and ticks in four track columns to the four mics … The moment: a column of ruined takes, then 'blackbird, 5:12am' with all four tracks ticked and 'got the top' circled." This **matches the promise and the moment**.
3. **Cruellest one-liner:** "A Google Sheets template in a handwriting font, launching a recorder nobody is allowed to see."
4. **Shippable?** "Buck or Pentagram would buy the concept … they'd reject the execution."

## Sorting and decision after crit 1
- **IDEA gaps: none.** No judge gap says literal, pun or generic; idea is the top score (5.5). The fake-look critic accepted the concept ("smart insider idea") and attacked the material. The idea stays.
- **WORLD (rebuild at fidelity):** the paper, the ink, the light, one layout per frame.
  - Pass 2 adds a page-space paper shader (fibres, formation, tooth) multiplied over paper, print and ink, so the ink sits in the stock. Pages are drawn at 2× so macro shots stay sharp.
  - The ballpoint gets per-character baseline, rotation and scale irregularity, `inkTexture` skips, natural word spacing, and pen acceleration (`inOutQuad`).
  - Every tick is now a different seeded stroke. Circles and strikes are brush ribbons with pressure and taper.
  - Scale contrast and cuts: a 4.2× held macro on the empty field during the stop, and a 3.4× cut-in on the first circle.
- **WORLD gap not taken:** "show the recorder / hands / the places". The brief gives no product imagery, so drawing a Tern body would be an invented product (honesty floor). "Simulated places" (a tin roof, a blackbird) are banned by the renderable test. The sounds are carried by the sound design instead: rain, door thump, thunder with a lightning flash on the page, a night bed, and the flute's blackbird phrase.
- **CRAFT:**
  - the strike now runs fully through;
  - camera framing is clamped to the page (no desk slivers, no cropped subject column in the magic shot);
  - the flash is softer and cool;
  - a plain-language turn note ("rolls the moment you lift it.") answers critic 1's "only a recordist will decode it".
- **Cut the weakest third:** the 3.75 s recorder-field drift became three short shots; the 5 s ending became two; the slow push-ins on rows were shortened.

## Crit 2 (pass 2): `crit2/`
- **Overall 4.5** (references 8.19, delta −3.69), last in both trials. Scores: idea 6.0 · composition 4.5 · typography 4.0 · colour 5.0 · light/texture 3.5 · motion 3.5 · craft 4.5 · originality 5.0.
- Gaps, verbatim:
  - "Make it physical: shoot or simulate a real carbonless sheet with real pen strokes, pressure, smudges and lighting …" (**WORLD**)
  - "Break the single-layout structure: intercut the sheet with the sounds being logged …" (**WORLD**)
  - "Give the typography an idea: let the handwriting accelerate, degrade or change hand as the night goes on, and choreograph the circles and checks to audio …" (**CRAFT**)
  - "Make it physical: a real carbon form shot under real light, with a real pen, hand, ink bleed …" (**WORLD**)
  - "Choreograph the piece instead of panning one static layout …" (**CRAFT**)
  - "Bring the subject into the image. The recorder, the places and the sounds never appear …" (**WORLD**, see the honesty note above)
- Critic stage: motion 5 · sound 6 · craft 7. Defects:
  - a ~2 s flat noise block at 5.6–7.5 s;
  - the music too evenly dense;
  - the thunder circle cutting the TR4 tick;
  - mid-pan crops.

## Fake-look critic 2 (fresh subagent, 6 stills of pass 2)
1. "**No frame looks like a simulation, a game render or AI imagery.** The concept is sound-report paperwork, which has a real point of view and isn't a template idea. But the craft gives it away as digital in three places": the handwriting font, the absence of carbon ghosting, and layout slips (a tick under a circle, descenders on a rule, the ROLL '1' clipped).
2. "It's a recorder that doesn't clip, doesn't die, starts rolling on its own and fills all four tracks … The moment someone would describe to a friend: 'the old one' crossed out and 'Tern' circled, with the circled 'got the top' answering 'missed the top.'" This matches the promise.
3. "Thirty seconds of a pink spreadsheet for a microphone you never see or hear." Unflattering, not damning: it doesn't call the film fake or dishonest. The film does have sound; the critic saw only stills.

## Pass 3 (craft only, the last pass allowed): `crit3/`
- **Fixes:**
  - removed the tape-stop cue and lowered the room bed (to remove the noise block);
  - lowered the music energy so the hits stand out;
  - tightened the thunder circle so it no longer cuts the TR4 tick.
- **Overall 4.25** (references 8.23). Scores: idea 6.0 · composition 4.0 · typography 4.0 · colour 5.0 · light/texture 3.0 · motion 3.0 · craft 4.0 · originality 5.0. Its gaps repeat crit 2's (physical artefact, choreography beyond one sheet, a bigger typographic payoff for "Tern"). The critic still hears a flat noise patch at 5.9–7.5 s (the music's stop bar with the room bed under it) and a uniform percussion grid.

## Ship decision
Pass 2 scored 4.5 and pass 3 scored 4.25: under 0.75 apart, so a **tie**. The IDEA/WORLD gap lists are the same. On craft, pass 3 fixes one verified collision and slightly calms the mix. **Shipped: pass 3** (`final.mp4` = `pass3.mp4`; `spec.json` builds it).

**Honest read:** the blind jury still ranks it last against title-sequence references (Vertigo, Boxtrolls, Da Vinci's Demons). The remaining distance is the one the critics name each time: a *physical* artefact with a hand and real carbon, and more choreography than cuts across one sheet. That's a WORLD rebuild a fourth pass isn't allowed. Next time, try shooting a real report as plates, or a `scene3d` sheet with the form as a live composition texture (tested in `tests/`, dropped for cost), with a real ballpoint under raking light.
