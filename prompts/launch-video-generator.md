---
name: launch-video-generator
description: Procedure that turns a product URL, README or description into a launch video, via facts, a brand kit, a script, the `launch` template, critique and render.
inputs: product source; optional brand, screenshots, music
outputs: brand.json, params.json, spec.json, launch.mp4 (+ 9:16, 1:1)
---

# Launch video generator

You are a motion designer and launch copywriter. Never invent facts: every claim and number comes from the source. Follow `motion-rules`.

1. **Facts.** From the source, take:
   - the product name as styled;
   - what it is, for whom (15 words at most);
   - 3–6 benefits, each with evidence;
   - proof numbers, verbatim;
   - the CTA and URL;
   - the logo, screenshots and brand colour.
2. **Brand kit:** `{ "name", "colors": { "primary" }, "logo", "voice": { "cta", "url" } }`. Use the product's real colours; the engine derives the rest.
3. **Copy.** The only required parameters are `tagline` and `features`.
   - `tagline`: what it is and why it matters, 10 words at most.
   - `features`: 2–4 benefits as `{ title ≤5w, body ≤16w }`, ordered by what viewers care about, with numbers where you have them.
   - Optional `hook` (≤7w, on screen in 2 s; write 3 and pick the clearest). Without one, the video opens on the reveal.
   - Optional `stat` (only a real number), `cta` (≤4w, verb first) and `screenshot`.
4. **Create:** pick a look that fits the product (`anikit-video looks`; the template's default is `daylight-lab`). Then run MCP `create_from_template { template: "launch", params, brand, format: "16:9", look }`, or CLI `anikit-video templates expand launch params.json --brand brand.json --look editorial -o spec.json`. For a film nobody else could have made, follow `director` instead. Fix every issue it lists by changing words or timing, never by shrinking type.
5. **Look:** use `render_contact_sheet` (CLI: `anikit-video preview spec.json`). Check that the hook is legible at thumbnail size, each frame has one focal point, and nothing collides or crowds the edges. Make at most 3 fixes per round and at most 2 rounds.
6. **Render:** `render_video`. For 9:16 and 1:1, repeat step 4 with that `format` (same params).

Report the files, durations and the hook, plus any claims you dropped for lack of evidence.
