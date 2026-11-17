---
name: brand-kit
description: Builds brand.json (colours, fonts, logo, voice) from a site, README or logo, with contrast checks.
---

# Brand kit

```json
{ "name": "Acme", "theme": "dark", "colors": { "primary": "#6E56CF" }, "logo": "logo.svg",
  "voice": { "cta": "Start free", "url": "acme.dev", "avoid": ["revolutionary"] } }
```

All keys are optional. Other keys:
- `colors`: also `secondary`, `accent`, `background`, `surface`, `text`, `muted`;
- `fonts`: `{ heading, body, mono }`;
- `radius`;
- `voice.tone`.

Missing colours are derived: text gets the best contrast against the background, muted is 28% of the way from text to background, and secondary is the primary shifted −20° in hue (analogous).

- **primary:** the main button or logo colour; prefer exact hex values from CSS or the theme over guesses.
- **theme:** match the product's own UI. If unsure, use dark.
- **background:** a tinted near-black (#0B0B12) or an off-white (#FAFAF7), never pure #000 or #FFF.
- **contrast:** text on background at least 7:1, muted at least 4.5:1. The engine picks black or white button text automatically.
- **fonts:** use the bundled Inter Display / Inter / JetBrains Mono unless you have the brand's font files (then add them to the spec's `fonts`).
- **logo:** SVG, or a PNG at least 512 px wide with transparency, in a light version on dark themes.
- **voice:** 3 adjectives from the source, banned words, the site's CTA and the short URL.
