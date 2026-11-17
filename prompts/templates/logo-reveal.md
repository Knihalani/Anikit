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

- A template gives every product the same structure, which is what the user rejected: use it for a fast draft, then make it this product's (a palette from its own assets, a typeface chosen for it, its real UI or object as the hero, a score whose instruments fit it). For an original film, use the `director` prompt or skills/anikit-launch-video. Run the taste pass first: `anikit-video docs taste`.
- Hold every line for max(0.83 s, chars/15, words × 0.3) after it settles, and keep words crisp (no blur or glow while they are read). Cut words, never type size.
- Chrome with god rays and a lens flare is the default 3D look the user rejected. Prefer `minimal`, or a reveal built from the brand's own shape, unless the brand really is chrome.
