# Why the generated music doesn't sound advanced: diagnosis

The user's verdict was "The music is not nearly advanced enough." This document measures where
our scores fall short of real production music in the same genres, and ranks the gaps by how
much each one is likely to be heard.

**Method.** The numbers come from `cargo run --release -p anikit-music --example diagnose`
(statistics per style for a 60 s render, seed 1), from `-- depth` (how many samples sit behind
each bundled instrument), and from the listening-proxy metrics in [README.md](README.md). The
reference points are well-documented conventions of commercial library, film and pop music;
they are not a measured reference corpus.

**Caveat.** Only the stereo-correlation and loudness figures come from audio. Everything else
is a note-level or sample-bank proxy. Nobody has listened to these renders, so the rankings are
judgements built on those measurements.

## Ranked gaps (most audible first)

### 1. Harmony is one 4-chord loop for the whole piece, mostly plain triads

- **Measured:**
  - Every section loops the same progression: intro, verse, chorus and outro all play, say, `I V vi IV`.
  - 10 of 15 styles use only triads: 0 of 16 chords have a 7th, 9th or sus. Pop, rock, corporate, acoustic, orchestral, cinematic, synthwave, trap, tech and dnb are all like this.
  - The chord changes exactly once per bar, every bar.
  - There are no cadences at section ends, no borrowed or secondary chords outside a couple of fixed `bVII`/`bII` loops, no modulation, no inversions, and no pedal points.
- **What real tracks do:**
  - Verse and chorus have different progressions.
  - Harmonic rhythm speeds up at phrase ends (two chords in the last bar).
  - A pre-chorus lifts with a IV–V or a borrowed iv; the chorus lands on I or vi.
  - The voicings carry colour (add9, maj7, sus2, 7sus4) even in pop.
  - Basslines pass through inversions (I – V/7 – vi).
  - Film cues use pedal points and lifts, such as a modulation up a step or a ♭VI–♭VII–I.
- **Effect:** after 8 bars the listener has heard everything, and that is the main reason it sounds "loop-based" rather than composed.

### 2. Melody has no motif, no phrasing and no hook

- **Measured:**
  - In every style, almost no lead melody bar repeats the bar 4 bars earlier (0 to 3 bars). Nothing comes back, so there is no hook.
  - House and pop leads are 35–46 % leaps of a 4th or more, which reads as random rather than singable. Real top lines are about 60–75 % steps.
  - The contour is drawn at random for each motif. There is no call and response, no question/answer phrase pair, no cadence on bars 4 and 8, and no climax note.
  - cinematic-hybrid has no theme at all, only an ostinato.
  - There is no countermelody in any style.
- **What real tracks do:** a 1–2 bar motif is stated, then developed: repeated, sequenced onto the next chord, rhythmically varied, and answered. The chorus hook returns note-for-note. A sustained countermelody (guide tones) runs under the lead in the big sections.

### 3. The arrangement only switches layers on and off; nothing evolves inside a section

- **Measured:**
  - Sections differ only in which layers play and in velocity.
  - Drums repeat one bar-long pattern for the whole section, broken only by a snare roll in the last bar before a bigger section.
  - There are no breakdowns, stops or drops to silence before a peak, no pickup bars, and no instrumentation changes beyond the energy gates.
  - No automation is used: no filter sweeps in builds, no swells into downbeats.
  - Four-bar exact-repeat rates are high for the accompaniment: pads 11–16 of every 17–27 bars, arpeggios 14 of 14, tech bass 24 of 26, dnb drums 29 of 37.
- **What real tracks do:**
  - Drums vary every 2, 4 and 8 bars: ghost notes, open-hat lifts, small fills on bar 4 and big fills on bar 8.
  - Builds sweep a low-pass filter and thin the low end, and there is a beat of silence before the drop.
  - A breakdown strips to pad and melody before the final chorus.
  - Countermelodies enter on the second chorus.

### 4. Performances sound mechanical: flat dynamics, one note length, no articulation

- **Measured:**
  - Velocity standard deviation is 0.02–0.13 per part, and drums in house, pop, tech and corporate are 0.02–0.03, effectively flat. Real played or programmed parts sit around 0.10–0.20, with accents, phrase arches and crescendos into sections.
  - Note lengths: most accompaniment parts use exactly one length ("1 length"), so every pad, bass and arpeggio note has the same gate.
  - Articulation: there is no legato or staccato choice per phrase, no sustain-pedal model for piano, and strings never switch between sustain, spiccato and pizzicato.
  - Humanisation is independent Gaussian jitter (±6 ms at the default amount). There is no groove template: hats are not pushed, hip-hop snares are not laid back, and there is no swing per instrument.
  - Strums are uniform.

### 5. Sample depth: the bundled General MIDI bank is thin

Measured with `diagnose -- depth`:

| Instrument | Velocity layers | Distinct samples |
|---|---|---|
| Grand piano | 8 | 17 for 88 keys (about one per 5 semitones, so wide stretches are pitch-shifted) |
| Fast strings | 2 | – |
| Steel guitar | 1 | – |
| Finger bass | 1 | – |
| Trumpet | 1 | – |
| Standard kit | 4 | – |

- SF2 has no round robin, so repeated notes ("machine-gun" hats and snares) are identical.
- The Vorbis q6.5 compression costs a little top end on top of this.
- The good libraries, Salamander Grand (16 layers, a sample every 3 semitones) and VSCO-2 CE strings and brass (round robin, articulations), are supported but optional, and no style uses them by default. Only the `piano` style asks for `salamander-piano`, falling back to GM.

### 6. The mix is static and nearly mono

- **Measured:** the L/R correlation of the masters is 0.80–0.98, which is close to mono. Commercial mixes typically sit around 0.2–0.7.
- **Why:**
  - Parts are mostly panned to the centre: only a few layers are panned, and the guitars aren't double-tracked.
  - There is one shared reverb.
  - There are no group buses: no drum-bus compression, no bus EQ for "glue".
  - Levels are fixed per section and nothing is automated.
- Loudness is already right: −14 LUFS, −1 dBTP, no clipping.

### 7. Scoring to picture is minimal

- Only `hits` exist. Each one gives an impact, a section change at the first hit, and a nudge to the tempo grid.
- The video's own cut times are not used.
- There are no stops, swells, reverse swells into cuts, or tempo or feel changes at scene changes.
- Section lengths follow the style's form, not the edit.

## Plan, in order of audible impact

1. **Composition engine** (gaps 1–2):
   - progressions for each section role (verse, pre-chorus, chorus, bridge), with cadences, harmonic-rhythm acceleration, borrowed and secondary chords, and colour tones by style;
   - a bass with inversions and passing tones;
   - motif-based melodies: a motif, then repetition, sequencing, variation and an answer, with the hook returning in every chorus, step-wise contour and a climax;
   - a countermelody in the peak sections.
2. **Section-aware arrangement** (gap 3):
   - drum variation every 2/4/8 bars and real fills;
   - in builds, a filter sweep, a thinned low end and a stop before the drop;
   - a breakdown before the final peak when there is room;
   - countermelodies and extra layers on repeats.
3. **Performance** (gap 4): velocity arcs and accents, groove templates per style and role, articulation (staccato, legato, sustain pedal), and varied note lengths.
4. **Stereo and mix** (gap 6): role-based panning and double-tracking, wider pads, and drum-bus glue.
5. **Better samples by default** (gap 5): use `salamander-piano` and VSCO when they are cached, and make one command fetch them. Their licences (CC BY 3.0 for Salamander, CC0 for VSCO) allow it, but they are 1–3 GB, so they won't be bundled.
6. **Score to picture** (gap 7): the video's cut times drive section boundaries, crashes and impacts, with a stop before big reveals.

Before/after renders of four genres will be added next to this file as the fixes land.

## Progress: round 1 (composition engine, arrangement, performance)

The new code is in `src/compose.rs`, wired in by `style.rs`. It fixes gaps 1–4, part of 6, and nothing yet for 5 and 7.

- **Harmony:**
  - Verse, build and peak sections get different progressions.
  - Builds and pre-peak bars end on the dominant (a half cadence), and the piece ends with IV–V before the final tonic.
  - Every 8 bars a dominant lands on the last two beats before the loop restarts on the tonic.
  - Colour tones are added by style: add9, sus2, m7, maj7 and V7; lofi and jazz get 9ths.
  - The bass moves by step using inversions.
  - Repeats bring borrowed and secondary chords: iv, V/V, and a major V in minor keys.
- **Melody:**
  - Each line is an 8-bar sentence built from one motif: statement, sequence on the next chord, variation, half cadence, statement, sequence, climax a third higher, then a full cadence.
  - Strong beats sit on chord tones, phrasing is legato, and dynamics arch towards the climax.
  - The same section role always gets the same motif seed, so the chorus hook returns note-for-note.
- **Countermelody:** a guide-tone line (the 3rd or 7th of each chord, voice-led) plays under the lead in peak sections.
- **Arrangement:**
  - Drums get ghost snares, an open-hat lift and snare pickup every 4 bars, a tom fill every 8, and a crash every 8 bars at full energy.
  - Electronic styles sweep a low-pass filter open through each build.
  - Everything stops for the half beat before each drop.
- **Performance:**
  - Hats are accented by position in the beat (velocity SD went from 0.02–0.10 to 0.14–0.23).
  - Lofi and jazz have a laid-back backbeat.
  - Piano-family chords sustain to each chord change (a sustain-pedal model).
- **Stereo:** each role gets a default pan and pads are widened. Correlation is now 0.4–0.98 (cinematic 0.66, ambient 0.40); lofi, jazz and pop were still about 0.95 (fixed later, see the progress table below).

Measured with `diagnose`:

| Measure | Before | After |
|---|---|---|
| Drum bars exactly repeating 4 bars earlier | up to 29 of 37 | mostly 0–4 (trap 13 of 29, dnb 9 of 37) |
| House/pop melody steps (vs leaps) | 34–37 % | 59–63 % |
| Distinct note lengths in accompaniment | 1–2 | 2–6 |

- Every style still masters to −14.0 LUFS and −1.02 dBTP with no clipping.
- **Speed** for single 40 s renders: cinematic 29.5×, house 31×, acoustic 41×, lofi 43× real time.

Before and after renders, 40 s each:

| Genre | Before | After |
|---|---|---|
| Cinematic | [v2-cinematic-before.ogg](v2-cinematic-before.ogg) | [v2-cinematic-after.ogg](v2-cinematic-after.ogg) |
| Acoustic | [v2-acoustic-before.ogg](v2-acoustic-before.ogg) | [v2-acoustic-after.ogg](v2-acoustic-after.ogg) |
| Lofi | [v2-lofi-before.ogg](v2-lofi-before.ogg) | [v2-lofi-after.ogg](v2-lofi-after.ogg) |
| Electronic (house) | [v2-electronic-before.ogg](v2-electronic-before.ogg) | [v2-electronic-after.ogg](v2-electronic-after.ogg) |

**Next:** score to picture (the video's cuts drive section boundaries and hits), better samples by default when the packs are cached, and stereo for the narrow styles.

## Progress: overnight push

Each change has a before/after render in this folder.

| Change | Measured | Before | After |
|---|---|---|---|
| Short forms (idents, spots under 8 bars) get set-up → lift → payoff and a 1–3 s ring-out after the button (d7a15ee) | ring-out room is 15 % of the length, clamped to 1–3 s; the button is the tonic in the lead, with an impact (≤30 s) and a half-beat breath before it (≤20 s) | [ident-house-5s-before](ident-house-5s-before.ogg), [piano 5 s](ident-piano-5s-before.ogg), [cinematic 10 s](ident-cinematic-10s-before.ogg), [corporate 15 s](ident-corporate-15s-before.ogg) | [ident-house-5s-after](ident-house-5s-after.ogg), [piano 5 s](ident-piano-5s-after.ogg), [cinematic 10 s](ident-cinematic-10s-after.ogg), [corporate 15 s](ident-corporate-15s-after.ogg) |
| Lofi tape texture: wow 0.15 % at 0.8 Hz, flutter, an 8.5 kHz roll-off, crackle and hiss (e7036a8) | — | [lofi-before](lofi-before.ogg) | [lofi-texture-after](lofi-texture-after.ogg) |
| Recorded samples by default when the essentials packs are cached: Splendid grand, Wurlitzer (1a829eb) | — | [samples-piano-gm](samples-piano-gm.ogg), [samples-lofi-gm](samples-lofi-gm.ogg) | [samples-piano-packs](samples-piano-packs.ogg), [samples-lofi-packs](samples-lofi-packs.ogg) |
| Mono synths glide only on legato notes; trap 808 slides into octave jumps; hat rolls only at phrase ends; kit round-robin (8f5e584) | every synthwave bass note used to glide | [groove-trap-before](groove-trap-before.ogg), [groove-synthwave-before](groove-synthwave-before.ogg) | [groove-trap-after](groove-trap-after.ogg), [groove-synthwave-after](groove-synthwave-after.ogg) |
| FreePats CC0 electric bass (finger/picked), nylon and clean electric guitars, part of `fetch essentials` | in tune within 4 cents at A4; bass parts (keys 33–50) stretch the top sample at most 5 semitones | [samples-pop-gm](samples-pop-gm.ogg), [samples-acoustic-gm](samples-acoustic-gm.ogg), [samples-rock-gm](samples-rock-gm.ogg) | [samples-pop-packs](samples-pop-packs.ogg), [samples-acoustic-packs](samples-acoustic-packs.ogg), [samples-rock-packs](samples-rock-packs.ogg) |
| Trailer grammar for cinematic-hybrid cues of 45 s or more: act I, a build whose low-tom pulse goes quarters → 8ths → 16ths, a first climax that stops dead on a hit, a false ending (lone piano and cello), a rise, then act III with a reverse cymbal into the drop | 60 s cue: from −13 dB at the climax down to −52 dB within 1 s of the hit; the false ending sits at about −25 dB, then act III | [trailer-60s-before](trailer-60s-before.ogg) | [trailer-60s-after](trailer-60s-after.ogg) |
| Stereo for played styles: acoustic kits play in a short stereo room (ambience, 25 % wet), and electric pianos get their stereo tremolo (new `autopan` effect, 1/4 note, depth 0.55). Panning a mono sample had left each part mono | side/mid above 300 Hz: lofi −12.1 → −9.7 dB (drums alone −20 → −12 dB), rock −9.2 → −8.7 dB; every played style now sits between −4.6 and −9.7 dB; below 120 Hz the mix stays mono (−25.6 dB) | [stereo-lofi-before](stereo-lofi-before.ogg), [stereo-rock-before](stereo-rock-before.ogg) | [stereo-lofi-after](stereo-lofi-after.ogg), [stereo-rock-after](stereo-rock-after.ogg) |
| Synthwave's 80s gated snare (its "about" promised one, but it had none): the snare gets its own track through a plate that is cut 0.22 s after each hit (new `gated-reverb` effect) | above 1 kHz on the snare beats, the tail is +6 to +10 dB louder at 60–180 ms and back to the plain level by 300 ms; the backbeat balance against the kick stays within 0.3 dB | [gated-synthwave-before](gated-synthwave-before.ogg) | [gated-synthwave-after](gated-synthwave-after.ogg) |
| Alternate mixes for editors (`anikit-video music … --alts DIR`): underscore, no-drums, and 30 s / 15 s cutdowns recomposed with their own endings | a 40 s pop bed plus its 4 alts render in 3.7 s | — (new) | [alts-pop-underscore](alts-pop-underscore.ogg), [alts-pop-15s](alts-pop-15s.ogg) |
| A real acoustic drum kit: Virtuosity Drums (CC0) via `fetch virtuosity`. It replaces the GM kit for standard/room/rock/power/pop/lofi when cached. Also new: `amp_velcurve_N` support (the kit's soft layers were 20 dB too quiet) and a 7 ms trim of the room mic's travel time | kick dynamic range from vel 0.3 to 0.9 is 20 dB (was 37 dB without velcurves); onsets 1.3–4.6 ms from the grid (GM kit 0.5–2.8 ms; untrimmed 8–12 ms); side/mid above 300 Hz rock −8.7 → −7.3 dB, lofi −9.7 → −7.5 dB, pop −4.6 → −3.7 dB | [kit-rock-before](kit-rock-before.ogg), [kit-lofi-before](kit-lofi-before.ogg) | [kit-rock-after](kit-rock-after.ogg), [kit-lofi-after](kit-lofi-after.ogg) |
| Scoring to picture: a cut that opens a bigger section gets a reverse cymbal swelling into it (riser styles already have risers into drops) | acoustic, cuts at 8 s and 20 s: above 4 kHz the 1.5 s before the 20 s cut rises from −31 to −20 dB (before: flat at about −35 dB) | [cut-swell-before](cut-swell-before.ogg) | [cut-swell-after](cut-swell-after.ogg) |
| Strums: each stroke's sweep speed varies ±20 %, strokes on the beat hit a little harder than between beats, and each stroke's level wanders ±8 % | acoustic strum: velocity SD 0.14 → 0.16; 4-bar exact repeats 5/19 → 0/19 | [strum-acoustic-before](strum-acoustic-before.ogg) | [strum-acoustic-after](strum-acoustic-after.ogg) |
| Open-ended stops: any explicit section takes `"stop": beats`, silence right before it in any style (the trailer hole, a false ending, a breath before a chorus) | piano, `stop: 3` before the chorus: −13 dB → −44 / −60 / −77 dB over the 3 beats, then the chorus lands at −13.5 dB | — (new) | [section-stop](section-stop.ogg) |
| Splendid grand (spaced-pair samples: L/R correlation −0.22…0.43 per note, up to 4 dB lost summed to mono) is narrowed to 50 % side | piano style: correlation −0.07 → 0.46; mono-sum loss −3.3 → −1.4 dB | [piano-width-before](piano-width-before.ogg) | [piano-width-after](piano-width-after.ogg) |
| Jazz ride: the pattern hit 16th-note "e" and "a" positions, which swing never uses; it now plays the standard spang-a-lang figure (1, 2, 2&, 3, 4, 4&, swung) with the hi-hat foot on 2 and 4 | ride hits per beat position: before {0, 1, 2, 3} sixteenths, now {0, 2} only (test) | [jazz-ride-before](jazz-ride-before.ogg) | [jazz-ride-after](jazz-ride-after.ogg) |

The underscore mix (a6b081c) needs no render: under voice-over it mutes the lead, melody and countermelody tracks, and `underscore_tests` checks that.
