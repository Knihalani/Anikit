---
name: template:changelog
description: How to fill the 'changelog' template well.
---

# Template guide: Changelog / what's new (`changelog`)

A 12–30 s release recap: the version badge and headline beside the product's app screen, then one beat per change with a solid NEW / IMPROVED / FIXED chip, the change in big type and a progress ring counting through the release, then a version end card with a CTA and a sparks burst.

**Typical length:** 12–30 s. **Formats:** 16:9, 9:16 and 1:1 (pass `--format`); the layout adapts automatically.

## Parameters

| Param | Type | Required | Limits | What to write |
|---|---|---|---|---|
| `product` | string |  | ≤ 3 words | Product name. Default: the brand kit's `name`. |
| `version` | string | yes | ≤ 12 chars | Version shown in the badge, e.g. 'v2.4' or '2026.11'. |
| `title` | string |  | ≤ 4 words | Headline above the version badge. Default: `"What's new"`. |
| `date` | string |  | ≤ 24 chars | Optional release date. |
| `items` | array | yes | 3–6 items | The changes, most exciting first: a short line each, plus an optional detail. |
| `items[].type` | enum | yes | new / improved / fixed | Kind of change; sets the pill colour. |
| `items[].text` | string | yes | ≤ 8 words, ≤ 34 chars | The change as a benefit. Short: it sits in a card. |
| `items[].body` | string |  | ≤ 14 words, ≤ 90 chars | Optional one-line detail under the change. |
| `cta` | string |  | ≤ 4 words | Call to action on the end card. Default: `"Update now"`. |
| `url` | string |  | ≤ 32 chars | Where to read the full notes. Default: the brand kit's `voice.url`. |
| `music` | string |  |  | Optional music file (MP3/WAV). |

## Example

```json
{"product":"Anikit","version":"v2.4","title":"What's new","date":"November 2026","items":[{"type":"new","text":"Vertical video templates","body":"Export 9:16 and 1:1 cuts from the same project."},{"type":"new","text":"MCP server for agents"},{"type":"improved","text":"Faster still previews"},{"type":"fixed","text":"Emoji in captions"}],"cta":"Update now","url":"anikit.dev/changelog"}
```

Create: MCP `create_from_template {template: "changelog", params, brand, format}`, or `anikit-video templates expand changelog params.json --brand brand.json -o spec.json`. Word limits are reading-time budgets: cut words, don't shrink type.

## Tips

- A template gives every product the same structure, which is what the user rejected: use it for a fast draft, then make it this product's (a palette from its own assets, a typeface chosen for it, its real UI or object as the hero, a score whose instruments fit it). For an original film, use the `director` prompt or skills/anikit-launch-video. Run the taste pass first: `anikit-video docs taste`.
- Hold every line for max(0.83 s, chars/15, words × 0.3) after it settles, and keep words crisp (no blur or glow while they are read). Cut words, never type size.
- Use the release's real wording and name the user-facing effect of each change, not the ticket title.
