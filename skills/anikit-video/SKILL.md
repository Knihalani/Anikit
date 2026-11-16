---
name: anikit-video
description: Make professional motion-graphics videos (product launches, feature announcements, changelogs, demos, social teasers, data stories, testimonials, tutorials, countdowns, logo reveals) from a JSON spec or a template with the native anikit-video renderer. Use when asked to create, edit, review or render a video, an animated explainer or a launch/teaser clip.
---

# Making videos with Anikit Video

You produce videos by writing data, not by driving a browser. Work in this order and verify each step.

1. **Pick a template**: `anikit-video templates` (launch, feature, changelog, demo, teaser, data-story, testimonial, tutorial, countdown, logo-reveal). If nothing fits, write a spec by hand (reference: `docs/video/spec.md` or MCP resource `anikit://docs/spec`).
2. **Brand kit**: write `brand.json` with at least `name` and `colors.primary`; add `logo` and `voice.cta`/`voice.url` if known (`anikit-video prompt brand-kit`).
3. **Scaffold**: `anikit-video new <template> --brand brand.json [--from README.md] [--format 16:9|9:16|1:1]`. It creates `params.json`, `spec.json` and `NEXT.md`.
4. **Write the copy** in `params.json`, following `anikit-video templates guide <template>`:
   - the hook is 3–7 words and on screen within 1 s;
   - every slot's `maxWords` is a reading-time budget (about 3 words per second plus 1 s);
   - use benefits with real numbers, and no filler.
   For a launch video, follow `anikit-video prompt launch-video-generator` (facts → script → storyboard → spec).
5. **Expand and review**:
   - `anikit-video templates expand <template> params.json --brand brand.json --format 16:9 -o spec.json`;
   - `anikit-video check spec.json`: fix every error;
   - `anikit-video critique spec.json`: fix every warning by changing copy or timing, never by shrinking type;
   - `anikit-video preview spec.json -o sheet.png`: then **look at the PNG**.
6. **Improve**: apply `anikit-video prompt critic`, making at most 3 exact edits per round and at most 2 rounds.
7. **Render**: `anikit-video render spec.json -o out.mp4`. For other aspect ratios, re-expand with `--format 9:16` / `1:1` from the same params.

Rules that always apply: one focal point per moment; at most 3 type levels; the accent colour on one element; `outExpo`/`outCubic` entrances over 0.5–0.8 s; staggers of 0.04–0.12 s; transitions of 0.3–0.7 s; text inside the safe areas (9:16: clear the top 12% and the bottom 20%); contrast of at least 4.5:1; an end card held for at least 3 s. The full rules: `anikit-video prompt motion-rules`.

With MCP (`anikit-video mcp`), use the tools of the same names: `list_templates`, `get_template_schema`, `create_from_template`, `validate_spec`, `critique_spec`, `render_contact_sheet`, `render_still`, `render_video`, `get_prompt`.
