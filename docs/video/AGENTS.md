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

## Cold-agent dry runs

A fresh agent with only `skills/anikit-video/SKILL.md` did three tasks end to end (CLI, rendered MP4s): a launch for a note app, a 9:16 fitness teaser with captions, and a v2.3 changelog with 4 items. Tokens = characters the agent read from the tool ÷ 4.

| Run | Task | Attempts | ≈ Tokens read | Score | Main issues found → fixed |
|---|---|---|---|---|---|
| 1 | Launch | 1 | 1.2k | 6/10 | finance sample data in device screens → screens show the product's own copy; no `--help` → added |
| 1 | Teaser | 2 | 0.7k | 7/10 | `captions: any` undocumented → described; doubled words → captions opt-in |
| 1 | Changelog | 3 | 0.4k | 6.5/10 | an item overflowed its card → 34-char limit; schema text cut at "e.g." → fixed |
| 2 | Launch | 3 | 2.2k | 6.5/10 | fake dashboard KPIs → neutral `app` screens only; long static holds → shorter; issues not printed by `expand` → printed |
| 2 | Teaser | 2 | 1.0k | 5.5/10 | beat advice failed the critique → aligned (0.6–0.7 s); symbol scramble, overlapping captions → reworked |
| 2 | Changelog | 2 | 0.4k | 5/10 | tiny cards, unclear type labels → redesigned; misleading screenshot note → template-aware |

| 3 | Launch | 3 | 2.0k | 6.5/10 | over-length not scored → `length` warning with per-scene seconds; title repeated 3× per frame (toast) → removed; accent drifted to crimson → accent = primary lifted to 4.5:1 |
| 3 | Teaser | 4 | 1.2k | 5/10 | white captions unreadable on lime flash frames → light primaries flash a darkened tint with light text |
| 3 | Changelog | 2 | 1.0k | 6.5/10 | red IMPROVED chip read as an error → semantic blue; NEW = primary, FIXED = green |

Runs 2 and 3 used a stricter grader ("10 = ship it for a real launch"). Rendering was 165–240 fps; `preview` takes about 0.04 s.
