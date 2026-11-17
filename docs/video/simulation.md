# Simulation in `scene3d`: rigid bodies and particles

`scene3d` (see [components.md](components.md#3d)) can simulate thousands of
rigid bodies and millions of particles inside its ray-traced scene. Both are
simulated once per spec, deterministically, and cached, so a render can be split,
resumed or rendered out of order and still get identical frames.

Examples (`examples/video/sim/`):

- `dominoes.json`: 10,000 glass panes with live video textures falling as dominoes;
- `particles.json`: 3M sand particles swirling into a word, with self-shadows and floor shadows;
- `glow.json`: 2M glowing particles with disc bokeh and motion streaks;
- `custom.json`: everything as expressions (spawn ring, vortex force, colour by age and height);
- `bridge.json`: a chain of beads pinned at both ends, with boxes raining past.

## Rigid bodies: `physics`

```json
"physics": {
  "gravity": -9.8,
  "timeScale": "=1 + 2*smoothstep(1, 1.8, t)",
  "bodies": [
    { "type": "pane", "count": 10000, "layout": "dominoes", "path": "fan", "angle": 110,
      "size": [0.9, 1.6, 0.03], "material": "glass",
      "texture": { "composition": "film", "frame": "=(i*7 + t*30) % 120", "face": "both" } }
  ],
  "impulses": [ { "t": 3, "at": [0, 0.2, -8], "strength": 6, "radius": 3 } ]
}
```

A body group is an ordinary scene object (`type`, `size`, `material`, `texture`,
`scale` …) plus physics keys. Its transforms come from the simulation.

| `physics` key | Default | Meaning |
|---|---|---|
| `bodies` | required | Body groups (below). |
| `gravity` | `-9.81` | Metres/s² down, or `[x, y, z]`. |
| `timeScale` | `1` | Simulated seconds per video second; an expression of `t` gives speed ramps, e.g. `"=1 - 0.8*smoothstep(2, 2.2, t)"` for slow motion from 2 s (expressions have no `?:`; use `smoothstep` or `step`). |
| `start` | `0` | Video time when the simulation starts; bodies hold their layout before. |
| `ground` | `true` | A fixed ground at the floor's `y` (`false`, or a number for another height). |
| `friction`, `restitution` | `0.5`, `0.05` | Defaults for groups. |
| `rate` | `120` (`240` if any body is under 2 cm thick) | Solver steps per simulated second. |
| `iterations` | `4` | Solver iterations per step. |
| `record` | `60` | Recorded poses per video second; frames in between interpolate. |
| `freeze` | `0.4` | Seconds a body must be still before it's frozen in place (`false` = never). |
| `impulses` | none | Radial pushes `{ "t", "at": [x, y, z], "strength" (m/s at the centre), "radius", "up" (0.3) }`. They also unfreeze bodies they reach. |

| Body group key | Default | Meaning |
|---|---|---|
| `type` or `instance` | `box` | Any object type, or (`instance`) the `name` of an object in `objects`, which then serves as a template and isn't drawn on its own. |
| `count` | `1` | Bodies in the group (max 200,000). |
| `layout` | `pile` (`expr` with `pos`) | `dominoes`, `grid`, `wall`, `pile`, `rain`, `expr`. |
| `density` | `2.5` glass, else `1` | Mass per volume (relative). |
| `friction`, `restitution` | from `physics` | |
| `velocity`, `angular` | `0` | Initial velocity (m/s) and spin (degrees/s): `[x, y, z]`, may use `i`, `n`. |
| `ccd` | `false` | Continuous collision for very fast bodies. |
| `awake` | `true` (`false` for dominoes) | Asleep bodies don't move until something touches them. |
| `seed` | group index | Randomness for `pile` and `rain`. |

**Layouts.**

- `dominoes`: standing panes, thin axis along the direction of travel; the
  first one is tipped (`trigger`, rad/s, default `1.6`; `false` for none).
  Pane size defaults to `[0.9, 1.6, 0.03]`. `path`: `fan` (rows of arcs
  `spacing` apart, widening over `angle` degrees: the wave sweeps away as a
  growing arc), `line`, `spiral` (arms `pitch` apart) or `snake` (sine,
  `turns`, amplitude `angle` metres). `spacing` defaults to 0.55 × height,
  `gap` (between panes in a row) to 0.12 × width, `heading` to `180` (away
  from a camera on +Z), `origin` to `[0, 0, 0]`.
- `grid`: a lattice `grid: [nx, ny, nz]` at `spacing` (default: the body size),
  centred on `origin`; `jitter` adds random yaw (degrees). `wall`: the same
  with every other row offset by half a body (bricks).
- `pile`, `rain`: a jittered lattice filling `region: { "min": [...], "max": [...] }`
  (default: a column above `origin`); `pile` adds random orientations;
  `stagger` (m) spreads the drop heights.
- `expr`: the object's own `pos`/`rot` expressions of `i`.

**Joints.** `"chain": "ball"` (or `fixed`, `hinge`, `spring`, `rope`, or
`{ "type", "stiffness", "damping" }`) links each body of a group to the next
at their midpoint: chains, ropes of beads, hanging banners, bridges.
`"pin": "first" | "last" | "ends" | [indices]` fixes bodies to the world.
Any other constraint goes in `physics.joints`:

```json
"joints": [ { "type": "hinge", "a": [0, 3], "b": "world", "anchor": [0, 2, 0], "axis": [0, 0, 1] },
            { "type": "spring", "a": [0, 0], "b": [1, 0], "length": 0.5, "stiffness": 300, "damping": 6 } ]
```

`a` and `b` are `[group, index]` (or `"world"`); `anchor` defaults to the
midpoint between the bodies at the start; hinges turn about `axis`; ropes
hold bodies at most `length` apart; springs pull to `length` with
`stiffness` and `damping`. Jointed bodies don't collide with each other.

**Any shape.** A body group is any scene object: `{ "type": "model", "src":
"part.glb" }` (glTF; it collides as the convex hull of all its parts),
`{ "text": "A" }`, a torus, a custom-sized box.

**Colliders.** Boxes, slabs, panes and phones collide as boxes, spheres as
exact spheres, cylinders as cylinders, and text, tori and other meshes as their
convex hull. Any object in `objects` with `"collide": true` takes part:
static objects as fixed colliders, animated ones (`keys`, expressions of `t`)
as kinematic pushers that shove bodies along their path.

**How it runs.** Rapier (pure Rust, `enhanced-determinism`: the same spec
gives the same motion on every machine). The first frame that needs time `t`
simulates up to `t` and records every body at `record` Hz; other threads wait.
Standing dominoes sleep until hit; bodies that have been still for `freeze`
seconds become fixed, so a pile of thousands of fallen panes costs nothing
and only the wave front is solved.

**Cache.** Records are also appended to `$ANIKIT_CACHE/sim/phys-<hash>.bin`
(default `~/.cache/anikit`; `ANIKIT_CACHE=off` disables it), keyed by
everything that affects the motion. Each record stores only the bodies that
moved, exact `f32`. A later `still` or render of the same physics reads the
frames instead of simulating: a cold 1080p still of the 10k cascade at 5 s
goes from 9.9 s to 3.0 s, with identical pixels.

## Particles: `{ "type": "particles" }` in `objects`

```json
{ "type": "particles", "count": 2000000,
  "emit": { "text": "ANIKIT", "height": 1.0, "pos": [0, 0.6, 0] },
  "flow": { "strength": 0.8, "scale": 0.8, "evolve": 0.2 },
  "gravity": 0.15, "drag": 0.4, "life": 4,
  "size": 0.0035, "color": ["#ffb35c", "#ff5a1f", "#7a2bff"], "colorBy": "age",
  "emissive": 3, "blend": "add" }
```

| Key | Default | Meaning |
|---|---|---|
| `count` | `100000` | Up to 20,000,000. |
| `emit` | sphere r = 1 at `[0, 1, 0]` | Where particles are born (below). |
| `velocity`, `spread`, `normalSpeed` | `0` | Initial velocity, random speed, and speed along the emitter's surface normal. |
| `life` | none | Seconds; particles are born spread over one life and recycle. Without it they're all born at `start` and live for ever. |
| `flow` | none | Curl-noise flow: `{ "strength" (m/s), "scale" (m, feature size), "evolve" (how fast the pattern changes), "drift": [x, y, z] (m/s) }`, or a number (strength). |
| `attractors` | none | `[{ "pos": [x, y, z] or "=[…t…]", "strength", "swirl", "radius" }]`. |
| `gravity` | `0` | m/s² up (number) or `[x, y, z]`. |
| `drag` | `0.2` | Per second. |
| `target` | none | Assemble into a shape: an emitter plus `from`/`to` seconds. Each particle springs to its own point on the target, the lowest arriving first. |
| `collide`, `bounce`, `friction` | `true`, `0.3`, `0.3` | Floor collisions. |
| `size`, `sizeVar` | `0.004`, `0.5` | Radius in metres, and random variation. |
| `color` | `#ffffff` | A colour or a gradient (array), picked by `colorBy`: `random`, `age`, `speed` (over `speedRange` `[0, 2]` m/s), `height`, `source` (image emitters' pixels, the default for them). |
| `opacity` | `1` | |
| `emissive` | `1` for `add`, `0` for `over` | Emitted light. |
| `blend` | `add` | `add`: glowing light. `over`: particles absorb and reflect light (dust, sand, smoke, confetti); dense clouds turn into volumes. |
| `lit` | `true` for `over` | Shade by the key light and environment, with a sphere phase function; particles that have arrived on a `target` shade like its surface. |
| `shadows` | `true` for `over` | Absorbing particles shadow each other (dense clouds darken inside, like volumes) and cast soft shadows on the floor and objects. |
| `stretch` | `1` | Motion streak length (× shutter). |
| `substeps` | `2` | Integration steps per frame. |
| `start`, `end`, `seed` | `0`, ∞, `1` | |

**Anything custom: expressions.** Every particle key below that takes a
value per particle also takes an expression of the particle: `x`, `y`, `z`
(m), `vx`, `vy`, `vz` (m/s), `t` (s), `age` (0–1 over its life), `i` (its
id), with all expression functions (`sin`, `noise`, `rand`, `mix`, `clamp` …):

```json
{ "type": "particles", "count": 1500000, "life": 3,
  "emit": { "pos": "=[1.6*cos(rand(i)*6.2832), 0.05, 1.6*sin(rand(i)*6.2832)]", "vel": "=[0, 0.4 + 0.6*rand(i+3), 0]" },
  "force": "=[-z*2.2 - x*0.6, 0.35 + 0.4*noise(t + x*2), x*2.2 - z*0.6]",
  "size": "=0.0018 + 0.0022*rand(i+11)",
  "color": "=[0.4 + 1.6*age, 0.6 + 0.6*sin(y*2 + t), 2.0 - 1.4*age]",
  "opacity": "=1 - age" }
```

- `force`: an acceleration field (m/s²), added to gravity, flow and attractors.
- `emit.pos` / `emit.vel`: a custom spawn rule (`i` the particle, `age` its spawn cycle).
- `size`, `color` (linear RGB), `opacity`: per particle, per frame.

Expressions cost about 30–60 ns each per particle and substep, so a custom
force on 1.5M particles adds about 0.2 s per frame (`examples/video/sim/custom.json`).

**Emitters.** `{ "shape": "point" | "sphere" | "ball" | "box" | "disc", "pos", "radius", "size", "axis" }`
(`sphere` emits on the surface, `ball` inside); any scene object description
(`{ "text": "ANIKIT", "height": 1 }`, `{ "type": "torus", … }`), sampled
uniformly over its surface; the `name` of an object in `objects`; or an image:
`{ "image": "logo.png" | "composition": "name", "frame": 0, "size": [w, h] (m), "pos", "rot", "threshold" }`
(pixels become points with their colour, weighted by alpha, or by brightness
when the image is opaque).

**How it runs.** Positions and velocities are flat `f32` arrays stepped in
parallel chunks. The flow is the curl of a periodic noise potential,
precomputed on a 64³ grid (two of them, cross-faded over time), so it is
divergence-free (no clumping) and costs two trilinear lookups per particle.
The simulation steps frame by frame; frames a render batch asks for out of
order come from a snapshot cache (positions plus 16-bit velocities), and
earlier frames restart from checkpoints kept every 2 s. Checkpoints are also
written to the disk cache (`$ANIKIT_CACHE/sim/parts-<hash>-<frame>.bin`,
exact), so a cold still at 4.5 s of 3M particles steps 15 frames from the
4 s checkpoint instead of 135 from the start: 20.6 s → 3.3 s, identical
pixels. The cache keeps itself under `ANIKIT_CACHE_MAX_GB` (default 8),
oldest files first.

Each particle is projected and drawn with the footprint a real lens gives
it: a disc of its own projected size combined with its defocus (circle of
confusion), seen through a tent pixel filter.

- **Up to 4 px** (in focus and slightly defocused): an exact stamp from a
  precomputed table (radius in 1/16 px, 16×16 sub-pixel phases, bilinear
  between phases).
- **Larger** (bokeh): a point splat into a defocus bin at quarter-octave
  radii (4 … 64 px), held at the pyramid level where its disc is 2–3 texels
  across. Each used bin is convolved with its disc once, so out-of-focus
  particles get real disc bokeh at the cost of a point splat. Each particle
  is split between the two bins bracketing its blur, matched in variance.
- **Motion:** fast particles are drawn several times along their screen
  motion over the shutter (camera motion included), at most one texel apart.

Particle slots are kept in Morton (Z-curve) order of position, re-sorted
every second with stable ids, so neighbours in space are neighbours in memory
and both the splats and the flow lookups stay in cache.

**Fidelity.** `cargo run --release -p anikit-video --example particles_reference`
renders 1080p frames with the fast path and with a reference: exact discs
sampled densely, 32 instants over the shutter. It compares them with PERF's
qbench thresholds (SSIM ≥ 0.995, PSNR-Y ≥ 42 dB, ΔE p99.9 ≤ 2.3); see the
table in the PR notes for current numbers.

Particles are depth tested against
the traced scene and composited before bloom, so bright ones glow.

Shadows use an opacity shadow map from the key light: 384² texels × 64
slices along the light, filled with each particle's optical depth (tent
filtered in all three axes), accumulated from the light side. Particles look
up their own transmittance (biased two slices towards the light, so a surface
doesn't shadow itself) and every scene pixel's key light is multiplied by the
transmittance at its world position before the denoiser.
