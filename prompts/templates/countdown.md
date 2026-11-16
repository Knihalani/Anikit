---
name: template:countdown
description: How to fill the 'countdown' template well.
---

# Template guide: Event countdown (`countdown`)

An event intro (name, date, place), a big N…1 countdown where each number pops in inside a ring that draws itself over the second, then a 'Live now' or save-the-date end card.

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

## Example parameters

```json
{
  "event": "Launch Week Day 1",
  "kicker": "Live stream",
  "date": "Thu 12 Dec · 17:00 UTC",
  "location": "youtube.com/@anikit",
  "from": 5,
  "live": true,
  "liveText": "We're live",
  "cta": "Join the stream",
  "music": "music.mp3"
}
```

## Make it

```bash
anikit-video new countdown --brand brand.json            # scaffolds countdown/params.json from this example
anikit-video templates expand countdown countdown/params.json --brand brand.json --format 16:9 -o countdown/spec.json
anikit-video critique countdown/spec.json && anikit-video sheet countdown/spec.json -o countdown/sheet.png
anikit-video render countdown/spec.json -o countdown.mp4
```

MCP: `create_from_template` with `{"template": "countdown", "params": {…}, "brand": {…}, "format": "9:16"}`.

## Copy rules (all templates)

- The hook or first line is readable within 2 s: 3–7 words, never "Introducing…".
- Every limit above is a reading-time budget: about 3 words per second plus 1 s. Cut words; don't shrink type.
- Benefits with real numbers; no filler (seamless, revolutionary, unlock, leverage).
- Run `anikit-video critique` and fix every warning before rendering.
