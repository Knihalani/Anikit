---
name: template:data-story
description: How to fill the 'data-story' template well.
---

# Template guide: Data story (`data-story`)

A 15–30 s data narrative on a living brand mesh: a kinetic headline over a rising area chart, 1–3 animated bar charts beside their conclusions (the key bar highlighted), a big counting stat, and a takeaway with a sparkline stat tile and its source.

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

## Example

```json
{"kicker":"2026 in numbers","headline":"Rendering got 10× faster this year","charts":[{"title":"Frames per second at 1080p","suffix":"","bars":[{"label":"Browser","value":24},{"label":"Canvas","value":61},{"label":"Anikit","value":250}]}],"stat":{"value":10,"suffix":"×","label":"faster than a headless browser"},"takeaway":"No browser in the loop means minutes become seconds.","source":"Source: internal benchmarks, 4-core laptop"}
```

Create: MCP `create_from_template {template: "data-story", params, brand, format}`, or `anikit-video templates expand data-story params.json --brand brand.json -o spec.json`. Word limits are reading-time budgets: cut words, don't shrink type.
