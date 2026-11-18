# Simulation craft: what makes particle, fluid and physics motion look expensive

Research notes for agents and engine work. Advice, not rules. The aim is to
separate the studio look (Houdini, X-Particles and Redshift title work, the
abstract "ink in water" and "smoke type" spots) from the amateur one, and to
list what Anikit still lacks.

## Principles

1. **Motion with intent.** Studio particle work is "random motion with
   intent" (Jason Poley's Node Fest titles breakdown): noise for texture, but
   one dominant force that reads from across the room (an attractor pulling
   to the logo, a wind front, a vortex). Amateur work is uniform turbulence:
   everything jitters, nothing travels.
2. **Scale tells.** Big sims look big because of a range of scales: large
   slow eddies carrying small fast detail. One noise frequency reads as a
   screensaver. Use 2–3 octaves, with the large one slow (period ≥ 2 s) and the
   small one about 4× finer and weaker (0.3–0.5 amplitude).
3. **Detail preserved, not added.** Numerical sims lose swirl: grid
   diffusion kills vortices. Vorticity confinement (Fedkiw, Stam & Jensen
   2001) puts the curl back. That's the difference between "milk" and "ink
   in water". Over-applying noise instead gives unnatural motion
   (SideFX/cgchannel FLIP tips).
4. **Shading sells the medium.** The same density field reads as smoke,
   ink, fire or dust only through shading:
   - ink: absorption (Beer–Lambert) with colour that shifts with thickness;
   - smoke: self-shadowing from a key light, a bright rim and a dark core;
   - fire: emission ramped by temperature.
   Vorticity and speed are good shading drivers (a ramp from vorticity is a
   standard Houdini trick for showing mixing).
5. **Density contrast.** A frame needs voids. Particle clouds that fill the
   frame evenly look cheap. Use 10–30% of the frame for the dense core, wispy
   tails and empty negative space.
6. **Trails describe motion; don't double it.** Geometry trails already
   carry the motion, and heavy motion blur on top makes them muddy (C4D
   trails guide). Vary trails by group or field, not uniformly at random.
7. **Timing.** Sims are best as one event: build, release, settle. Speed
   ramps around the impact frame (2–4× fast, then 0.25–0.5× slow-mo for 0.5–1 s).
   Slow sims under a slow camera are the "boring" rejection in BAR.md.
8. **Lens and light.** Depth of field with real bokeh, a key light raking
   at 30–60° across the volume, rim light from behind, and bloom only from
   real highlights.

## Techniques with numbers

| Look | Technique | Starting numbers |
|---|---|---|
| Ink in water | 2D or 3D stable fluids, dye advection, vorticity confinement, absorption shading | Grid 256²–512², confinement ε 0.2–0.5, dye dissipation 0.995 per frame, 20–40 Jacobi iterations |
| Smoke type | Density from glyph masks, buoyancy up, curl noise and confinement | Buoyancy 1–3 m/s², 2 noise octaves, self-shadowed |
| Particle dissolve | Particles on the logo surface, a wiping field releases them, curl flow carries them | Release front 0.5–1 s across the word, flow 0.5–1.5 m/s |
| Assemble | Particles spring to targets, staggered by position | Lowest or leftmost first; 1.5–3 s total; critically damped |
| Trails / ribbons | Draw each particle's last N positions as a strip, width and opacity tapering to the tail | 8–32 samples, tail alpha → 0, width ∝ speed |
| Dust in light | `over` particles, self-shadowed, key from behind (backlit) | Size 1–3 px, 1–10M, opacity 0.3–0.7 |
| Dominoes / destruction | Rigid bodies with a speed ramp; camera follows the wavefront | Wave 5–15 m/s on screen, 2–3 s per shot |

## Studio vs amateur

| Studio | Amateur |
|---|---|
| One readable force plus layered detail | Uniform noise everywhere |
| Vortices that live, then decay | Smeared, diffused blobs |
| Shading by thickness, light and vorticity | One flat colour or a rainbow |
| Voids, density gradients, and a core that sits off the centre of the frame | An evenly filled frame |
| Event timing (build, release, settle) with speed ramps | Constant speed |
| Real lens: bokeh, motion blur and bloom from highlights | Glow on everything |

## Gap list against Anikit (Nov 2026)

| Need | Status |
|---|---|
| Millions of particles, curl flow, attractors, targets, expressions | Done (scene3d `particles`) |
| Self-shadowed volumes, disc bokeh, exact motion streaks | Done |
| Rigid bodies, joints, dominoes, speed ramps | Done (scene3d `physics`) |
| **Fluid dynamics** (ink, smoke, fire with vortices) | Building now: `fluid` (2D stable fluids + confinement, ink/smoke/fire shading) |
| **Trails / ribbons** | Missing (only motion streaks over the shutter) |
| Particles advected by a fluid | Missing |
| Cloth / soft banners | Missing (only joint chains) |
| Metaball / surface look for dense particles | Missing (2D `metaballs` component only) |
| Sprite textures for particles | Missing |

## Sources

- Jos Stam, *Stable Fluids*, SIGGRAPH 1999; Mark Harris, *Fast Fluid Dynamics Simulation on the GPU*, [GPU Gems ch. 38](https://developer.nvidia.com/gpugems/GPUGems/gpugems_ch38.html).
- Fedkiw, Stam & Jensen, *Visual Simulation of Smoke*, SIGGRAPH 2001 (vorticity confinement).
- Crane, Llamas & Tariq, [*Real-Time Simulation and Rendering of 3D Fluids*, GPU Gems 3 ch. 30](https://developer.nvidia.com/gpugems/gpugems3/part-v-physics-simulation/chapter-30-real-time-simulation-and-rendering-3d-fluids).
- Bridson, Houriham & Nordenstam, *Curl-Noise for Procedural Fluid Flow*, SIGGRAPH 2007.
- [10 expert tips for better Houdini FLIP simulations (CG Channel)](https://www.cgchannel.com/?p=113678); [SideFX fluid tips](https://www.sidefx.com/docs/houdini/fluid/tips).
- [Simulating thin ink with Houdini (Lester Banks)](https://lesterbanks.com/2021/09/tips-and-tricks-for-simulating-thin-ink-with-houdini/); [pepefx: ink in water](https://pepefx.blogspot.com/2014/07/ink-in-water.html).
- [Node Fest titles 2018, particle breakdown (Jason Poley)](https://jasonpoley.myportfolio.com/node-fest-titles-2018-particle-breakdown).
- [Creating renderable particle trails in C4D (Novedge)](https://novedge.com/blogs/design-news/cinema-4d-tip-creating-renderable-particle-trails-with-tracer-and-sweep-in-cinema-4d); [INSYDIUM case studies](https://insydium.ltd/news/case-studies/star-trek-discovery).


## Notes from building it (2026-11-18)

- **Tracer particles as ink filaments: not yet.**
  - Tracers carried by a stable-fluids grid (1.5 M at 1080p) read as stipple, then as straight hatching with long streaks. Neither looks like ink.
  - Two causes:
    1. Interpolated grid velocity isn't divergence-free below the cell scale, so tracers drift into 2–4 px clumps. Moving them by the curl of an interpolated stream function fixes this part.
    2. More than one tracer per pixel is needed before filaments read as continuous (about 10 per pixel), or each tracer must draw its own curved path history rather than a straight streak.
  - The dye field hides both, because of its numerical diffusion.
- **MacCormack on the velocity grid** sheared the fine dye into axis-aligned
  steps. MacCormack on dye only, with velocity first-order, keeps the sharp
  filaments without the artefacts.
- **Sub-grid noise at the grid's Nyquist** (2 dye cells) aliases into
  sawtooth edges; keep the finest turbulence octave at 4+ cells.
- **Motion-streak sample spacing** in the particle splatter: 0.5 px is
  needed. At 0.7 and 1.0 px the reference gate fails (ΔE p99.9 2.80 and 4.55 vs 2.3).
