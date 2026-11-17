---
name: director
description: Direct an original video from a brief: one idea only this brief could have, the medium and pace that idea calls for, a shot list of what the viewer understands, then a spec judged in motion against the taste canon.
---

# Director

You make videos people remember. Clean but generic doesn't land, and neither does a house style: two briefs tend to deserve two very different films. Read `anikit-video docs taste` first (the full canon is `docs/video/taste/TASTE.md`).

1. **Logline:** who it is for, and what changes for them. One sentence. If you can't write it, ask for the missing fact.
2. **Ideas in volume, then turn the obvious one** (docs/video/craft/art-direction.md):
   - list what you already believe about the subject (your clichés) and 20–40 concrete facts: how it's made, how it fails, who loves it;
   - write 30+ one-line concepts; the first ten are usually the obvious ones;
   - turn the obvious one with a provocation: escape (remove what's taken for granted), reverse, exaggerate, borrow a medium from another world (Honda "Cog" shows the parts, not the car);
   - ask who made these images, with what and in what mood (Se7en's titles look made by the killer);
   - shortlist 3 directions far apart, each with references *outside* tech and SaaS (print, title design, scientific illustration, posters, documentary, architecture) and what you take from each;
   - keep the one that fails the swap test least (a competitor's name shouldn't fit) and differs from every earlier piece in this project.
3. **Avoid the landing page in motion:** a UI screenshot or phone beside copy in an empty frame, or "UI on dark with one accent" by default, reads as a template. If the brief is literally a UI demo, stage the UI (extreme scale contrast, crops, the camera moving through it, UI as a texture on the world). Fill frames with made imagery: illustration from paths, shader worlds, particles and sims as imagery, texture (TASTE.md §11).
4. **Medium from the idea, not from habit:** set type, 2D illustration or paths, real UI, photography, a shader world, or ray-traced 3D. 3D tends to earn its cost where depth carries meaning; reusing a previous piece's environment, material or motif tends to make the work read as samey.
5. **Pace from the idea:** declare `"intent"`. Fast (cuts on the beat every 0.5–1.5 s) is right for hype and social; a held, calm film is right for craft and trust. Either way, vary shot lengths (a run of short cuts, then a hold), and give every hold a change.
6. **Treatment and style frames:** a one-line idea, world rules (palette roles, how type behaves, camera verbs, materials, what's never in this world), a structure chosen from docs/video/craft/storytelling.md §3, and 3–5 `still` frames that hold up as stills. It's the cheapest place to catch boring.
7. **Shot list:** write each shot as "*the viewer understands …*", then the image, then the seconds. One focal point per shot; the order makes an argument (tension → turn → proof → payoff); with the sound off, the shots alone tell it. Plan contrast over time: quiet shots so the loud ones land.
8. **Look:** palette from the subject (its material, era, brand), values first: decide what is light and what is dark before hue. One base family + one accent. Type chosen for the subject, 2–4 sizes on one clear ratio. Light with a source you could point to. A built-in look (`anikit-video looks`) is a starting point to override, not the answer.
9. **Sound:** instrumentation from the concept (piano, acoustic, orchestral, jazz, rock, found percussion…); synths only when the concept is electronic. Spot it: 3–6 hit points per minute, one planned silence before the reveal, risers rare.
10. **Spec:** `docs/video/spec.md`, `defs` for repeated parts. Words are budgets: hold text `words/3 + 1` s after it settles; cut words before shrinking type; words crisp (no blur, defocus or glow while read).
11. **Judge:** `anikit-video critique spec.json --render --pack rv/` and fix every error and warning; then check the strip and three full-size stills against `docs taste`. Ask of each frame "understood in 1 s?" and "would a studio post it?". Fix the 3 weakest, re-check.
12. **Render.** Report the logline, the idea, why this medium/pace/palette/score, and the file.
