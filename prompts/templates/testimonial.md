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

## Example parameters

```json
{
  "headline": "Loved by 4,000 teams",
  "quotes": [
    {
      "quote": "We replaced a week of motion work with one prompt. The renders are faster than our preview used to be.",
      "author": "Maya Okafor",
      "role": "Head of Growth, Fieldnote",
      "rating": 5
    },
    {
      "quote": "It just does what the spec says. Every time.",
      "author": "Jonas Berg",
      "role": "Staff Engineer, Tiller"
    }
  ],
  "cta": "Join them",
  "url": "fieldnote.app",
  "music": "music.mp3"
}
```

## Make it

```bash
anikit-video new testimonial --brand brand.json            # scaffolds testimonial/params.json from this example
anikit-video templates expand testimonial testimonial/params.json --brand brand.json --format 16:9 -o testimonial/spec.json
anikit-video critique testimonial/spec.json && anikit-video sheet testimonial/spec.json -o testimonial/sheet.png
anikit-video render testimonial/spec.json -o testimonial.mp4
```

MCP: `create_from_template` with `{"template": "testimonial", "params": {…}, "brand": {…}, "format": "9:16"}`.

## Copy rules (all templates)

- The hook or first line is readable within 2 s: 3–7 words, never "Introducing…".
- Every limit above is a reading-time budget: about 3 words per second plus 1 s. Cut words; don't shrink type.
- Benefits with real numbers; no filler (seamless, revolutionary, unlock, leverage).
- Run `anikit-video critique` and fix every warning before rendering.
