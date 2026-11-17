---
name: template:countdown
description: How to fill the 'countdown' template well.
---

# Template guide: Event countdown (`countdown`)

An event countdown: kicker badge, event, date and place beside a spinning globe with live arcs; a big N…1 countdown with glowing rings that draw each second and a spark burst at zero; then a particle-text 'Live now' with confetti, or a save-the-date card.

**Typical length:** 8–18 s. **Formats:** 16:9, 9:16 and 1:1 (pass `--format`); the layout adapts automatically.

## Parameters

| Param | Type | Required | Limits | What to write |
|---|---|---|---|---|
| `event` | string | yes | ≤ 6 words | Event name. |
| `kicker` | string |  | ≤ 3 words | Small label above the event name. Default: `"Coming up"`. |
| `date` | string |  | ≤ 32 chars | Date and time as it should read, e.g. 'Thu 12 Dec · 17:00 UTC'. |
| `location` | string |  | ≤ 40 chars | Venue or where to watch. |
| `from` | integer |  |  | Number to count down from (one second each). Default: `5`. |
| `live` | boolean |  |  | true: end on 'Live now' (for the start of a stream). false: end on a save-the-date card. Default: `true`. |
| `liveText` | string |  | ≤ 3 words | End card headline when live is true. Default: `"Live now"`. |
| `cta` | string |  | ≤ 4 words | Optional button on the end card. |
| `music` | string |  |  | Optional music; a track with a clear downbeat makes each number land. |

## Example

```json
{"event":"Launch Week Day 1","kicker":"Live stream","date":"Thu 12 Dec · 17:00 UTC","location":"youtube.com/@anikit","from":5,"live":true,"liveText":"We're live","cta":"Join the stream"}
```

Create: MCP `create_from_template {template: "countdown", params, brand, format}`, or `anikit-video templates expand countdown params.json --brand brand.json -o spec.json`. Word limits are reading-time budgets: cut words, don't shrink type.

## Tips

- A template gives every product the same structure, which is what the user rejected: use it for a fast draft, then make it this product's (a palette from its own assets, a typeface chosen for it, its real UI or object as the hero, a score whose instruments fit it). For an original film, use the `director` prompt or skills/anikit-launch-video. Run the taste pass first: `anikit-video docs taste`.
- Hold every line for max(0.83 s, chars/15, words × 0.3) after it settles, and keep words crisp (no blur or glow while they are read). Cut words, never type size.
- Use the globe and particles only when the event really is global. Otherwise, setting the countdown in type is stronger.
