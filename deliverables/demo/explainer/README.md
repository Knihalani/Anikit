# i-explainer rebuild: how noise-cancelling headphones cancel sound (30 s, 16:9)

**Device:** a single dark liquid surface.
- Sound is a ring on it.
- Noise is thousands of rings.
- The earcup is a brushed-metal ring that rises out of the liquid. Inside it the speaker's mirror wave shows as cyan troughs under the amber crests, and the surface goes glass-still.
- The type lies *under* the liquid and bends with every wave. It reads cleanly only when the water is still.
- At the collapse (21 s, 70 %) the ink-black liquid turns bone-white and the room goes silent.

**Sound is the subject:**
- The noise bed is cancelled by summing it with its own inverted copy, exactly as the earcup does. The residual is 0.
- Every ring the shader births gets a pressure thump, panned to its place. The timing comes from the shader's own formula.
- A Shepard riser builds into the collapse.
- After the collapse there is true digital silence (even the room tone stops), then one glass note in D.
- Mastered by the spec's `"sound"` key to −14 LUFS.

| | overall | idea | comp | type | colour | light | motion | craft | orig | critic (motion/sound/craft) |
|---|---|---|---|---|---|---|---|---|---|---|
| r1 | 2.75 | 3 | 3 | 2 | 3 | 3 | 3 | 3 | 2 | |
| r2 | 2.88 | 3 | 3 | 3 | 3 | 3 | 2 | 4 | 2 | |
| v2 | 4.0 | 4 | 4 | 3 | 4 | 5 | 4 | 5 | 3 | 6/5/6 |
| v3 | 3.62 | 4 | 4 | 3 | 4 | 4 | 4 | 3 | 3 | 6/6/6 |
| **v4 (final)** | **4.25** | 4 | 5 | 4 | 4 | 5 | 4 | 5 | 3 | 6/6/7 |

Same judge invocation as r1/r2 (`scripts/judge.py run … --tags explainer-2d,editorial-data,illustration,motion-design`, 3 trials). Full results are in `judge/`. The jury still ranks it below the Kurzgesagt and Cosmos references (0/3 professional).

What they still ask for: a more specific, ownable metaphor (a physical object, a real space, a human moment), more change of scale between shots, and type that is authored further.

**Files:**
- `spec.json`: built by `build.py`.
- `surface.glsl`: the whole world, 12 animated uniforms.
- `mix.py` and `stems/`: the soundtrack.
- `contact.png`
- `final.mp4`: the engine output re-encoded with x264 CRF 20. The animated grain makes the engine's own file about 260 MB.

The render took 253 s with `ANIKIT_RENDER_CACHE=0`, and `scripts/defects.py` reports it clean. It needed one engine fix: render estimation panicked on shader channels that hold layers (commit "Fix render estimate panic…").
