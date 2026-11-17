# Music and sound

Generated, instrument-based score (no files needed; MUSIC's engine, `anikit-video docs music` shows the full page when installed):

```json
{ "music": { "style": "synthwave", "bpm": 120, "key": "Am", "mood": "driving", "hits": ["@bar5"] },
  "scenes": [ { "duration": "@bar2", "layers": [ { "text": "ONE", "size": "20vh" } ] },
              { "duration": "@bar2", "transition": "cut", "layers": [ { "text": "TWO", "size": "20vh" } ] } ] }
```

- `"music": "lofi"` is enough; styles: `anikit-video music styles`. It fills the video, is mastered to −14 LUFS, ducks under voice-over, and its exact beat grid drives `"@beat17"`, `"@bar5+0.25"` everywhere a time goes.
- Your own track instead: `"audio": [ { "src": "track.mp3", "volume": 0.8, "fadeOut": 1 } ]`; `anikit-video beats track.mp3` prints its grid.
- SFX on any beat: `{ "sfx": "whoosh"|"impact"|"riser"|"click"|"pop"|"ding", "at": "@beat9" }` in `audio`.
- Don't hand-write synthesisers: use `music` and cut on `@beat` markers.
