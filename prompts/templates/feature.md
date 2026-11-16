---
name: template:feature
description: How to fill the 'feature' template well.
---

# Template guide: Feature announcement (`feature`)

A 12–20 s spotlight on one new feature: a 'New' kicker and headline as the hook, an optional screenshot in a browser frame, 1–3 supporting points that build in, and a call to action.

**Typical length:** 12–20 s. **Formats:** 16:9, 9:16 and 1:1 (pass `--format`); the layout adapts automatically.

## Parameters

| Param | Type | Required | Limits | What to write |
|---|---|---|---|---|
| `product` | string |  | ≤ 3 words | Product name shown on the end card. Default: the brand kit's `name`. |
| `kicker` | string |  | ≤ 3 words | Small label above the headline. Default: `"New"`. |
| `headline` | string | yes | ≤ 7 words | The feature as a benefit, readable in 2 s. |
| `subhead` | string |  | ≤ 14 words | One sentence that explains the headline. |
| `screenshot` | image |  |  | Optional screenshot of the feature (16:10 works best), shown in a browser frame. |
| `url` | string |  | ≤ 32 chars | Address in the browser bar and on the end card. Default: the brand kit's `voice.url`. |
| `points` | array |  | 1–3 items | 1–3 short supporting points, most important first. |
| `cta` | string |  | ≤ 4 words | Call to action on the end card. Default: the brand kit's `voice.cta`. |
| `music` | string |  |  | Optional music file (MP3/WAV). |

## Example parameters

```json
{
  "product": "Anikit Video",
  "kicker": "New in 2.0",
  "headline": "Render stills in 40 ms",
  "subhead": "Preview any frame instantly, straight from your agent.",
  "screenshot": "screenshot.png",
  "url": "anikit.dev",
  "points": [
    "Any frame, any time",
    "PNG or base64 for agents",
    "Contact sheets in one call"
  ],
  "cta": "Try it now"
}
```

## Make it

```bash
anikit-video new feature --brand brand.json            # scaffolds feature/params.json from this example
anikit-video templates expand feature feature/params.json --brand brand.json --format 16:9 -o feature/spec.json
anikit-video critique feature/spec.json && anikit-video preview feature/spec.json -o feature/sheet.png
anikit-video render feature/spec.json -o feature.mp4
```

MCP: `create_from_template` with `{"template": "feature", "params": {…}, "brand": {…}, "format": "9:16"}`.

## Copy rules (all templates)

- The hook or first line is readable within 2 s: 3–7 words, never "Introducing…".
- Every limit above is a reading-time budget: about 3 words per second plus 1 s. Cut words; don't shrink type.
- Benefits with real numbers; no filler (seamless, revolutionary, unlock, leverage).
- Run `anikit-video critique` and fix every warning before rendering.
