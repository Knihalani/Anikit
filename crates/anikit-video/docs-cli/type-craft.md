# Type craft: choosing faces, scale, type as material

```json
{ "scenes": [ { "duration": 3, "background": "#0a0a0a", "layers": [
  { "id": "word", "text": "MOLTEN", "font": "Big Shoulders", "weight": 900, "size": "62vh", "lineHeight": 0.8,
    "kinetic": "mask by char up 0.7s 50ms outExpo" },
  { "shader": "liquid-chrome", "width": "100%", "height": "100%", "matte": "word" } ] } ] }
```

## Choosing (from docs/video/craft/typography.md)

- Give a reason in one sentence ("compressed gothic because it's sport and the frame is wide"). One family in 2 weights plus
  one accent face beats three families. Make contrast with size first, then weight, then face.
- Size contrast in motion is 10:1 to 25:1: a headline at 30–60 vh beside a caption at 2.2–3 vh. 9 vh over 3 vh reads as a slide.
- Set text flush left on a grid. Centre only one word, a date or a logo.
- Track display lowercase −0.01 to −0.05 em; track caps and small labels +0.05 to +0.12 em. Leading: display 0.85–1.0, text 1.25–1.45.
- Hold time ≈ 0.5 s + 0.3 s per word.

Starting faces by brief (try them; `fonts NAME` for pairings): **Swiss** Inter Tight, Archivo (wdth), Schibsted Grotesk + IBM Plex Mono ·
**Editorial** Bodoni Moda / Playfair (opsz) + Jost · **Documentary** Newsreader + Schibsted Grotesk · **Sport** Big Shoulders, Anton, Saira (wdth 50–125) ·
**70s** Fraunces SOFT 100, Young Serif, Shrikhand · **Brutalist** Archivo Black, Syne 800, Anybody (wdth 50→150), Six Caps ·
**Tech** Martian Mono, Space Mono, VT323, Recursive · **Luxury** Cormorant Garamond light, caps +0.2em, Mrs Saint Delafield ·
**Gothic** UnifrakturCook, Grenze Gotisch · **Retro computing** Sixtyfour / Workbench (animate SCAN, BLED) · **Playful** Kablammo (MORF), Shantell Sans (BNCE) ·
**Fake 3D** Tilt Warp / Prism / Neon (XROT, YROT ±45).

Colour fonts (Nabla, Honk, Bungee Spice) draw their own palette; `color` doesn't tint them.

## Type as material and as a window

- **Gradients:** `color: "linear-gradient(90deg, #ff3d6e, #ffd23d)"` works on plain, split and kinetic text. On kinetic text
  the gradient is drawn through each glyph's animated alpha.
- **Anything inside the letters** (footage, image, shader, 3D, a group): give the text an `id` and put the fill layer *after* it with
  `"matte": "<id>"`. The text then becomes the window and isn't drawn itself. Kinetic and split animations still drive the window:
  (example at the top of this page).
  `{"layer": "word", "mode": "alphaInverted"}` knocks the word *out* of a plate. Footage inside a 50–70 vh compressed sans is the
  classic title move; use heavy weights (800+) so enough of the image shows.
- **Outline:** `stroke` + `strokeWidth`; `color: "none"` leaves only the outline. Layering a solid copy with a hollow copy
  offset by 0.04 em gives an inline/offset print look.
- **Hard offset shadow** (70s, print, poster): `"shadow": {"x": 14, "y": 14, "blur": 0, "color": "#2c1a10"}` on the text layer. It follows
  per-glyph animators. Offset about 0.05–0.07 em, in a darker shade of the paper rather than black.

## Choreography (numbers from typography.md §2: advice, not rules)

- **Hold ≈ 0.5 s + words ÷ 3**, counted from when the *last* word is legible. Count a number as two words.
- **Stagger:** 15–30 ms per char reads as one gesture, 40–80 ms is deliberate (3–12 char titles), 60–150 ms per word, 80–200 ms per line.
  Keep a reveal of more than 8 characters inside 1.2 s overall.
- **Ease:** `outExpo`/`outQuart` in; `inQuart` out, at 0.6–0.7× the entrance. Overshoot only for playful briefs, and gently.
  Travel 0.3–0.6 em: further than that reads as a slide-in, not a type entrance.
- **Animate in, then hold still.** Text that wobbles while it's being read is a tell. Spend the motion on one property (position *or* tracking *or* weight).
- **Signature moves:** mask by line up (`"kinetic": "mask by line up"`, 120 ms per line); tracking converge (`animate` `letterSpacing` from +0.25 em to 0
  over 1.5–3 s); weight punch (`axes.wght` 100→800 in 250–450 ms as the word lands); width morph pinned to the left edge (`axes.wdth`);
  a weight wave (animator `axes`); a slam cut on a beat (0–2 frames in, held 12–24 frames).
- **Escalate.** Repeating one entrance on every card reads as a template. Vary unit, direction or property across beats and build toward the payoff.
- Wrapped headlines get balanced lines automatically (`"balance": false` turns it off).
