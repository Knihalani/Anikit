---
name: anikit-launch-video
description: Make a launch, announcement, teaser or product video with anikit-video that looks designed by a top studio and is built to be shared. A gated process (film type → promise and shareable moment → research and references → ideas → visual system → technique tests → style frames → beats → pass 1 → critique → pass 2 → platform cuts and post copy), built from blind tests against professional films and research on 60+ viral and AI product launches. It is a method, not a template. Use for any launch, product, feature, announcement or teaser video.
---

# Launch video

Read `skills/anikit-video/SKILL.md` for the spec, docs, previews and render. This skill adds the launch method. Each step writes a file in `launch/`, and the next step reads it. **Don't skip steps or start the next before its file exists.** Every step fixes a failure we measured or that users rejected (`docs/video/quality/BAR.md`).

## What we learned (the short version)

- **What spreads** (`docs/video/craft/launch-viral.md`, `launch-ai-viral.md`):
  - the product's real output or a live, unedited magic moment;
  - one event a viewer can retell in a sentence;
  - a hook in the first frame;
  - honesty.

  Every famous backfire (staged Gemini demo, Humane, Apple "Crush") was a proof failure or contempt.
- **What looks expensive** (`launch-visual-craft.md`): a visual system *derived from the product's own behaviour*, real UI with real strings in close crops, strict type and grid, a small motion vocabulary, nothing moving without a cause.
- **What users rejected as artificial or "AI slop":**
  - simulated real-world scenes (a CG marching band on a field);
  - cream paper with an editorial serif;
  - warm dark ground with a glowing serif;
  - fogged terrain;
  - chrome type;
  - the tasteful-AI kit (tiny tracked label plus centred headline on dark).
- **What the blind jury punishes:**
  - a literal idea;
  - one beat per feature;
  - a new look per shot;
  - slide type;
  - flat light.
- **The judge is not the bar.** A film it passed was rejected by the user as artificial. Ship only what a person would also call designed, not generated.
- **Not artificial isn't enough** (v4 demos: no critic called them fake, yet all scored about 4.2 against professional films at about 6.5–8). Three new failures:
  - **One trick for the whole film:** fireworks for 32 s, a clock-numeral mask used three times, 30 s on one pink form. The device must *escalate*: at least three distinct states, scales or media, and no single move used more than twice.
  - **Flat light:** light and texture was the lowest score in every film (2–3.5 out of 10), because grounds were flat gradients or flat paper. Put the world on a lit surface: a `scene3d` plane with a raking key and real material, the screen as a lit object, or ink in the paper's tooth with a shadow.
  - **The product never seen:** the hardware film showed a form about the recorder but never the recorder. A physical product appears, really modelled, by the end at the latest.
- **Rushing:** these three were finished in about 20 minutes, with 3 technique tests where the skill asks for 4–8. A strong film takes 60–90 min: the research and the tests are where the idea comes from.

## The steps

0. **`launch/0-type.md`: film type and cuts** (see `modes.md`).
   - **Pick one film type:**
     - **Proof**, the product working (most software and AI launches);
     - **Idea film**, a brand film told through type, diagram or graphic system;
     - **Object teaser**, a physical product as the only 3D subject.
   - **Pick the cuts:** which platforms and aspect ratios (X 16:9 hero 30–60 s; Reels/TikTok 9:16 15–34 s; teaser 6–10 s). Each cut gets its own composition.
1. **`launch/1-promise.md`: promise, truth, moment** (see `idea.md`, `viral.md`).
   - **The unownable truth:** what competitors can't claim.
   - **The promise:** ≤ 15 words, without the product name.
   - **The shareable moment:** one event, ≤ 15 words. Something *happens*.
   - **The target emotion:** awe or amusement.
   - **The send test:** who would DM this, to whom, and why.
   - **The hostile retell:** the cruelest one-line description, which must not be damning.
   - **Not in this film:** list the brief's other facts.
1b. **`launch/refs/`: research and references** (see `research.md`).
   - **The subject's world and users:** insider details, in `refs/world.md`.
   - **Professional work, with the moves to steal** (`refs/refs.md`):
     - `python3 scripts/judge.py refsheet --tags product-launch,<style> --out launch/refs/sheets` gives frame sheets of professional films;
     - the curated reference lists in `docs/video/craft/launch-ai-viral.md` §5 and `launch-visual-craft.md`;
     - web search.
   - **The product's real material:** UI, screenshots, logo, colours, its actual output.
2. **`launch/2-ideas.md`: ≥ 4 ideas, then pick one** (see `idea.md`).
   - Use different families, one out there, each citing an insider detail and a reference move.
   - **Gates:**
     - **literal:** the idea survives deleting the product name and mechanism;
     - **swap:** it fails with a competitor's name on it;
     - **renderable:** no simulated reality;
     - **fresh:** it doesn't repeat an earlier film's look;
     - **proof-inside-the-moment:** delete the product and the moment no longer makes sense.
3. **`launch/3-system.md`: one visual system** (see `world.md`).
   - **The derivation line:** *product behaviour → visual rule*.
   - **One material world, light logic and palette family**, from the brand and product.
   - **Type:** ≤ 2 families, ≤ 3 sizes, on a declared grid.
   - **Motion:** ≤ 3 easing curves and ≤ 3 duration tiers.
   - **The CG blacklist and banned looks.**
   - **What changes per shot:** subject, scale, camera.
3b. **`launch/tests/`: play** (see `research.md`).
   - Make 4–8 quick technique tests across ≥ 3 engine capabilities, one new to you.
   - Compare them on one sheet. Note what surprised you, what looked stock, and **what looked artificial**.
4. **`launch/frames/`: three style frames** of the key moment, compared side by side.
   - Run the **fake-look check** (`review.md`) on them before going on. If any frame looks simulated, templated or AI-made, change the system, not the polish.
5. **`launch/4-beats.md`: the beat sheet on the music** (see `shape.md`, `viral.md`).
   - **Frame 0:** a poster, never a title card.
   - **Product in motion:** by 2 s.
   - **The moment:** by 3 s in short cuts.
   - **The magic shot:** input and result in one unbroken shot.
   - **One hard stop** before the payoff.
   - **An ending made of the film's world**, with the real URL or handle.
   - **Mute test:** it reads with the sound off.
   - **Loop:** vertical cuts loop.
6. **Pass 1.** Build, preview, then render with a contact sheet: `render --sheet launch/pass1.png`.
7. **`launch/5-crit.md`: two critiques** (see `review.md`).
   - **The blind comparison** against professional films (`judge.py run … --practice`).
   - **A fresh human-eye critic.** It sees 6 stills and answers: does any frame look like a simulation, template or AI? What does the product do? What's the hostile retell?
   - **Sort every gap** into IDEA, WORLD or CRAFT. **An IDEA gap or a fake-look YES forces a new idea or system, not polish.** Cut the weakest third.
8. **Pass 2.** Rebuild, render (`ANIKIT_RENDER_CACHE=0`, foreground), and critique again.
   - Ship the better pass (a difference under 0.75 on the judge is a tie; the fake-look check decides).
   - At most 3 passes. Copy the shipped pass to `final.mp4`.
9. **`launch/6-ship.md`: cuts and post.**
   - Render each platform cut as its own composition.
   - **The post copy:** a concrete first line stating the outcome; one explicit ask; the real URL.
   - **The proof object:** a real metric, file, repo or "unedited" label.
   - **Honest labels:** "sped up" wherever time is compressed.

## Ground rules

- **Honesty floor:** use only the brief's real facts, names, UI and output. No invented numbers, quotes, reviews, logos, customers or capabilities. No staged demos presented as live. No engagement bait.
- **If the product makes media** (video, images, music, code, text), the film is its own proof: show real output it made, and say so.
- **Vary across films, hold within one.** Don't repeat the last film's world (check `deliverables/launch*/`, `deliverables/demo/`).
- **Cost:** keep a final render under ~5 min (check the `preview` estimate). Spend time on research, tests and passes instead: about 60–90 min in all.
