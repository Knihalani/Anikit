# 5 · Critique of pass 1

## Blind comparison (`judge.py run --practice --trials 2`, tags product-launch, ui-product → `crit1/pass1/result.json`)
- **Candidate overall 4.12** vs reference mean 6.46 (delta −2.34). Verdict FAIL, judged professional 0/2. Ranking: 3rd of 4 in both trials (Vertigo 9.31 and Tiffany Holiday 7.06 above us; Wealthsimple Trade 3.0 below us).
- **Gaps (verbatim):**
  1. "The type has no point of view: replace the default grotesk and slide-style headline placement with type that interacts with the image, as A's credits wrap the spiral" — **WORLD/CRAFT**
  2. "Build a visual metaphor for 'reading overnight' instead of showing UI cards; the clock numerals are the seed of an idea but are only used as background wallpaper" — **IDEA (device not used as a mechanism)**
  3. "No light, texture or depth; frames need layering, real material and composition that changes from shot to shot rather than one template" — **WORLD**
  4. "The overnight time-passing idea is set out as a slide. It needs a visual metaphor with weight, the way A makes the spiral literally come out of the eye." — **IDEA**
  5. "The typography is a default grotesk with no custom treatment. The giant clock numerals could become the typographic system, morphing, melting or interacting with the UI, rather than serving as wallpaper." — **WORLD**
  6. "The gradient ground has no light or texture. A real material world (bedroom light, dawn through a window) would give it depth and mood." — **WORLD** (we will NOT simulate a bedroom: banned by the system; the light comes from the numerals instead)
- **Critic stage:** motion 5, sound 6, craft 6. Fails: arc, freeze-frame, material. Pace FAIL: median shot 4.0 s, longest 6.0 s (target ≤ 2.0 / 2.5). Dead tail 26–31 s. Energy peak at 35 % (storm), then a walk-down. "Top studios would make the clock the transition engine… the flood collapsing… on a hard sync hit, then release into the morning calm."

## Fake-look critic (fresh subagent, 6 stills only)
1. **Simulation/template?** "No simulation or AI imagery. **All six look like a template**: navy-to-dawn gradient, giant clock numeral at ~20 % opacity behind content, rounded card, amber accent, Inter-style grotesk — the default Linear-ish SaaS launch kit." Frame 2: amber underline on a "Filed" row, so the accent stops meaning anything. Frame 5: a ghost card at the left edge looks like a compositing leftover.
2. **What it does:** "reads your mail overnight, sorts into needs-you and handled, drafts replies, you send them in the morning." Retellable moment: Dana's 11:58 PM ask answered by 7 am in one click — but the click isn't clearly seen, and the draft says "attached" with no attachment.
3. **Cruellest line:** "Thirty-two seconds of the same rounded rectangle, captioned by a clock."
4. **Would a top studio ship it?** "No — it has no idea of its own; the giant clock is wallpaper, not a mechanism."
5. **Weakest frame:** the ending ("Inbox · nothing waiting" pill over a faded 07:02 — inbox zero is the most worn-out ending). **One change:** "make the clock the spine of the film instead of a background… cards pile up and collapse into the brief as the numerals roll… land on a real tap of Send."

## Sorting and decisions
- **IDEA / fake-look YES (template):** both critics say the same thing — the device exists but isn't a mechanism. Per the rules, the *system* changes, not the polish. The promise and moment (the 07:00 handover, Send) passed critic 2's retell, so the idea family stays; the **world is rebuilt so the clock is the mechanism**:
  - **New derivation:** *Relay works inside the night → the night's inbox lives inside the clock numerals.* The numerals are a window (matte) onto the scrolling inbox, lit like a screen in a dark room (they are the light source). Rolling digits change the window; the storm floods the digits; at 07:00 the inbox **drains out of the numerals** and the brief rises; at the end the numerals (and the name) are filled with the morning's sent replies.
  - Accent meaning is fixed: amber only on "Draft ready" and Send.
- **WORLD:** light from the numerals (glow halo with a reason: screen light in the dark), depth from blurred numerals behind macro cards, grain kept.
- **CRAFT:** shots ≤ 2.5 s (15 shots), build the Send cascade to the climax at 70–78 %, no static tail (ending alive: rows still scrolling inside the name), draft copy fixed ("attached" removed), cursor holds on Send through the click.
- **Cut the weakest third:** the S7 brief-list duplicate is cut to one 2.4 s beat; the 6 s ending card is cut to 3.5 s; the 4 s first-light race becomes 2.4 s; the "Inbox · nothing waiting" pill is removed.

---

# Crit 2 (pass 2: the inbox inside the numerals) — `crit2/pass2/result.json`
- **Judge:** candidate **4.44** vs ref mean 6.25 (delta −1.81), 3rd of 4 both trials, judged professional 0/2. Critic: motion 5, sound 6, craft 6; idea ✓, arc ✓, invariant ✗, material ✗. Pace: median 2.2 s, longest 5.0 s.
- **Gaps (verbatim):** "The idea stops after the first act. The clock or time device should carry the whole film instead of giving way to UI slides" (IDEA→structure) · "The type is generic (default grotesk, slide placement)…" (WORLD) · "The gradient and pills look is generic; it needs a deliberate palette and real light or texture…" (WORLD) · "Push the clock-mask idea through the whole film…" · critic: "no single transition system linking night → brief → send → logo… ~3 s static tail".
- **Fake-look critic:** (1) nothing AI/simulated; reads as stock motion graphics because the type-as-window move is repeated, and the masks "look broken" — rectangular slabs on the 1 and 7. *Diagnosis:* the mask was correct; Bricolage's flat-footed 1 and the 7's bar, filled with cards, read as slabs. (2) Retell: "Dana's 11:58 PM board request is answered with one click before 9." ✓ (3) "Thirty-two seconds of a clipping mask counting to seven, sponsored by a Send button." (5) Weakest: the 07:00 frame washed out; the brief itself under-shown.
- **Pass 3 changes:** hairline glyph outline on every filled numeral (shapes read as type, not slabs); the numerals carry the second half (each Send fills the 07:0x numerals with sent replies, building to the full 07:02); brief held 3.4 s across two shots; 07:00 ink digits snap in at 0.2 s; tail cut to 3 s.

# Crit 3 (pass 3) — `crit3/pass3/result.json`
- **Judge:** candidate **4.25** vs ref mean 6.46 (delta −2.21), 3rd of 4, judged professional 0/2. Critic: **motion 6, sound 7, craft 6**; idea ✓, **invariant ✓**, ending ✓, sync ✓; arc ✗, material ✗. Pace: 17 shots, median 1.7 s, longest 4.4 s.
- **Gaps (verbatim):** "It needs a progression, not one mask trick repeated…" · "Replace the default UI card and grotesk with a typographic system that has a point of view…" · "Add material, light and texture (paper, screen glow, a real dawn) in place of flat gradients and an empty frame" · "Push the clock-mask idea through choreography: digits rolling, cracking or filling as mail is cleared…" · critic: "no second build to a 60–80% climax before the Relay resolve"; "the numerals never deform, carry weight or hand off with overlap".
- **Fake-look critic:** (1) "None of it looks like a simulation, a game render or AI imagery. The type and UI are clean and specific." Template smell from the knockout-type move used three times; the "1" edge again. (2) Retell: "Dana's 11:58 PM 'Q3 numbers before 9?' is sitting there answered at 7 AM, and all you do is click Send" — matches the promise and moment. The reply-all storm is "the one funny beat". (3) "A clock-digit mask effect looking for a product, then a Figma brief card and a cursor clicking Send." (4) No — "the clock-numerals-as-window gimmick carries the film instead of the product"; also the literal cursor.

# Decision (pass limit reached: 3)
- Judge: 4.12 / 4.44 / 4.25 — all differences < 0.75, a tie. Tie-break on gaps: **pass 3 ships.** Fake-look (1) is "no" for pass 3 (no simulation/AI), against pass 1's "all six look like a template". Pass 3 is the only pass with the critic's invariant check passing, the best motion/sound scores, and the best pace (median 1.7 s). Pass 2's illegible/washed 07:00 beat and the static tail are fixed in pass 3.
- **Open gaps we ship with (honest):** WORLD — the ground is still a gradient, not a lit material; the type system is a chosen grotesk pairing but the judge reads it as default; the knockout move repeats (night, 07:02, name) — that repetition is the device's escalation by design, but two critics read it as a stock trick. A 4th pass would make the numerals *perform* (roll/fill as mail clears) and replace the cursor with an implied tap.
