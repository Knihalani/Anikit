# Sound design: one key, synced to the picture, mastered

`"sound": "auto"` designs the film's sound from what moves on screen. It runs at more than 50× real time, gives the same output every run, and needs no sample files.

- **Moves:** a whoosh that peaks on the fastest frame. Speed and size set its length, brightness and weight, and it pans with the travel.
- **Hard stops and spring overshoots:** a landing on the contact frame (thump, wood or glass by palette; an impact for the hero).
- **Pop-ins and reveals:** pops, plus soft air on wipes and masks.
- **Text:** a tick when text changes; counters get one ticker.
- **Physics (`scene3d`):** each collision clacks in its body's material (metal, glass, ceramic, wood, stone), panned through the camera, and is heard in slow motion too.
- **Transitions:** a swish on slide, push, wipe, zoom and glitch.
- **The bus:** pans each sound to its place on screen (±0.6), shares one space, dips the music 2–5 dB under hits (starting 30 ms early), adds an optional ambience bed, and masters to **−14 LUFS / −1 dBTP**.
- **Restraint:** a density budget lets the most salient events win; 30–50 % of motion stays silent. Camera moves count once, and steady loops make no sound.

```json
{ "sound": { "palette": "organic", "ambience": "room" },
  "scenes": [{ "duration": 3, "layers": [
    { "shape": "rect", "width": 300, "height": 180, "fill": "#f80", "x": 1200, "y": 540, "sfx": { "land": "wood" },
      "animate": [{ "from": { "x": -300 }, "duration": 0.6, "ease": "outBack" }] },
    { "text": "Settled.", "y": 800, "enter": "fadeUp" } ] }] }
```

**`sound` keys:**

- `design`: `auto`, or `off` for explicit cues, beds and the bus only.
- `palette`: `auto` `clean` `cinematic` `organic` `digital` `luxury` `playful`. `auto` follows the music style.
- `density`: 0..1, default 0.5. At 0.3 only hero moves make a sound.
- `ambience`: `room` `studio` `city` `nature` `wind` `ocean` `rain` `night` `office` `cafe` `space`, or `{"type":"rain","level":-6}`.
- `space`: `auto` `dry` `room` `studio` `chamber` `plate` `hall` `cathedral` `outdoor`.
- `loudness`: −14 by default; use −16 for Apple, −23 for broadcast, or `"off"`.
- `truePeak` −1 · `duck` true · `pan` true · `level` (dB) 0 · `transitions` true · `seed` 1.
- `cues`: explicit hits, e.g. `[{"sfx":"glass","at":"@beat9","x":0.7,"gain":-3,"duck":3}]`.

Shorthand: `"sound": "organic"` (a palette name) or `"sound": "off"`. Tonal sounds snap to `music.key`. Repeats vary by ±1 semitone and ±1.5 dB.

**Per layer, `sfx`:** `"auto"` sounds the layer even when `design` is off, and `false` never sounds it. A name sets the sound for its main event. `{"in":"swish","land":"wood","out":"air","gain":-3}` sets each event.

**Sounds:** motion `air` `swish` `whoosh` `whoosh-air` `whoosh-deep` `passby` `swoosh` `whip` · builds `riser` `downer` `reverse-cymbal` `swell` `braam` `sub-drop` `tape-stop` · hits `impact` `boom` `hit` `thump` `stone` `shatter` · foley (modal, tuned) `glass` `metal` `ping` `wood` `plastic` `ceramic` `paper` `page` `cloth` `droplet` · UI `click` `tick` `pop` `blip` `ticker` `ding` `chime` `typing` `shutter` `snap` `zap` `glitch` · magic `sparkle` `magic` · beds: the ambience names.

**Parameters:** `duration`, `pitch` (semitones), `intensity`, `size` (the sound's own tail), `motion`, `speed`, `tone`, `count`, `seed`.

**Layering:** `"impact+glass"` stacks sounds aligned on their hits.

**CC0 packs** are fetched on first use (public domain, no credit needed):

- `kenney-interface/` `click|tick|select|toggle|confirmation|error|glass|glitch|drop|open|close|scroll|pluck`
- `cc0-foley/` `glass|metal|wooden|paper|dishes|door|slam|hit|spring|splash|plop|bell|gong`
- `kenney-switches/` `switch|click|rollover`

**CLI:** `anikit-video sound plan s.json [--sound auto]` lists the cues (time, pan, reason) in milliseconds · `sound mix s.json -o a.wav` renders the soundtrack only · `sound render glass pitch=3` renders one sound · `sound list|packs|fetch <pack>`.

**Taste** (advice; sources in `docs/video/craft/sound-design.md`):

- Restraint reads as premium. Aim for one hero hit every 4–8 s and at most 2–3 onsets a second; `density` 0.3–0.5 suits brand films.
- Transients land on the contact frame; audio more than 45 ms early reads as wrong.
- Use one space that matches the picture: `room`/`studio` for product and UI, `chamber` for warm work, `hall` for cinematic.
- Beds sit far under everything.
- Avoid a whoosh on every move, trailer hits under a calm film, and anything louder than −14 LUFS: platforms turn it down anyway.
