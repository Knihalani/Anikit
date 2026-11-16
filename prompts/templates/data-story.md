---
name: template:data-story
description: How to fill the 'data-story' template well.
---

# Template guide: Data story (`data-story`)

A 15–30 s data narrative: a headline hook, 1–3 bar charts that grow in with counting value labels (the key bar highlighted), an optional big counter stat and a one-line takeaway with its source.

**Typical length:** 15–30 s. **Formats:** 16:9, 9:16 and 1:1 (pass `--format`); the layout adapts automatically.

## Parameters

| Param | Type | Required | Limits | What to write |
|---|---|---|---|---|
| `kicker` | string |  | ≤ 4 words | Small label above the headline: the dataset or period. |
| `headline` | string | yes | ≤ 8 words | The finding, stated as news: 'Renders got 10× faster'. |
| `charts` | array | yes | 1–3 items | 1–3 bar charts, each making one point. |
| `charts[].title` | string | yes | ≤ 7 words | What the chart shows, ideally its conclusion. |
| `charts[].prefix` | string |  | ≤ 3 chars | Before each value, e.g. '$'. |
| `charts[].suffix` | string |  | ≤ 5 chars | After each value, e.g. '%' or ' fps'. |
| `charts[].highlight` | integer |  |  | Index of the bar to highlight. Default: the largest. |
| `charts[].bars` | array | yes | 2–6 items |  |
| `stat` | object |  |  | Optional big number after the charts. |
| `stat[].value` | number | yes |  |  |
| `stat[].prefix` | string |  | ≤ 3 chars |  |
| `stat[].suffix` | string |  | ≤ 5 chars |  |
| `stat[].label` | string | yes | ≤ 8 words |  |
| `takeaway` | string | yes | ≤ 12 words | The one sentence to remember. |
| `source` | string |  | ≤ 60 chars | Data source, shown small on the last scene. |
| `music` | string |  |  | Optional music file (MP3/WAV). |

## Example parameters

```json
{
  "kicker": "2026 in numbers",
  "headline": "Rendering got 10× faster this year",
  "charts": [
    {
      "title": "Frames per second at 1080p",
      "suffix": "",
      "bars": [
        {
          "label": "Browser",
          "value": 24
        },
        {
          "label": "Canvas",
          "value": 61
        },
        {
          "label": "Anikit",
          "value": 250
        }
      ]
    }
  ],
  "stat": {
    "value": 10,
    "suffix": "×",
    "label": "faster than a headless browser"
  },
  "takeaway": "No browser in the loop means minutes become seconds.",
  "source": "Source: internal benchmarks, 4-core laptop"
}
```

## Make it

```bash
anikit-video new data-story --brand brand.json            # scaffolds data-story/params.json from this example
anikit-video templates expand data-story data-story/params.json --brand brand.json --format 16:9 -o data-story/spec.json
anikit-video critique data-story/spec.json && anikit-video sheet data-story/spec.json -o data-story/sheet.png
anikit-video render data-story/spec.json -o data-story.mp4
```

MCP: `create_from_template` with `{"template": "data-story", "params": {…}, "brand": {…}, "format": "9:16"}`.

## Copy rules (all templates)

- The hook or first line is readable within 2 s: 3–7 words, never "Introducing…".
- Every limit above is a reading-time budget: about 3 words per second plus 1 s. Cut words; don't shrink type.
- Benefits with real numbers; no filler (seamless, revolutionary, unlock, leverage).
- Run `anikit-video critique` and fix every warning before rendering.
