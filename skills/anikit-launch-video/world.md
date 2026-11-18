# One visual system (steps 3–4)

Sources: `docs/video/craft/launch-visual-craft.md` (studio systems: Pentagram, Ordinary Folk, DixonBaxi, Instrument, ManvsMachine, BUCK, Linear, Vercel, Apple), `launch-evidence.md` (what our blind jury punished), and `BAR.md` (what the user rejected).

The best-documented launch films derive their whole visual system from **the product's own behaviour or material**. Ordinary Folk made Spline's launch film *in Spline*. DixonBaxi built ROLI's motion from the instrument's gestures. Instrument used real customers' real Dropbox folders. Professional films hold one material, light logic and palette for the whole film, and change subject, scale and camera in every shot.

## Write in `3-system.md`

1. **Derivation line:** *product behaviour → visual rule*, in one sentence, cited by every scene. For example: "settles instantly → nothing in frame ever waits; every element arrives on the cut", or "reads the soil → the frame is a cross-section; the reading is the light". Fail if a competitor's logo could be swapped in.
2. **Subject per shot:** the product (real UI, its output, the object), a diagram of how it works, or type stating the claim. **Never a simulated real-world place or event** (field, office, street, crowd, band, nature, food), unless drawn in a clearly non-photoreal illustration style.
3. **CG blacklist** (for 3D): humans, animals, plants, grass, terrain, water, fog banks, food, crowds, chrome or metallic type on a sweep or cyc, physics props used as metaphor (dominoes, tiles, balls, cloth, marbles). The count must be 0. 3D is for the product object and for abstract forms driven by a product parameter.
4. **Banned looks:**
   - indigo-to-violet gradients;
   - cream + terracotta;
   - near-black with one acid accent;
   - a serif that glows, or sits as the hero on paper;
   - chrome type;
   - glass or glow without a depth reason;
   - the tasteful-AI kit (tiny tracked caps or mono label + centred bold headline on dark);
   - robots, brains, glowing particle nets, "neural" gradients.

   Critics now name cream/terracotta, near-black/acid accent and high-contrast serif as the *second wave* of AI defaults. Don't flee one cliché into the next. Take colour from the brand assets or sampled product screenshots.
5. **Colour lives on the product:** one neutral ground (tinted, not pure black or white), the brand's colours, and ≤ 1 accent. Saturated pixels outside the product, UI or accent stay under ~10% of the frame.
6. **Type:**
   - ≤ 2 families (one may be mono), chosen for this product and why (`anikit-video fonts`);
   - ≤ 3 sizes per frame;
   - every text box on a declared column grid, outer margins ≥ 5% of the width;
   - high scale contrast (one huge word beats three medium lines);
   - type is inside the scene in at least half of the text frames: the UI's own content, printed on the product, part of the diagram, or a `matte` window.
7. **Motion vocabulary:**
   - **Easing:** ≤ 3 named curves. Entries ease out or use a critically damped spring; exits are faster than entries; moves ease in-out. No `linear` except constant scrolls. Scale starts ≥ 0.9; overshoot ≤ 5% unless the product causes the event.
   - **Duration:** ≤ 3 tiers (e.g. 0.4 / 0.8 / 1.6 s, scaled to tempo).
   - **Causes:** **nothing moves without a cause.** No float, drift, glow pulse, particle haze or camera wobble. One primary mover per shot and at most one secondary.
8. **Texture:** at most one (grain, halftone or boil), applied to the whole film. CG materials need roughness or albedo variation. Holds over 2 s aren't pixel-frozen: grain or a caused change satisfies this, drift doesn't.
9. **Depth:** build it from layered flat planes (overlap, scale, parallax). Use a perspective camera only on 3D objects. Never orbit a flat screen.
10. **What changes per shot:** subject, scale (macro ↔ context) and camera. No two shots share a layout.

## Anikit means (pick what the system needs)

| Need | Tools |
|---|---|
| Real UI as hero | the product's screenshots as image layers or lit `scene3d` planes; `product-shot`; macro crops via `camera` zoom; cursor paths |
| Kinetic type | variable `axes`, `animator`, `matte`, text on path, `fit` (`docs type`, `docs type-craft`) |
| Graphic systems | `grid`, halftone, riso, `print`, brush, editorial charts (`docs graphic`) |
| Product object | `scene3d` with PBR `material.texture`, machined `pattern`s, `decal`, `rig`, `lens`, `focus`, practicals (`docs 3d`, `recipes precision-macro`) |
| Abstract worlds | library shaders and `ak_` blocks, `fluid`, particles (`docs shader`, `docs fluid`) |
| Motion craft | `inertia`, `smear`, `carry`, `zoomThrough`, `dolly`, `timeRemap` (`docs mograph`) |
| Sound | `music` (real instruments), `"sound": "auto"`, `onNotes` (`docs music`, `docs sound`) |

## Style frames (step 4)

1. Render three `still`s of the key moment, each varying one decision (type treatment, crop, light, colour), and view them side by side at full size.
2. Run the fake-look check from `review.md` on them.
3. Pick the one that's a finished poster: one focal subject, holds up at 100% crop, and **looks designed rather than generated**.
4. Write why in `3-system.md`.
