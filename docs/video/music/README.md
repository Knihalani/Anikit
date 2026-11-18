# Anikit Music — generated soundtracks

`crates/anikit-music` is the music engine behind the video spec's `"music"` key and
`anikit-video music`. Agent cheat sheet: `anikit-video docs music`.

```json
{"music": {"style": "cinematic-hybrid", "key": "Dm", "hits": ["@bar5"]}}
```

Every render is mastered to **−14 LUFS integrated, ≤ −1 dBTP**. It is deterministic:
the same spec and `seed` give bit-identical audio. `@beatN` / `@barN` markers in the
video resolve to the music's exact beat grid, and the tempo is nudged by up to ±8 % so
that `hits` land on beats.

## Demos

These are 20–24 s Vorbis renders at q2. Each comes from a one-line spec, with
`"length":"24s"` and the `seed` shown in `crates/anikit-music/examples/demos.rs`.

| file | spec |
|---|---|
| [lofi.ogg](lofi.ogg) | `{"style":"lofi","bpm":84,"key":"Dm"}` |
| [house.ogg](house.ogg) | `{"style":"house","bpm":124,"key":"Am","hits":["@7.74"]}` (the drop lands on the hit) |
| [cinematic.ogg](cinematic.ogg) | `{"style":"cinematic-hybrid","key":"Dm","hits":["@9.6"]}` (riser, braam and impact at 9.6 s) |
| [orchestral.ogg](orchestral.ogg) | `{"style":"orchestral","key":"D"}` |
| [synthwave.ogg](synthwave.ogg) | `{"style":"synthwave","key":"Am"}` |
| [corporate.ogg](corporate.ogg) | `{"style":"corporate"}` |

## Make anything: custom instruments, effects, automation

Agents are not limited to the presets. [custom.json](custom.json) (rendered as [custom.ogg](custom.ogg))
defines its own instruments, gives each track its own effect chain, and automates parameters over time:

- **`define`** names custom instruments:
  - `{"synth": {…}}`: up to three oscillators (saw, square, tri, sine, supersaw; unison voices and detune), sub, noise, filter, amp and filter envelopes, LFO, pitch envelope and glide. A definition can start from any synth preset via `base`.
  - `{"sampler": {"zones": […]}}`: your own audio files with key range, root note, velocity layers, loop points, tuning and round robin.
- **`fx`** is a per-track effect chain. Available types: eq, comp, delay (tempo-synced `note`), reverb (wet/dry insert), chorus, saturate, widener, transient, filter, gain, autopan (a stereo tremolo: `rate` in Hz or `note` such as "1/8", `depth` 0–1) and gated-reverb (the 80s snare: `preset`, `wet`, `hold` and `release` in seconds; the tail is cut `hold` after each hit).
- **`automate`** changes values over time, as keyframes in beats or `@bar`/`@beat` times, or as `"=expr"` written in `beat`, `bar` and `t`. It can drive two kinds of target:
  - track parameters (volume, pan, swept low-pass and high-pass filters, reverb and delay sends), applied continuously;
  - patch parameters of a custom synth (`synth.cutoff`, `synth.res`, `synth.drive`, `synth.env`, `synth.vib`, …), applied at each note-on.

Tests: `tests/timing.rs::custom_instruments_fx_and_automation`, which covers a sampler zone from a WAV file, determinism, a cutoff sweep that measurably brightens the line, and the error messages.

## Listening-proxy metrics (all 15 styles)

Produced by `cargo run --release -p anikit-music --example demos -- out/` with the bundled bank only (no packs fetched, reverb cache warm); regenerated after the overnight changes. Two caveats:
nobody has listened to these renders yet, and metrics cannot judge musicality.

| style | bpm | LUFS | dBTP | clipped | LRA (LU) | bands sub / low / low-mid / high-mid / air (dB re total) | L/R corr | kick onset error, mean abs / max (ms) | render |
|---|---|---|---|---|---|---|---|---|---|
| lofi | 84.0 | -14.02 | -1.02 | 0 % | 3.3 | -6 / -3 / -6 / -19 / -28 | 0.93 | 3.2 / 29.9 | 0.66 s (37×) |
| house | 124.0 | -14.01 | -1.02 | 0 % | 4.6 | -2 / -6 / -10 / -19 / -17 | 0.89 | 0.9 / 1.5 | 0.69 s (35×) |
| cinematic | 100.0 | -14.00 | -1.02 | 0 % | 7.7 | -9 / -2 / -6 / -19 / -19 | 0.83 | 11.7 / 27.7 | 0.62 s (39×) |
| orchestral | 92.0 | -13.99 | -1.02 | 0 % | 5.1 | -10 / -5 / -2 / -19 / -29 | 0.82 | – | 0.45 s (53×) |
| synthwave | 104.0 | -14.00 | -1.02 | 0 % | 5.7 | -3 / -5 / -9 / -16 / -17 | 0.91 | 0.6 / 1.0 | 0.53 s (45×) |
| trap | 140.0 | -14.01 | -1.02 | 0 % | 6.3 | -1 / -7 / -15 / -18 / -19 | 0.96 | 4.6 / 26.3 | 0.39 s (52×) |
| ambient | 70.0 | -14.00 | -1.42 | 0 % | 2.1 | -16 / -4 / -2 / -20 / -29 | 0.42 | – | 0.51 s (47×) |
| rock | 128.0 | -14.00 | -1.02 | 0 % | 4.7 | -18 / -4 / -3 / -10 / -21 | 0.85 | 6.7 / 25.9 | 0.32 s (62×) |
| corporate | 116.0 | -14.04 | -1.02 | 0 % | 5.9 | -13 / -3 / -5 / -12 / -14 | 0.89 | 1.6 / 2.7 | 0.53 s (38×) |
| jazz | 132.0 | -13.99 | -1.02 | 0 % | 2.2 | -12 / -3 / -5 / -13 / -23 | 0.95 | 2.9 / 27.2 | 0.30 s (66×) |
| piano | 76.0 | -14.00 | -1.02 | 0 % | 6.0 | -29 / -6 / -1 / -21 / -32 | 0.75 | – | 0.26 s (93×) |
| acoustic | 96.0 | -14.02 | -1.02 | 0 % | 6.6 | -11 / -4 / -5 / -11 / -10 | 0.88 | 0.3 / 0.8 | 0.34 s (59×) |
| pop | 112.0 | -14.06 | -1.02 | 0 % | 7.6 | -8 / -3 / -6 / -12 / -15 | 0.94 | 1.4 / 17.6 | 0.36 s (55×) |
| tech | 120.0 | -14.01 | -1.02 | 0 % | 4.6 | -3 / -4 / -9 / -18 / -18 | 0.90 | 0.8 / 1.3 | 0.37 s (54×) |
| dnb | 172.0 | -14.04 | -1.02 | 0 % | 6.3 | -3 / -4 / -10 / -17 / -15 | 0.78 | 2.6 / 12.7 | 0.49 s (41×) |

- **Loudness and peak.** Every style is within ±0.1 LU of −14 LUFS and at or below −1.00 dBTP (ambient peaks lower, −1.42), with no clipped samples.
  - The meter matches EBU Tech 3341 cases 1–5 within 0.02 LU and the true-peak cases exactly.
  - Loudness range matches EBU Tech 3342 cases 1–4 exactly.
- **Timing.** The sequencer is sample-accurate.
  - On an isolated click track with no humanisation, onsets on the fitted grid are within the detector's 0.5 ms resolution (`tests/timing.rs`).
  - In full mixes, kick onsets read 0.3–3 ms from the beat on average (the attack shape plus humanisation of up to ±0.7 ms).
  - The larger maxima for lofi, cinematic, trap, rock, pop and jazz come from the detector, not from timing. In those kits the kick sits under slow-attack booms, toms, 808 tails, ghost snares or the room, so the steepest energy rise near the beat belongs to another drum.
- **Spectral balance.**
  - Electronic styles carry the most sub energy: trap −1.4 dB, house −2.2 dB.
  - Acoustic and orchestral styles centre on the low-mids.
  - Lofi and piano are deliberately dark, with air around −30 to −50 dB.

## Speed

Measured on the 4-vCPU cloud container (2.1 GHz Xeon), release build, end to end
from spec to mastered audio:

- **15 demos, all in one process:** 35–93× real time.
- **A fresh CLI run** (`anikit-video music --style house --length 20s`): about 0.66 s, roughly 30×.
- **A 2-minute house track:** about 3.6 s (≈33×). That time splits into voices 1.8 s, mix 1.0 s and master 0.8 s.

**The 100× goal is not met yet.** The mastering chain is serial:

- glue compressor
- true-peak limiter, run 2 iterations
- loudness meter

It costs ≈0.8 s per 2 minutes. The synth voices cost 500–1000× real time each.

What already went in to get here:

- the reverb bus and SFX tails use the reverb's impulse response, cached per preset, with FFT convolution;
- the limiter's 4× true-peak detector is computed once, in parallel chunks, and rescaled each iteration;
- the sidechain envelope is shared across tracks;
- worker threads run with FTZ/DAZ, and the loudness filter flushes subnormals (this fixed a 5× slowdown on parts with rests);
- voices are created in parallel;
- render groups are cut by voice time.

Audio renders on its own thread while the video frames render (`render_ak264_job`). For
typical videos the soundtrack is therefore finished before the frames are, and adds no wall time.

Per-component speed (single thread, × real time):

- **Sampler:** about 300–370× for a 16-voice piano part.
- **Physical and modal models:** 1,600–4,000× per voice.
- **Synth pads:** 660–810× per voice.
- **DSP:** EQ ≈1,000×; FDN reverb 80–130×; convolution with a 3 s IR about 570×; full `master()` 62× for 60 s.

## How it fits together

- **Instruments.**
  - Sampled: the bundled GeneralUser GS General MIDI bank, re-encoded as compact Vorbis SF3 (5.9 MB). SF2, SF3 and SFZ files also load.
  - Optional packs: `anikit-video music fetch essentials` (≈210 MB: Splendid grand, Wurlitzer/CP80/Pianet, FreePats CC0 bass, nylon and clean electric guitars). Once cached, they replace the GM samples for piano, bass, pick-bass, nylon-guitar and clean-guitar automatically. Groups: `fetch orchestra` (vsco-strings + vsco-winds, ≈930 MB) and `fetch all`. Bigger packs one by one: `fetch salamander`, `fetch vsco2-ce`, `fetch vsco-strings` (≈410 MB download: real string sections, which replace the GM strings for strings, pizzicato, tremolo and spiccato once cached, each register played by its own section from contrabass to violins), `fetch vsco-winds` (≈520 MB: horn, trumpet, trombone, tuba with staccatos, flute, piccolo, oboe, clarinet, bassoon, harp, timpani; "brass" becomes a section from tuba to trumpets), and `fetch virtuosity` (≈250 MB download: a real acoustic kit, stereo mid-room mic, velocity layers and round robins, which replaces the GM kit for standard, room, rock, power, pop and lofi kits once cached).
  - Physical and modal models: Karplus-Strong guitars, harp and koto; modal marimba, vibraphone, bells, kalimba and handpan.
  - Synths and drum machines.
  - See `LICENSES.md` for every licence.
- **Composition.**
  - Theory: keys, modes, chord symbols and Roman numerals, voice leading.
  - The note language and drum grids.
  - Arrangement helpers: play modes, basslines, a motif-based melody generator.
  - 15 data-driven styles, with sections and energy, fills, crashes, risers and impacts. Any explicit section can take `"stop": beats` for silence right before it (the trailer hole, a false ending), in any style.
  - Long cinematic-hybrid cues (45 s or more) get a three-act trailer form: a build with a pulse that speeds up, a climax that stops dead, a false ending, then act III.
- **Mix.**
  - Every track is loudness-matched to its role's level, so balance does not depend on where an instrument came from (sampled, model or synth).
  - Per-track processing: EQ, compressor, drive, width and pan, plus a sidechain pump from the kicks.
  - Reverb and delay buses; SFX are placed by their hit point.
  - Master: low cut and tilt, glue compressor, gain, then the true-peak limiter, iterated until it reaches target.
  - Stems: drums, bass, music and fx. They carry the master gain and sum to the mix.
  - Alternate mixes for editors (`--alts DIR`): `<name>-underscore` (no melody, for voice-over), `<name>-no-drums`, and 30 s and 15 s cutdowns that are recomposed with their own endings rather than truncated.
