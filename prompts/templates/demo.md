---
name: template:demo
description: How to fill the 'demo' template well.
---

# Template guide: Product demo walkthrough (`demo`)

A 15–35 s walkthrough: a title hook, then 2–5 steps, each a screenshot in a browser window (or a phone) with an animated cursor that glides to the click point, a click pulse and a callout, then a call to action.

**Typical length:** 15–35 s. **Formats:** 16:9, 9:16 and 1:1 (pass `--format`); the layout adapts automatically.

## Parameters

| Param | Type | Required | Limits | What to write |
|---|---|---|---|---|
| `product` | string |  | ≤ 3 words | Product name. Default: the brand kit's `name`. |
| `title` | string | yes | ≤ 7 words | Opening line: what the viewer will see done. |
| `device` | enum |  | browser / phone | Frame for the screenshots: a browser window (16:10 shots) or a phone (9:19.5 shots). Default: `"browser"`. |
| `url` | string |  | ≤ 32 chars | Address shown in the browser bar and on the end card. Default: the brand kit's `voice.url`. |
| `steps` | array | yes | 2–5 items | The steps in order. Each shows one screenshot and where to click. |
| `steps[].image` | image | yes |  | Screenshot for this step. |
| `steps[].title` | string | yes | ≤ 6 words | What happens in this step, as an action: 'Pick a template'. |
| `steps[].callout` | string |  | ≤ 3 words | Tiny label next to the cursor: 'Click here'. |
| `steps[].x` | number |  |  | Click point, 0 (left) .. 1 (right) of the screenshot. Default 0.5. |
| `steps[].y` | number |  |  | Click point, 0 (top) .. 1 (bottom) of the screenshot. Default 0.5. |
| `cta` | string |  | ≤ 4 words | Call to action on the end card. Default: the brand kit's `voice.cta`. |
| `music` | string |  |  | Optional music file (MP3/WAV). |

## Example parameters

```json
{
  "product": "Anikit Studio",
  "title": "From prompt to video in 3 clicks",
  "device": "browser",
  "url": "studio.anikit.dev",
  "steps": [
    {
      "image": "step1.png",
      "title": "Pick a template",
      "callout": "Launch",
      "x": 0.3,
      "y": 0.4
    },
    {
      "image": "step2.png",
      "title": "Drop in your brand kit",
      "x": 0.7,
      "y": 0.6
    },
    {
      "image": "step3.png",
      "title": "Render in seconds",
      "callout": "Render",
      "x": 0.82,
      "y": 0.18
    }
  ],
  "cta": "Try the demo"
}
```

## Make it

```bash
anikit-video new demo --brand brand.json            # scaffolds demo/params.json from this example
anikit-video templates expand demo demo/params.json --brand brand.json --format 16:9 -o demo/spec.json
anikit-video critique demo/spec.json && anikit-video sheet demo/spec.json -o demo/sheet.png
anikit-video render demo/spec.json -o demo.mp4
```

MCP: `create_from_template` with `{"template": "demo", "params": {…}, "brand": {…}, "format": "9:16"}`.

## Copy rules (all templates)

- The hook or first line is readable within 2 s: 3–7 words, never "Introducing…".
- Every limit above is a reading-time budget: about 3 words per second plus 1 s. Cut words; don't shrink type.
- Benefits with real numbers; no filler (seamless, revolutionary, unlock, leverage).
- Run `anikit-video critique` and fix every warning before rendering.
