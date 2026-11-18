# Rhythm: pictures driven by the music

Three levels, from coarse to exact. All are deterministic and read the same music the video plays.

1. **Cut on the grid:** `"cut": "bar"` or a scene `"duration": "2 beats"`; markers `"@beat17"`, `"@bar5+0.25"`.
2. **Continuous:** expressions read `beat` `bar` (fractional), `level` (loudness 0..1), `bpm`. A pulse on every beat: `"scale": "=1 + 0.15 * (1 - fract(beat)) ^ 3"`.
3. **Note-exact:** `onNotes` places a copy of a layer on each note of a generated `music` track.

```json
{ "music": { "style": "house", "bpm": 122 },
  "scenes": [ { "duration": 8, "layers": [
    { "onNotes": { "track": "drums", "keys": ["kick"],
      "layer": { "shape": "circle", "width": "=120 + 200 * $vel", "fill": "$accent", "duration": 0.4,
                 "animate": [ { "to": { "scale": 1.6, "opacity": 0 }, "duration": 0.4, "ease": "outCubic" } ] } } },
    { "onNotes": { "track": "lead",
      "layer": { "shape": "rect", "anchor": "left", "height": 10, "radius": 5, "fill": "#fff",
                 "x": "=200 + $t * 180", "y": "=900 - $pitch * 700", "width": "=max(8, $dur * 180)" } } } ] } ] }
```

- `track`: a track name or role (`drums bass chords pad lead arp …`); omit it for every note. `keys`: MIDI numbers or drum names (`kick snare hat ohh clap rim tom crash ride shaker`). `every: 2` keeps every other note.
- Per note: `$t` (video seconds), `$dur` (s), `$key` (MIDI), `$pitch` (0..1 across the notes this onNotes selects), `$vel` (0..1), `$beat` `$bar` (from the music's start), `$i` `$n`. Use them anywhere in the layer, inside expressions too.
- Timing: each copy starts on its note; a number `start` is an offset from it (`-0.05` anticipates). It lasts the note's length unless it sets `duration`, `end` or `frames`. A scene only gets the notes inside it.
- It works in a scene's `layers` and inside groups. Copies are ordinary layers: presets, expressions, `update` scripts, shaders all apply.

Avoid: binding every hi-hat to a big move (visual noise; pick one or two voices), and long `duration`s on dense tracks (copies pile up; fade them out).
Note: notes are on the grid (swing included, humanisation ignored). With `"auto"` scene lengths the binding uses the default scene length; give durations for exact sync.
