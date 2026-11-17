---
name: template:testimonial
description: How to fill the 'testimonial' template well.
---

# Template guide: Testimonial / quote (`testimonial`)

1–3 customer quotes, each revealed word by word with stars and the author beside a large avatar with floating reactions; an optional 'loved by' hook with an avatar stack and a scrolling company strip; a gradient CTA end card with confetti.

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
| `companies` | array |  |  | Optional customer or company names for a scrolling logo strip on the hook and end card. |
| `product` | string |  | ≤ 3 words | Name on the end card. Default: the brand kit's `name`. |

## Example

```json
{"headline":"Loved by product teams","quotes":[{"quote":"We replaced a week of motion work with one prompt.","author":"Maya Okafor","role":"Head of Growth, Fieldnote","rating":5},{"quote":"It just does what the spec says. Every time.","author":"Jonas Berg","role":"Staff Engineer, Tiller"}],"cta":"Join them","url":"fieldnote.app","companies":["Fieldnote","Tiller","Northwind","Lumen","Arcade"],"product":"Anikit Video"}
```

Create: MCP `create_from_template {template: "testimonial", params, brand, format}`, or `anikit-video templates expand testimonial params.json --brand brand.json -o spec.json`. Word limits are reading-time budgets: cut words, don't shrink type.

## Tips

- A template gives every product the same structure, which is what the user rejected: use it for a fast draft, then make it this product's (a palette from its own assets, a typeface chosen for it, its real UI or object as the hero, a score whose instruments fit it). For an original film, use the `director` prompt or skills/anikit-launch-video. Run the taste pass first: `anikit-video docs taste`.
- Hold every line for max(0.83 s, chars/15, words × 0.3) after it settles, and keep words crisp (no blur or glow while they are read). Cut words, never type size.
- Use real quotes, real names and the person's real photo, or leave the avatar out.
