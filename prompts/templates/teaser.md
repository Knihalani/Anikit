---
name: template:teaser
description: How to fill the 'teaser' template well.
---

# Template guide: Social teaser (9:16 first) (`teaser`)

A 6–12 s vertical teaser for Reels, TikTok and Shorts: rapid kinetic words, one per beat, in huge type with punch-in camera hits and colour flashes, ending on the logo and a payoff line. Also works in 16:9 and 1:1.

**Typical length:** 6–12 s. **Formats:** 16:9, 9:16 and 1:1 (pass `--format`); the layout adapts automatically.

## Parameters

| Param | Type | Required | Limits | What to write |
|---|---|---|---|---|
| `words` | array | yes | 3–10 items | The beats, in order: one to three words each. Together they read as one sentence or a rhythm ('Faster.' 'Smaller.' 'Native.'). |
| `beat` | seconds |  |  | Seconds per one-word beat (two- and three-word beats get 1.5×). 0.5–0.65 feels energetic; match it to the music (60 / BPM × 1 or × 2). Default: `0.6`. |
| `payoff` | string |  | ≤ 6 words | Line under the logo at the end: a date, a URL or a call to action. |
| `flash` | boolean |  |  | Flash every third beat to the brand colour for energy. Default: `true`. |
| `music` | string |  |  | Optional music file; a strong beat is what makes a teaser work. |

## Example parameters

```json
{
  "words": [
    "No browser.",
    "No waiting.",
    "Just",
    "JSON",
    "in.",
    "Video",
    "out."
  ],
  "beat": 0.6,
  "payoff": "Out now · anikit.dev",
  "flash": true
}
```

## Make it

```bash
anikit-video new teaser --brand brand.json            # scaffolds teaser/params.json from this example
anikit-video templates expand teaser teaser/params.json --brand brand.json --format 16:9 -o teaser/spec.json
anikit-video critique teaser/spec.json && anikit-video preview teaser/spec.json -o teaser/sheet.png
anikit-video render teaser/spec.json -o teaser.mp4
```

MCP: `create_from_template` with `{"template": "teaser", "params": {…}, "brand": {…}, "format": "9:16"}`.

## Copy rules (all templates)

- The hook or first line is readable within 2 s: 3–7 words, never "Introducing…".
- Every limit above is a reading-time budget: about 3 words per second plus 1 s. Cut words; don't shrink type.
- Benefits with real numbers; no filler (seamless, revolutionary, unlock, leverage).
- Run `anikit-video critique` and fix every warning before rendering.
