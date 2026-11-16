# Anikit Video for agents: quick start

Anikit Video turns a JSON spec into an MP4. It is native Rust with no browser, renders in parallel on every core, and was designed for agents to drive: one declarative spec, typed templates, "did you mean" errors, cheap previews, and a design critic built in.

**The 30-second version:**

```bash
anikit-video new launch --brand brand.json --from README.md   # scaffold: params.json, spec.json, NEXT.md
# edit launch/params.json (the copy), then:
anikit-video templates expand launch launch/params.json --brand brand.json --format 16:9 -o launch/spec.json
anikit-video critique launch/spec.json        # reading time, contrast, safe areas, pacing: fix every warning
anikit-video preview launch/spec.json -o sheet.png   # look at it
anikit-video render launch/spec.json -o launch.mp4
```

## Choose your path

| You want | Do this |
|---|---|
| A launch, feature, changelog, demo, teaser, data, testimonial, tutorial, countdown or logo video | **Use a template.** `anikit-video templates` lists them; `templates guide <name>` explains each parameter and its word limit. Templates look premium by default, follow your brand kit, and adapt to 16:9, 9:16 and 1:1. |
| A launch video from a product URL or README | Follow `anikit-video prompt launch-video-generator`: facts → brand kit → script → storyboard → spec → critique → render. |
| Something custom | Write a spec ([`spec.md`](spec.md)), or expand the closest template and edit its output. |
| To generate videos from code | TypeScript: `import { video } from 'anikit/video'`. Python: `from anikit.video import video`. Both produce the same spec and call the CLI. |
| To work through MCP | `anikit-video mcp` (stdio). See **MCP** below. |

## The loop that produces good videos

1. **Write the copy first**, with word budgets. A line held `h` seconds can carry `3 × (h − 1)` words. Templates enforce `maxWords` per slot and explain any overrun.
2. **Expand or write the spec**, then run `check` (structure) and `critique` (design). Both print a JSON path and a fix for every finding.
3. **Look, cheaply**: `preview` renders one settled frame per scene into a single PNG, and `preview --at 0.8` shows the hook. Don't render the full video to find a typo.
4. **Review like a designer**: `anikit-video prompt critic` gives a 10-point checklist and asks for at most 3 fixes per round, each an exact spec edit.
5. **Render** the final video, plus the other aspect ratios from the same params.

## Rules that make it look professional

The full list is `anikit-video prompt motion-rules`. These are the ones that matter most:

- **The hook is on screen by 1 s**, 3–7 words. Never open on a logo or "Introducing".
- **Reading time:** hold text for at least `words / 3 + 1` seconds after it settles. About 12 words per screen; 20 at most.
- **One focal point per moment; at most 3 type levels; the accent colour on one thing.**
- **Easing:** entrances use `outExpo` or `outCubic` over 0.5–0.8 s; staggers are 0.04–0.12 s; never `linear` for movement. Transitions take 0.3–0.7 s, and there should be one transition family per video.
- **Safe areas:** keep text 7–8% from the edges; on 9:16, clear the top 12% and the bottom 20% (platform UI).
- **Contrast:** at least 4.5:1 for body text, 3:1 for large text. Use theme tokens (`$text`, `$muted`, `$primary`), not hard-coded colours.
- **End card:** held for at least 3 s, with the CTA and the URL.

## Brand kit (`brand.json`)

```json
{ "name": "Acme", "theme": "dark", "colors": { "primary": "#6E56CF" }, "logo": "logo.svg",
  "voice": { "cta": "Start free", "url": "acme.dev", "avoid": ["revolutionary"] } }
```

Everything is optional; missing colours are derived with contrast checks (`anikit-video prompt brand-kit`). Templates read it as `brand.*`, and specs see it as theme tokens.

## Templates

| Name | Use it for | Length |
|---|---|---|
| `launch` | Product launch: hook, reveal, 2–4 features, proof stat, CTA | 20–35 s |
| `feature` | One new feature: "New" kicker, screenshot, supporting points | 12–20 s |
| `changelog` | "What's new in vX": version badge plus New/Improved/Fixed items | 12–25 s |
| `demo` | Walkthrough: screenshots in a browser or phone, an animated cursor, callouts | 15–35 s |
| `teaser` | 9:16 social teaser: kinetic words on a beat, logo payoff | 6–12 s |
| `data-story` | Charts that grow in, counters, a takeaway with its source | 15–30 s |
| `testimonial` | 1–3 quotes with author, avatar and stars | 10–30 s |
| `tutorial` | Steps with code windows that type themselves | 20–60 s |
| `countdown` | Event intro, then N…1 with drawing rings, then "Live" | 8–16 s |
| `logo-reveal` | 3–6 s brand sting: glow, draw, assemble or minimal | 3–6 s |

Formats: `--format 16:9 | 9:16 | 1:1 | 4:5 | 720p | 4k | WxH`. Parameter errors name the parameter, suggest the closest name, and say how to fix the copy:

```
unknown parameter 'hok' (did you mean 'hook'?); this template takes: product, hook, tagline, …
features[1].title: 7 words is too long for this slot (max 5); viewers read about 3 words per second, so cut it to the essential phrase
```

## Builders

```ts
import { video, template } from 'anikit/video';

await video({ size: '9:16' })
  .theme({ colors: { primary: '#7c5cff' } })
  .scene((s) => s
    .text('Ship faster', { style: 'title', split: 'words', enter: { preset: 'fadeUp', stagger: 0.08 } })
    .text('JSON in. MP4 out.', { style: 'subtitle', y: '62%', animate: 'rise' }), { duration: 3 })
  .render('teaser.mp4');

const spec = await template('launch', { hook: 'Video, without the browser.', tagline: '…', features: [/* … */] },
                            { brand: 'brand.json', format: '1:1' });
```

```python
from anikit.video import video, template

v = video(size="9:16")
with v.scene(duration=3) as s:
    s.text("Ship faster", style="title", split="words", enter={"preset": "fadeUp", "stagger": 0.08})
v.render("teaser.mp4")
```

## MCP

Add the server to your client:

```json
{ "mcpServers": { "anikit-video": { "command": "anikit-video", "args": ["mcp"] } } }
```

| Tool | Use |
|---|---|
| `list_templates`, `get_template_schema` | Discover templates and their typed parameters, with examples and a writing guide. |
| `create_from_template` | `{template, params, brand, format}` → a spec, plus a critique. |
| `validate_spec` | Errors and warnings with paths and "did you mean". |
| `critique_spec` | Design review with no rendering: reading time, hook, contrast, safe areas, sizes, pacing. |
| `render_still`, `render_contact_sheet` | A PNG returned inline: look at your work cheaply. |
| `render_video` | The final render. |
| `get_prompt` | The prompt library and the per-template guides. |

Prompts (`prompts/list`) and resources (`anikit://docs/spec`, `anikit://docs/agents`, `anikit://prompts/motion-rules`) are served too.

## Prompt library

`anikit-video prompt` lists them (the sources are in `prompts/`):
- `launch-video-generator`
- `script-writer`
- `storyboarder`
- `critic`
- `copy-guard`
- `brand-kit`
- `motion-rules`
- `template:<name>` for each template

## Troubleshooting

- `check` prints `error` lines: the spec can't render. Fix those first; each has a path and a suggestion.
- `critique` warnings are design problems. Fix them by changing copy or timing, not by shrinking type.
- Asset paths are relative to the spec file. The builders and the MCP server make them absolute for you.
- A template field isn't enough? Expand the template, then edit the spec it produced. It's ordinary JSON.
