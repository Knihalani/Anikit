---
name: template:demo
description: How to fill the 'demo' template well.
---

# Template guide: Product demo walkthrough (`demo`)

A 15–35 s walkthrough: a kinetic title beside the product, then 2–5 steps, each a screenshot in a browser (or phone) with an animated cursor that glides to the click point, clicks with a ripple and shows a tooltip callout, then a CTA with confetti.

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

## Example

```json
{"product":"Anikit Studio","title":"From prompt to video in 3 clicks","device":"browser","url":"studio.anikit.dev","steps":[{"image":"step1.png","title":"Pick a template","callout":"Launch","x":0.3,"y":0.4},{"image":"step2.png","title":"Drop in your brand kit","x":0.7,"y":0.6},{"image":"step3.png","title":"Render in seconds","callout":"Render","x":0.82,"y":0.18}],"cta":"Try the demo"}
```

Create: MCP `create_from_template {template: "demo", params, brand, format}`, or `anikit-video templates expand demo params.json --brand brand.json -o spec.json`. Word limits are reading-time budgets: cut words, don't shrink type.

## Tips

- A template gives every product the same structure, which is what the user rejected: use it for a fast draft, then make it this product's (a palette from its own assets, a typeface chosen for it, its real UI or object as the hero, a score whose instruments fit it). For an original film, use the `director` prompt or skills/anikit-launch-video. Run the taste pass first: `anikit-video docs taste`.
- Hold every line for max(0.83 s, chars/15, words × 0.3) after it settles, and keep words crisp (no blur or glow while they are read). Cut words, never type size.
- Show the product's real screenshots. A generated screen is a placeholder.
