---
name: template:changelog
description: How to fill the 'changelog' template well.
---

# Template guide: Changelog / what's new (`changelog`)

A 12–25 s release recap: a version badge as the hook, then 3–6 changes building in as a list (three per scene) with colour-coded New / Improved / Fixed pills, then an end card.

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

## Example parameters

```json
{
  "product": "Anikit",
  "version": "v2.4",
  "title": "What's new",
  "date": "November 2026",
  "items": [
    {
      "type": "new",
      "text": "Vertical video templates"
    },
    {
      "type": "new",
      "text": "MCP server for agents"
    },
    {
      "type": "improved",
      "text": "Stills render in 40 ms"
    },
    {
      "type": "fixed",
      "text": "Emoji in captions"
    }
  ],
  "cta": "Update now",
  "url": "anikit.dev/changelog"
}
```

## Make it

```bash
anikit-video new changelog --brand brand.json            # scaffolds changelog/params.json from this example
anikit-video templates expand changelog changelog/params.json --brand brand.json --format 16:9 -o changelog/spec.json
anikit-video critique changelog/spec.json && anikit-video preview changelog/spec.json -o changelog/sheet.png
anikit-video render changelog/spec.json -o changelog.mp4
```

MCP: `create_from_template` with `{"template": "changelog", "params": {…}, "brand": {…}, "format": "9:16"}`.

## Copy rules (all templates)

- The hook or first line is readable within 2 s: 3–7 words, never "Introducing…".
- Every limit above is a reading-time budget: about 3 words per second plus 1 s. Cut words; don't shrink type.
- Benefits with real numbers; no filler (seamless, revolutionary, unlock, leverage).
- Run `anikit-video critique` and fix every warning before rendering.
