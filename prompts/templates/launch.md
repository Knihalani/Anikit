---
name: template:launch
description: How to fill the 'launch' template well.
---

# Template guide: Product launch (`launch`)

A 20–35 s launch film: a 2-second hook, the product reveal (logo, name, tagline, optional hero screenshot), 2–4 feature beats, an optional proof number and a call to action. Paced by word count so every line is readable.

**Typical length:** 20–35 s. **Formats:** 16:9, 9:16 and 1:1 (pass `--format`); the layout adapts automatically.

## Parameters

| Param | Type | Required | Limits | What to write |
|---|---|---|---|---|
| `product` | string |  | ≤ 3 words | Product name as it should appear on screen. Default: the brand kit's `name`. |
| `hook` | string |  | ≤ 7 words | Optional opening line: a tension, a promise or a surprising fact, readable in 2 s. Without it the video opens on the product reveal. |
| `tagline` | string | yes | ≤ 10 words | What it is and why it matters, in one breath. |
| `screenshot` | image |  |  | Optional hero screenshot shown in a browser frame during the reveal (16:10 works best). |
| `url` | string |  | ≤ 32 chars | Address shown in the browser bar and on the end card. Default: the brand kit's `voice.url`. |
| `features` | array | yes | 2–4 items | The 2–4 strongest benefits, most important first. One idea each. |
| `features[].title` | string | yes | ≤ 5 words | Benefit, not feature: 'Renders 10× faster'. |
| `features[].body` | string |  | ≤ 16 words | One supporting sentence. |
| `features[].image` | image |  |  | Optional screenshot for this beat. |
| `stat` | object |  |  | Optional proof point shown as a big animated number. |
| `stat[].value` | number | yes |  |  |
| `stat[].prefix` | string |  | ≤ 3 chars |  |
| `stat[].suffix` | string |  | ≤ 4 chars |  |
| `stat[].label` | string | yes | ≤ 8 words |  |
| `cta` | string |  | ≤ 4 words | Call to action on the end card. Default: the brand kit's `voice.cta`. |
| `music` | string |  |  | Optional music file (MP3/WAV). Pick something with a clear beat around 100–124 BPM. |

## Example

```json
{"product":"Anikit Video","hook":"Video, without the browser.","tagline":"Programmatic video for AI agents, rendered natively.","url":"anikit.dev","features":[{"title":"10× faster renders","body":"Native Rust, every core, no headless browser."},{"title":"Built for agents","body":"One JSON spec, clear errors, an MCP server."}],"stat":{"value":935,"suffix":" fps","label":"rendered per second on a laptop"},"cta":"Start building"}
```

Create: MCP `create_from_template {template: "launch", params, brand, format}`, or `anikit-video templates expand launch params.json --brand brand.json -o spec.json`. Word limits are reading-time budgets: cut words, don't shrink type.
