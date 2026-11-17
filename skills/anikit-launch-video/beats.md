# Timing picture to music

There is no fixed tempo or slot table. Choose the music for the film you designed (`anikit-video docs music`), then
use these tools to make picture and sound move together.

## Beat maths

- One beat = `60 / bpm` s; one bar (4/4) = `240 / bpm` s. At 96 BPM a beat is 0.625 s; at 140 BPM, 0.429 s.
- A length in whole bars ends cleanly: `bars = length × bpm / 240` (rounded), then set `bpm` so it divides exactly,
  or let the score's `"end": "ring"` tail cover the remainder.
- `"@beatN"` / `"@barN"` (1-based) are the exact grid of the generated score. Use them for `at`, `start`, sfx and
  `hits`, instead of computing seconds.

## Tools

- **Hits:** `"music": {"hits": ["@bar5"]}` puts an impact and a section change there (the tempo is nudged ±8% so
  hits land on beats). Put a hit where your film's turn is, wherever that is.
- **Expressions:** `beat`, `bar` (fractional), `level` (loudness 0–1), `bpm`. A pulse:
  `"scale": "=1 + 0.04*(1 - fract(beat))"`; light on the kick: `"emissiveStrength": "=2 + 4*level"`.
- **Sfx:** `whoosh` on fast moves, `impact` on arrivals, `riser` peaking *at* the moment it builds to, `click`/`tick`
  on UI changes, `chime` on a resolve.

## Cutting

- **On the downbeat** (beat 1 of a bar): weight and arrival, for reveals, changes of world and the name.
- **On off-beats or the "and":** forward motion and syncopation; good for montages and type.
- **Against the beat:** tension; use it sparingly, then resolve onto a downbeat.
- **Holds:** a calm film can hold for bars, as long as something stays alive (camera drift, light, particles,
  parallax). Silence or a filtered drop before the turn makes it land harder.
- Vary shot lengths; a single repeated rhythm reads as a template. Let the music's sections shape the film's.
- For fast intents, `critique --render` and `pace.py` check the limits (`docs pacing`). For calm intents they check
  for dead air instead.
