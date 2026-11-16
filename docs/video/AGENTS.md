# Anikit Video for agents

JSON spec → MP4, rendered natively (no browser). Templates + a brand kit cover most videos; the critic catches design mistakes before you render.

```bash
anikit-video templates                                    # 10 templates, one line each
anikit-video templates schema launch                      # its params (or: templates guide launch)
anikit-video templates expand launch p.json --brand b.json --format 16:9 -o spec.json   # prints issues + critique
anikit-video preview spec.json                            # contact sheet PNG: look at it
anikit-video render spec.json -o out.mp4
```

Minimal launch params: `{"product":"Lumen","tagline":"Your notes, searchable in seconds.","features":[{"title":"Instant search"},{"title":"Offline first"}]}`. Brand: `{"name":"Lumen","colors":{"primary":"#0F766E"},"voice":{"cta":"Try it free","url":"lumen.app"}}`.

**MCP** (`anikit-video mcp`, the same flow): `list_templates` → `get_template_schema` → `create_from_template` (returns a spec path plus issues) → `render_contact_sheet` → `render_video`. A full launch-video session costs about 1.9k tokens (`cargo test -p anikit-video --test token_budget -- --nocapture`).

**Rules:** hook in ≤ 2 s · hold text words/3 + 1 s · one focal point · `outExpo`/`outCubic` entrances · 9:16 keeps the top 12% and the bottom 20% clear · contrast ≥ 4.5:1. Fix issues by cutting words, not by shrinking type.

**More:**
- spec reference: [`spec.md`](spec.md);
- prompts: `anikit-video prompt` (launch-video-generator, critic, motion-rules, script-writer, storyboarder, copy-guard, brand-kit, template:<name>);
- builders: `import { video, template } from 'anikit/video'` and `from anikit.video import video, template`;
- scaffold from a README: `anikit-video new launch --from README.md --brand b.json`;
- examples: `examples/video/`.
