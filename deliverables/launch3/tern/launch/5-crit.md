# 5 · Critique against professional films

## Crit 1 — pass 1 (dust in the sunbeam) · `crit1/pass1/result.json`
`judge.py run launch/pass1.mp4 --practice --trials 2 --format 16:9 --tags product-launch,cg-3d`
- **Overall 3.5** (full incl. motion/sound 3.89) · refs: tiffany-holiday 7.75, da-vincis-demons 7.94, wealthsimple-trade 3.06 (ref mean 6.25, delta −2.75) · judged professional 0/2 · rankings: [D tiffany, A davinci, **C us**, B wealthsimple] and [A davinci, C tiffany, **D us**, B wealthsimple] — we beat one pro reference in both trials.
- Scores: idea 4 · composition 3.5 · typography 3 · colour 4.5 · light/texture 4 · motion 3 · craft 3 · originality 3.

Gaps (verbatim) and sort:
- WORLD — "Build a real lighting setup: volumetric light through actual dust, a shadow that matches the object, and a light patch that moves over time to carry the idea of recording a moment"
- CRAFT — "Give the type a point of view: a custom or carefully chosen face, scale that relates to the window grid, and type revealed by the light itself rather than dropped onto the end card"
- WORLD — "Vary the shots and choreograph them: a macro of the grille, the gesture of lifting the device, sound visualised in the space, instead of the same wall framed five times"
- IDEA — "The frames need a real light-driven idea: choreograph the sun patch moving or the shadow interacting with the device, instead of reframing the same static setup."
- WORLD — "The product needs physically based materials, contact shadows and real geometry. Lighting the window should actually shape the object."
- CRAFT — "Replace the default italic sans with a considered typographic system that is part of the scene, not stamped on top of it."
Tells (verbatim, selected): "One composition repeated: a four-pane window light patch on a grey wall appears in five of six frames" (WORLD) · "Effects with no subject: generic particle dust and concentric ripple rings" (IDEA) · "The product … is a primitive capsule … reads like a stock 3D model" (WORLD) · "The idea is literal and thin: quiet room, light, device, name and tagline" (IDEA) · "Concentric ripple rings are a literal 'sound' signifier" (IDEA) · "end card uses a default heavy italic grotesk … dropped onto the wall" (CRAFT).
Critic defects: one window can't cast both the wall and floor patches; ring dots read as a halftone preset; too many same-size motes; shot 2 is an empty floor frame; slow linear drifts.

**Binding rule applied:** IDEA gaps present → idea replaced with one from a different family (not polished). New idea (from the rejects list's spirit, a structural/POV device): **the recorder's-eye view from inside a pocket** — the world (a sea at dusk) only arrives as a muffled sliver through the pocket mouth until the lift, when the mouth tears open past the frame and the sound opens up (score's low-pass sweeps open at the lift); it ends snapped back in the pocket on the woven label (name, "Lift to record.", the specs as the care-label line). New world: waxed-cotton lining + open sea (`pocket.glsl` + library `ocean`); the weakest third of pass 1 (floor shot, stop shot, repeated wall framings, the ring effect itself) was cut entirely. Spec: `spec-pass2.json`.

## Crit 2 — pass 2 (the pocket) · `crit2/pass2/result.json`
- **Overall 3.19** (full 3.39) · same refs (mean 6.33, delta −3.14) · judged professional 0/2 · ranked last in both trials.
- Scores: idea 3 · composition 3 · typography 2 · colour 4.5 · light/texture 4 · motion 4 · craft 3 · originality 2.
Gaps (verbatim) and sort:
- IDEA — "Needs an idea that connects the product (sound, listening, recording) to the imagery; the sea currently does nothing for the brand"
- CRAFT — "Needs real typography: … instead of a pasted card with default grotesk and spec bullets"
- WORLD — "Needs controlled light and material: a designed grade, crisp focal planes and texture in place of the blurry procedural water and muddy surround"
- IDEA — "The product and the idea of 'lifting to record' need to be dramatised visually … instead of a sunset plate and a text card."
- CRAFT — "The typography needs to be designed …"
- WORLD — "It needs real light and material … and varied, choreographed shots instead of drifting blur."
Critic: opening ~4 s read as murky mush; end card static for ~4 s; whooshes at mechanical intervals; a lone click at 12.5 s.

## Decision
3.5 vs 3.19 = 0.31 apart, under 0.75 → a **tie**. On a tie the rule is fewer IDEA/WORLD gaps: both carry two IDEA gaps; pass 2 adds "product never shown" and "muddy" world gaps, and pass 1 scored higher overall and on the critic's full score (3.89 vs 3.39) and beat a professional reference in both trials. **Shipped pass 1** → `final.mp4` (re-encoded from the pass-1 render at CRF 21 for the repo's file-size limit; same frames and audio), `contact.png` = its sheet, `spec.json` = its spec.
A third pass was not run. Lesson for the next film: both ideas lost on "the image doesn't make listening visible without a signifier". The next try should start from the audience's ritual (a musician catching the take they didn't plan), with a real performer or object at full fidelity.
