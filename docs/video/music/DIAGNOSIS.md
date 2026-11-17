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
