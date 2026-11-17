# Specimen 07: what held the engine back

**What it is:** a 10 s, 9:16, 30 fps generative piece. A living Gray-Scott reaction-diffusion organism grows from about 40 spores inside a petri dish, simulated live: 4 steps a frame, 120 generations a second, 1,200 in total. On every kick of the score, a new spore lands at a beat-chosen spot. The edit cuts on the beat: every 2 beats in the intro, every beat through the drop (beats 8–16). Each cut changes the microscope (zoom, centre, roll) and the stain: bone-white on black, ember on brown, and an inverted "printed plate" in black ink on bone. On beat 16 the camera pulls back to reveal the whole dish, with specimen labels and a live generation counter. Score: a hand-written 124 bpm piece (909 kit, glassy FM arpeggio, a "cell" pitch-drop percussion voice, 808 bass, glass pad, riser/impact into the drop, sub-drop at the end), −14.2 LUFS.

**Files:** `bloom.mp4`, `contact-sheet.jpg` (3 fps), `compare.jpg` (ours on the left, Sage Jenson on the right), `bloom.json`, `build.py`, `glsl/rd_step.glsl` (the simulation step, run as buffers A→B→C→D), `glsl/view.glsl` (dark-field lighting, membranes as constant-width iso-line filaments, 22 contour lines inside the body, the dish's glass wall, per-cut camera and stain), `score.json` → `score.wav`, `refs/`.

## References
| Ref | URL | What makes it great |
|---|---|---|
| Sage Jenson, *physarum* / *36 points* (4 stills) | https://cargocollective.com/sagejenson/physarum | Millions of agents give organic complexity at every scale. Volumetric, glowing filament walls with hot specular pinpoints. Monochrome, so form carries everything. Shallow depth of field gives the microscope feeling. |
| Refik Anadol, *Machine Hallucinations* (König Galerie) | https://refikanadol.com/works/machine-hallucinations-nature-dreams/ | A dense "data pigment" particle mass that behaves like fluid fabric. Scale and density read as material. |

## Honest verdict next to the references
The cutting, the stains and the reveal work, and it reads as a real organism growing on the beat. At 100% the printed-plate cuts look engraved. Jenson's work is in another league for organic complexity: agents (physarum) in 3D with volumetric light, against our 2D Gray-Scott, which forms simple worms and rings at 1,200 generations. The engine's per-frame step budget and the lack of particle↔field coupling are why.

## Gaps, ranked
1. **No agent/particle simulation that writes into a field.** Physarum (Jenson), boids leaving trails and ink diffusion all need particles depositing into a texture and reading it back. Fragment buffers can only *gather*, never *scatter*. scene3d particles can't read or write a 2D field either. *Workaround:* Gray-Scott, which is gather-only. *Want:* a `sim` layer with N agents (positions in a float buffer), a deposit pass (splat to a buffer) and a sense pass, or a scatter/atomic-add op for buffers.
2. **Only 4 buffer passes a frame cap the simulation speed.** A Gray-Scott pattern needs thousands of steps, so this one gets 120 a second. *Workaround:* a coarse stencil (`SC = 3.6` px offsets plus 1-px coupling) to make the pattern grow 3.6× faster in screen space. *Want:* `"steps": N` on a buffer (run the pass N times a frame), or buffer ping-pong loops.
3. **Chunked parallel render replays the feedback simulation in every chunk.** Expensive layers render as up to 16 chunks, and each chunk replays the buffers from frame 0. That gave 1 fps and 27 min of CPU for 300 frames. The engine's own estimate said 94 s; it took 277 s of frames plus ~2.5 min of setup. *Want:* chunk boundaries that carry buffer state (render feedback layers sequentially and encode in parallel), and an estimate that counts replays.
4. **The simulation depends on resolution: `still --width`/`--scale` and `--quality draft` change the outcome.** Buffers are layer-sized, so a 540-wide preview ran a *different* organism. Previews were misleading until I rendered every still at full size (7–35 s each). *Want:* buffers at a fixed declared size (`"buffers": {"size": [w, h]}`) independent of output scale, and previews that upsample them.
5. **`iBeat`/`iAudio` ignore generated `music`.** They only follow an `audio` cue with a `src`, so with `"music": {...}` the buffers silently got 0 (a warning, but the fix isn't obvious). *Workaround:* render the score to `score.wav` with `anikit-video music`, use it as `audio`, and compute the beat in GLSL from `iTime` and the BPM. *Want:* generated music drives `iBeat`/`iAudio`, plus a `uniform beat` built-in.
6. **There's no way to prewarm a simulation.** I wanted the organism to have run for 5 s before frame 0. `trimStart` on a composition was the only route, and it multiplies the replay cost of every still. *Want:* `"prewarm": seconds` on buffer layers, computed once and cached.
7. **Shader-only pieces get almost no help from `preview`/critique.** The contact sheet is slow (it runs the simulation), and the critique can't judge shader pixels.
8. **I lost 20 minutes to my own bug** (a uv computed with the coarse stencil's `px`), with zero diagnostics. A `debug: "buffer:A"` option that shows a buffer's channels as the output would have found it in one still. (I first suspected composition replay; that suspicion was wrong and I've withdrawn it.)

## What worked
- Buffers A–D with feedback, float precision and exact determinism. The simulation was bit-stable across stills and the final render.
- Lighting a simulation field in the image pass: gradient normals, constant-width iso-line walls and contour engraving give a crisp look at 1080×1920 from a coarse field.
- The `{value:,}` text formatting with an expression gave an honest live generation counter.
