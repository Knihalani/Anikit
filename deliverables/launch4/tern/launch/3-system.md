# 3 · Visual system

**Derivation line:** *Tern decides "record" and "level" for you → the recordist's hand stops writing excuses and only circles.* The only thing that ever moves on its own is the ballpoint (writing, circling). Everything else moves because the camera follows the pen, or because a recorded sound hits the page.

**Material world:** one three-part carbonless sound report. Pink (mixer copy), yellow and white sheets of the same printed form. Printed in one near-black ink (`#2A2427`), written in Bic-blue ballpoint (`#2343A6`). No other colours. The yellow and white copies show the *carbon* impression: the same handwriting, lighter and softer, slightly offset.
- **Light logic:** one desk lamp at the upper left, so the page falls off gently toward the lower right (multiply radial, darkest ~80%). It's the same in every shot.
- **Palette family:** NCR stock (pink `#F2C4CB`, yellow `#F3E3A2`, white `#F4F2EC`) + print ink + ballpoint blue. Colour lives in the paper, the product's own world. The blue ink is the single accent and covers well under 10% of the frame.

**Type:** 2 families on the form's own grid (columns at 80 / 520 / 610 … 1050 / 1840, rows every 86 px).
- **Archivo** (`wdth` 62–75): the printed form. Engineered, condensed, the voice of business forms.
- **Reenie Beanie**: the recordist's ballpoint.
- **Sizes per frame ≤ 3:** form labels 17 px, handwriting 56 px, display 86 px (the "SOUND REPORT" header). The camera's zoom sets the apparent scale. At the end the logotype is the handwritten "Tern" in the Recorder field, pushed to macro, so the name is made of the world.
- All type is *inside* the scene: printed on the form or written on it. No overlay captions.

**Motion vocabulary**
- **Easing:** `inOutSine` (pen strokes, loops), `outCubic` (camera arrivals), `inOutCubic` (camera moves between rows).
- **Durations:** 0.4 s (a circle, a short word), 0.8 s (a long note), 1.6 s (a camera move between areas).
- **Causes:** the pen writes → the camera follows. A recorded sound (door, thunder) jolts the page (a camera shake of ≤ 10 px), and that is the only shake. No drift, no float, no glow.

**Texture:** one texture only, light film grain over the whole film (paper tooth). `inkTexture` 0.12 on printed rules only.

**CG blacklist:** no 3D at all; count 0. **Banned looks checked:** not cream + terracotta (the stock is NCR pink and yellow, with blue ink), no glowing serif, no near-black + acid, no tracked-caps-over-headline kit (the form's tracked labels are a real form's furniture, inside the document).

**What changes per shot:** subject (a row, the header field, the legend, the whole stack), scale (macro 2.8× ↔ full page ↔ three sheets), camera (lock, follow-pan, push, pull-back, a hard jolt).

## Style frames (step 4)
See `frames/frames.png` and `frames/notes.md`.
