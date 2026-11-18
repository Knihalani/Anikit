# Critique (steps 4, 7 and 8)

You can't see your own film fresh, and a critic who sees only your film shares your blind spots. In our tests, the only changes that beat noise came from rebuilding against a **blind comparison with professional films**. So the critique puts your film next to real professional launch films and asks a fresh jury which is which.

## Run the comparison (about 3 min, about $0.80)

```
python3 scripts/judge.py run launch/pass1.mp4 --practice --trials 2 --format <16:9|9:16|1:1> \
  --tags product-launch,<one or two style tags> --out launch/crit1
```

- **Style tags:** pick from `cg-3d`, `ui-product`, `illustration`, `kinetic-type`, `brand-idents`, `generative`, `explainer-2d`, `social-vertical`, `event-promo`, `music-video`, `data-story`.
- **Pick tags whose references match your film type:** `ui-product` for a proof film, `cg-3d` for an object teaser, and `brand-idents` or `graphic` for an idea film. `kinetic-type` draws feature title sequences (*Vertigo*), which no 30 s launch film resembles.
- **`--practice`:** keeps the references used for evaluation out, so your crit never trains on the test.
- **What it does:** it shows your 6 frames and 2–3 professional films' frames, shuffled and unlabelled, to a fresh model, which ranks them, scores them and names gaps.
- **Where the result goes:** `launch/crit1/<name>/result.json`. Read `candidate.overall`, `candidate.gaps`, and the gaps it names for the professional entries; those show what "better" looks like. The critic stage adds motion, sound and craft notes.

If `scripts/judge.py` can't run (no `claude` CLI, no network), spawn a fresh subagent with the prompt at the end of this page and the contact sheet, and answer the checks.

## The fake-look critic (always, alongside the comparison)

The blind judge compares stills against professional films and rewards idea and composition. It passed a film the user rejected as "horrid, so artificial and weird" (a CG marching band). So a second critic looks with a person's eye. Spawn a fresh subagent and give it **only** 6 evenly spaced stills at full size (or the contact sheet) plus the one-line brief:

> You are a senior designer who hates AI-generated and template-looking work. Look at these 6 frames from a product launch video. Answer bluntly, with frame numbers:
> (1) Does any frame look like a simulation, a video-game render, a template, stock motion graphics, or AI-generated imagery? Which, and what gives it away?
> (2) From the frames alone, what does the product do? What is the one moment someone would describe to a friend?
> (3) Write the cruelest one-line description a critic could post about this film.
> (4) Would a top studio (Buck, ManvsMachine, Pentagram, Apple, Linear) have shipped this? If not, what's the single biggest reason?

Paste its answers into `5-crit.md`. **Ship only if (1) is "no",** (2) matches your promise and moment, and (3) isn't damning. A YES on (1) is a WORLD or IDEA gap: change the system or the medium, not the polish.

## Read it honestly, in `5-crit.md`

Paste the overall score, the ranking, and every gap verbatim. Then sort each gap into one of three kinds:

- **IDEA:** "literal", "pun", "generic", "no idea beyond…", "effect demo", "could be any brand", "the product's name drawn".
- **WORLD:** "placeholder", "flat light", "no material", "three looks stitched", "same layout every frame", "empty frames".
- **CRAFT:** type, timing, legibility, a specific frame.

## The rules (binding)

1. **Any IDEA gap, or a YES from the fake-look critic about the idea itself, means the idea is replaced, not polished.**
   - Go back to `2-ideas.md` and choose an idea from a **different family** (a different device and a different world), not a variation.
   - Run the literal and swap tests again, write `3-system.md` again, and do style frames again.
   - We measured this: polishing a literal idea plateaus at the same score, every time.
2. **WORLD gaps** mean rebuilding the world at full fidelity:
   - real material at 100% crop;
   - a key light with falloff;
   - a different subject, scale and camera in every shot.
3. **CRAFT gaps** get fixed directly.
4. **Always cut the weakest third** of shots or effects.
5. **Pass 2** is rendered, then **crit 2** (`--out launch/crit2`, same tags).
   - Ship whichever pass scored higher. A difference under 0.75 is a tie (judge noise ±0.5); on a tie, ship the one with fewer IDEA/WORLD gaps.
   - If crit 2 still has an IDEA gap and you have time for one more pass, replace the idea again.
   - Stop after 3 passes.

## The 17 checks (also answer them yourself on the contact sheet, frame by frame)

**Hold constant:**
1. One light logic and palette family in every frame (an arc is fine, a swap is not)?
2. One material world in every frame?
3. A visible key light with falloff in every frame?

**Change:**
4. Every frame differs in subject, scale or camera (no shared layout with different words)?
5. Every frame is a poster: one focal subject, made imagery, ≤ ~40% flat background?

**Idea and type:**
6. Literal test: does the idea survive deleting the product name and its mechanism?
7. Swap test: does it fail with a competitor's name on it? (It should.)
8. No frame exists to show a feature or technique, and no caption names one?
9. In at least half the frames with text, is the type inside the scene, not laid over it?
10. No "kit" type (tiny tracked-caps or mono label plus headline; centred headline on a backdrop)?
11. At most one claim line on screen at a time?

**Defects and ending:**
12. No garbled or accidentally clipped glyph?
13. The last stretch is alive (no ≥ 2 s static card)?
14. The ending resolves the device into the name?
15. At least 6 shots or major changes per 15 s?
16. Does the device escalate (≥ 3 distinct states, scales or media), with no single move used more than twice?
17. Is every frame's ground a lit surface (falloff, material or shadow), not a flat fill or gradient?

## Fallback critic prompt (only if `judge.py` can't run)

> You are the creative director of a top motion-design studio reviewing a junior team's launch film for [product]. You see only the contact sheet at [path]. Be blunt.
> (1) What is the idea, in one sentence? Does it survive deleting the product name? Would it work for a competitor?
> (2) Answer the 17 checks YES/NO with frame numbers.
> (3) Give the three weakest frames and why.
> (4) Name the single change that would lift it most; if the idea or the world is the problem, say so.
> (5) Which third would you cut?
