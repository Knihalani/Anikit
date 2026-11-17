# Critique: the reviewer

```bash
anikit-video critique spec.json --render             # summary + top issues (--detail for all, --json)
anikit-video critique spec.json --render --pack rv/  # then open rv/review.md and LOOK at every image
```

- Judged against the spec's `intent` (or one inferred from music BPM and look): pace, dead air, rhythm, energy arc, abrupt endings, cuts on beats.
- At real pixels: contrast, overlaps, cut text, safe areas, empty frames, centred-slide layouts, repeated layouts; plus spec checks (reading time, hook, sameness, BAR, a recipe reused with new words) and taste checks from `docs taste` (tips, not verdicts): `type-scale` (> 4 sizes, or two within 20%), `text-crisp` (words left blurred), `palette` (≥ 5 saturated hue families), `composition` (centred-everything), `rhythm` (equal shot lengths), `music` (stock corporate bed, or synths against the look), `motion` (one ease and duration on most tweens), `copy` (weightless vocabulary readers associate with generated text); with `--render`: an empty first frame, an end mark held under ~1.2 s, and `eye-trace` (the focal point jumping across the frame on most fast cuts).
- The pack: a 1 fps strip, before/after frames at each cut, the worst frame per detector (boxed), the energy peak, and a rubric you answer by looking.
- The score is **not a quality verdict**: the frames are. Read docs/video/quality/BAR.md once.
- MCP: `critique_spec { path, render: true, detail }`.
