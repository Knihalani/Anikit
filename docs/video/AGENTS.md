# Anikit Video for agents

JSON spec → MP4, rendered natively (no browser). You direct; the engine renders; the critic checks every frame. Read [`skills/anikit-video/SKILL.md`](../../skills/anikit-video/SKILL.md) first (a 1-page method) and [`spec.md`](spec.md) (a 1-page spec).

```bash
anikit-video looks                                  # 10 art-direction worlds; looks <name> for one's direction
anikit-video critique spec.json --render            # contrast, overlaps, cut text, pacing, sameness
anikit-video preview spec.json                      # contact sheet PNG: look at it
anikit-video render spec.json -o out.mp4
anikit-video templates                              # quick starts; expand <t> --look <name> to restyle
```

**Method:** logline → one feeling → a look → a shot list of what the viewer *understands* → one signature image (3D `text-3d`/`model`/`globe`/`scene3d`, generative, real footage, depth parallax) → vary the layout every shot → judge in motion. `anikit-video prompt director` gives the full procedure. A worked example is [`examples/video/directed/`](../../examples/video/directed/).

**Looks:** `"look": "noir"` at the top of a spec sets the palette, type, text styles, entrance, transition, camera, backdrop and post. `["editorial", "noir"]` composes looks; `{"base": "swiss", "theme": {"colors": {"primary": "#0050FF"}}}` overrides one; a scene with `"look": false` opts out. Spec keys always win.

**Rules:** hook in ≤ 2 s · hold text words/3 + 1 s · one focal point per frame · 9:16 keeps the top 12% and the bottom 20% clear · contrast ≥ 4.5:1 · cut words, don't shrink type.

**MCP** (`anikit-video mcp`): `list_looks`, `list_templates`, `get_template_schema`, `create_from_template` (with `look`), `validate_spec`, `critique_spec` (`render: true`), `render_still`, `render_contact_sheet`, `render_video`, `get_prompt` (director, critic, storyboarder, script-writer, copy-guard, brand-kit, motion-rules, launch-video-generator, template:<name>). A full template session costs about 2k tokens (`cargo test -p anikit-video --test token_budget -- --nocapture`).

**Builders:** `import { video, template } from 'anikit/video'` and `from anikit.video import video, template`. Scaffold from a README: `anikit-video new launch --from README.md --brand b.json`.

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
| 4 | Launch / Teaser / Changelog | 2–4 each | 1.1–2.7k | 6.5 / 6 / 5 | white labels on brand fills (80% of render warnings) → `fill` token + auto label colour; particles over text → behind; expand now runs the render check |
| 5 | Calibration: flagship film *Telemetry* | – | – | 8/10 | the scale's anchor for this engine |
| 5 | Launch / Teaser / Changelog | 2 each | 1.0–2.0k | 6 / 6 / 5.5 | gaps vs Telemetry: static generic hero UI, one layout repeated, little typographic density → launch redesign; teaser flash never full-bleed → brand fill |

Runs 2–5 used a stricter grader ("10 = ship it for a real launch"). Rendering was 165–240 fps; `preview` takes about 0.04 s.
