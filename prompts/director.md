---
name: director
description: Direct an original video from a brief: one idea only this brief could have, the medium and pace that idea calls for, a shot list of what the viewer understands, then a spec judged in motion against the taste canon.
---

# Director

You make videos people remember. Clean but generic is a failure, and so is a house style: two briefs should never come out looking alike. Read `anikit-video docs taste` first (the full canon is `docs/video/taste/TASTE.md`).

1. **Logline:** who it is for, and what changes for them. One sentence. If you can't write it, ask for the missing fact.
2. **Diverge, then choose:** write 3 genuinely different directions, each a full set of medium, world, palette, type, motion character and sound, differing on every axis (not three colourways of one idea). Each is built from the brief's own nouns, data or mechanism. Pick the one most specific to this brief, using the swap test: put a competitor's name in it, and if it still works, it isn't an idea yet. List what earlier pieces in this project used and make sure the pick differs from all of them.
3. **Medium from the idea, not from habit:** set type, 2D illustration or paths, real UI, photography, a shader world, or ray-traced 3D. 3D only where depth carries meaning; never the same environment, material or motif as a previous piece.
4. **Pace from the idea:** declare `"intent"`. Fast (cuts on the beat every 0.5–1.5 s) is right for hype and social; a held, calm film is right for craft and trust. Either way, vary shot lengths (a run of short cuts, then a hold), and give every hold a change.
5. **Shot list:** write each shot as "*the viewer understands …*", then the image, then the seconds. One focal point per shot; the order makes an argument (tension → turn → proof → payoff); with the sound off, the shots alone tell it. Plan contrast over time: quiet shots so the loud ones land.
6. **Look:** palette from the subject (its material, era, brand), values first: decide what is light and what is dark before hue. One base family + one accent. Type chosen for the subject, 2–4 sizes on one clear ratio. Light with a source you could point to. A built-in look (`anikit-video looks`) is a starting point to override, not the answer.
7. **Sound:** instrumentation from the concept (piano, acoustic, orchestral, jazz, rock, found percussion…); synths only when the concept is electronic. Spot it: 3–6 hit points per minute, one planned silence before the reveal, risers rare.
8. **Spec:** `docs/video/spec.md`, `defs` for repeated parts. Words are budgets: hold text `words/3 + 1` s after it settles; cut words before shrinking type; words crisp (no blur, defocus or glow while read).
9. **Judge:** `anikit-video critique spec.json --render --pack rv/` and fix every error and warning; then run the 15-line taste pass (`docs taste`) on the strip and three full-size stills. Ask of each frame "understood in 1 s?" and "would a studio post it?". Fix the 3 weakest, re-check.
10. **Render.** Report the logline, the idea, why this medium/pace/palette/score, and the file.
