# Music craft for short-form video: what professionals do, with numbers

This is the research behind the next round of `crates/anikit-music` work. The bar is that
a 5–60 s soundtrack (ad, launch, title or ident, trailer, social cut) should sound as if a
working library composer or producer made it. It complements
[../music/DIAGNOSIS.md](../music/DIAGNOSIS.md), which measures where we are, and
[TASTE.md §7](../taste/TASTE.md#7-sound-and-music), which holds the aesthetic rules.

**How to read it.**
- Each factual claim links to its source. Most of these are practitioner sources (library
  composers, trade magazines, standards bodies). Production numbers are conventions, not laws.
- Lines marked *(heuristic)* are our own working values where no source gave a number.
  Treat them as defaults to tune by ear.
- §9 is the gap list against the engine as it stands (November 2026), ranked by how much
  each gap is likely to be heard.

---

## 1. Principles

1. **Write the ending first.** Library music is judged on its ending. "You must have a good
   ending", and it should be written, not faded from a repeating chorus
   ([Sound on Sound, *Library Work*](https://www.soundonsound.com/techniques/library-work)).
   Editors call a firm stop on a downbeat that rings out a *button*, and most cues need a
   "sting/button" style ending ([Music Library Report](https://musiclibraryreport.com/composer-tips/cue-tips/);
   [Taxi forum](https://forums.taxi.com/post108895.html)). Crossfading a track out "is sloppy and
   won't sound as good as properly resolving your track"
   ([Frame.io](https://blog.frame.io/2017/11/08/make-music-fit-your-video/)).
2. **A short cut is its own piece, not the first N seconds of a long one.** A 30 s cut
   "should have its own musical arc"
   ([Tonal Chaos, stems and alt mixes](https://tonalchaos.pages.dev/blog/stems-and-alt-mixes/)).
   Commercial composers treat a spot as intro → development → coda inside the time
   ([Sound on Sound](https://www.soundonsound.com/techniques/library-work)).
3. **Leave room, then build.** Production tracks start with a "minimal intro", give several
   builds and beds, and end with a "large end build" plus several short end "bumps, hits and
   pieces" that editors can grab ([ALIBI, *Anatomy of an ALIBI track*](https://alibimusic.com/blog/anatomy-of-an-alibi-track)).
4. **Give the editor cut points.** Hits, drum breaks and sudden silences make edit points.
   Avoid key and tempo changes, which make clean edits hard
   ([Sound on Sound](https://www.soundonsound.com/techniques/library-work)). Breaks between
   sections let editors jump straight to the end
   ([Music Library Report](https://musiclibraryreport.com/?p=30710)).
5. **Deliver alternates.** Expected alternates are a 30 s version, a 60 s version, an
   underscore (no melody) and a ~6 s sting
   ([Warner Chappell PM track page](https://www.warnerchappellpm.com/track/9-lives-music-nlm241-59/MjIxMDE4MDAwLWY3ODUzMA)).
   Also expected: a "backing track" mix with no lead for use under programmes
   ([SOS](https://www.soundonsound.com/techniques/library-work)), and stems.
   Epidemic ships four stems with every track: drums, bass, instrument and melody
   ([Epidemic help](https://help.epidemicsound.com/hc/en-us/articles/25921163391762-Download-the-music-in-stems-or-audio-layers)).
   Creators mute the melody when it clashes with dialogue
   ([Epidemic blog](https://www.epidemicsound.com/blog/what-are-stems-in-music-production/)).
6. **Silence is structure.** A quiet moment before the climax "makes the finale hit harder"
   ([KillTheDJ, trailerizing](https://killthedj.com/how-to-trailerize-your-music-to-boost-your-career-make-additional-income-as-a-musician));
   see also TASTE §7.
7. **Front-load identity.** Stock-music courses tell composers to deliver the main drive
   within the first 5–10 s, because buyers skip quickly
   ([Udemy course summary](https://www.udemy.com/course/how-to-compose-uplifting-corporate-music/)).
   For a 15 s spot the hook has to arrive in the first bar or two *(heuristic)*.
8. **Mono and small speakers decide TV mixes.** "Bottom end and mono compatibility" are the
   key factors. Keep a strong bass around 80 Hz, high-passed below about 60 Hz if needed, and
   hard-pan nothing crucial ([SOS](https://www.soundonsound.com/techniques/library-work)).

---

## 2. What separates studio work from amateur work

| Area | Amateur / generator tell | Professional practice | Source |
|---|---|---|---|
| Ending | fade-out, or stops mid-phrase | button on a downbeat that rings; the tail decays fully before the picture goes to black | [SOS](https://www.soundonsound.com/techniques/library-work), [Taxi](https://forums.taxi.com/post108895.html) |
| Length fit | slowing the tempo to stretch 30 s to 45 s | write an extra section instead | [Taxi forum](https://forums.taxi.com/tips-for-button-endings-t8928.html) |
| Cut-downs | first N seconds, then a fade | each cut has its own arc; edited in multitrack, not on the stereo mix | [Tonal Chaos](https://tonalchaos.pages.dev/blog/stems-and-alt-mixes/), [SOS](https://www.soundonsound.com/techniques/library-work) |
| Edit points | continuous wall of sound | hits, breaks and silences on bar lines | [SOS](https://www.soundonsound.com/techniques/library-work) |
| Groove | every hit at the same velocity, on the grid | ghost notes ~20–40 velocity, hats accented on the beat, kick and snare tight while hats stay loose | [Loopcloud](https://www.loopcloud.com/cloud/blog/5254-7-Drum-Programming-Tips-to-Improve-Any-Groove), [Splice](https://splice.com/blog/humanize-your-drums/) |
| Swing | 50 % or a 66 % triplet, nothing between | 54 % "loosens up the feel without it sounding like swing"; 50–70 % holds many subtle settings | [Roger Linn, Attack](https://www.attackmagazine.com/features/interview/roger-linn-swing-groove-magic-mpc-timing/) |
| Repetition | identical repeated samples ("machine-gun") | round robins, two alternating samples, small variation per repeat | [Loopcloud](https://www.loopcloud.com/cloud/blog/5254-7-Drum-Programming-Tips-to-Improve-Any-Groove), [Versilian](https://versilian-studios.com/virtuosity-drums/) |
| Width | near-mono, or phasey super-wide | low end mono below ~100–150 Hz; full mix correlation between 0 and +1, ≥ +0.3 after widening | [Mastering The Mix](https://www.masteringthemix.com/pages/how-to-get-the-right-stereo-width), [mastrng](https://mastrng.substack.com/p/correlation-meter) |
| Dynamics | flat loudness, wall-to-wall | a quiet section exists; silence before the peak | [KillTheDJ](https://killthedj.com/how-to-trailerize-your-music-to-boost-your-career-make-additional-income-as-a-musician), TASTE §7 |
| SFX in score | riser or whoosh on every cut | many "crash, bang, booms… braams" are SFX cut in by the editor, kept separate from the music | [Taxi forum](https://forums.taxi.com/post483850.html) |
| Licensing | unclear sample sources | every sample traceable to a licence that allows the use (see §8) | – |

---

## 3. Techniques with numbers

### 3.1 Tempo and feel

| Genre | Typical BPM | Notes | Source |
|---|---|---|---|
| Lo-fi hip-hop | 70–90 (some say 60–80) | swing ≈ 55–62 % on hats and snares | [Soundtrap](https://blog.soundtrap.com/?p=649), [UJAM](https://www.ujam.com/tutorials/the-ultimate-guide-to-lofi-hip-hop/) |
| House | 118–130, centre 124 | | [Beatkitchen](https://beatkitchen.io/guides/electronic-music/01-genre-landscape/), [Ableton Learning Music](https://learningmusic.ableton.com/make-beats/tempo-and-genre.html) |
| Techno / "tech" | 120–140 | | [Beatkitchen](https://beatkitchen.io/guides/electronic-music/01-genre-landscape/) |
| Trap | 130–160, sweet spot ≈ 140 | snare on beat 3 gives a half-time feel (felt as 65–80) | [Native Instruments](https://www.native-instruments.com/blogs/blog/how-to-make-a-trap-beat), [Reason Studios](https://www.reasonstudios.com/news/post/trap-drum-basics-super-neat-beat-cheat-sheet) |
| Drum & bass | 170–176, default 174 | bass and stabs move at half time (≈ 85) | [EDMProd](https://www.edmprod.com/how-to-make-drum-and-bass/), [Violet Recording](https://violetrecording.com/how-to-make-drum-and-bass/) |
| Synthwave | 95–135, sweet spot 108–118 (one source) | | [BeatKey](https://beatkey.pages.dev/how-to-make-synthwave-music) (weak source) |
| Pop | 100–130 | | [Beatkitchen](https://beatkitchen.io/guides/electronic-music/01-genre-landscape/) |
| Rock | 110–140 | | [Beatkitchen](https://beatkitchen.io/guides/electronic-music/01-genre-landscape/) |
| Corporate / uplifting | listings cluster at 100–135 | | [Shockwave-Sound listings](https://shockwave-sound.com/browse/production-type/business-corporate/stock-music/order-relevance/standard/50/23) |
| Hip-hop (general) | 70–90 (sources range 60–115) | | [Ableton](https://learningmusic.ableton.com/make-beats/tempo-and-genre.html) |

**Swing conventions.**
- In MPC/Logic terms, 50 % is straight, 54 % loosens without sounding swung, and 66 % is a
  perfect triplet ([Roger Linn in Attack](https://www.attackmagazine.com/features/interview/roger-linn-swing-groove-magic-mpc-timing/)).
- FL Studio and Cubase count differently: 0 % there is 50 % in Logic/MPC, and 100 % there is
  66.6 % ([Attack](https://www.attackmagazine.com/technique/passing-notes/daw-drum-machine-swing/)).
- **Jazz swing depends on tempo.** Drummers' ride swing ratio is about 2:1 only near
  200 BPM. It rises to about 3:1 (75 %) towards 120 BPM and falls to 1:1 near 300 BPM
  ([Friberg & Sundström 2002, via ASA](https://acoustics.org/pressroom/httpdocs/137th/friberg.html);
  [summary](https://forums.musicplayer.com/topic/154367-microtiming-studies-rhythmic-feel-swing-rhythm)).
  Soloists play straighter than the drummer.

> **Engine note.** In `seq.rs::Grid`, `swing` maps to an off-beat position of 50 % + 25 % ×
> `swing`. 0.33 is therefore 58 %, not a triplet (that needs 0.667), so the doc comment is
> wrong. Current defaults: lofi 0.45 → 61 % (good), house 0.08 → 52 %, and jazz 0.6 at 8ths →
> 65 %, which is straighter than a 132 BPM ride (≈ 70–74 % by Friberg).

### 3.2 Drum programming

| Parameter | Value | Source |
|---|---|---|
| Ghost-note velocity | ≈ 20–40 (MIDI), "felt more than heard"; at most 30–40 % of the main hit | [Loopcloud](https://www.loopcloud.com/cloud/blog/5254-7-Drum-Programming-Tips-to-Improve-Any-Groove), [Splice](https://splice.com/blog/humanize-your-drums/) |
| Velocity humanise | ±10 (Logic), or main hits within 87–102 | [MixElite](https://mixelite.com/blog/humanizing-midi-drums/), [Soundbridge](https://soundbridge.io/en/drum-programming-tips-for-producers-of-all-levels) |
| Timing humanise | kick and snare tight, hats looser; max ≈ 12 ticks at fast tempos; quantise strength ≤ 80–90 % | [MixElite](https://mixelite.com/blog/humanizing-midi-drums/) |
| Hat accents | stronger on the beat, weaker off it; alternate loud and soft strokes | [Musicianwave](https://www.musicianwave.com/make-software-midi-sample-drums-sound-real/) |
| Repetition | vary each repeat; alternate two similar hat samples | [Loopcloud](https://www.loopcloud.com/cloud/blog/5254-7-Drum-Programming-Tips-to-Improve-Any-Groove) |
| Trap hat rolls | 32nds or 8T/16T triplets as accents before snares or at 4-bar phrase ends, not constantly | [MusicRadar](https://www.musicradar.com/how-to/how-to-program-mixed-resolution-trap-style-hi-hat-patterns), [Reason](https://www.reasonstudios.com/news/post/trap-drum-basics-super-neat-beat-cheat-sheet) |
| DnB break | kick on 1, snares on 2 and 4 at 174, with a busy scatter of ghost hats and shuffled hits between | [Violet Recording](https://violetrecording.com/how-to-make-drum-and-bass/) |

### 3.3 Dynamics processors and space

| Use | Settings | Source |
|---|---|---|
| Sidechain pump (house/EDM) | attack 1–10 ms (too fast clicks); release from tempo (60 000/BPM ms per quarter), 200–350 ms at 128 BPM for an audible pump; depth 6–12 dB on sub, 3–6 dB on mid bass, 1–3 dB for subtle ducking; ratio 3:1–6:1 | [Gearnews](https://gearnews.com/sidechain-compression), [emastered](https://emastered.com/blog/sidechain-compression), [Futureproof](https://futureproofmusicschool.com/blog/how-to-use-sidechain-compression-for-cleaner-kicks), [mastering.com](https://mastering.com/sidechain-compression-guide/) |
| Mix-bus "glue" | 2:1 (or 4:1), attack 10 or 30 ms, auto release, 2–4 dB GR; many engineers stop at 1–2 dB | [Waves SSL guide](https://www.waves.com/how-to-use-ssl-bus-compressor-classic-mix-glue), [MusicRadar](https://www.musicradar.com/how-to/compress-mix-bus) |
| Reverb pre-delay | 1/64–1/32 note, or 10–80 ms, keeps the dry attack clear | [Sage Audio](https://www.sageaudio.com/articles/timing-effects-like-a-pro), [emastered](https://emastered.com/blog/reverb-pre-delay) |
| Reverb decay at tempo | a snare reverb that outlasts the beat clashes with the next hit (2 s at 140 BPM does); bar length = 4 × 60 000/BPM is a ceiling | [Violet Recording calculator](https://violetrecording.com/tools/reverb-calculator/) |
| Gated snare (synthwave) | 100 %-wet long reverb on a send, gated (keyed from the snare); start with a 2–3 s decay and a 150–250 ms gate | [Audiotent](https://www.audiotent.com/blogs/production-tips/gated-snare-reverb), [BeatKey](https://beatkey.pages.dev/how-to-make-synthwave-music) |
| Ambient pads | attack 2–8 s; RT60 4–15 s; high-pass the upper layers and keep one element on the bottom | [BeatKey](https://beatkey.pages.dev/how-to-make-ambient-music), [Violet Recording](https://violetrecording.com/how-to-make-ambient-soundscapes/) |
| Reverb and delay returns | high-pass below ≈ 150 Hz to keep the lows clean | [Mastering The Mix](https://www.masteringthemix.com/blogs/learn/how-to-diagnose-and-fix-stereo-problems-in-your-mix) |
| Spiccato / short strings | a transient designer with a modest attack boost brings them through without raising level | [Epicomposer](https://epicomposer.com/trailer-music-production-tips/) |

### 3.4 Lo-fi "worn" processing

| Effect | Value | Source |
|---|---|---|
| Wow/flutter depth | good cassette ≈ 0.08 %, average and car decks ≥ 0.2 %; above 0.1 % is audible to most listeners. Go easy on wow, harder on the other effects | [Lindos](https://lindos.co.uk/articles/wow-and-flutter-measurement), [Wikipedia: wow](https://en.wikipedia.org/wiki/Wow_(recording)), [Mode Audio](https://modeaudio.com/magazine/lofi-hip-hop-5-production-essentials) |
| Wow/flutter rate | wow ≈ 0.5–2 Hz (1–2 per second); flutter faster, split at ≈ 4 Hz | [Sweetwater](https://www.sweetwater.com/insync/wow/), [Wikipedia: measurement](https://en.wikipedia.org/wiki/Wow_and_flutter_measurement) |
| Sample-rate colour | SP-1200 at 26.04 kHz | [Mastering The Mix](https://www.masteringthemix.com/blogs/learn/how-to-make-lo-fi-hip-hop) |
| Low-pass | "your best friend"; automate the cutoff over 8–16 bars so it evolves | [Soundtrap](https://blog.soundtrap.com/?p=649), [Routenote](https://create.routenote.com/blog/how-to-make-a-lofi-hip-hop-beat/) |
| Vinyl crackle | a low bed just under the drums, optionally ducked a little by the kick | [UJAM](https://www.ujam.com/blog/top-5-production-mixing-tips-for-lofi/), [Mode Audio](https://modeaudio.com/magazine/lofi-essential-production-tools) |
| Saturation band | ≈ 100 Hz–10 kHz | [Mastering The Mix](https://www.masteringthemix.com/blogs/learn/how-to-make-lo-fi-hip-hop) |
| Chords | maj7, m7, dom7, 9ths (Cmaj7 → Cmaj9), voiced high on Rhodes, Wurlitzer or old piano | [Flat.io](https://blog.flat.io/lofi-chord-progressions/), [Native Instruments](https://www.native-instruments.com/blogs/blog/lo-fi-chord-progressions) |

Starting values *(heuristic)*: LP 7–9 kHz, wow 0.15 % at 0.7 Hz, flutter 0.05 % at 6 Hz,
crackle at −36 dBFS RMS, 12-bit/26 kHz on the drum bus only.

---

## 4. Per-style cheat sheet (our 15 styles)

Columns: tempo · groove · signature sounds · harmony · arrangement density · effects.
Everything links back to §3 unless it has its own source.

| Style | Tempo | Groove | Sounds | Harmony | Density | FX / mix |
|---|---|---|---|---|---|---|
| **lofi** | 70–90 | 16th swing 55–62 %; laid-back snare; ghost snares | Rhodes/Wurli/old piano, upright or round bass, dusty kit, vinyl | m7/maj7/9ths, ii–V–I | 3–4 parts | LP with movement, wow ≤ 0.2 %, crackle, SR reduction on drums ([§3.4](#34-lo-fi-worn-processing)) |
| **house** | 118–130 (124) | four-on-the-floor, offbeat open hat, 16th swing 52–56 % | piano/organ stabs, rolling bass, clap on 2 and 4 | minor 7/9 loops, stabs | builds by layer | sidechain on bass, pads and stabs: 1–10 ms attack, quarter-note release, 6–12 dB on sub ([§3.3](#33-dynamics-processors-and-space)) |
| **synthwave** | ≈ 100–118 | straight 8ths, simple kick, snare on 2 and 4 | Juno-style chorused poly pad, 8th arp, gated snare, LinnDrum/707-style kit | minor key, sus2/add9/m7 | 4 parts: pad, arp, bass, drums | gated reverb 2–3 s with a 150–250 ms gate; arp pumped by sidechain ([Mode Audio](https://modeaudio.com/magazine/synthwave-5-production-essentials), [BeatKey](https://beatkey.pages.dev/how-to-make-synthwave-music)) |
| **trap** | 130–160 (140) | half time: snare or clap on beat 3; hats in 8ths/16ths with roll accents | tuned 808 with edited releases, layered snare+clap | minor, often 2-chord | sparse mids, heavy low end | 808 as the main bass, mono; rolls before snares ([NI](https://www.native-instruments.com/blogs/blog/how-to-make-a-trap-beat)) |
| **dnb** | 170–176 | two-step: kick on 1, snares on 2 and 4, ghost scatter | split bass: clean sub plus Reese (detuned, moving) | minor, half-time stabs | breaks + bass + pad | Reese modulated; sub mono ([Violet](https://violetrecording.com/how-to-make-drum-and-bass/), [EDMProd](https://www.edmprod.com/how-to-make-drum-and-bass/)) |
| **cinematic-hybrid** | ostinato-driven; pulse in 8ths/16ths | string ostinato, taiko, pulses on root notes | low brass, short strings, taiko, choir; impacts layered from kick/taiko | minor, pedal points, ♭VI–♭VII–i | three-act build | riser lands *on* the hit; transient boost on spiccato ([Epicomposer](https://epicomposer.com/trailer-music-production-tips/), [Ableton](https://www.ableton.com/es/blog/learn-how-to-make-high-impact-sounds-for-movies-and-trailers)) |
| **orchestral** | rubato-friendly 70–110 *(heuristic)* | none, or a light pulse | strings, horns, woodwinds, harp, timpani | functional harmony, suspensions | grows by family | hall reverb on a send (pre-delay 20–40 ms *(heuristic)*) |
| **ambient** | free or 60–80 | none | drones, slow pads, glass, piano | static or modal | 1–3 layers | attack 2–8 s, RT60 4–15 s, slow modulation of filters and pan ([BeatKey](https://beatkey.pages.dev/how-to-make-ambient-music), [MusicRadar](https://www.musicradar.com/tuition/tech/26-essential-ambient-production-tips)) |
| **rock** | 110–140 | straight 8ths, crash on section downbeats | double-tracked rhythm guitars panned hard L/R, real kit | power chords, I–♭VII–IV | band | room reverb short; drum-bus glue ([§3.3](#33-dynamics-processors-and-space)) |
| **pop** | 100–130 | straight or 54 % swing | piano/guitar, claps, synth bass | add9/sus2 colour, chorus lift | verse sparse, chorus full | bus glue 1–2 dB, wide chorus |
| **corporate** | 100–135 | steady 8ths or 4-on-floor | light guitar, piano, strings, glockenspiel, claps | major, I–V–vi–IV (TASTE flags this as a tell) | drive inside 5–10 s | bright and clean; avoid the TASTE "uplifting corporate" tell ([Depositphotos](https://depositphotos.com/music/management.html), [Udemy](https://www.udemy.com/course/how-to-compose-uplifting-corporate-music/)) |
| **tech** | 120–140 | 16ths, straight | arp plucks, sub, glassy pads | minor, few chords | grid-tight | sidechain, tempo-synced delays |
| **piano** | 60–90 | rubato feel | felt or grand piano | rich voicings, inversions | solo | pedal model, room or hall |
| **acoustic** | 85–110 | strummed 8ths/16ths with accents | steel and nylon guitar, upright bass, brushes | major, sus/add9 | 2–4 parts | wide doubled guitars |
| **jazz** | 100–200 | swing ratio by tempo (≈ 3:1 slow → 2:1 at 200 BPM) | brushes/ride, upright walking bass, piano comping | ii–V–I, 7/9/13 extensions | trio | tempo-dependent swing ([Friberg](https://acoustics.org/pressroom/httpdocs/137th/friberg.html)) |

Tempos for orchestral, ambient, tech, piano and acoustic are the engine's defaults and
general practice *(heuristic)*. No single authoritative chart was found for them.

---

## 5. Short-form structures (bar by bar)

**Rules.**
- At 120 BPM in 4/4, 1 bar = 2.0 s. In general, bar = 240/BPM seconds.
- Count the **ring-out** inside the length. The reverb tail must decay before the picture
  ends ([Taxi](https://forums.taxi.com/tips-for-button-endings-t8928.html)).
- UK TV spots carry 0.5 s of silence at each end, so a "30 s" track runs 29 s
  ([SOS](https://www.soundonsound.com/techniques/library-work)).
- Lay the bars out *backwards from the button*, then trim the intro. Never trim the ending.
- A sting is "a very short fragment of the tune, often the final notes, an opening phrase or a
  memorable hook" ([SOS](https://www.soundonsound.com/techniques/library-work)).
- A sonic logo is 1–5 s and 3–5 notes. Intel's is five notes in about 3 s
  ([Creative Bloq](https://creativebloq.com/features/audio-logos),
  [ALIBI](https://alibimusic.com/blog/creating-a-memorable-sound-logo)).

### 5 s ident / logo sting (≈ 2.5 bars at 120)

| Time | Bars | Content |
|---|---|---|
| 0.0–2.0 s | bar 1 | pickup: the 3–5 note motif (the theme's head) over a swell or reverse cymbal into the downbeat; no drums or one tick |
| 2.0 s | bar 2 beat 1 | **hit**: full chord (I, or i → major I for "lift"), sub boom, crash/timpani; logo on this frame |
| 2.0–5.0 s | ring | sustain + reverb tail 2.5–3 s; nothing new enters; tail at < −40 dB by 4.8 s *(heuristic)* |

### 10 s (5 bars at 120)

| Bar | Content |
|---|---|
| 1 | hook, sparse (1–2 voices) |
| 2 | add the pulse (bass + hats) |
| 3 | lift: chord change toward V, fill on beat 4 |
| 4 | beats 1–3 at full; **beat 4 rest** (silence or a reverse swell) |
| 5 | **button** on the downbeat (I, kick + crash), ring ≈ 2 s |

### 15 s (7.5 bars at 120; use 7 bars + 1 s ring)

| Bars | Section | Energy | Notes |
|---|---|---|---|
| 1 | intro + hook | 0.4 | the motif in bar 1; no slow fade-in |
| 2–3 | build | 0.6–0.75 | drums enter, filter opens; snare pickup at the end of bar 3 |
| 4–6 | peak | 1.0 | the hook returns (same notes); countermelody optional |
| 6 beat 4 | stop | 0 | 1 beat of silence, or a half beat at ≥ 140 BPM |
| 7 | button + ring | – | tonic hit, 2 s ring |

### 30 s (15 bars at 120; 14 bars + 2 s ring)

| Bars | Section | Notes |
|---|---|---|
| 1–2 | intro | identity at once: hook or signature sound, 1–2 voices |
| 3–6 | A | groove; motif stated and answered (4 + 4 beats); edit point at bar 6 |
| 7–8 | build | pre-chorus harmony (IV–V or ♭VI–♭VII), riser into bar 9; **last beat empty** |
| 9–13 | peak | hook returns in full, countermelody, crash on bar 9 |
| 14 | cadence + button | V (or IV–V) in beats 1–3, tonic hit on the downbeat of the "bar 15" ring |
| ring | | 1.5–2 s tail |

**A 30 s cut does not end in a low-energy outro.** It ends on the peak and a button.

### 60 s (30 bars at 120)

| Bars | Section | Notes |
|---|---|---|
| 1–2 | intro | |
| 3–10 | A (8) | theme, development; small fill every 4 bars, larger every 8 |
| 11–12 | build | |
| 13–14 | peak 1 → **false ending** at ≈ 28–30 s (button-like hit, 1–2 beats air) | SOS asks for "a very definite edit at 30 seconds — a false ending" so each half stands alone ([SOS](https://www.soundonsound.com/techniques/library-work)) |
| 15–18 | breakdown | pad + melody only (a quiet bed for VO) |
| 19–22 | build 2 | new layer (countermelody, octave-up strings); tempo unchanged |
| 23–28 | final peak | biggest density; drum variation every 2 bars |
| 29 | stop + cadence | |
| 30 | button + ring | |

### Trailer forms

- **Long form (2–2.5 min).** One documented example runs cold open 20 s, Act I 42 s, Act II
  48 s, Act III 40 s, then an outro, with pauses between acts and "a quiet moment before the
  climax" ([KillTheDJ](https://killthedj.com/how-to-trailerize-your-music-to-boost-your-career-make-additional-income-as-a-musician)).
- **The three acts.** Act I is ambient, emotional and minimal. Act II grows the rhythm and
  tension, often with ticking. Act III is full energy
  ([Rareform](https://www.rareformaudio.com/blog/how-production-music-reveals-trailer-structure),
  [Violet Recording](https://violetrecording.com/sound-design-for-trailers/)).
- **A 30–60 s trailer** compresses this proportionally *(heuristic)*:
  - Act I: 20 %, sustains and a pulse.
  - Act II: 35 %, ostinato 8ths → 16ths, ticking, a riser in the last 2 bars.
  - **Silence: 0.5–1 bar.**
  - Act III: 35 %, hits on cut points, full ensemble.
  - Button: 10 %, a sting or final braam plus a 2 s ring.
- **Energy without tempo changes.** Increase the subdivision (8ths → 16ths → 16th triplets)
  and the percussion density. Editors need a steady tempo to cut
  ([SOS](https://www.soundonsound.com/techniques/library-work)).

---

## 6. Scoring to picture

1. **Spot first.** Decide the start, stop and 3–6 hit points per minute (TASTE §7).
2. **Fit one tempo to many hits.**
   - Search BPM so that the hit times land on downbeats, and score the fit by the error
     ([Frans Absil, event-hit tool](https://fransabsil.nl/htm/eventhit.htm);
     [*Scoring to Picture in Logic Pro*](https://packtpub.com/en-za/product/scoring-to-picture-in-logic-pro-10-9781837636891/chapter/chapter-7-creating-tempo-maps/adjusting-the-tempo-to-match-the-hit-points)).
   - When one tempo cannot catch them all, use subtle tempo drift between hits
     ([SOS, media composers](https://www.soundonsound.com/techniques/media-composers);
     [Steinberg forum](https://forums.steinberg.net/t/warp-tool-or-tap-tempo-for-scoring-to-film/610765?page=2)).
3. **Hit tolerance.** Viewers detect audio up to 45 ms early or 125 ms late (ITU-R BT.1359,
   cited in TASTE §7). Land on the frame, or one frame late, never early.
4. **What happens at a hit.** The hit lands on a downbeat with a section change. A riser or
   reverse swell *ends exactly on* the hit, and there are 0.5–1 beat of air before it.
5. **Cuts that are not hits** get small accents (a cymbal or a fill), or nothing. Don't
   mickey-mouse (TASTE §7).
6. **Dialogue or voice-over.**
   - Keep music at least 10 LU under the commentary
     ([Torcoli et al., JAES 2019](https://cris.fau.de/publications/234640600)). Non-experts
     preferred about 4 LU more separation.
   - Voice-over beds are often 18–26 dB under the voice by ear
     ([Overseer](https://www.overseeros.com/blog/background-music-volume-under-voiceover)).
     No single standard exists ([Creative COW](https://creativecow.net/?p=2139815)).
   - Mute or remove the melody under VO
     ([Epidemic](https://www.epidemicsound.com/blog/what-are-stems-in-music-production/);
     the [SOS](https://www.soundonsound.com/techniques/library-work) "backing track").

---

## 7. Mix and master targets

| Target | Value | Source |
|---|---|---|
| YouTube | normalises playback to ≈ −14 LUFS, turning loud uploads down only | [Production Advice](https://productionadvice.co.uk/youtube-loudness/), [Frame.io](https://workflow.frame.io/guide/loudness-for-youtube) |
| TikTok / Instagram | no official figure; −14 LUFS / −1 dBTP is the common working target | [Fora Soft](https://www.forasoft.com/learn/audio-for-video/articles-audio/lufs-targets-per-platform-2026) (secondary) |
| Broadcast EU | EBU R128: −23 LUFS ±0.5 LU (±1 live), max −1 dBTP | [EBU R128](https://en.wikipedia.org/wiki/EBU_R_128) |
| Broadcast US (and CALM Act ads) | ATSC A/85: −24 LKFS (±2), max −2 dBTP | [NAB CALM best practices](https://www.nab.org/documents/resources/CALMBestPractices.pdf), [TV Tech](https://www.tvtechnology.com/news/atsc-tackles-audio-loudness) |
| Podcasts | ≈ −16 LUFS | [Fora Soft](https://www.forasoft.com/learn/audio-for-video/articles-audio/lufs-targets-per-platform-2026) |
| Low end | mono below ≈ 100–150 Hz; correlation near +1 in the lows | [Mastering The Mix](https://www.masteringthemix.com/pages/how-to-get-the-right-stereo-width), [Futureproof](https://futureproofmusicschool.com/blog/how-do-i-preserve-mono-low-end-while-adding-stereo-width-to-highs) |
| Width | full-mix correlation between 0 and +1; after widening keep ≥ +0.3 to +0.4 | [mastrng](https://mastrng.substack.com/p/correlation-meter), [Mastering The Mix](https://www.masteringthemix.com/blogs/learn/stereo-width-how-wide-is-too-wide) |
| TV bass | bass audible at ≈ 80 Hz; HPF ≈ 60 Hz if needed; mono-safe | [SOS](https://www.soundonsound.com/techniques/library-work) |
| Bus glue | 2:1, 10–30 ms attack, auto release, 1–4 dB GR | [Waves](https://www.waves.com/how-to-use-ssl-bus-compressor-classic-mix-glue) |
| Ducking (video mixer) | music ≥ 10 LU under speech | [Torcoli 2019](https://cris.fau.de/publications/234640600) |

Our master already hits −14 LUFS / −1 dBTP within 0.1 LU. The DIAGNOSIS correlation range
(0.80–0.98, still ≈ 0.95 for lofi, jazz and pop) sits above the 0.3–0.7 that commercial mixes
show.

---

## 8. Free instrument libraries: licence check

Verdict key:
- **USE**: permissive (CC0, public domain, MIT, CC-BY with credit). OK to fetch on first
  use, redistribute and use in ads.
- **CREDIT**: CC-BY. Usable, but the render must carry the credit string (we already do
  this for Salamander).
- **AVOID**: non-commercial, share-alike on the samples, advertising-excluded, no
  redistribution, or unclear.

Sizes are the download sizes stated by the source.

### 8.1 Recommended small packs (< 150 MB each)

| Need | Library | Licence | Size / format | Quality notes | URL | Verdict |
|---|---|---|---|---|---|---|
| Grand piano | **Splendid Grand Piano** (Akai Steinway) | Public domain ("released as public domain in early 2000 by Akai") | 77 MB, SFZ + FLAC | 4 velocity layers + 1 filtered; dated but clean | [github.com/sfzinstruments/SplendidGrandPiano](https://github.com/sfzinstruments/SplendidGrandPiano) | **USE**: best default piano under 150 MB |
| Upright piano | **Upright Piano KW** (FreePats) | CC0 1.0 | 32 MiB SFZ+FLAC (69 MiB WAV); "small" 2.9 MiB | 2 velocity layers | [freepats.zenvoid.org/Piano](https://freepats.zenvoid.org/Piano/acoustic-grand-piano.html) | **USE**: intimate / lofi / felt-ish |
| Grand piano (better) | **Headroom Piano** (Bengt Nilsson) | CC-BY-4.0 | 156 MB | – | [sfzinstruments.github.io/pianos/headroom_piano](https://sfzinstruments.github.io/pianos/headroom_piano) | **CREDIT**: just over budget |
| Electric pianos | **Greg Sullivan E-Pianos**: Wurlitzer EP200, Yamaha CP80, Hohner Pianet T | CC-BY-3.0 | 21.5 MB, SFZ + FLAC | no Rhodes; the Wurli suits lofi and jazz | [github.com/sfzinstruments/GregSullivan.E-Pianos](https://github.com/sfzinstruments/GregSullivan.E-Pianos) | **CREDIT** |
| Electric bass | **Bass Guitar YR** (FreePats; finger and pick) | CC0 1.0 | 3.1 MiB finger / 2.7 MiB pick, SFZ+FLAC | Yamaha RBX | [freepats.zenvoid.org/ElectricGuitar/clean-electric-bass.html](https://freepats.zenvoid.org/ElectricGuitar/clean-electric-bass.html) | **USE** |
| Nylon guitar | **Spanish Classical Guitar** (FreePats) | CC0 1.0 | 4.5 MiB SFZ+FLAC | – | [freepats.zenvoid.org/Guitar/acoustic-guitar.html](https://freepats.zenvoid.org/Guitar/acoustic-guitar.html) | **USE** |
| Clean electric guitar | **FSBS Electric Guitar Clean #1 / #2 (Jazz)** | CC0 1.0 | 118 MiB / 63 MiB SFZ+FLAC; "small" 2.4–3.0 MiB | bridge pickup, v2026-08-07 | [freepats.zenvoid.org/ElectricGuitar/clean-electric-guitar.html](https://freepats.zenvoid.org/ElectricGuitar/clean-electric-guitar.html) | **USE** |
| Steel acoustic guitar | **FSS Steel-String** (FreePats) | GPL-3+ with the FreePats exception: using the sounds in a composition doesn't make the composition GPL | 13 MiB WAV / 2.7 MiB small | – | [freepats.zenvoid.org/Guitar/steel-acoustic-guitar.html](https://freepats.zenvoid.org/Guitar/steel-acoustic-guitar.html), [licence](https://freepats.zenvoid.org/licenses.html) | **USE** for renders; distributing the pack carries GPL obligations |
| Drum kit (multi-velocity, randomised) | **MuldjordKit** (DrumGizmo / FreePats) | CC-BY-4.0 | 157 MiB SFZ+FLAC; 53 MiB SF2 (velocity only) | velocity layers + "randomized sounds" (round robin) | [github.com/freepats/muldjordkit releases](https://github.com/freepats/muldjordkit/releases/download/2020-10-18/MuldjordKit-SFZ+FLAC-20201018.7z) | **CREDIT**: best fit; downmix the mics to stereo and re-encode → well under 150 MB |
| Tuba / low brass | **War Tuba** (Karoryfer) | CC0-1.0 | 104 MB | – | [shop.karoryfer.com/pages/free-war-tuba](https://shop.karoryfer.com/pages/free-war-tuba) | **USE** (shop page, not git) |
| GM fallback | **MuseScore_General** (from FluidR3) | MIT; parts PD / CC0 | SF2/SF3 | better than GM average | [Debian copyright](https://people.debian.org/~tg/sf/MuseScore_General_Full.copyright) | **USE** |
| GM fallback | **FluidR3_GM** | MIT | SF2 | classic | [MuseScore licence file](https://src.opensuse.org/rpm/musescore/src/branch/factory/MuseScore_General_License.md) | **USE** |
| Tiny GM | **Sonivox EAS** wavetable | Apache-2.0 | embedded in C | very small, low quality | [github.com/pedrolcl/sonivox](https://github.com/pedrolcl/sonivox) | USE only as a last resort |

### 8.2 High quality but large (make derived subsets)

| Library | Licence | Size / format | Notes | URL | Verdict |
|---|---|---|---|---|---|
| **Salamander Grand V3** | CC-BY-3.0 | 394 MiB WAV 44.1/16; 707 MiB FLAC 48/24; 296 MiB SF2 | 16 velocity layers | [FreePats page](https://freepats.zenvoid.org/Piano/acoustic-grand-piano.html), [sfzinstruments](https://github.com/sfzinstruments/SalamanderGrandPiano) | **CREDIT**: already optional. A Vorbis "lite" (8 layers, every 3rd note) should land near 100 MB *(estimate)* |
| **Virtuosity Drums** (Versilian) | CC0 | 1.1 GB; SFZ | snare/toms/kick up to 36 dynamic layers; cymbals use fewer layers × 4 round robins; 6 mixable mics | [versilian-studios.com/virtuosity-drums](https://versilian-studios.com/virtuosity-drums/), [zip](https://versilian-studios.com/Distro/Virtuosity_Drums_v0.925.zip) | **USE**: best licence × quality for drums. Make a stereo mixdown with 8 layers × 2–4 RR (≈ 50–100 MB *(estimate)*) |
| **VSCO-2 CE** | CC0-1.0 | ≈ 3,200 WAV, a few GB | strings, brass, woodwinds, harp, timpani, mallets, organ, upright | [github.com/sgossner/VSCO-2-CE](https://github.com/sgossner/VSCO-2-CE) | **USE**: fetch per instrument (sparse checkout) |
| **VCSL** | CC0 | ≈ 5 GB in all; 20–75 MB per instrument | pianos, harpsichords, organs, percussion, mallets; no orchestral strings, brass, guitar or EP; 2–3 velocity layers, mostly 1 RR | [github.com/sgossner/VCSL](https://github.com/sgossner/VCSL), [Versilian](https://versilian-studios.com/vcsl/) | **USE**: per instrument |
| **DRSKit** (DrumGizmo) | CC-BY-4.0 (credit needed when performer credits are listed) | 754 MB, 13 mics | | [drumgizmo.org](https://drumgizmo.org/wiki/doku.php?id=kits:drskit) | **CREDIT**: mixdown needed |
| **Naked Drums** | CC-BY-4.0 | 1.3 GB | up to 5 velocity layers × 10 RR | [sfzinstruments drums](https://sfzinstruments.github.io/drums/) | **CREDIT** |
| **SM Drums** | Public domain | 2.2+ GB (Google Drive) | | [sfzinstruments drums](https://sfzinstruments.github.io/drums/sm_drums) | USE (too big; derive) |
| **Karoryfer**: Black and Green Guitars (500 MB), Shinyguitar (352 MB), Growlybass (160 MB), Swagbass (138 MB), Meatbass and D. Smolken double bass, Bigcat Cello, Big Rusty / Swirly / Unruly drums (1.6–2.3 GB) | CC0-1.0 | SFZ | high quality, made for music | [sfzinstruments basses](https://sfzinstruments.github.io/basses/), [strings](https://sfzinstruments.github.io/strings/), [guitars](https://sfzinstruments.github.io/guitars/), [drums](https://sfzinstruments.github.io/drums/) | **USE**: downloads are from the Karoryfer shop pages (free), not git |
| **University of Iowa MIS** | "may be downloaded and used for any projects, without restrictions" | per-note AIFF | anechoic single notes, raw; needs mapping and trimming | [theremin.music.uiowa.edu/MIS.html](https://theremin.music.uiowa.edu/MIS.html) | USE (raw material only) |

### 8.3 Avoid

| Library | Problem | Source |
|---|---|---|
| **Sonatina Symphonic Orchestra** | CC Sampling Plus 1.0: "**All advertising and promotional uses are excluded**", and no use "primarily intended for… commercial advantage" of the work as such; licence retired in 2011. Disqualifying for ads | [CC legal code](https://creativecommons.org/licenses/sampling+/1.0/legalcode), [sfzinstruments](https://sfzinstruments.github.io/orchestra/sso) |
| **jRhodes3d / jRhodes3c** (the only good free Rhodes) | samples CC-BY-NC-4.0 / CC-BY-NC-SA-4.0: non-commercial | [sfzinstruments pianos](https://sfzinstruments.github.io/pianos/) |
| **Philharmonia Orchestra samples** | commercial use in music is fine, but they "must not be sold or made available 'as is' (i.e. as samples or as a sampler instrument)", so we can't ship or fetch them as a pack. User-supplied only | [philharmonia.co.uk](https://philharmonia.co.uk/audio-sound-samples/) |
| **Timbres of Heaven**, **Arachno** | personal / non-commercial, no redistribution | [MuseScore forum](https://musescore.org/en/node/36806), [Polyphone](https://www.polyphone.io/en/soundfonts/instrument-sets/255-arachno-soundfont) |
| **Rickenbacker 4001** (SFZ) | CC-BY-NC-SA-3.0 | [sfzinstruments basses](https://sfzinstruments.github.io/basses/) |
| **Salamander Drumkit**, **Sam's Sonor**, LivingRoom Upright | CC-BY-SA: share-alike on the samples. Renders are probably fine, but a derived pack must stay SA. Use only if nothing else fits | [sfzinstruments drums](https://sfzinstruments.github.io/drums/) |
| **Hydrogen kits** (e.g. GMkit) | mixed or undefined licences; GMkit flagged as likely Roland-derived | [Debian bug #858281](https://lists.alioth.debian.org/pipermail/pkg-multimedia-maintainers/2017-March/058203.html), [Hydrogen manual](https://hydrogen-music.org/documentation/manual/manual_en_chunked/ch20.html) |
| **Samples From Mars** freebies | "royalty-free" for use, but no published redistribution terms | [samplesfrommars.com](https://samplesfrommars.com/pages/free-sp-from-mars) |
| **Northern Trumpets**, "Custom"-licensed entries (Standard Bass, Maestro Grand…) | free to download, licence not stated | [sfzinstruments brass](https://sfzinstruments.github.io/brass/) |

**Rhodes.** We found no free Rhodes with a commercial-friendly licence. Keep a synthesized
tine EP (FM or modal) for `rhodes` and use the CC-BY Wurlitzer for the "worn keys" colour.

**Proposed first-use pack** (≈ 140 MB after Vorbis re-encoding, *estimate*):
- Splendid Grand (PD)
- Upright KW (CC0)
- Greg Sullivan Wurli/CP80 (CC-BY)
- YR bass (CC0)
- Spanish guitar (CC0)
- FSBS clean small (CC0)
- MuldjordKit stereo mixdown (CC-BY)
- VSCO-2 CE strings and brass sustains, plus spiccato subset (CC0)

---

## 9. Gap list against Anikit (ranked by audible impact)

The first ten are the most important. Each is concrete and testable with the `diagnose` or
`demos` examples.

1. **Endings: button, sting and ring-out as a first-class system.** *(arrangement)*
   - **Now:** `end: ring` appends an 8-beat tonic with a kick and crash only when ≥ 0.4 s
     remains after the last whole bar. Auto forms for 30–60 s finish on an `outro` at energy
     0.3–0.45.
   - **Do:**
     - Plan the timeline backwards from the end: button downbeat = `length − ring`, with
       ring = 1.5 s for 5–10 s pieces, 2 s for 15–30 s and 2.5–3 s for trailers.
     - Precede the button with V (or IV–V) and a **1-beat stop**.
     - The button is tonic + kick + crash or timpani + sub, with the melody landing on the
       tonic or 3rd.
     - Verify that the tail is below −40 dB at `length`.
     - For lengths ≤ 30 s, replace the low outro with peak → button.
   - **Endings menu:** `button`, `sting` (the motif head + chord), `ring`, `fade` (only if
     asked), `cut`.
2. **Short-form templates that match §5 exactly** (5/10/15/30/60 s).
   - Bars are counted from the button. The hook comes in bar 1.
   - The 60 s template has a **false ending at ≈ 30 s** (an SOS requirement).
   - Builds end with an empty last beat. The 5 s ident is pickup → hit → ring.
   - Today, `< 8 bars` has an arc but no stop or cadence, and long forms put the peak before
     an outro.
3. **Better default samples, fetched on first use** (§8.1).
   - A multi-velocity, round-robin drum kit (MuldjordKit mixdown or a Virtuosity subset)
     replaces the 4-layer GM kit with no RR.
   - Splendid or Upright KW replaces the 17-sample GM piano.
   - Add the Wurlitzer for lofi and jazz, and real bass and guitar (FreePats CC0).
   - Embed credit strings in the render metadata.
   - Round robin alone removes the "machine-gun" hats and snares that DIAGNOSIS §5 notes.
4. **Air before every main hit.**
   - Generalise the drop's half-beat stop to all `hits`, idents and the button: 1 beat
     (0.5 beat at ≥ 140 BPM), plus 0.5–1 bar in trailers.
   - Add an optional "suck-back" reverse swell that ends on the hit frame.
5. **Lofi texture chain.** A per-style master insert:
   - LP 7–9 kHz with slow movement;
   - wow 0.15 % at 0.5–1 Hz and flutter 0.05 % at ≈ 6 Hz;
   - vinyl crackle bed ≈ −36 dBFS, lightly ducked by the kick;
   - 26 kHz / 12-bit colour on the drum bus;
   - saturation 100 Hz–10 kHz;
   - Wurli or Rhodes voiced with 9ths.
   - Today lofi only has swing, dark EQ and the keys choice.
6. **Groove engine per style and role.**
   - Ghost snares at velocity 20–40.
   - On-beat hat accents, timing humanise on hats only (±8 ms), kick and snare ≤ ±2 ms.
   - "Straight" pop and house at 54 % 16th swing (house now 52 %).
   - Trap 32nd and triplet rolls only at phrase ends.
   - DnB two-step with a ghost scatter.
   - **Jazz swing as a function of tempo** (≈ 75 % at 120 BPM, 67 % at 200, 59 % at 260; from
     Friberg's drummer data), with soloists straighter.
   - Fix the `Grid::swing` doc (0.667 = triplet).
7. **Stereo: get correlation into 0.3–0.7.**
   - Stereo samples (piano and kit overheads).
   - Double-tracked guitars hard L/R with different takes (seed + 10–20 ms offset).
   - A short room reverb on a separate bus from the hall.
   - Pads and returns widened.
   - Everything below 120 Hz summed to mono on the master.
   - Lofi, jazz and pop are still ≈ 0.95.
8. **Genre signature processing**, data-driven per style:
   - house: sidechain with 1–10 ms attack, release locked to tempo, 6–12 dB on sub and pads;
   - synthwave: gated snare (2–3 s reverb, 150–250 ms gate) and Juno-style chorus on pads;
   - trap: an 808 with pitch glide and release trimmed to the next note;
   - dnb: sub + Reese split;
   - ambient: 2–8 s attacks and 4–15 s tails;
   - cinematic: spiccato transient boost.
9. **Alt mixes and stems for editors.**
   - `alts: ["underscore","30","15","sting"]` renders an underscore (no lead or
     countermelody, mids thinned by 2–3 dB at 1–4 kHz *(heuristic)*), cut-downs with their
     own arcs, and a 3–6 s sting from the final notes.
   - Stems in Epidemic's split: drums, bass, instruments, melody. Ours today: drums, bass,
     music, fx.
   - Auto-select the underscore when the video has VO. The video mixer already ducks 12 dB;
     also enforce music ≥ 10 LU under speech (Torcoli).
10. **Trailer grammar for `cinematic-hybrid`** (30–60 s).
    - Act I (pulse and sustains) → Act II (ostinato subdivision 8ths → 16ths, ticking, riser
      timed to *end* on the hit) → silence → Act III (hits on cuts) → sting.
    - Energy rises through subdivision, not tempo.
    - Braam only when the mood is epic (TASTE).
11. **Tempo fitting across all hits and cuts.**
    - Search BPM within the style's range to minimise the error over every hit (as the
      Absil tool does), rather than nudging ±6–8 % for the first one.
    - Allow gentle tempo drift between distant hits when one tempo can't catch them all.
    - Never land early (ITU-R BT.1359).
12. **Delivery loudness presets.** `master: "web"` (−14 LUFS / −1 dBTP), `"broadcast-eu"`
    (−23 LUFS / −1 dBTP), `"broadcast-us"` (−24 LKFS / −2 dBTP), `"podcast"` (−16). The spec
    already has `lufs` and `ceiling`; add the named presets and check them in `review`.
13. **Sonic-logo generator.** A 3–5 note motif taken from the piece's own hook, 1–3 s, on a
    signature instrument with one hit. Reuse it as the sting so the ident and the bed share
    material (SOS: a sting is the tune's fragment).
14. **Drum fills that read as a player.** Small fills on bar 4, big fills on bar 8 (in
    DIAGNOSIS round 1), plus crash *chokes* before stops and a snare pickup into the button.

### Quick numbers to put in `style.rs`

| Style | swing (our scale → %) | sidechain | reverb send / decay | extra |
|---|---|---|---|---|
| lofi | 0.40–0.48 → 60–62 % | none or 1–2 dB from the kick on keys | room 0.6–0.9 s *(h)* | crackle, wow, LP 8 kHz |
| house | 0.16–0.24 → 54–56 % | 6–12 dB sub, 3–6 dB pads; 5 ms attack, release ≈ 60 000/BPM × 0.5 | plate 1.2 s, pre-delay 1/64 | offbeat open hat |
| synthwave | 0 | 3–4 dB on the arp | gated 2–3 s, gate 150–250 ms on the snare | chorus on the pad |
| trap | 0–0.08 (≤ 52 %) | 808 ducks under the kick | short | rolls at phrase ends |
| dnb | 0 | sub ducks under the kick | short room | 174 BPM default |
| jazz | tempo-dependent (≈ 74 % at 132) | none | room 1 s | brushes, walking bass |
| ambient | 0 | none | 4–15 s | attack 2–8 s |
| cinematic | 0 | none | hall 2–3 s *(h)* | silence before Act III |
