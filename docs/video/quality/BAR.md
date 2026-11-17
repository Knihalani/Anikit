# The quality bar (read before showing anyone a frame)

The user has rejected, in their own words, everything in `rejected/`:

- "all look terrible, like AI slop and are boring";
- "the ones that say no browser are REALLY BAD";
- "extremely basic", "no 3d", "don't make sense", "glitchy".

Those frames scored 100/100 on `critique --render` and up to 8/10 from an LLM grader. **Automated scores are not evidence of quality.** Only this checklist plus a look at full-size, in-motion frames counts.

## A frame FAILS if any of these is true

1. **Centred headline on a gradient.** A bold sans headline (or any big centred text) sits over a gradient, blob or mesh backdrop.
2. **Soft blob backdrop.** The background is a blurred colour-blob, mesh-gradient or aurora wash doing the work of "design".
3. **Placeholder UI.** Rounded cards, pills, progress bars or placeholder lines stand in for UI or content (grey bars instead of real text or data).
4. **Small content in an empty frame.** The content occupies a small part of a mostly empty 1920×1080 frame.
5. **Flat 2D only.** There is no depth, material or light: no real 3D, no shading, no lens behaviour (DOF, bloom from real highlights), no parallax.
6. **Slide-deck structure.** Scenes are a sequence of title cards or slides rather than one designed world with motion that has weight.
7. **Stock-demo look.** It resembles a stock template or tutorial: checker floors, primitive spheres and cubes and tori, rainbow palettes, default Inter-on-dark, generic "SaaS dashboard".
8. **Effect without subject.** An effect is shown for its own sake (a lens flare, particles or glow over nothing).
9. **Any visible defect.** Stepping or juddering motion, banding, aliasing or shimmer, clipped text, or a pop at a cut.

## A frame PASSES only if

- it reads as a **designed image** a top studio would post: a clear subject, deliberate composition, real light and materials, and a considered palette and type;
- it holds up at **100% crop** and as **consecutive frames** taken from the MP4.

## Process

- Judge every shot against this list *before* sending it to the parent. A failing frame never reaches the user.
- Show work as one composed shot (5–8 s) at a time, not as galleries of isolated parts.
