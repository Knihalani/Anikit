# Graphic design craft for moving image (2D)

How studios build striking flat, graphic frames, written for agents writing Anikit Video specs. This doc goes deeper on **graphic and 2D design** than the general canon: grids, colour systems, print processes, pattern, image treatment, shape language and editorial data graphics, with numbers converted to a 1920×1080 frame.

Read first:
- [`../quality/BAR.md`](../quality/BAR.md): what the user has rejected and why ("AI slop", "boring", "generic", the "tasteful-AI" cream + serif + terracotta look, the SaaS layout).
- [`../taste/TASTE.md`](../taste/TASTE.md) §1–§3: type scale, size floors, read time, value structure, palette size, composition, occupancy. This doc does **not** repeat them; it links to them.

**How to read this.** It is advice: what tends to work and why, with numbers practitioners use. None of it is a rule. A Swiss grid and a Polish brush poster are both right in the right brief. The one through-line, as in TASTE: every choice is made for this subject, and you can say why. Numbers marked *(heuristic)* are our own proposals built on the sources; tune them by eye. Sources are numbered [n] and listed at the end.

---

## 1. Principles

**1.1 A system, not a layout.** The studios that make memorable graphic work design a *programme*: a small set of rules that produces many different frames. Gerstner called the grid "a programme par excellence" when it acts as a proportional regulator [1][2]. Studio Dumbar's Dutch police identity is remembered for one device, the diagonal striping, which made fewer cars feel like more police: "even when there's less surveillance, the public still think there's more!" [17][18]. Experimental Jetset's Whitney "responsive W" is a zigzag line that changes its slope to fit the space left by each artwork at its true proportions [19]. Wolff Olins' London 2012 mark came from "an energy grid we drew of lines that moved around, contained within a rectangle, which we stopped at one particular moment", and the same grid generated the patterns for the venues [20]. In video that means: before designing shot 1, write the rule that would generate shots 2–20. A sequence where every frame was laid out from scratch reads as slides (BAR §6).

**1.2 One device, pushed hard.** Paula Scher's Public Theater work (from 1994) took one source, the brash, mixed-weight wood type of 19th-century American playbills, and pushed it until it was the institution's voice: "PUBLIC" set largest, faces of different widths jammed together [14][15]. When it was redrawn in 2008 in Knockout, each season "changes in the use and spirit of Knockout", not the typeface itself [16]. Collins' Spotify identity (c. 2015) is one treatment, duotone "drawn from the duotone screen-printing used throughout music's history", in "vibrating" colour pairs, which turns anyone's photo into a Spotify image [21]. A recognisable piece usually has fewer ideas, executed at a higher density, than a generic one.

**1.3 Motion is part of the identity, not a fix.** Simon Dixon (DixonBaxi): "I've heard so many times from designers that 'This will look better in motion'. Of course it will, but it's not the role of motion or the motion designer to solve the design problem." The question is "What kind of motion defines us?" [24]. Koto derived Deezer's motion principles (rhythm, modularity, responsiveness) from the strategy, "music as a form of movement" [23]. So decide the motion behaviour together with the grid and palette: what moves, on which axis, at what tempo, and what never moves.

**1.4 Surprise inside a system.** Richard Turley at *Bloomberg Businessweek* (2010–14): "A cover should just be surprising." He chose Helvetica precisely because it was "just a typeface, rather than a branding exercise", so the surprise came from ideas, scale and images, many "sitting completely free of the grid" [22]. Strict structure plus one outrageous move beats loose structure plus many small moves.

**1.5 Craft shows as evidence of a process.** It's Nice That's 2025 trends piece calls out "far too slick and sometimes soulless motion that pervades the design world", and celebrates labour-intensive work (riso-printed films, stop-motion), whose "beautifully twitchy" quality comes from a real process [25]. A riso look that is just a grain overlay is the filter-preset version. A riso look that models separate ink plates, overprint, registration drift and paper is the crafted version (§5).

**1.6 Distinct beats safe.** "Blanding" (Brunfaut and Greenwood, 2018) is the convergence of identities on the same thin geometric sans and neutral palette; D&AD's 2025 piece quotes Nadine Chahine: "if a common chapter in all brands' stories is that they start to look the same, then we're losing distinctiveness" [26]. Brat's acid green was chosen *because* it was "off trend" [25]. Agents default to the median of everything they have seen; the median is the blanded look.

---

## 2. Grids and modular systems (with numbers)

### 2.1 What the masters actually specified

- **Müller-Brockmann**, *Grid Systems in Graphic Design* (1981): grids of **8 to 32 fields** [3]. Fields are separated by an interval of one (or two) lines of text, and every field's height is a whole number of text lines, so type, images and field edges share one vertical rhythm. The book's own cover is set on 32 fields [3].
- **Gerstner's "mobile grid"** for *Capital* (1962–63): **58 units** wide [1][2]. With 2-unit gutters, 58 divides cleanly into 1, 2, 3, 4, 5 *and* 6 columns:

  | Columns | Column width (units) | Check |
  |---|---|---|
  | 1 | 58 | 58 |
  | 2 | 28 | 2×28 + 1×2 = 58 |
  | 3 | 18 | 3×18 + 2×2 = 58 |
  | 4 | 13 | 4×13 + 3×2 = 58 |
  | 5 | 10 | 5×10 + 4×2 = 58 |
  | 6 | 8 | 6×8 + 5×2 = 58 |

  This is why the page can change rhythm (a 2-column spread, then a 5-column table) while staying obviously one magazine. Gerstner warned it "requires considerable study" before it can be used freely [2].
- **Public Theater 2008**: posters on "the strict 90° angles of a De Stijl-inspired grid" [15].

### 2.2 Gerstner's 58 at 1920×1080

Choose a unit of **30 px**: 58 × 30 = 1740 px of content, leaving **90 px** side margins (inside the 96 px EBU graphics-safe zone, so treat 90 as the image edge and keep live text ≥ 96 px from the frame edge, or use a 29 px unit and 119 px margins).

| Columns | Column px | Gutter px |
|---|---|---|
| 2 | 840 | 60 |
| 3 | 540 | 60 |
| 4 | 390 | 60 |
| 5 | 300 | 60 |
| 6 | 240 | 60 |

Use the same 30 px unit vertically: 1080 / 30 = 36 rows. With 2-unit (60 px) top and bottom margins that leaves 32 rows: 2 bands of 15 + one 2-unit gap, 4 bands of 6.5 (use 4 × 6 + 3 × 2 = 30, one spare row top and bottom), or 3 bands of 9 + 2 × 2 = 31 (one spare row as extra top margin). Pick the row count with the column count, so cells are near-square or a deliberate 1:2.

### 2.3 Müller-Brockmann fields at 1920×1080 *(heuristic conversion)*

Baseline **12 px**. Fields 18 lines tall (216 px), gaps 2 lines (24 px):
- **4 × 4 = 16 fields:** rows 4×216 + 3×24 = 936 px, so top and bottom margins are 72 px. Columns (1728 − 3×24) / 4 = **414 px** with 96 px side margins.
- **6 × 4 = 24 fields:** columns (1728 − 5×24) / 6 = **268 px**.
- **8 × 4 = 32 fields:** columns (1728 − 7×24) / 8 = **195 px**.
- Type sizes then come from the baseline: 12 px × n line heights. Text 36/48 (4 lines), sub-heads 72/84, display 192/216 (one whole field row). The display line's cap height then sits in the field, not on top of it.

### 2.4 How grids stay invisible yet felt

- **Shared edges, not filled cells.** A grid reads when 3+ elements share an edge (a flush-left text block, an image's left edge, a rule) and other cells stay empty. Filling every cell gives the bento grid (§7). In Müller-Brockmann's posters, most fields are empty: "white space in an asymmetric grid is active" (TASTE §3).
- **Hang, don't centre.** Hang text from a field's top line; align images to the cap height of adjacent display type, not to its bounding box.
- **Break it once, on purpose.** One element per frame may leave the grid: oversize, rotated, bleeding off frame (Turley's "free of the grid" [22]). If many elements break it, there is no grid to break.
- **Motion on grid lines.** Elements travel along columns/rows and stop on grid lines; the destination of every move is a cell edge. The grid is felt in time even when it is never drawn. Snap moves so 1 unit = 30 px multiples; a move that stops 7 px off a grid line reads as a mistake.
- **Change the column count between scenes** (Gerstner's mobile grid) instead of changing everything else. 2-column scene → 5-column scene is a rhythm change the eye accepts as the same world.
- **Show the grid only as content.** Hairline rules (1–2 px at 1080p; under 1 px it shimmers, see §4.6), registration marks, crop marks or column numbers can make the system visible when the concept is about structure. Don't overlay a decorative grid pattern as background texture; that is the "grid" background component used as a default.

### 2.5 Scale contrast in posters

TASTE §1 and §3 give the type-scale and hero-to-secondary ratios (≥ 3:1, up to 10:1). Posters go further:
- Swiss concert posters and Turley-era covers put the hero at **10–30×** the size of the smallest text: at 1080p, a 640 px headline word with 24–36 px credits *(heuristic)*.
- Scher mixes scales *within* a word or line (wood type tradition): a 3:1 jump between adjacent words reads as a voice, not an error, because the jump is large. Jumps under ~1.5× read as accidents.
- Turley: "We start small and scale our charts up according to their merit" [22]. Size is earned by content.
- At 1080p the floor for anything meant to be read stays at TASTE's ~36 px; the 10–30× ratio is reached by making the hero bigger (bleeding off frame if needed), never by making the small text smaller.

---

## 3. Colour for graphic work

TASTE §2 covers value-first thinking, palette size and sourcing. Additions specific to print-derived graphic work:

### 3.1 Spot colour thinking

- **Design in inks, not RGB.** A print designer chooses 2–3 inks, then gets every other colour from tints (10–100% screens) and overprints. That constraint is what makes riso, screen-print and two-colour editorial work look coherent. For a spec: list 2–3 ink colours plus the paper colour, and derive every fill as `ink × tint` or `ink1 × ink2` (multiply).
- **Overprint = multiply.** Transparent process and riso inks overprint subtractively; the overlap is approximately `a × b` in linear light. Fluorescent Pink × Medium Blue gives a deep violet; Yellow × Blue a green [7]. Overlap colours are free extra colours, and they are what make a two-ink piece look like more than two colours.
- **Knockout vs overprint.** Print designers choose per element: a type element either knocks out the ink below (shows paper) or overprints it (multiplies). Mixing both in one frame on purpose (knocked-out headline, overprinted image) is a craft signal.
- **Paper is the third colour.** Riso and screen inks are translucent, so the paper colour tints everything [7]. Choose stock deliberately: bright white, newsprint grey, kraft, or coloured stock (sky, neon, black with opaque inks). **Avoid the default cream #F4F1EA-family ground**: BAR lists it as the "tasteful-AI" tell. If the concept truly needs warm stock, give it texture and fibre (§4.4) and pair it with inks that are not terracotta.

### 3.2 Risograph inks (hex approximations)

From Matt DesLauriers' `riso-colors` dataset, compiled from stencil.wiki, with Pantone references [7]. On-screen values are approximations: real riso ink is translucent and shifts with paper.

| Ink | Hex | Pantone | Notes |
|---|---|---|---|
| Fluorescent Pink | `#FF48B0` | 806 U | The signature riso colour; overprints beautifully on blue |
| Medium Blue | `#3255A4` | 286 U | Workhorse dark ink; carries type |
| Blue | `#0078BF` | 3005 U | Cleaner, cyan-leaning |
| Federal Blue | `#3D5588` | 288 U | Greyer, editorial |
| Yellow | `#FFE800` | Yellow U | Print first (lightest) |
| Teal | `#00838A` | 321 U | |
| Aqua | `#5EC8E5` | 637 U | |
| Green | `#00A95C` | 354 U | |
| Hunter Green | `#407060` | 342 U | |
| Bright Red | `#F15060` | 185 U | |
| Scarlet | `#F65058` | Red 032 U | |
| Orange | `#FF6C2F` | Orange 021 U | |
| Fluorescent Orange | `#FF7477` | 805 U | |
| Sunflower | `#FFB511` | 116 U | |
| Purple | `#765BA7` | 2685 U | |
| Burgundy | `#914E72` | 235 U | |
| Flat Gold | `#BB8B41` | 1245 U | |
| Light Gray | `#88898A` | 424 U | |
| Black | `#000000` | Black U | Riso black is a warm, slightly translucent grey-black in print; `#1D1D1F`–`#2A2A2A` reads more honestly on screen *(heuristic)* |

Proven two- and three-ink sets: Fluorescent Pink + Medium Blue; Yellow + Federal Blue; Fluorescent Orange + Teal; Sunflower + Burgundy; Yellow + Fluorescent Pink + Blue (a "CMY" riso set). Print order is usually lightest first [7].

### 3.3 Duotone, tritone and gradient maps

- **How it works.** A duotone prints one greyscale image twice: a dark ink carries shadows, a lighter ink carries mid-tones and highlights, and the paper is the white. On screen that is a **gradient map on luma**: luma 0 → shadow ink, luma 1 → highlight ink or paper.
- **Value range is everything.** The shadow ink needs to be dark (OKLab L ≲ 0.35) and the highlight light (L ≳ 0.80), or the image goes mid-value and mumbles (TASTE §2). Collins' Spotify pairs "vibrate" because they keep a big value gap *and* a big hue gap [21].
- **Classic pairs** (print practice): black + warm grey (photographic books); black + sepia/ochre; dark blue + cyan. **Graphic pairs** *(heuristic)*: Medium Blue `#3255A4` → Fluorescent Pink `#FF48B0`; Hunter Green `#407060` → Sunflower `#FFB511`; near-black `#14141A` → Fluorescent Orange `#FF7477`; Burgundy `#914E72` → Aqua `#5EC8E5`.
- **Tritone.** Black + two greys, or a dark + mid + light ink. Put the middle stop at luma 0.45–0.55 and use it to set the mood.
- **Prep the image first.** Before mapping, raise contrast (black point at p2, white point at p98) and decide the gamma. A flat photo duotoned stays flat; this pre-curve is the difference between a crafted duotone and a preset.
- **Posterise as a step.** 3–5 luma levels before the map gives screen-print flats. Fewer levels need cleaner source images.

### 3.4 Where studio palettes come from

- **A process:** the inks a riso shop stocks, a screen printer's 2 colours, a newspaper's spot red.
- **An object or era:** the subject's packaging, a 1970s timetable, a uniform. Kandinsky's Bauhaus questionnaire (1923) paired yellow–triangle, red–square, blue–circle [6]; a later replication found about half of students chose differently, so treat it as a style source, not a law.
- **The brand's real colours**, extended with ink logic (tints and overprints), not with new hues.
- **Anti-sourcing:** colour-wheel generators, "trending palettes" sites and model defaults give the blanded median.

### 3.5 Cliché palettes in 2025–2026 (beyond TASTE §9)

- Cream + editorial serif + terracotta (BAR, fourth rejection).
- Indigo/violet → blue gradients; aurora and mesh washes (TASTE §2).
- Dark navy/teal + amber "dashboard" (BAR, fifth verdict).
- "Neo-brutalist" pastel cards: lilac, mint, lemon fills with 3–4 px black borders and hard offset shadows.
- Acid green + black after Brat: it was off-trend in 2024 [25], it is a trend now.
- Y2K chrome and holographic iridescence.
- Beige/taupe "blanded" neutrals [26].

---

## 4. Pattern and texture

### 4.1 Generative patterns with a lineage

- **Truchet tiles.** Sébastien Truchet (1704): squares split diagonally into two coloured triangles, in 4 orientations. Cyril Stanley Smith (1987) replaced the triangles with **two quarter-circle arcs joining the midpoints of adjacent sides**, 2 orientations, which links into continuous meandering curves [4]. Variants: 3 points per side (4 arcs per tile), multi-scale tiles (a tile subdivides into 4 half-size tiles). In motion, flip tiles one at a time on the beat; each flip reroutes whole curves.
- **Vasarely's unités plastiques** (patented 1959): a geometric figure (circle, square, triangle, rhombus…) inside a square of contrasting colour; grids of these units vary figure size, rotation and colour to make volume and swelling [5]. *Kezdi* (1966) is a 17 × 17 = 289-module grid of red squares with lavender circles graded in diameter [5]. In motion, animate the *grading field* (a radial or noise function sampled per cell), not each cell.
- **Gerstner's colour and form programmes** [1]: permutations of a fixed alphabet (all combinations of N forms × M colours) laid out in a matrix. Showing the full permutation set is a strong title-sequence structure.
- **Op-art line fields** (Riley, Vasarely): parallel lines whose phase or width is modulated by a function produce apparent depth.
- **Memphis surface patterns**: Du Pasquier "designed patterns and not textiles", working "cut and paste" [32]; Sottsass's Bacterio laminate is irregular squiggles at a uniform density. The crafted feel is the hand-cut irregularity at a strict overall density.

### 4.2 Halftone

- **Angles.** Classic CMYK: C 15°, M 75°, Y 0°, K 45°; C, M and K sit 30° apart to minimise moiré and make the rosette; yellow sits only 15° from cyan but is too pale for its moiré to show [9]. A single-ink halftone is set at **45°** (the angle the eye notices least). Duotones: dark ink at 45°, light ink at 75° (30° apart).
- **Rulings in print.** Newsprint ~85 lpi (75–100); four-colour magazines 133–150 lpi; art books and annual reports ~175 lpi [10][11].
- **At 1080p, real rulings are invisible.** An A4 page at 133 lpi is ~1550 lines tall; mapped to 1080 px, the dot is under 1 px. So screen halftone is always a *magnified* halftone, a close-up of print. Choose the cell by how close the "camera" is:
  - **6–10 px** cell: fine newsprint read up close; still reads as continuous image at a glance.
  - **12–24 px**: pop-art, Lichtenstein Ben-Day scale; dots are the subject.
  - **32–80 px**: super-graphic, dots as shapes (good for title cards and transitions).
  - **Below ~5 px** H.264 4:2:0 and phone downscaling turn it into mush or moiré, especially when moving *(heuristic)*.
- **Dot shapes.** Round (classic, soft), elliptical/chain (smoother mid-tones; dots join along one axis at ~50%), square (harsh, 1960s), line screens (engraving, banknote look), and stochastic/FM (no screen angle, grain-like; good for avoiding moiré in motion).
- **Anchor the screen.** Anchored to the **frame**, the screen stays still while the image moves under it (dots change size: reads as print being "re-screened"). Anchored to the **layer**, it travels with the image (reads as a printed object being moved). Pick one deliberately per piece. Rotating or scaling a screen continuously produces swimming moiré: an effect, not a default.
- **Dot gain.** Real print fattens dots ~10–25% in the mid-tones on uncoated stock; a gain curve (mid-tones pushed darker) is what makes a digital halftone look printed.

### 4.3 Misregistration

- **Real tolerances.** Offset presses register within a fraction of a millimetre. Riso operators report about **±1–2 mm** of shift between drums, with drift across the sheet and more movement at higher speeds [8]; screen printing by hand is similar or worse.
- **Convert print mm to 1080p px** by deciding what physical size the frame represents:
  - frame = A4 portrait height (297 mm): 1080 / 297 ≈ **3.6 px/mm**, so riso ±1–2 mm → **4–7 px**;
  - frame = A2 poster height (594 mm): ≈ **1.8 px/mm** → **2–4 px**;
  - offset print (0.1–0.2 mm) → under 1 px: invisible, so don't add it.
- **Shift the plate, not the element.** Each ink is offset by its own constant (dx, dy) and a small rotation (0.1–0.3°) for the whole frame; every element printed in that ink moves together. Random per-element offsets read as a glitch filter, not print.
- **In time.** A real print run changes registration per sheet. Animate the plate offsets as a held value that changes every 2–4 frames ("on twos" or slower; It's Nice That's "beautifully twitchy" [25]), not as smooth noise. Hold still when type must be read (≥ 4.5:1, TASTE §1): a plate slip on a headline costs legibility.
- **Trap.** Where two inks meet edge-to-edge, printers spread the lighter ink 0.1–0.3 mm under the darker. In riso pieces, letting gaps show (paper slivers) is part of the look; choose either trap or gap.

### 4.4 Paper and photocopy

- **Paper** is a large-scale tone variation (low-frequency, ±2–4% luma), fibre (directional, 1–3 px streaks), and **ink absorption**: ink is lighter where the paper is high, and edges roughen 1–2 px. A uniform grain overlay over everything (paper and ink identically) is the preset version; texture that affects ink coverage is the crafted version.
- **Photocopy** is threshold + toner noise + edge darkening (toner builds up at the edges of solid areas) + slight scale/rotation drift between generations + dropout of light tones. A generational copy loses mid-tones each pass: posterise harder per generation.
- **Grain amount.** For print looks, a stochastic grain of 1–2 px at 3–8% opacity per ink plate *(heuristic)*; animate it per frame only if the piece is "filmed" (the paper is in front of a camera), otherwise per sheet change. Grain-on-everything at the same strength is a 2025 cliché (§7).

### 4.5 Pattern discipline

- **One pattern family per piece**, at 1–3 scales. Scale steps of ×2 or ×3 between pattern levels read as a system.
- **Pattern needs a flat to rest against.** Studio work alternates busy (pattern, image) with quiet (flat ink, type); a frame that is all pattern has no figure.
- **Pattern carries meaning or motion.** Dumbar's stripes signal "police" at a glance [17]; London 2012's shards came from the logo grid [20]. A dot pattern because "it's nice" is decoration.

### 4.6 Video-specific limits

- **Line and dot pitch ≥ 4 px**, ≥ 8 px if the pattern moves, or the encoder and 4:2:0 chroma smear it (and YouTube re-encodes at lower bitrate) *(heuristic)*.
- **Hairlines ≥ 1.5 px** at 1080p (2 px safer) and pixel-aligned when static; 1 px lines crawl when they move sub-pixel.
- **High-contrast stripe and checker patterns** can trigger photosensitive reactions; keep full-frame flashing below 3 per second (WCAG 2.3.1) and avoid large black-and-white stripe fields that shift phase rapidly.
- **Saturated chroma patterns** (pink dots on blue) lose detail at 4:2:0; carry the pattern in luma (dark ink on light) and let colour come from areas.

---

## 5. Image treatment: crafted vs preset

The same treatments (duotone, halftone, cutout, collage, photocopy, posterise) look crafted or cheap depending on these choices:

| Crafted | Preset |
|---|---|
| Image chosen *for* the treatment: strong silhouette, clear value structure, no mid-grey mush | Any image with a filter on |
| Contrast and gamma set before the map (§3.3) | Map applied to the raw image |
| Treatment follows a process model: plates, inks, screen angle, paper | One overlay or LUT for everything |
| Variation by element: photo in halftone, type solid, pattern flat | Same effect on every layer, type included |
| Imperfection is structured (plate offset, dot gain, toner edges) and held | Random noise, jitter on every element every frame |
| Cutouts have a decided edge: knife-clean, torn with a white paper rim (3–8 px), or a hand-cut polygon with ~1° wobble | Soft feathered alpha or AI background removal halo |
| Collage obeys scale and light: shadows consistent (one light direction, 4–12 px offset, sharp for paper-on-paper) | Floating stickers, mismatched scale and light |
| Cieślewicz-style: "reworking printed matter" into a new image with a single idea [30] | Collage as mood-board |

Notes:
- **Cutout + flat colour** is the Public Theater recipe: silhouetted photo, flat colour blocks, wood type [14]. The silhouette needs a strong shape at 200 px thumbnail size.
- **Don't treat type the way you treat images.** Type stays crisp and solid ink (BAR: "text is not crisp" was a rejection). Misregistration on type is limited to ≤ 2 px and only on display sizes; never halftone type that must be read.
- **Collage in motion**: pieces enter as paper does: slide, slap down with a 1–2 frame settle, never a fade. Shadows appear on contact.

---

## 6. Shape language

### 6.1 Superellipses and corners

- **Superellipse** |x/a|^n + |y/b|^n = 1. n = 2 is an ellipse; n → ∞ a rectangle.
- **Piet Hein, Sergels torg (Stockholm, 1959)**: n = **2.5**, axis ratio a/b = 6/5 [12]. A friendly, rounded rectangle-ellipse hybrid; good for plazas, tables, badges.
- **"Squircle"**: n = 4 is the usual meaning [12]. Apple's icon shape is often approximated as n ≈ 5 but is actually built from tuned Bézier curves; Figma's "iOS" corner smoothing is **60%** [13]. Treat any exponent as an approximation.
- **What they buy you.** Continuous curvature (no visible "kink" where a straight edge meets a circular corner). At 1080p the difference between a rounded rect and a squircle is visible from ~120 px corner radius upward, on large shapes; under that, a plain radius is fine.
- **But**: squircle cards on dark backgrounds are a UI cliché (BAR §3, "placeholder UI"). Use superellipses as *forms* (plazas, lenses, pebbles, frames for images) more than as card containers.

### 6.2 Systems of primitives

- **Bauhaus/Kandinsky**: circle, square, triangle with primary colours [6]. The craft is in the composition rules (overlap, tangency, alignment to a grid), not in the shapes.
- **Ikko Tanaka, *Nihon Buyo* (1981)**: a dancer's face built from squares and circles in flat colour, a Japanese subject in a Western modernist vocabulary [31]. The lesson: a figurative subject from 4–8 primitives, each in one flat colour, with no outlines.
- **Memphis**: squiggles, terrazzo confetti, grids of dots, zigzags, half-circles, columns, mixed with flat pastel and black-and-white patterns [32][33]. Consistent rule: shapes are cut with no shading, and patterns are in black on white or white on colour.
- **Dumbar's police stripe**: a few parallel bands at a fixed angle in a fixed colour order [17]. One shape rule, applied to anything (cars, uniforms, buildings).

### 6.3 Consistency rules that make a shape language feel designed

- **One corner logic.** All sharp; or one radius family (e.g. r = 0, 12, 48 px); or one superellipse exponent. Mixing sharp and rounded randomly reads as assembled from libraries.
- **One stroke weight family**: 2 or 3 weights in fixed ratios (e.g. 2 / 6 / 18 px, ×3 steps).
- **One angle set.** 0/90 only (Swiss, De Stijl), or 0/45/90, or a single signature angle (Dumbar stripes, a brand's 15°). Rotation animates between members of the set, not to arbitrary angles.
- **Blobs** need constraints: a fixed number of control points (5–8), bounded curvature, a deliberate silhouette. Free-floating blurred blobs are BAR §2's "soft blob backdrop". Crisp, flat-coloured, cut-paper blobs (Matisse cut-outs) are a different thing.
- **Scale family.** Shapes appear at 2–4 sizes in a fixed ratio, like type (TASTE §1).

---

## 7. Data visualisation with editorial polish

TASTE §1 covers type size floors, which apply to every label.

- **Annotation is the journalism.** Amanda Cox (Bloomberg): the annotation layer, "the words on the chart, make it a completely different experience" [29]. John Burn-Murdoch (FT): "A narrative title helps. A bit of text annotation goes a long way", and he spends "a lot of effort and time into getting that title right" [28]. Title = the finding ("Rents doubled in eight years"), not the topic ("Rent, 2016–2024").
- **Highlight one thing.** "Minimize distraction. Maximize contrast": "a line of red in a sea of grey lines will immediately stand out" [28]. One highlight hue on ≤ 3 series; everything else in 1–2 greys.
- **Direct labelling**, not legends: "You're asking someone to keep checking back and forth between the legend and the lines" [28]. In video, legends are worse: the viewer can't pause.
- **Gridlines**: horizontal only for bars and lines, 1–2 px at 15–25% contrast against the background; label the top gridline with the unit ("$bn", "%") so the axis needs no title. Zero baseline in full contrast.
- **Tabular figures** (`tnum`) for any number that changes or aligns in columns; proportional figures in running text. Numbers right-aligned in tables.
- **Hierarchy (1080p, heuristic)**: title 56–72 px, subtitle 32–40 px, annotations 28–32 px (above TASTE's ~28 px floor), axis labels 24–28 px only if they need not be read in the time given; source line 20–24 px only as texture.
- **Order of reveal.** Burn-Murdoch notes viewers scan title → axes → data [28]. In motion: title, then axis and scale, then the data draws, then the highlight and annotation land on the beat. The number arrives *before* the line reaches it only for a reveal; otherwise let the line arrive and the label follow.
- **Scale by merit.** Turley's rule [22]: a chart's size follows its importance. A single strong number can be a 600 px figure; a minor comparison stays small.
- **Human data.** Giorgia Lupi's *Data Humanism* (2017): data represents "human experiences, behaviors, and actions"; her manifesto pairs handwriting against monospace [27]. Hand-drawn marks, custom glyphs per data point, and visible imperfection suit personal or social data; crisp FT-style charts suit markets and policy.
- **Reuters, Bloomberg, FT** differ in look but share: one highlight colour, greys for context, direct labels, a sourced footer, and heavy use of maps and small multiples [28][29][34].

---

## 8. Studio work vs template/AI work (2025–2026)

**What studio work has:**
- A rule that generates the frames (§1.1), visible across the sequence.
- A process model behind every texture: ink, plate, screen, paper, camera.
- Specific source material: real wood type, a real photograph, a real data set, a real era.
- Asymmetry, scale jumps of 3× and more, and empty cells left empty.
- A palette defined as inks + paper, with overprint colours.
- One outrageous move per frame at most, against a strict system.
- Motion with a behaviour (what moves, along which axis, what never moves).

**Tells of template or AI work (graphic-specific; TASTE §9 has the general list):**
- **Bento grids**: equal-weight rounded cards filling every cell ("cards all asking for the same amount of attention", TASTE §9).
- **Gradient blobs and mesh gradients** behind anything.
- **"Neo-brutalist" cards**: pastel fills, 3–4 px black borders, hard black offset shadows, chunky sans. Not brutalism (§9.7).
- **Grain on everything** at the same strength over flat vectors, standing in for "texture".
- **Y2K chrome**, holographic gradients, liquid-metal type.
- **Corporate Memphis figures** (flat people with long limbs): unrelated to Memphis design.
- **Blanded geometric sans wordmark** in lowercase with a neutral palette [26].
- **Riso preset**: pink + blue with a grain overlay and no plate logic, overprint or misregistration.
- **Halftone preset**: one dot size on everything, type included, or a moiré-swimming screen.
- **Decorative grid lines** drawn in the background with nothing aligned to them.
- **Stickers and badges** ("NEW!", starbursts, sparkles) as fillers.
- **Fake editorial**: numbered labels 01/02/03 on non-sequences, tiny tracked mono captions, serif headlines with one italic word (BAR fourth rejection).

---

## 9. Recipes

Eight distinct graphic worlds. Each: palette (hex), type, grid, texture, motion. Fonts in parentheses beyond the bundled ones (Inter, Inter Display, Instrument Serif, JetBrains Mono) need to be supplied via the spec's `fonts` map; see §10. Palettes marked *ours* are proposals, not from a source. Vary them per brief: two pieces from the same recipe should still look different (TASTE, *Diverge*).

### 9.1 Swiss concert poster

- **Idea**: the poster as a score; geometry carries rhythm (Müller-Brockmann's Musica Viva posters).
- **Palette** *(ours)*: white `#FFFFFF`, black `#111111`, one signal colour per piece: red `#E3241B`, ultramarine `#1F3FBF` or orange `#FF5A1F`. Tints of black (20/40/60%) for the arcs.
- **Type**: neo-grotesk, flush left, ragged right, 2 sizes only (one at 10× the other). Inter Display at 800–900 with −0.03 em tracking works; (Archivo, Schibsted Grotesk, TeX Gyre Heros as a Helvetica stand-in).
- **Grid**: 4×4 or 8×4 fields (§2.3); text hangs from field tops; arcs and bars snap to field edges.
- **Texture**: none, or a very subtle offset-paper tone. The rigour is the texture.
- **Motion**: concentric arcs and bars rotate or extend on the beat in 15° steps; type is placed, not animated in (hard cut on), or slides along its column. Easing: sharp out (`outExpo`, 0.25–0.4 s). Scene change = column count change.

### 9.2 Risograph zine

- **Idea**: a photocopied/riso fanzine for a scene, a club, a community.
- **Palette**: two or three riso inks on stock (§3.2): Fluorescent Pink `#FF48B0` + Medium Blue `#3255A4` on bright white `#FBFBF8`; or Yellow `#FFE800` + Federal Blue `#3D5588` on newsprint grey `#E6E5DF`. All fills = tints or overprints (multiply).
- **Type**: big condensed or grotesque display, cut and pasted (Anton, Archivo Condensed/Expanded, Bricolage Grotesque); body in a typewriter or mono (JetBrains Mono, Space Mono, Courier Prime).
- **Grid**: loose 2–3 column A4 page; elements rotated by ±1–3° as if pasted.
- **Texture**: halftone photos (12–20 px cell at 45°), plate misregistration 4–7 px (A4 scale, §4.3), stochastic grain per plate, ink coverage modulated by paper.
- **Motion**: stepped time (8–12 fps feel, held frames) for the print layers; plate offsets change per "pull"; pages flip and slap down; type stays still once it lands so it can be read.

### 9.3 Corporate identity system (generative device)

- **Idea**: one device, applied everywhere (Dumbar stripes, Whitney's responsive W, London 2012's grid shards).
- **Palette** *(ours)*: 2 brand colours + 2 neutrals, e.g. deep green `#0B3D2E` + electric yellow `#E8FF3A` + white `#FFFFFF` + mid grey `#8C918D`. Overprint not used; flat, clean.
- **Type**: one family in 2–3 weights (Familjen Grotesk, Hanken Grotesk, Instrument Sans, IBM Plex Sans); figures and labels in the same family.
- **Grid**: Gerstner 58 at 30 px (§2.2); the device lives on the grid (stripe angle and width in grid units).
- **Texture**: none; material only from the device's application (stripe on vehicle, building, page).
- **Motion**: the device has one behaviour (it stretches, slopes, or wipes), and every transition is that behaviour. The device is the transition system; there are no other wipes.

### 9.4 Editorial data story

- **Idea**: a magazine feature as film: a finding, a chart, a detail, a person.
- **Palette** *(ours)*: white `#FFFFFF` or deep ink `#0E1A24` ground, greys `#C9CDD1`/`#6B737B` for context, **one** highlight (e.g. hot red `#FF3B2F` or cyan `#00A6D6`), black text. Avoid the FT's trademark salmon: it is their brand.
- **Type**: a grotesk for headlines and annotations with tabular figures (IBM Plex Sans + Plex Mono, Inter with `tnum`, Source Sans 3); optionally a news serif for the title only (Newsreader, Source Serif 4) if the concept is newspaper, not the Instrument Serif "tasteful-AI" look.
- **Grid**: 6-column (240 px columns, §2.2); charts span 4–6 columns; annotations in the remaining columns, aligned to data points.
- **Texture**: none, or a halftone photograph as the "lead image" scene.
- **Motion**: §7 reveal order; lines draw at constant speed; highlight colour arrives last on the beat; camera pushes into the annotated point (scale 1→2.5) and cuts to the human detail.

### 9.5 Memphis

- **Idea**: furniture-as-graphics, anti-tasteful, playful (Sottsass, Du Pasquier, 1981–87).
- **Palette** *(ours, from Memphis objects)*: pink `#F7A8C4`, mint `#8ED1C0`, lemon `#FFE04B`, cobalt `#2A4BD7`, red `#E8402A`, black `#111111`, white `#FFFFFF`. Black-and-white squiggle and dot patterns between fills.
- **Type**: geometric, chunky, or wide (Syne ExtraBold, Rubik Mono One, Unbounded); sometimes outlined.
- **Grid**: objects in perspective-free axonometric arrangement; shapes stacked like furniture (columns, half-discs, slabs).
- **Texture**: flat; patterns (squiggles, terrazzo confetti, dot grids) as laminates on shapes [32].
- **Motion**: objects assemble like a set being built: drop, bounce with 1 overshoot, settle; patterns slide inside their shapes (masked). Avoid "Corporate Memphis" figures entirely.

### 9.6 Polish poster

- **Idea**: one metaphor in paint, cut paper and hand lettering (Tomaszewski, Lenica, Cieślewicz, Fangor) [30].
- **Palette** *(ours)*: 3–4 gouache colours on raw paper: black `#1B1A19`, cadmium red `#D8291C`, ochre `#D9A21B`, sour green `#8DB33A` on off-white `#EDEBE4` with real fibre texture. (Not terracotta; this paper is grey-white, not cream.)
- **Type**: hand lettering, integrated into the image, never a set typeface; drawn as paths (brush strokes with wobble). Fonts can't do this convincingly.
- **Grid**: none visible; one central image, lettering placed where the image leaves room.
- **Texture**: brush texture in fills (dry edges), cut-paper edges, visible underdrawing.
- **Motion**: stop-motion at 8–12 fps; strokes paint on; cut-paper pieces swap (Lenica's animation); "boil" (lines re-drawn every 2–3 frames with 1–2 px variation).

### 9.7 Brutalist (actual, not "neo-brutalist")

- **Idea**: the raw material of the medium, shown honestly: default system type, HTML structure, hard rules, information density.
- **Palette** *(ours)*: white `#FFFFFF`, black `#000000`, link blue `#0000EE`, visited purple `#551A8B`, highlighter yellow `#FFFF00`. No shadows, no radius.
- **Type**: system or mono only (JetBrains Mono, Space Mono, Arial/Liberation Sans), one size for everything except one huge element; underlines as styling.
- **Grid**: the document's own flow; tables with 1–2 px rules; dense.
- **Texture**: none; screen pixels, scrollbars and cursors as found material.
- **Motion**: no easing (linear or instant); hard cuts every 0.3–0.8 s; scrolling text at constant speed; elements appear in a single frame.

### 9.8 Op-art

- **Idea**: perception as the subject (Vasarely, Riley).
- **Palette**: black `#000000` + white `#FFFFFF`; or two complementaries at equal luminance (only when no text needs reading); or Vasarely's six-colour "plastic alphabet" graded in 12–15 steps [5].
- **Type**: minimal; a monoline geometric (Major Mono Display, Unbounded, Inter Display at 900) in knocked-out bars, or none.
- **Grid**: a strict cell grid (e.g. 17×17 or 32×18 cells of 60 px at 1920×1080) or line fields at a fixed pitch (≥ 8 px, §4.6).
- **Texture**: none; the pattern is the texture.
- **Motion**: animate the modulation field (a sphere "swelling" through the grid; a wave travelling across line widths); moiré by rotating one line screen 1–3° against another. Check photosensitivity (§4.6).

---

## 10. Typefaces

Bundled in Anikit Video: **Inter** (variable, with **Inter Display**), **Instrument Serif** (+ italic), **JetBrains Mono** (+ italic), Noto Sans Hebrew and Arabic, Twemoji. Anything else goes in the spec's `fonts` map (`{ family: path }`). The faces below are free (SIL OFL unless noted); verify the licence of the exact files before shipping.

| Face | Character | Recipes |
|---|---|---|
| Inter / Inter Display | Neutral neo-grotesk; with tight tracking and 900 weight it reads Swiss | Swiss, data, identity |
| Instrument Serif | Condensed editorial serif; the "tasteful-AI" tell when paired with cream + terracotta | Use sparingly, for a reason |
| JetBrains Mono | Mono | Brutalist, data labels, zine body |
| Archivo (incl. widths: Condensed → Expanded, Black) | Grotesque with a width axis | Swiss, zine, identity |
| Space Grotesk / Space Mono | Quirky grotesk from a mono | Zine, brutalist |
| Familjen Grotesk | Warm Scandinavian grotesk | Identity |
| Hanken Grotesk, Instrument Sans, Schibsted Grotesk | Clean grotesks | Identity, data |
| Bricolage Grotesque | Variable, optical-size, ink-trap-ish | Zine, editorial |
| IBM Plex Sans / Mono / Serif | Engineered, excellent figures | Data, identity |
| Source Sans 3 / Source Serif 4, Newsreader | News text | Data, editorial |
| Fraunces | "Wonky" soft serif with optical sizes | Editorial (watch the tasteful-AI combination) |
| Syne | Wide-to-extra-bold geometric | Memphis, art events |
| Unbounded, Rubik Mono One | Wide, chunky | Memphis, op-art |
| Anton, Bebas Neue, League Gothic, Oswald | Condensed display, wood-type poster lineage | Public Theater-style, zine |
| Big Shoulders Display | Condensed, Chicago signage | Posters, identity |
| Major Mono Display | Geometric mono display | Op-art |
| Courier Prime | Typewriter | Zine, documents |
| TeX Gyre Heros (GUST Font Licence) | Helvetica-metric clone | Swiss, Turley-style editorial |

Avoid as defaults: Montserrat, Poppins, Playfair Display, Satoshi, Clash Display, General Sans (the 2020s template faces); Geist and Manrope as unchosen "tech" defaults.

---

## 11. Gap list: what a 2D engine needs for this work

Status (updated after the GRAPHIC session's work; `anikit-video docs graphic`):
- **Shipped:**
  - 1: `grid` with named `areas`, `cell` placement, baseline leading, `show: true | "under"`. Gerstner-style multiple column counts: give a group its own `grid`.
  - 2: `duotone`, `gradientMap`, `posterize`, `toneRange`, `toneAmount` on any layer or group.
  - 3, single ink: `halftone` with dot, line or square spots, gain, ink/`source`, paper, frame-anchored. Multi-ink halftone works by stacking `multiply` layers with different angles.
  - 6: `step` ("twos", fps).
  - 7: `squircle`/`superellipse`.
  - 8: `blob`, animatable with `wigglePath`.
  - 9: `pattern` layers, 15 kinds with ramp and jitter. Inside text, use a pattern layer with `matte`.
  - 4, partly: `multiply` overprint, `boil` for drifting plates, `inkTexture` for uneven coverage.
  - 5, partly: `inkTexture` plus `post.grain`.
- **Shipped later:** 10 (`cutout`), 12 (`brush`), 13 (`look: "editorial"` on line and bar charts), 14 (pattern `field`), and `theme.palette` plus `riso:` inks.
- **Open:** 11 (photocopy: compose it as `posterize: 2` + `duotone` + `toneRange` + `inkTexture`), 15 (critique checks: off-grid elements, pattern pitch, halftone on reading text. Compile-time warnings for pitch < 4 px and halftone on small text exist).

1. **Named modular grid.** A scene-level `grid` (columns, rows, unit, gutters, margins, baseline) with Gerstner-style multiple column counts on one unit, and layer placement by cell (`"cell": "B2:D3"`, `"col": "2/5"`), with baseline snapping of text. Moves that target grid lines.
2. **Gradient map / duotone / tritone per layer and per group**, with a pre-curve (black point, white point, gamma) and posterise levels. (In progress as `Tone`.)
3. **Halftone per layer**: cell size, angle, dot shape (round, ellipse, square, line, stochastic), dot gain, ink + paper colours, frame- or layer-anchored. Multi-ink halftone with per-ink angle. (Single-ink in progress.)
4. **Ink/plate model**: assign layers to named inks; each plate composites with multiply onto a paper colour; per-plate offset and rotation (misregistration) held or stepped in time; knockout vs overprint per element; trap/gap control.
5. **Paper substrate**: paper colour plus low-frequency tone, fibre, and an ink-absorption map that modulates ink coverage and roughens edges, shared by all plates.
6. **Stepped time per layer** ("on twos/threes", `fps` per layer or group) for stop-motion, riso pulls and boil, while the camera and other layers stay at 60 fps.
7. **Superellipse shape** (`shape: "superellipse"`, exponent n, or `cornerSmoothing` on `rect`).
8. **Constrained blob shape**: N control points, seed, curvature bound, animatable morph between seeds, crisp edges.
9. **Pattern fills**: tiled patterns as `fill` for any shape or text (stripes, dots, checks, Truchet with seed and per-tile flip animation, Vasarely units driven by a field function, Memphis squiggle scatter at a density), with pattern transform independent of the shape.
10. **Cutout and collage**: alpha edge styles (clean, torn with paper rim of N px, hand-cut polygonal), paper drop shadow with consistent light.
11. **Photocopy filter**: threshold, toner noise, edge darkening, generation loss.
12. **Brush and hand-lettering strokes**: variable-width strokes with dry-brush texture and paint-on animation; a path "boil" (re-randomise every N frames).
13. **Data-viz editorial defaults**: direct labels, single highlight series, top-gridline unit labels, tabular figures exposed on `text` (`features` exists in `text/layout.rs`; check that the spec exposes it).
14. **Op-art line fields**: parallel lines whose width or phase is driven by a field; checks for minimum pitch against the output resolution and a photosensitivity warning.
15. **Critique checks** (`anikit-video critique`): elements off-grid by a few px; pattern pitch under 4 px; halftone on readable text; per-element random jitter; grain on all layers at equal strength; bento-grid detection (N equal rounded cards).

---

## Sources

1. Karl Gerstner, *Designing Programmes* (1964; Lars Müller, 2019). Excerpt PDF: https://openlab.citytech.cuny.edu/langecomd3504fa2019/files/2018/10/Gerstner_DesigningProgrammes-1.pdf
2. ms-studio, "Karl Gerstner's layout grid" (58-module *Capital* grid, rebuilt in Figma): https://ms-studio.net/notes/karl-gerstners-layout-grid/
3. Josef Müller-Brockmann, *Grid Systems in Graphic Design* (Niggli, 1981; 8 to 32 fields): https://draw-down.com/products/grid-systems-in-graphic-design
4. Truchet tiles, incl. C. S. Smith, "The Tiling Patterns of Sebastien Truchet and the Topology of Structural Hierarchy", *Leonardo* 20(4), 1987: https://en.wikipedia.org/wiki/Truchet_tiles
5. Victor Vasarely (unités plastiques, 1959 patent; *Kezdi*): https://en.wikipedia.org/wiki/Victor_Vasarely
6. Immelmann et al., "Kandinsky's Bauhaus Questionnaire" (2020): https://dc2.philarchive.org/rec/IMMKBQ
7. Matt DesLauriers, `riso-colors` (data from stencil.wiki; hex and Pantone): https://github.com/mattdesl/riso-colors
8. Riso registration tolerance (operator reports) and College for Creative Studies, "Registration & Trapping": https://www.copytechnet.com/forum/technical-forums/digital-duplicators/30366-risograph-gr-3770-offset-problem ; https://campus.collegeforcreativestudies.edu/imaging-center/2021/06/22/registration-trapping/
9. PrintPlanet, "Screen angles: why are they 15, 45, 75 and 90?": https://printplanet.com/threads/screen-angles-why-are-they-15-45-75-and-90.10650/post-114308
10. Adobe InDesign manual, "About halftone screen frequency": https://www.manualsdir.com/manuals/753855/adobe-indesign.html?page=770
11. PrintWiki, "Screen Ruling": https://printwiki.org/Screen_Ruling
12. John D. Cook, "History of the term squircle" (Piet Hein n = 2.5, a/b = 6/5): https://www.johndcook.com/blog/2019/04/02/history-of-the-term-squircle/
13. Figma Help, "Adjust corner radius and smoothing" (iOS = 60%): https://help.figma.com/hc/en-us/articles/360050986854
14. Cooper Hewitt, "Everyone's Public Theater": https://www.cooperhewitt.org/2018/04/05/everyones-public-theater/
15. Fonts In Use, "The Public Theater logo evolution 1994–2008": https://fontsinuse.com/uses/2197/the-public-theater-logo-evolution-1994-2008
16. Pentagram, *Twenty-Five Years at the Public: A Love Story*: https://pentagram.com/work/25-years-at-the-public-a-love-story/story
17. Studio Dumbar, Dutch Police: https://studiodumbar.com/work/dutch-police
18. Print, "Design Classics: The Dutch Police Identity and Striping": https://www.printmag.com/branding-identity-design/design-classics-the-dutch-police-identity-and-striping/
19. It's Nice That, "Bold new identity for the Whitney Museum from Experimental Jetset"; Design Observer, "The Whitney Identity: Responding to W(hat)?": https://itsnicethat.com/articles/whitney-museum-identity ; https://designobserver.com/the-whitney-identity-responding-to-what/
20. Design Week, "'It was a strange moment in time': Wolff Olins on London 2012 ten years later": https://www.designweek.co.uk/issues/20-26-june-2022/london-2012-olympics-brand/
21. Collins, Spotify: https://www.wearecollins.com/work/spotify/
22. Eye magazine, "Taking care of business" (Richard Turley, *Bloomberg Businessweek*, 2011): https://eyemagazine.com/feature/article/taking-care-of-business
23. Creative Boom, "Motion branding's moment" (Koto, Deezer): https://www.creativeboom.com/insight/motion-brandings-moment-why-brands-need-to-catch-up/
24. Creative Boom, "How to do motion-first branding better" (Simon Dixon, DixonBaxi): https://www.creativeboom.com/insight/how-to-do-motion-first-branding-better/
25. It's Nice That, "Forward thinking: five graphic trends to keep in your sights in 2025": https://itsnicethat.com/features/forward-thinking-five-graphic-trends-2025-graphic-design-130125
26. D&AD, "Beyond the sans serif: how type can move on from 'blanding'": https://www.dandad.org/insights/features/beyond-sans-serif-how-type-can-move-blanding-awards-insights
27. Giorgia Lupi, "Data Humanism: The Revolutionary Future of Data Visualization", *Print* (2017): https://www.printmag.com/article/data-humanism-future-of-data-visualization/
28. iMEdD Lab, "From data to storytelling: concept and design tips from the Financial Times' John Burn-Murdoch": https://lab.imedd.org/en/from-data-to-storytelling-concept-and-design-tips-from-the-financial-times-john-burn-murdoch/
29. Datawrapper, interview with Amanda Cox (Bloomberg): https://www.datawrapper.de/blog/amanda-cox-bloomberg
30. Polish School of Posters: https://en.wikipedia.org/wiki/Polish_School_of_Posters ; BFI, "Polish poster art for classic British films": https://www.bfi.org.uk/features/polish-poster-art-classic-british-films
31. AIGA Eye on Design, "Poster Picks: Steven Heller on 5 top Ikko Tanaka designs": https://eyeondesign.aiga.org/poster-picks-steven-heller-on-5-top-ikko-tanaka-designs/
32. Metropolis, "Q&A: Nathalie Du Pasquier, the Queen of Memphis": https://metropolismag.com/profiles/qa-nathalie-dupasquier-queen-memphis/
33. Dezeen, "The return of Memphis" (2014): https://www.dezeen.com/2014/04/13/the-return-of-memphis/
34. Design Week, "In-house teams: How Reuters Graphics visualises 'catastrophic' world events": https://www.designweek.co.uk/issues/27-january-2-february-2020/in-house-teams-how-reuters-graphics-visualises-catastrophic-world-events/
35. Internal: [`../quality/BAR.md`](../quality/BAR.md), [`../taste/TASTE.md`](../taste/TASTE.md), [`../taste/sources.md`](../taste/sources.md).
