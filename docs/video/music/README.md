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
- **`fx`** is a per-track effect chain. Available types: eq, comp, delay (tempo-synced `note`), reverb (wet/dry insert), chorus, saturate, widener, transient, filter and gain.
- **`automate`** changes values over time, as keyframes in beats or `@bar`/`@beat` times, or as `"=expr"` written in `beat`, `bar` and `t`. It can drive two kinds of target:
  - track parameters (volume, pan, swept low-pass and high-pass filters, reverb and delay sends), applied continuously;
  - patch parameters of a custom synth (`synth.cutoff`, `synth.res`, `synth.drive`, `synth.env`, `synth.vib`, …), applied at each note-on.

Tests: `tests/timing.rs::custom_instruments_fx_and_automation`, which covers a sampler zone from a WAV file, determinism, a cutoff sweep that measurably brightens the line, and the error messages.

## Listening-proxy metrics (all 15 styles)

Produced by `cargo run --release -p anikit-music --example demos -- out/`. Two caveats:
nobody has listened to these renders yet, and metrics cannot judge musicality.

| style | bpm | LUFS | dBTP | clipped | LRA (LU) | bands sub / low / low-mid / high-mid / air (dB re total) | L/R corr | kick onset error, mean / max (ms) | render |
|---|---|---|---|---|---|---|---|---|---|
| lofi | 84.0 | -14.08 | -1.02 | 0 % | 5.9 | -6 / -4 / -5 / -24 / -31 | 0.93 | 0.9 / 1.8 | 0.91 s (27×) |
| house | 124.0 | -14.01 | -1.02 | 0 % | 4.2 | -2 / -5 / -10 / -19 / -17 | 0.92 | 0.9 / 1.3 | 0.96 s (25×) |
| cinematic | 100.0 | -14.00 | -1.02 | 0 % | 7.9 | -8 / -3 / -6 / -19 / -19 | 0.91 | 14.6 / 27.2 | 0.97 s (25×) |
| orchestral | 92.0 | -14.00 | -1.02 | 0 % | 4.3 | -19 / -5 / -2 / -18 / -30 | 0.88 | – | 0.48 s (50×) |
| synthwave | 104.0 | -14.00 | -1.02 | 0 % | 5.9 | -3 / -5 / -10 / -17 / -17 | 0.95 | 0.5 / 1.5 | 0.74 s (33×) |
| trap | 140.0 | -14.07 | -1.02 | 0 % | 10.2 | -1 / -7 / -16 / -17 / -18 | 0.98 | 3.9 / 19.9 | 0.66 s (30×) |
| ambient | 70.0 | -14.00 | -1.02 | 0 % | 2.9 | -12 / -5 / -2 / -20 / -31 | 0.66 | – | 0.57 s (42×) |
| rock | 128.0 | -14.01 | -1.02 | 0 % | 5.8 | -20 / -3 / -4 / -11 / -21 | 0.88 | 6.0 / 25.6 | 0.38 s (52×) |
| corporate | 116.0 | -14.01 | -1.02 | 0 % | 4.7 | -11 / -3 / -5 / -12 / -13 | 0.92 | 1.5 / 2.7 | 0.44 s (46×) |
| jazz | 132.0 | -14.00 | -1.02 | 0 % | 3.2 | -16 / -3 / -4 / -13 / -23 | 0.98 | 2.1 / 21.5 | 0.33 s (61×) |
| piano | 76.0 | -14.01 | -1.00 | 0 % | 1.5 | -30 / -7 / -1 / -22 / -50 | 0.90 | – | 0.30 s (80×) |
| acoustic | 96.0 | -14.00 | -1.02 | 0 % | 2.6 | -12 / -5 / -4 / -11 / -9 | 0.88 | 0.3 / 1.5 | 0.33 s (61×) |
| pop | 112.0 | -14.01 | -1.02 | 0 % | 6.4 | -4 / -5 / -6 / -17 / -16 | 0.97 | 1.9 / 24.5 | 0.60 s (34×) |
| tech | 120.0 | -14.01 | -1.02 | 0 % | 6.0 | -4 / -4 / -9 / -16 / -16 | 0.93 | 0.7 / 1.3 | 0.61 s (33×) |
| dnb | 172.0 | -14.05 | -1.02 | 0 % | 11.3 | -3 / -5 / -11 / -17 / -15 | 0.80 | 2.9 / 12.8 | 0.68 s (29×) |

- **Loudness and peak.** Every style is within ±0.1 LU of −14 LUFS and at or below −1.00 dBTP, with no clipped samples.
  - The meter matches EBU Tech 3341 cases 1–5 within 0.02 LU and the true-peak cases exactly.
  - Loudness range matches EBU Tech 3342 cases 1–4 exactly.
- **Timing.** The sequencer is sample-accurate.
  - On an isolated click track with no humanisation, onsets on the fitted grid are within the detector's 0.5 ms resolution (`tests/timing.rs`).
  - In full mixes, kick onsets read 0.3–2 ms late (the attack shape plus up to ≈0.4 ms of humanisation).
  - The larger maxima for cinematic, trap, rock, pop and jazz come from the detector, not from timing. In those kits the kick sits under slow-attack booms, toms or 808 tails, so the steepest energy rise near the beat belongs to another drum.
- **Spectral balance.**
  - Electronic styles carry the most sub energy: trap −1.5 dB, house −2.4 dB.
  - Acoustic and orchestral styles centre on the low-mids.
  - Lofi and piano are deliberately dark, with air around −30 to −50 dB.

## Speed

Measured on the 4-vCPU cloud container (2.1 GHz Xeon), release build, end to end
from spec to mastered audio:

- **15 demos, all in one process:** 25–80× real time.
- **A fresh CLI run** (`anikit-video music --style house --length 20s`): about 0.8 s, roughly 25×.
- **A 2-minute house track:** about 3.7 s (≈33×). That time splits into voices 1.5 s, mix 1.0 s and master 1.1 s.

**The 100× goal is not met yet.** The mastering chain is serial:

- glue compressor
- true-peak limiter, run 2 iterations
- loudness meter

It costs ≈1 s per 2 minutes. The synth voices cost 500–1000× real time each.

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
  - Optional packs: `anikit-video music fetch essentials` (≈210 MB: Splendid grand, Wurlitzer/CP80/Pianet, FreePats CC0 bass, nylon and clean electric guitars). Once cached, they replace the GM samples for piano, bass, pick-bass, nylon-guitar and clean-guitar automatically. Bigger packs: `fetch salamander`, `fetch vsco2-ce`.
  - Physical and modal models: Karplus-Strong guitars, harp and koto; modal marimba, vibraphone, bells, kalimba and handpan.
  - Synths and drum machines.
  - See `LICENSES.md` for every licence.
- **Composition.**
  - Theory: keys, modes, chord symbols and Roman numerals, voice leading.
  - The note language and drum grids.
  - Arrangement helpers: play modes, basslines, a motif-based melody generator.
  - 15 data-driven styles, with sections and energy, fills, crashes, risers and impacts.
- **Mix.**
  - Every track is loudness-matched to its role's level, so balance does not depend on where an instrument came from (sampled, model or synth).
  - Per-track processing: EQ, compressor, drive, width and pan, plus a sidechain pump from the kicks.
  - Reverb and delay buses; SFX are placed by their hit point.
  - Master: low cut and tilt, glue compressor, gain, then the true-peak limiter, iterated until it reaches target.
  - Stems: drums, bass, music and fx. They carry the master gain and sum to the mix.
