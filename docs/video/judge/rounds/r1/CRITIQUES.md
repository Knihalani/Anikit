# Round r1: recurring critiques by domain

12 Anikit briefs + 3 HyperFrames, built on main `75cc69ab`. **15 of 15 FAIL, 0 of 45 trials judged professional.** Anikit scored 2.75–4.5 (median 3.44), refs 6.87–8.67. HyperFrames on the same briefs scored 2.75–3.38 (ours: 3.75, 3.62, 3.62 on those three briefs). Per-dimension medians (ours): originality 2, typography 3, light/texture 3, motion 3, idea 3.5, composition 4, colour 4, craft 4. Critic yes/no checks failed: freeze-frame poster 14/15, energy arc 11/15, material at 100% 8/15, type carries meaning 6/15, sync 5/15, ending 5/15.

Best: k-sports 4.5 (floodlight bulbs become the 19:45 dot-matrix: a real device). Worst: h-ident and i-explainer 2.75.

Compared with the rejected div-a…g pieces (median 3.38), r1 is about level. The changes merged since then haven't yet moved what the jury sees.

## 1. Idea (SKILLS / TASTE): 15 of 15
- Literal readings of the brief: "Lisbon = tiles", "kestrel in front of the sun", "the name pun on rails", "word + waveform", "macro of dial, then specs", "textbook diagram". The jury asks every time for **one device that the whole film is built from**, drawn from the subject's own behaviour: the kestrel's stoop drawing the K, the engraving tool's path writing the type, an offset value driving every move, the drawing physically becoming the world.
- When a good device exists, it is **played once and never escalated** (g-titles offset, f-3d light carving the guilloché, k-sports floodlight digits).

## 2. Typography (TYPE): 15 of 15
- The same default lockup in every category: a centred serif or bold grotesk headline, a tracked-caps or mono label, sitting *on* the image "like a slide". The jury's words: "default", "no point of view", "wordmark plus caption end-card".
- What it asks for: a face chosen for the world (or custom lettering), **extreme scale contrast**, and type that lives *in* the scene: built from the tile system, emitted by the switchgear, reacting to the sound, written by the tool.

## 3. Composition and structure (GRAPHIC): 14 of 15
- "One layout reused with the words swapped": the same frame recurs across beats, and the freeze-frame poster test fails 14 of 15 times.
- Flat vector shapes on a solid or gradient field; mostly empty frames.

## 4. Motion (MOGRAPH): 15 of 15
- **No weight**: linear slides, crossfades, sine bobs and snaps, with no anticipation, overshoot or settle.
- **No hand-offs**: shots are joined by dissolves or whoosh cuts, and no object carries across a cut as a match-cut or morph.
- **No arc**: 11 of 15 have no build to a 60–80% peak. The energy either peaks early and dies (a-social, hf-g-titles, k-sports) or stays flat (d, j).
- **Dead holds**: static end cards of 2–3 s, and about 3 s of hovering in a 6 s ident.
- **No camera**: locked-off planes; asks for push-ins, drifts and moves through layers.

## 5. Light and material (3D / SHADER / GRAPHIC): 13 of 15
- "Flat vector", "glowing disc", "grey vignette", "procedural pattern under flat light". The jury asks for ceramic glaze and crazing, brushed-steel anisotropy and sapphire reflections, ice texture and coastlines, paper and grain.
- f-3d scored highest on light (5) but still "a grey vignette, not a lit set".

## 6. Sound (SOUND): sound is one of the critic's top 3 gaps in 14 of 15; sound scores 4–6 (median 4)
- Thin generated beds: static pad, pluck loop, noise risers, the same click sample repeated eight times.
- No foley drawn from the subject: tile clinks, crown ratchet, switchgear hum, a tick per data year.
- Hits don't land on picture events (5 of 15 fail sync), and there is no arrangement arc.

## 7. Performance (PERF)
- Agent wall time was 8–26 min, of which 2.5–6 min was the cold cargo build (r2 uses the prebuilt release). Cost per brief $1.14–3.15, about $33 for the round.
- Final render 15–110 s for most briefs, but **i-explainer took 291 s, j-music 237 s and l-generative 1,056 s** for 20–30 s films. Peak RSS was 0.57–1.75 GB. l-generative's agent also ended its turn with the render still running.

## What would move the score most (from the jury's own gaps)
1. A device-first concept step that names the one device and escalates it in every beat.
2. Type set inside the image, with real scale contrast and a face that isn't the default.
3. A motion grammar with weight (anticipation, overshoot, settle), match-cut hand-offs and a 60–80% peak.
4. A lit, textured world: grain, materials, light falloff.
5. Foley drawn from the subject, synced to picture events.
