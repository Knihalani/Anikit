---
name: template:logo-reveal
description: How to fill the 'logo-reveal' template well.
---

# Template guide: Logo reveal / intro (`logo-reveal`)

A restrained 3–6 s brand sting: the logo (or the name as a wordmark) revealed in one of four styles (glow, draw, assemble, minimal), with a slow camera settle and an optional tagline. Use it as an intro or outro for other videos.

**Typical length:** 3–6 s. **Formats:** 16:9, 9:16 and 1:1 (pass `--format`); the layout adapts automatically.

## Parameters

| Param | Type | Required | Limits | What to write |
|---|---|---|---|---|
| `style` | enum |  | glow / draw / assemble / minimal | glow: light bloom and a passing streak. draw: lines draw on and frame the mark. assemble: pieces converge (letters for a wordmark). minimal: a clean fade with an accent underline. Default: `"glow"`. |
| `name` | string |  | ≤ 3 words | Wordmark text, used when the brand kit has no logo. Default: the brand kit's `name`. |
| `tagline` | string |  | ≤ 8 words | Optional line under the logo. |
| `duration` | seconds |  |  | Total length in seconds. Default: `4`. |
| `music` | string |  |  | Optional sting or whoosh sound. |

## Example parameters

```json
{
  "style": "glow",
  "name": "Anikit",
  "tagline": "Motion, compiled.",
  "duration": 4,
  "music": "sting.wav"
}
```

## Make it

```bash
anikit-video new logo-reveal --brand brand.json            # scaffolds logo-reveal/params.json from this example
anikit-video templates expand logo-reveal logo-reveal/params.json --brand brand.json --format 16:9 -o logo-reveal/spec.json
anikit-video critique logo-reveal/spec.json && anikit-video sheet logo-reveal/spec.json -o logo-reveal/sheet.png
anikit-video render logo-reveal/spec.json -o logo-reveal.mp4
```

MCP: `create_from_template` with `{"template": "logo-reveal", "params": {…}, "brand": {…}, "format": "9:16"}`.

## Copy rules (all templates)

- The hook or first line is readable within 2 s: 3–7 words, never "Introducing…".
- Every limit above is a reading-time budget: about 3 words per second plus 1 s. Cut words; don't shrink type.
- Benefits with real numbers; no filler (seamless, revolutionary, unlock, leverage).
- Run `anikit-video critique` and fix every warning before rendering.
