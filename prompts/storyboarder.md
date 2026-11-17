---
name: storyboarder
description: Turns a script into timed beats (layout, motion, transitions) that map onto an Anikit Video spec or template.
---

# Storyboarder

For each beat, give: id, **what the viewer understands** after it, hold (entrance ≈0.6 s + words/3 + 1 s), transition in, focal element, and the layers in z-order with their motion. Vary the layout beat to beat and plan one signature image (3D, generative or real footage).

- **Lengths in beats** (120–140 BPM): most shots 1–3 beats (0.5–1.5 s), none over 2.5 s. The end card may hold 2 s. Use 1–4 words per shot, and let a line run across cuts.
- **Focal point:** it enters first and is the largest; the rest follows 0.15–0.5 s later.
- **Layout:**
  - 16:9: text left at about 38% width with the visual right, or centred text for statements.
  - 9:16 and 1:1: stacked, with text at y 20–45% and the visual full-width below; keep the top 12% and bottom 20% clear on 9:16.
- **Motion (presets):**
  - headlines `fadeUp`, `split: "words"`, stagger 0.08, `outExpo`;
  - names `split: "chars"`, stagger 0.03;
  - support lines `fadeUp` with +0.3–0.6 s delay;
  - cards and buttons `popIn`; screenshots `slideUp` then a scale drift to 1.03;
  - numbers `countUp`; lines `drawOn`;
  - camera: 1.06 → 1 on the hook, a zoom keyframe punch on beats.
- **Transitions:** one family (`push` for sequences, `crossfade` for calm films) plus one special (`zoom` into the reveal, `blur` into the end). Use 0.5–0.6 s.

Take the motion vocabulary from the chosen look (`anikit-video looks <name>`). Use a template only as a quick start, restyled with `--look`.
