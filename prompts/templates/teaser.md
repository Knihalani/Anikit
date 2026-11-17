---
name: template:teaser
description: How to fill the 'teaser' template well.
---

# Template guide: Social teaser (9:16 first) (`teaser`)

A 6–12 s vertical teaser for Reels, TikTok and Shorts: one word per beat in huge uppercase type with camera punches, palette-flip frames in the brand colour, kinetic dot fields and metaballs, a beat counter, optional phrase captions, and a particle wordmark payoff.

**Typical length:** 6–12 s. **Formats:** 16:9, 9:16 and 1:1 (pass `--format`); the layout adapts automatically.

## Parameters

| Param | Type | Required | Limits | What to write |
|---|---|---|---|---|
| `words` | array | yes | 3–10 items | The beats, in order: one to three words each. Together they read as one sentence or a rhythm ('Faster.' 'Smaller.' 'Native.'). |
| `beat` | seconds |  |  | Seconds per one-word beat (two- and three-word beats get 1.5×). 0.6–0.7 feels energetic and stays readable; match it to the music (60 / BPM × 1 or × 2). Default: `0.6`. |
| `payoff` | string |  | ≤ 6 words | Line under the logo at the end: a date, a URL or a call to action. |
| `flash` | boolean |  |  | Flip every third beat to a full-bleed brand-colour frame (the palette flip) for energy. Default: `true`. |
| `music` | string |  |  | Optional music file; a strong beat is what makes a teaser work. |
| `captions` | any |  |  | Optional burned-in captions. true: one caption per phrase (beats grouped at . ! ? or every 4), each ending before the next starts. Or a list of {text, at, end} cues in seconds (e.g. for a voice-over), or {"src": "subs.srt"}. The beat words are already on screen, so captions matter most with a voice-over. |

## Example

```json
{"words":["No browser.","No waiting.","Just","JSON","in.","Video","out."],"beat":0.6,"payoff":"Out now · anikit.dev","flash":true}
```

Create: MCP `create_from_template {template: "teaser", params, brand, format}`, or `anikit-video templates expand teaser params.json --brand brand.json -o spec.json`. Word limits are reading-time budgets: cut words, don't shrink type.
