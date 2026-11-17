---
name: template:logo-reveal
description: How to fill the 'logo-reveal' template well.
---

# Template guide: Logo reveal / intro (`logo-reveal`)

A 3–6 s brand sting: a chrome 3D wordmark (or your logo) revealed with god rays and a lens flare (glow), dropping in with sparks (draw), assembled from particles (assemble), or matte and calm (minimal), plus an optional tagline.

**Typical length:** 3–6 s. **Formats:** 16:9, 9:16 and 1:1 (pass `--format`); the layout adapts automatically.

## Parameters

| Param | Type | Required | Limits | What to write |
|---|---|---|---|---|
| `style` | enum |  | glow / draw / assemble / minimal | glow: light bloom and a passing streak. draw: lines draw on and frame the mark. assemble: pieces converge (letters for a wordmark). minimal: a clean fade with an accent underline. Default: `"glow"`. |
| `name` | string |  | ≤ 3 words | Wordmark text, used when the brand kit has no logo. Default: the brand kit's `name`. |
| `tagline` | string |  | ≤ 8 words | Optional line under the logo. |
| `duration` | seconds |  |  | Total length in seconds. Default: `4`. |
| `music` | string |  |  | Optional sting or whoosh sound. |

## Example

```json
{"style":"glow","name":"Anikit","tagline":"Motion, compiled.","duration":4}
```

Create: MCP `create_from_template {template: "logo-reveal", params, brand, format}`, or `anikit-video templates expand logo-reveal params.json --brand brand.json -o spec.json`. Word limits are reading-time budgets: cut words, don't shrink type.

## Tips

- A template gives every product the same structure, so used as-is it tends to look like every other video made from it. It works well as a fast draft that you then make this product's own: a palette from its assets, a typeface chosen for it, its real UI or object as the hero, a score whose instruments fit it. For an original film, see the `director` prompt or skills/anikit-launch-video, and `anikit-video docs taste`.
- Lines read comfortably when held for max(0.83 s, chars/15, words × 0.3) after they settle, sharp, with no blur or glow while read. If a line doesn't fit, cutting words usually works better than shrinking type.
- Chrome with god rays and a lens flare tends to read as the default 3D sting. `minimal`, or a reveal built from the brand's own shape, usually feels more specific, unless the brand really is chrome.
