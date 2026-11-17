---
name: template:feature
description: How to fill the 'feature' template well.
---

# Template guide: Feature announcement (`feature`)

A 12–20 s spotlight on one new feature: a 'New' badge and kinetic headline beside the product in a browser (your screenshot or a generated UI), the points checking off next to it with a success toast, then a CTA that bursts into confetti.

**Typical length:** 12–20 s. **Formats:** 16:9, 9:16 and 1:1 (pass `--format`); the layout adapts automatically.

## Parameters

| Param | Type | Required | Limits | What to write |
|---|---|---|---|---|
| `product` | string |  | ≤ 3 words | Product name shown on the end card. Default: the brand kit's `name`. |
| `kicker` | string |  | ≤ 3 words | Small label above the headline. Default: `"New"`. |
| `headline` | string | yes | ≤ 7 words | The feature as a benefit, readable in 2 s. |
| `subhead` | string |  | ≤ 14 words | One sentence that explains the headline. |
| `screenshot` | image |  |  | Optional screenshot (16:10). Without it, a generated product UI in the brand colours is shown. |
| `url` | string |  | ≤ 32 chars | Address in the browser bar and on the end card. Default: the brand kit's `voice.url`. |
| `points` | array |  | 1–3 items | 1–3 short supporting points, most important first. |
| `cta` | string |  | ≤ 4 words | Call to action on the end card. Default: the brand kit's `voice.cta`. |
| `music` | string |  |  | Optional music file (MP3/WAV). |

## Example

```json
{"product":"Anikit Video","kicker":"New in 2.0","headline":"Render stills in 40 ms","subhead":"Preview any frame instantly, straight from your agent.","screenshot":"screenshot.png","url":"anikit.dev","points":["Any frame, any time","PNG or base64 for agents","Contact sheets in one call"],"cta":"Try it now"}
```

Create: MCP `create_from_template {template: "feature", params, brand, format}`, or `anikit-video templates expand feature params.json --brand brand.json -o spec.json`. Word limits are reading-time budgets: cut words, don't shrink type.
