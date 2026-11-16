---
name: template:testimonial
description: How to fill the 'testimonial' template well.
---

# Template guide: Testimonial / quote (`testimonial`)

1–3 customer quotes, each with a big accent quote mark, the words fading up in reading rhythm, the author (optional circular avatar, role and company) and an optional star rating, then a logo end card. Paced by word count.

**Typical length:** 8–30 s. **Formats:** 16:9, 9:16 and 1:1 (pass `--format`); the layout adapts automatically.

## Parameters

| Param | Type | Required | Limits | What to write |
|---|---|---|---|---|
| `headline` | string |  | ≤ 6 words | Optional opening line before the quotes (the hook). Leave it out to open on the first quote. |
| `quotes` | array | yes | 1–3 items | 1–3 quotes, strongest first. Cut each to the sentence that carries the emotion. |
| `quotes[].quote` | string | yes | ≤ 30 words | The quote, without quotation marks. Under 20 words reads best. |
| `quotes[].author` | string | yes | ≤ 4 words | Person's name. |
| `quotes[].role` | string |  | ≤ 7 words | Role and company, e.g. 'Head of Design, Linearly'. |
| `quotes[].avatar` | image |  |  | Optional square headshot; shown in a circle. |
| `quotes[].rating` | integer |  |  | Optional star rating (1–5). |
| `cta` | string |  | ≤ 4 words | Call to action on the end card. Default: the brand kit's `voice.cta`. |
| `url` | string |  | ≤ 32 chars | Address on the end card. Default: the brand kit's `voice.url`. |
| `music` | string |  |  | Optional music file. Warm, mid-tempo (80–110 BPM) suits testimonials. |

## Example

```json
{"headline":"Loved by 4,000 teams","quotes":[{"quote":"We replaced a week of motion work with one prompt. The renders are faster than our preview used to be.","author":"Maya Okafor","role":"Head of Growth, Fieldnote","rating":5},{"quote":"It just does what the spec says. Every time.","author":"Jonas Berg","role":"Staff Engineer, Tiller"}],"cta":"Join them","url":"fieldnote.app"}
```

Create: MCP `create_from_template {template: "testimonial", params, brand, format}`, or `anikit-video templates expand testimonial params.json --brand brand.json -o spec.json`. Word limits are reading-time budgets: cut words, don't shrink type.
