# Board: a look from reference images

```bash
anikit-video board refs/ -o board.json --sheet board.jpg     # a folder, or image files (png, jpg, webp)
```

Gather **3–6 real-world references from outside tech**: a painting, a print, a poster, a photograph, packaging, architecture, a film still. `board` measures them and merges the result into a look you can apply. Take their colour, value, texture, composition and line character, never their subject matter or artwork.

**board.json**
- `images[]`, one per reference:
  - `palette` (OKLab k-means with shares) and `accents` (vivid colours clustered on their own, so a small neon or red stamp isn't lost);
  - `key`, `contrast`, `saturation`, and `grade` (black/mid/white points, warmth, tint, label);
  - `grain`, `detail`, `hard_edges` (graphic ≥ 0.08, painterly or photographic ≤ 0.05);
  - `layout` (centre of weight, symmetry) and `negative_space` (share of quiet tiles);
  - `lines` (horizontal, vertical or diagonal shares and a character: rectilinear, diagonal/dynamic, organic, curving).
- `palette`, `accents`: merged over the board, each image weighted equally.
- `tokens`: `bg surface text muted primary accent`. Text is at least 4.5:1 on bg. Primary and accent are the most vivid colours that show on the ground.
- `spec_patch`: `{ "theme": { "colors": … }, "post": { grain, vignette, lift } }`. Merge it into your spec:

```json
{ "theme": { "colors": { "bg": "#F2EAD1", "text": "#244160", "primary": "#234161", "accent": "#E5B494" } }, "post": { "lift": 0.15 } }
```

- `summary`: one paragraph to paste into your concept notes.

**board.jpg**: each reference with its palette and accent strips, its key, contrast, negative space and line character; then the merged palette and the six tokens.

Apply more than colour:
- **Value:** key and contrast set backgrounds and type weight.
- **Texture:** grain and detail set `post` and how busy a frame may be.
- **Composition:** negative space and centre of weight set where the subject sits.
- **Line character:** rectilinear means grids and hard cuts; organic means curves and eased motion.

Check stills against the board sheet side by side.
