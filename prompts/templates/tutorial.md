---
name: template:tutorial
description: How to fill the 'tutorial' template well.
---

# Template guide: Tutorial / code walkthrough (`tutorial`)

A coding tutorial: a title beside an editor; then 2–6 split-screen steps, each a numbered badge, title and note beside a code editor that types itself (with highlighted lines) or a terminal for one-line shell commands; then a checklist recap and a CTA with confetti.

**Typical length:** 15–60 s. **Formats:** 16:9, 9:16 and 1:1 (pass `--format`); the layout adapts automatically.

## Parameters

| Param | Type | Required | Limits | What to write |
|---|---|---|---|---|
| `title` | string | yes | ≤ 8 words | What the viewer will be able to do, as an outcome. |
| `subtitle` | string |  | ≤ 12 words | Optional line under the title (who it's for, or the time it takes). |
| `language` | string |  | ≤ 16 chars | Label shown in the code window bar, e.g. 'bash', 'json', 'ts'. Default: `""`. |
| `steps` | array | yes | 2–6 items | The steps in order. Keep each code block short (ideally ≤ 8 lines, ≤ 50 characters per line). |
| `steps[].title` | string | yes | ≤ 6 words | The action, imperative: 'Install the CLI'. |
| `steps[].body` | string |  | ≤ 18 words | Why or what to notice, in one sentence. |
| `steps[].code` | text | yes | ≤ 420 chars | Code shown in the window; use \n for new lines. |
| `steps[].language` | string |  | ≤ 16 chars | Overrides the top-level language for this step. |
| `steps[].highlight` | array |  |  | 1-based line numbers to highlight after typing finishes. |
| `recap` | string |  | ≤ 10 words | Closing line on the end card. Default: `"That's it."`. |
| `cta` | string |  | ≤ 4 words | Call to action on the end card. Default: the brand kit's `voice.cta`. |
| `url` | string |  | ≤ 32 chars | Address on the end card. Default: the brand kit's `voice.url`. |
| `music` | string |  |  | Optional music file. Calm and low (lo-fi, 70–95 BPM) so it doesn't fight the reading. |

## Example

```json
{"title":"Render a video from JSON in 3 steps","subtitle":"No browser, no timeline editor.","language":"bash","steps":[{"title":"Install the CLI","body":"One binary, no browser to download.","code":"cargo install anikit-video","language":"bash"},{"title":"Describe your video","body":"Scenes and layers in plain JSON.","code":"{\n  \"scenes\": [\n    { \"layers\": [\n      { \"text\": \"Hello\", \"enter\": \"fadeUp\" }\n    ] }\n  ]\n}","language":"json","highlight":[4]},{"title":"Render it","body":"Every core, straight to MP4.","code":"anikit-video render hello.json -o hello.mp4","language":"bash"}],"recap":"Your first video, from one JSON file.","cta":"Read the docs","url":"anikit.dev/docs"}
```

Create: MCP `create_from_template {template: "tutorial", params, brand, format}`, or `anikit-video templates expand tutorial params.json --brand brand.json -o spec.json`. Word limits are reading-time budgets: cut words, don't shrink type.

## Tips

- A template gives every product the same structure, which is what the user rejected: use it for a fast draft, then make it this product's (a palette from its own assets, a typeface chosen for it, its real UI or object as the hero, a score whose instruments fit it). For an original film, use the `director` prompt or skills/anikit-launch-video. Run the taste pass first: `anikit-video docs taste`.
- Hold every line for max(0.83 s, chars/15, words × 0.3) after it settles, and keep words crisp (no blur or glow while they are read). Cut words, never type size.
- Show real code from the product and keep each step to one idea.
