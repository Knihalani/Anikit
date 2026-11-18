# l-generative demo rebuild: "Signal" (SHADER)

The piece is the wood-wide web. Under a night forest floor, trees signal to each other through a mycelial network. The first tree fires in cold light, and the signal runs along the veins at a finite speed. Each tree it reaches answers in warm light, and the cascade lights the whole floor. On the 15 s hit, the veins converge into the season's name, which then lies in the forest floor among the trees.

There are five shots: a macro on the first root node; low across the floor among the trunks; high over the cascade; top-down on the word as it ignites; and back down to the word lying in the floor. Each ignition lands on a bell in the score (1.5, 4.0, 6.5, 8.0, 9.5, 10.5, 11.25 s). It is a single stateless shader (`signal.glsl`) plus the word as a channel.

| pass | change | judge (`judge.py brief l-generative`) |
|---|---|---|
| r1 | (eval) | 3.12 |
| r2 | (eval) | 3.62 |
| 1 | Voronoi network grows from a spark into the word, top-down pull-back (`signal-pass1.glsl`) | 3.38 |
| 2 | Forest world: perspective shots, trunks, fog and DoF, cuts, monumental serif wordmark | 3.12 |
| 3 | Organic mycelial veins (noise isolines), colour by which tree's signal arrived, the word lying in the floor in perspective, anticipation before the first spark | 3.12 |

Plateau: every pass is within the judge's ±0.4 noise of r1 and r2, and none beats r2. The jury keeps naming the same gaps:

- It is an effect with no subject.
- The type is set, not designed.
- The light is procedural defaults.

They read the trunks as "pillars" and the veins as "noise contours". The story (trees talking) does not come across in five sampled frames.

Final render: 154 s, 600 frames, `ANIKIT_RENDER_CACHE=0`. `scripts/defects.py` is clean.
