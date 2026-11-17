---
name: brand-kit
description: Builds brand.json (colours, fonts, logo, voice) from a site, README or logo, with contrast checks.
---

# Brand kit

```json
{ "name": "Acme", "theme": "dark", "colors": { "primary": "#D9481C" }, "logo": "logo.svg",
  "voice": { "cta": "Start free", "url": "acme.dev", "avoid": ["revolutionary"] } }
```

All keys are optional. Other keys:
- `colors`: also `secondary`, `accent`, `background`, `surface`, `text`, `muted`;
- `fonts`: `{ heading, body, mono }`;
- `radius`;
- `voice.tone`.

Missing colours are derived: text gets the best contrast against the background, muted is 28% of the way from text to background, and secondary is the primary shifted −20° in hue (analogous).

- **primary:** the main button or logo colour; prefer exact hex values from CSS or the theme over guesses.
- **theme:** match the product's own UI and world (a paper product is light, a terminal tool may be dark). Never default to dark-with-violet: that is the house look of every generated video (`docs taste` §2).
- **background:** a tinted near-black (#0B0B12) or an off-white (#FAFAF7), never pure #000 or #FFF.
- **contrast:** text on background at least 7:1, muted at least 4.5:1. The engine picks black or white button text automatically.
- **fonts:** the brand's own faces if you have the files (add them to the spec's `fonts`). Otherwise choose a face for the subject (a serif for editorial or heritage, a grotesk for precision, mono for tools) and write down why; Inter is fine only as a decision, not a default (`docs taste` §1).
- **palette:** take it from the brand's real assets (site CSS, product photos, packaging), values first: decide light vs dark, then one base family and one accent. If the brand's primary is indigo or violet, use it, but don't add the blue gradient.
- **logo:** SVG, or a PNG at least 512 px wide with transparency, in a light version on dark themes.
- **voice:** 3 adjectives from the source, banned words, the site's CTA and the short URL.
