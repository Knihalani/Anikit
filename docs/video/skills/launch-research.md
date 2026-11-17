# Launch-video skills: research notes

About 15 minutes of web research (2026-11-17) for `skills/anikit-launch-video`. **(fetched)** means the page was opened and read; **(search)** means it was only seen in search results. Anything unsourced is marked *unverified*.

## Existing skills

| Skill | Source | Does well | Does badly |
|---|---|---|---|
| Remotion Agent Skills | https://github.com/remotion-dev/skills (fetched) | 13 skills that teach correct Remotion code: create, render, captions, studio. Widely adopted. | The README shows only framework correctness: no pacing, story, copy, music or review loop. One user reported ~191 exchanges to finish a video (search, unverified). |
| HyperFrames (HeyGen) | https://github.com/heygen-com/hyperframes, its [SKILL.md](https://raw.githubusercontent.com/heygen-com/hyperframes/main/skills/hyperframes/SKILL.md), [types/launch.md](https://raw.githubusercontent.com/heygen-com/hyperframes/main/skills/hyperframes/references/types/launch.md), [review.md](https://raw.githubusercontent.com/heygen-com/hyperframes/main/skills/hyperframes/references/review.md) (fetched) | The best one found. It writes three storylines, then a SPEC that "turns every adjective into a number". Its launch arc is hook → tension → reveal → proof → payoff → claim → end card. Rules are numeric: camera moves 0.4–0.6 s, text ≥ 26 px, mark lands in the final 5–10%, CTA held 1.5–3 s. It uses real UI only ("never invent a dashboard") and lists tells to avoid (typed-prompt opener, three feature cards, title over a dimmed screenshot, logo in a corner). QA: `check --json` (reading time, >3 s with no change, empty first frame, silent film) and contact sheets ranked P0/P1/P2, with "never score out of 10" and "remove a third of the motion". | No copy word budgets. Nothing on music or beat sync in the launch reference. A large system (many references plus a 450-block registry). Its mandated variety of 3–5 transitions can push toward busy films. |
| everyinc product-launch-video | https://skillselion.com/skills/everyinc/product-launch-video (fetched) | Storyboard first. Product visible within 3 s. Opens on the viewer's problem. The before/after is the centrepiece. Four critics (design, text, pacing, brand) each review about 20 stills. Burned-in captions. Never invents UI labels. | A narrow formula (before/after plus counters). Little on music. Critics see stills, so they can't judge rhythm. |
| memex-lab product-launch-video | https://www.ui-skills.com/skills/memex-lab/product-launch-video (fetched) | Acts Hook / Reveal / Showcase / Climax / Close, with duration ratios for 15/30/60/90 s. Warns against a single tempo. Has an "AI slop" list (purple gradients, uniform timing, emoji icons). | Incomplete code. Its schema uses descriptive strings where numbers belong. Mandatory density rules cause clutter. Thin on audio. |
| Agentic product demo playbook | https://www.claudecodehq.com/playbooks/agentic-product-demo (fetched) | A beat sheet with frame numbers before any code. Every visible list is traced to real data. "Counters count." A frame gate checks luminance jumps. Says outright that "stills can't show rhythm". | Focused on demo loops, so weak on hook and CTA. Its gate can't judge dead space. |
| Jitter templates | e.g. https://jitter.video/template/the-edit-product-launch/ (search) | Polished, fast kinetic type. | 3–11 s templates with a templated look; no narrative system. |
| anthropics/skills | https://github.com/anthropics/skills (search) | Document skills. | No video or launch skill seen (full listing unverified). |

## What makes launch videos great (craft rules adopted)

- **One promise, one carried object.** Keep a single repeatable sentence and one object moving through the whole film. Cut on completed ideas. Example: Linear Asks, per https://open-design.ai/blog/product-launch-video-storytelling/ (fetched). → our *motif* and *match-cut device*.
- **Real UI beats abstraction.** HyperFrames reports that films built around a real result drew far more views than effects-heavy ones. Raycast's teaser uses cropped fragments of the real UI (https://www.moonb.io/blog/product-launch-video, fetched). → screenshots as textures; real copy only.
- **Show the product early.** Across 80 SaaS feature videos, the median first product screen is at 0:09 (https://advids.co/blog/saas-feature-release-video-styles, fetched). That is the slow baseline to beat. Skills target ≤ 3 s. Ad editors put the hook at 0–1.5 s and open on texture or the unexpected, not the logo (https://vidpros.com/beauty-ad/, search). → hook in beats 1–2, reveal on `@bar3` (3.75 s).
- **Kinetic type at perception speed.** Apple's "Don't Blink" shows one- or two-syllable words for 5–7 frames each. Type acts out its meaning. Time is a character (slow-motion, then a hard stop). The payoff is a hard cut plus a sound hit. The film stays flat and breaks into depth once (https://motionographer.com/2016/09/14/4-ways-dont-blink-is-a-masterclass-of-type-and-rhythm/, fetched). → 1–4 words per shot, slams on beats, sfx on hits.
- **Short shots, varied tempo.** The average commercial shot was under 2 s by 1991 (MacLachlan & Logan via WARC, search). Alternate tension and release, and breathe before the climax. → BAR pace limits, plus the 60 s breakdown.
- **Restraint in copy.** One benefit per scene and one CTA (https://descript.com/blog/article/product-launch-videos, search; open-design). → word budgets of 20 / 40 / 80.
- **A performed end card.** The mark lands in the final 5–10% and the CTA holds 1.5–3 s (HyperFrames).
- Vercel Ship, Arc, Stripe Sessions, Figma Config, Notion and Framer: no published craft breakdowns were found, so claims about them are *unverified* and aren't used as rules.

## Gaps this skill fills

1. **Music as structure, not as a bed.** No skill we read defines how cuts relate to the beat. Ours teaches beat
   maths, `@beatN`/`@barN`, hits at the film's turn and sfx on moments. The tempo comes from the film's idea
   (`beats.md`); none is prescribed.
2. **No template.** The other skills prescribe a fixed arc (hook → tension → reveal → proof → CTA, or fixed act
   ratios), so their outputs share one shape. Ours gives principles and a process, plus contrasting shapes labelled
   as inspiration. It requires a structure invented from the product's own story, with the test "would this fit any
   product?".
3. **An idea method** (`concepts.md`): nouns and verbs, before → after, one motif, recorded so the next film differs.
4. **Mechanical quality checks:** no flat type (with a self-test), contrast and obstruction rules for hero words,
   the logo owning the end frame, and the BAR fail list.
5. **QA on motion, not stills:** `critique --render --pack`, and `pace.py` for fast intents.
6. **Truthfulness.** No fake stats. Proof is a fact from the brief or a demonstration.
