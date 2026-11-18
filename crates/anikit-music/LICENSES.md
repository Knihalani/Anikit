# Third-party content in anikit-music

<!-- sampler sections (src/sf2): maintained with the sampler -->

## Bundled: GeneralUser GS (General MIDI SoundFont)

- **File:** `assets/GeneralUser-GS.sf3` (5.9 MB), embedded in the binary by `src/sf2/bank.rs`.
- **Work:** GeneralUser GS **v2.0.3** (bank name `GeneralUser GS 2.0.3 BETA`), 287 presets
  (GM/GS melodic banks, 13 drum kits).
- **Author:** S. Christian Collins — <http://www.schristiancollins.com>
  (please link to the website, not to the author's download files, as the licence asks).
- **Licence:** *GeneralUser GS License v2.0*. Summary: the bank may be used without
  restriction for music creation, private or commercial; it may be used in software
  projects, and the bank and its packaging may be modified to suit. The samples carry
  the usage rights of their sources, all of which allow full use in music production
  including commercial recordings. The author notes that, as a long-running personal
  project, the provenance of some samples cannot be verified with certainty, though
  none came from commercial sample libraries and no ownership complaint has been
  received since 2000. Full text: `documentation/LICENSE.txt` of the GeneralUser GS
  distribution.
- **What we changed:** re-encoded to SF3 with `examples/build_sf3.rs`: every sample
  (mono; stereo pairs stay linked mono samples) encoded with libvorbis at quality 6.5,
  stored as *compact Vorbis records* (the Vorbis packets without Ogg framing or comment
  header; the three distinct setup headers are stored once). `ifil` set to 3.0,
  `ISFT` set to the converter, `sm24` dropped (the source has none). Presets, instruments,
  generators, modulators and sample headers are otherwise unchanged. Measured against
  the original 16-bit PCM: overall SNR 27.2 dB, median per-sample SNR 33.6 dB,
  mean log-spectral distance 2.6 dB (bins within 60 dB of the frame peak).
  To rebuild: `cargo run --release -p anikit-music --example build_sf3 -- GeneralUser-GS.sf2 assets/GeneralUser-GS.sf3 --quality 6.5 --measure`
  (needs `ffmpeg` with libvorbis at build time only; decoding is pure Rust via symphonia).

## Optional packs (downloaded on request by `src/sf2/packs.rs`; not committed)

### VSCO-2 Community Edition (`vsco2-ce`)

- **Source:** <https://github.com/sgossner/VSCO-2-CE> (branch `SFZ`), "VS Chamber
  Orchestra: Community Edition", recorded by Sam Gossner & Simon Dalzell, sample cutting
  by Elan Hickler / Soundemote. More information: <http://vis.versilstudios.net/vsco-community.html>
- **Licence:** CC0 1.0 Universal (verified from the repository's `LICENSE` file). No
  attribution required (crediting the authors is still courteous).
- **Contents:** ≈3,200 WAV samples with SFZ mappings (strings sections and solo violin,
  woodwinds, brass, harp, timpani, mallets, organ, upright piano, GM-style percussion).

### VSCO-2 CE string sections (`vsco-strings`)

- The same repository, licence and authors as `vsco2-ce` above, as a sparse checkout of only the
  string sections (violins, violas, cellos, contrabass) and the SFZ files (≈410 MB download).

### Salamander Grand Piano V3 (`salamander`)

- **Source:** <https://github.com/sfzinstruments/SalamanderGrandPiano> — Yamaha C5,
  recorded at 48 kHz/24-bit, 16 velocity layers, FLAC; SFZ v2/ARIA mapping by kinwie
  (retuned variant by Markus Fiedler; the default "Natural" key switch is used).
  Original samples: <https://archive.org/details/SalamanderGrandPianoV3>
- **Author:** Alexander Holm.
- **Licence:** Creative Commons Attribution 3.0 Unported (CC BY 3.0), verified from the
  repository's `LICENSE` and `README.md`.
- **Required credit** for anything published with it (e.g. a rendered video's credits or
  description): *"Salamander Grand Piano V3 by Alexander Holm, licensed under CC BY 3.0
  (https://creativecommons.org/licenses/by/3.0/)."* The SFZ repository additionally asks
  that derived SFZ files note they are derived from its mapping. The credit string is
  available at run time as `packs::PACKS[..].credit`.

### Splendid Grand Piano (`splendid`)

- **Source:** <https://github.com/sfzinstruments/SplendidGrandPiano>, Akai Steinway samples
  (4 velocity layers) with an SFZ mapping and FLAC encoding by the sfzinstruments project.
- **Licence:** public domain (released by Akai in 2000), verified from the repository's `README.md`.

### Greg Sullivan E-Pianos (`epianos`)

- **Source:** <https://github.com/sfzinstruments/GregSullivan.E-Pianos>: Wurlitzer EP200, Yamaha CP80
  and Hohner Pianet T, with SFZ mapping by kinwie.
- **Licence:** CC BY 3.0. **Required credit:** *"E-Pianos by Greg Sullivan (CC BY 3.0), SFZ mapping
  by kinwie"* (available as `packs::PACKS[..].credit`).

### FreePats packs (`freepats-bass`, `freepats-nylon`, `freepats-eguitar`)

- **Sources** (pinned tags):
  - <https://github.com/freepats/electric-bass-YR> (`2019-09-30`, Yamaha RBX finger and picked
    bass by Andrea Biasior);
  - <https://github.com/freepats/spanish-classical-guitar> (`v1.0.0`, by roberto@zenvoid.org);
  - <https://github.com/freepats/electric-guitar-FSBS-clean> (`2026-08-07`; only the small bridge
    set is checked out, ≈6 MB).
- **Licence:** CC0 1.0 Universal for all three, verified from each repository's `LICENSE`. No
  attribution required.

### Virtuosity Drums (`virtuosity`)

- **Source:** <https://github.com/sfzinstruments/virtuosity_drums> (tag `v0.925`), by Versilian
  Studios. Only the programs and the mid-mic samples are checked out (≈250 MB). After download, a copy of
  `06-mid-mic.sfz` is written as `anikit-mid-kit.sfz` that includes the default keymap (open hi-hat on
  GM key 46).
- **Licence:** CC0 1.0 Universal, verified from the repository's `LICENSE`. No attribution required.

<!-- end of sampler sections -->

## Everything else: original work (MIT, like the rest of Anikit)

- **Physical models, synths and drum machines** (`src/physical`, `src/synth`): all sound is
  synthesised at run time from code; no samples or third-party presets.
- **Sound effects** (`src/sfx.rs`): synthesised (filtered noise, oscillators, FM, the
  engine's own reverb); no samples.
- **Reverb impulse responses**: rendered at run time from the engine's own algorithmic
  reverb and cached in the music cache directory.
- **Demo renders** in `docs/video/music/*.ogg`: rendered by this engine from the specs in
  `docs/video/music/README.md`, using the bundled GeneralUser GS bank and the models above.
  They are covered by the GeneralUser GS licence (free use, including commercial) plus MIT.
