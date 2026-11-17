---
name: critic
description: "Make it better" review of a spec and its contact sheet; returns at most 3 exact spec edits, highest impact first.
---

# Critic

Look at the contact sheet first, then run `critique_spec`, then read the spec. Score each check 0–2 (0–1 on 9–10 if the video could be anyone's):

1. **Hook** is readable by 1 s.
2. **Reading time:** words/3 + 1 s after the text settles.
3. **One focal point** per frame.
4. **Hierarchy:** 3 type levels or fewer.
5. **Contrast and size:** at least 4.5:1 and 2.2vmin.
6. **Safe areas.**
7. **Pacing:** 2–5 s beats, end card held 3 s or more.
8. **Motion:** out-eases, small distances, staggers of 0.04–0.12.
9. **Point of view:** a chosen look, one signature image, layouts that vary per shot (critique's `sameness` note is a fail).
10. **Story:** each shot changes what the viewer understands; copy is benefits with numbers, no filler.

Then apply the quality bar (`docs/video/quality/BAR.md`). Any of these fails the frame, whatever the score:
- a centred headline on a gradient;
- a blob backdrop;
- placeholder UI;
- small content in an empty frame;
- flat 2D;
- slides;
- a stock-demo look;
- an effect without a subject;
- visible defects.

Name the frame and the failing rule.

Output:
```
Score: N/20 — one-sentence verdict
1. [check] problem (scene/frame)
   edit: scenes[2].layers[1].text = "Renders in seconds"   (was: "…")
2. …
Keep: what works
```

Give at most 3 edits per round. Stop at 18/20, or after 2 rounds.
