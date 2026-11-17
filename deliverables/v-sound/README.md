# Sound design: before/after on three existing films

These films were re-scored with `"sound"` (`anikit-video docs sound`): one key per film, and no hand-placed cues.

- **before**: the same film and the same generated music, rendered with no sound design (`sound mix … --sound none`).
- **after**: sound design added. The picture is unchanged; the `-after.mp4` files are the original deliverable video with the new audio.

| film | concept | `sound` | cues |
|---|---|---|---|
| Railyard (div-b) | payments as trains in a signal box | `{"palette":"organic","ambience":{"type":"room","level":-4},"space":"chamber"}` | 12 |
| Calibre 9 (div-f) | a guilloche watch dial | `{"palette":"luxury","ambience":"studio","space":"plate"}` | 10 |
| Offset 2027 (div-g) | a Swiss-grid design conference | `{"palette":"clean","density":0.45,"space":"studio"}` | 7 |

`cues.txt` lists every cue with its time and the reason it was placed, for example `scenes[2].layers[72]: lands`, or the camera pull-back in Railyard heard as a single `whoosh-air`. The spectrograms put "before" over "after". In them you can see:

- the whoosh arcs peaking on the fastest frames;
- landings on the contact frames;
- the key-tuned glass accent on Calibre's "72 h" reveal, and the alternating swish and air on Railyard's run of moves.

## Measured

Sources:
- Loudness and true peak: `sound mix` report, cross-checked with ffmpeg `ebur128`.
- Speed: `sound mix` and the bus profile.

- **Loudness:** every "after" mix is **−14.0 ± 0.2 LUFS integrated** with **−1.0 dBTP** true peak in the WAV. The OGG encodes overshoot by up to 0.6 dB, which is why platforms ask for −1 dBTP.
- **Cue planning** (display lists only, no pixels): Railyard 26 ms, Calibre 3 ms, Offset 2 ms.
- **Mix bus** (cues, shared space, ducking, bed, master): 170–270 ms for 12–15 s of audio. Measured on its own on 4 cores, a 15 s mix with 15 cues and a city bed runs at about 55× real time.
- **In the real render** the audio job runs on a thread alongside the frames. The music render is MUSIC's code, so `sound mix` as a whole runs at 17–34× real time.
- **Deterministic:** the same spec gives the same samples (tested).

## Files

- `*-before.ogg`, `*-after.ogg`: the soundtracks.
- `*-after.mp4`: the film with sound design. The video is re-encoded at CRF 22 to keep the repo small.
- `*-spectrogram.png`: before (top) and after (bottom).
- `cues.txt`: the cue lists.
