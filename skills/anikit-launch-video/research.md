# Research and play (steps 1b and 3b)

The best launch films come from people who know the subject's world and have tried things. Our agents' weakest films came from going straight from the brief to the first idea and the first technique. Two steps fix that: **research** before ideas, and **play** before locking the world.

## Step 1b, `launch/refs/`: research and reference material (about 10–15 min)

**1. The subject's world, in `refs/world.md`.**
- Who uses this, where, and what's their ritual?
- What do the product's materials, sounds and places actually look like? For example, a recordist's kit bag, a foundry's sand moulds, a back-office settlement queue.
- What do people in that world say? Collect their nouns, verbs and objects, and the real details only an insider would know.

Use web search if you have it. Otherwise reason it out carefully, and mark which details you're sure of and which you're guessing.

**2. Professional reference work, in `refs/refs.md`.**
- **What to collect:** 6–10 launch or brand films in this product's category and in neighbouring ones, from top studios. Also gather 2–3 references from **outside motion entirely**: a photographer, a painter, a print designer, a film-maker or an architect whose world fits.
- **Where to look:**
  - web search for studio case studies and award pages (Vimeo Staff Picks, Motion Awards, ADC, D&AD, studio sites such as Buck, ManvsMachine, Tendril, Ordinary Folk, Gretel and DixonBaxi);
  - `docs/video/judge/benchmark/<category>/*.md`, which already holds 87 professional pieces in 14 categories, each with notes on what makes it work;
  - `docs/video/craft/launch-research.md`, with 14 launch films analysed.
- **For each reference:** write the **moves** worth stealing, not the look. That means the idea, how the product appears, the structure, how type behaves, the light and material, the sound, and the ending.
- **Then write:** "what all the good ones do that ours usually don't", and "the cliché of this category to avoid".

**3. A look board (optional, when you have images).** Save 3–6 reference images you're allowed to use, such as your own renders or freely licensed photos, then run `anikit-video board refs/img/ -o refs/board.json --sheet refs/board.jpg`. It measures palette, value, texture and composition, and gives you tokens to use. Take their colour, light and composition, never their artwork.

**4. Real assets.**
- **Lighting environments, materials and models:** `anikit-video assets search <words> --type hdri|texture|model` (free CC0 HDRIs, PBR materials and models).
- **Fonts:** `anikit-video fonts <genre>`, about 130 typefaces with notes on each.
- **The product's own material:** the brand's colours, UI, logo and product photos, if the brief gives them.

**The gate:** step 2's ideas must each cite at least one insider detail from `world.md` and one move from `refs.md`.

## Step 3b, `launch/tests/`: play (about 10–20 min)

Before locking the world, try things. Make **4–8 quick technique tests**, each a 2–4 s clip or a still, that could carry your idea. Use **at least three different capabilities** of the engine, and include at least one you've never used. Draft quality is fine (`render --quality draft --from A --to B`, or `still`).

- **Ray-traced 3D:**
  - the product or a key object with a real material (`material.texture: "ambientcg:…"`) under an HDRI or a lighting rig;
  - a macro shot with rack focus.
  - (`recipes window-light`, `precision-macro`, `docs 3d`)
- **Shaders:**
  - a generative world (`recipes`, the library shaders, `ak_` blocks);
  - a custom GLSL look built from the subject (`docs shader`).
- **Simulation:** fluid or ink, smoke, cloth, liquid, particles with depth layers and glints, physarum (`docs fluid`, `docs particles`, `docs physarum`).
- **Type as image:**
  - variable axes animated per letter;
  - type as a window onto the world (`matte`);
  - type made of the material;
  - text on a path.
  - (`docs type`, `docs type-craft`)
- **Graphic and print:** halftone, riso, brush, cutout, grid choreography (`docs graphic`).
- **Illustration:** `illo` with one light and a hand, `landscape`, `plants` (`docs component illo`).
- **Motion:**
  - follow-through and smear;
  - an idea transition (`zoomThrough`, `carry`);
  - a dolly through depth;
  - time remap (`docs mograph`).
- **Sound and picture together:** visuals driven by the notes (`onNotes`), and sound from the picture (`"sound": "auto"`).

**Then:**
1. Put the tests side by side on one sheet: `launch/tests/sheet.png`.
2. In `tests/notes.md`, write which one made you stop, which looked stock, and what you learned about the engine's limits (what can't be made convincing yet).
3. Build the world (step 3) around the strongest test.

A test that surprises you is worth more than a plan that sounded good. A technique that only looks "fine" won't carry a launch film.
