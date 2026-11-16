---
name: anikit-video
description: Make motion-graphics videos (launches, feature announcements, changelogs, demos, social teasers, data stories, testimonials, tutorials, countdowns, logo reveals) with the native anikit-video renderer, from a template or a JSON spec. Use when asked to create, review or render a video or animated clip.
---

1. `anikit-video templates` → pick one; `anikit-video templates schema <t>` → its params (only the `*` ones are required).
2. Write `params.json` (respect the word limits: they are reading-time budgets) and a `brand.json` (`{name, colors:{primary}, logo, voice:{cta,url}}`).
3. `anikit-video templates expand <t> params.json --brand brand.json --format 16:9 -o spec.json`: fix every printed issue by cutting words or changing timing.
4. `anikit-video preview spec.json`, then look at the PNG; make at most 3 fixes and re-check.
5. `anikit-video render spec.json -o out.mp4`. For other aspects, use `--format 9:16` / `1:1` with the same params.

Custom videos: `docs/video/spec.md`. Launch from a README or URL: `anikit-video prompt launch-video-generator`. Over MCP, the tools have the same names.
