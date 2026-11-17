---
name: template:changelog
description: How to fill the 'changelog' template well.
---

# Template guide: Changelog / what's new (`changelog`)

A 12–25 s release recap: a version badge and kinetic title beside the product UI, then the changes as icon cards (New ✦, Improved ⚡, Fixed ✓) building in three at a time, then a CTA with confetti.

**Typical length:** 12–25 s. **Formats:** 16:9, 9:16 and 1:1 (pass `--format`); the layout adapts automatically.

## Parameters

| Param | Type | Required | Limits | What to write |
|---|---|---|---|---|
| `product` | string |  | ≤ 3 words | Product name. Default: the brand kit's `name`. |
| `version` | string | yes | ≤ 12 chars | Version shown in the badge, e.g. 'v2.4' or '2026.11'. |
| `title` | string |  | ≤ 4 words | Headline above the version badge. Default: `"What's new"`. |
| `date` | string |  | ≤ 24 chars | Optional release date. |
| `items` | array | yes | 3–6 items | The changes, most exciting first. Each is one short line. |
| `items[].type` | enum | yes | new / improved / fixed | Kind of change; sets the pill colour. |
| `items[].text` | string | yes | ≤ 8 words | The change as a benefit. |
| `cta` | string |  | ≤ 4 words | Call to action on the end card. Default: `"Update now"`. |
| `url` | string |  | ≤ 32 chars | Where to read the full notes. Default: the brand kit's `voice.url`. |
| `music` | string |  |  | Optional music file (MP3/WAV). |

## Example

```json
{"product":"Anikit","version":"v2.4","title":"What's new","date":"November 2026","items":[{"type":"new","text":"Vertical video templates"},{"type":"new","text":"MCP server for agents"},{"type":"improved","text":"Faster still previews"},{"type":"fixed","text":"Emoji in captions"}],"cta":"Update now","url":"anikit.dev/changelog"}
```

Create: MCP `create_from_template {template: "changelog", params, brand, format}`, or `anikit-video templates expand changelog params.json --brand brand.json -o spec.json`. Word limits are reading-time budgets: cut words, don't shrink type.
