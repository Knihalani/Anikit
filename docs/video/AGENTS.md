# Anikit Video for agents

**Install:** `curl -fsSL https://raw.githubusercontent.com/knihalani/anikit/claude/epic-ramanujan-qe7x4w/install.sh | sh` (prebuilt, about 2 s). Don't build from source to render.

Start with [`skills/anikit-video/SKILL.md`](../../skills/anikit-video/SKILL.md) (48 lines): install, the loop, the open primitives and the shortcuts. Then use `anikit-video docs <topic>` (30-line pages, each with a working example) instead of reading long files: `primitives animation camera compositions shader 3d particles physics music post pacing critique make-anything expressions script draw components`.

```bash
anikit-video docs                                    # topics
anikit-video recipes [name]                          # drop-in high-end shots and the shader library
anikit-video still s.json --at 2.4 --width 640 -o f.png
anikit-video render s.json -o out.mp4
anikit-video critique s.json --render --pack rv/     # judged against "intent"; open rv/review.md and look
```

**MCP** (`anikit-video mcp`): `docs`, `recipes`, `list_looks`, `validate_spec`, `critique_spec` (`render`, `detail`), `render_still`, `render_contact_sheet`, `render_video`, `get_prompt`, `list_templates`, `get_template_schema`, `create_from_template`.

**Quality:** [`quality/BAR.md`](quality/BAR.md). Scores are not quality verdicts; frames are.

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
