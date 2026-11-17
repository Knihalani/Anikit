# Critique: the reviewer

```bash
anikit-video critique spec.json --render             # summary + top issues (--detail for all, --json)
anikit-video critique spec.json --render --pack rv/  # then open rv/review.md and LOOK at every image
```

- Judged against the spec's `intent` (or one inferred from music BPM and look): pace, dead air, rhythm, energy arc, abrupt endings, cuts on beats.
- At real pixels: contrast, overlaps, cut text, safe areas, empty frames, centred-slide layouts, repeated layouts; plus spec checks (reading time, hook, sameness, BAR).
- The pack: a 1 fps strip, before/after frames at each cut, the worst frame per detector (boxed), the energy peak, and a rubric you answer by looking.
- The score is **not a quality verdict**: the frames are. Read docs/video/quality/BAR.md once.
- MCP: `critique_spec { path, render: true, detail }`.
