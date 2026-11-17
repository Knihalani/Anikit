---
name: anikit-video
description: Make motion-graphics videos (launches, feature announcements, changelogs, demos, social teasers, data stories, testimonials, tutorials, countdowns, logo reveals) with the native anikit-video renderer, from a template or a JSON spec. Use when asked to create, review or render a video or animated clip.
---

1. `anikit-video templates` → pick one; `anikit-video templates schema <t>` → its params (only the `*` ones are required); `anikit-video templates guide <t>` explains each one with an example.
2. Write `params.json` (without `screenshot`/`image` params, device frames show a generated placeholder app with sample data, so pass real screenshots when you have them) (respect the word limits: they are reading-time budgets) and a `brand.json` (`{name, colors:{primary}, logo, voice:{cta,url}}`; all optional, and without a logo the name is set as a wordmark).
3. `anikit-video templates expand <t> params.json --brand brand.json --format 16:9 -o spec.json`: fix every printed issue by cutting words or changing timing (`anikit-video critique spec.json` re-checks).
4. `anikit-video preview spec.json` (writes `spec-sheet.png`, one frame per scene; `--at 3.5 -o f.png` for a full-size still), then look at the PNG; make at most 3 fixes and re-check.
5. `anikit-video render spec.json -o out.mp4`. For other aspects, use `--format 9:16` / `1:1` with the same params.

Custom videos: `docs/video/spec.md`. Launch from a README or URL: `anikit-video prompt launch-video-generator`. Over MCP, the tools have the same names.
