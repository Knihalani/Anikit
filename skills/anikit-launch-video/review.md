# Outside critique (step 7)

You can't see your own film fresh. Our self-review questions became a rubber stamp: 2 of 12 agents changed anything substantive. A critic who sees only the result, and is asked to compare it with professional work, gets acted on.

## The critic prompt (spawn a fresh subagent; give it only this, the brief, and the contact-sheet path)

> You are the creative director of a top motion-design studio reviewing a junior team's launch film for [product, one line from the brief]. You see only the contact sheet at [path] (frames in time order). You have not seen the script. Be blunt; your job is to find what stops this from standing next to a Buck, ManvsMachine or Apple launch film.
> 1. In one sentence, what is the idea? Could it be told without the product's name? Would it still work with a competitor's name on it?
> 2. Answer each check YES/NO with frame numbers: [paste the 15 checks below].
> 3. Name the three weakest frames and exactly why.
> 4. Name the single change that would lift this film most. If the idea or the world is the problem, say so plainly; don't suggest polish.
> 5. Which third of the shots or effects would you cut?

Paste its answer into `5-crit.md`, then write **"Changes for pass 2"**. If it can't spawn, answer the checks yourself with frame numbers and treat every unsure answer as a NO.

## The 15 checks (6+ frames evenly spaced in time)

**Hold constant:**
1. Do all frames share one light logic and palette family (an arc is fine, a swap is not)?
2. Is there one material world (surface, medium, grain) in every frame?
3. Is there a visible key light with falloff in every frame (not a radial glow or gradient wash)?

**Change:**
4. Does every frame differ from the others in subject, scale or camera? (Fail: two frames share a layout with different words.)
5. Is every frame a poster: one focal subject, made imagery, ≤ ~40% flat untextured background?

**Idea and type:**
6. Literal test: does the idea survive deleting the product name and its mechanism?
7. Swap test: does it fail with a competitor's name on it? (It should.)
8. Is there no frame that exists to show a feature or technique, and no caption naming one?
9. In at least half the frames with text, is the type inside the scene (lit, printed, made of the material, a window), not laid over it?
10. Is there no "kit" type (tiny tracked-caps or mono label plus headline; centred headline on a backdrop)?
11. Is there at most one claim line on screen at a time, with no verbatim fact list?

**Defects and ending:**
12. Is no glyph garbled or clipped by accident?
13. Is the last stretch alive (no ≥ 2 s static card), and do the last two frames differ?
14. Does the ending resolve the device into the name (not a logo-and-tagline slide)?
15. Are there at least 6 shots or major changes per 15 s?

## Pass 2 rules

- **If 1, 2, 6 or 7 fail,** change the world or the idea, not the polish.
- **Always cut the weakest third,** then re-check that the promise still reads.
- **Ship when 1–11 pass.** Two passes without a concept change is the plateau: stop.
