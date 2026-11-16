---
name: brand-kit
description: Builds a brand.json (colours, fonts, logo, voice) from a website, README, logo or screenshots, with contrast checks.
inputs: URL / README / logo / screenshots
outputs: brand.json
---

# Brand kit builder

## The `brand.json` format

```json
{
  "name": "Acme",
  "theme": "dark",
  "colors": {
    "primary": "#6E56CF", "secondary": "#22D3EE", "accent": "#FF5CA8",
    "background": "#0B0B12", "surface": "#171725", "text": "#F5F5FA", "muted": "#9A9AB0"
  },
  "fonts": { "heading": "Inter Display", "body": "Inter", "mono": "JetBrains Mono" },
  "logo": "logo.svg",
  "radius": 28,
  "voice": { "tone": "confident, precise, warm", "avoid": ["revolutionary", "seamless"], "cta": "Start free", "url": "acme.dev" }
}
```

Every field is optional. The engine derives what is missing:
- `text` is black or white, whichever contrasts best with `background`;
- `muted` is the text colour 38% towards the background;
- `surface` is the background 4–7% towards the text;
- `secondary` is the primary colour rotated 40° in hue;
- `accent` defaults to `secondary`.

Templates also see derived tokens: `onPrimary`, `onAccent`, `border`, `bgDeep`.

## Procedure

1. **Primary:** the colour of the main button or link on the site, or the dominant logo colour. Prefer the brand's real hex code (from CSS custom properties, a theme file or a README badge) over guessing from a screenshot.
2. **Theme:** match the product's own UI. When unsure, dark: it is more forgiving for motion and glows.
3. **Background:** do not use pure #000 or #FFF. Use a near-black slightly tinted toward the primary (e.g. #0B0B12), or an off-white (#FAFAF7).
4. **Check contrast** (WCAG): text on background at least 7:1; muted on background at least 4.5:1; `onPrimary` on primary at least 4.5:1 (the CTA button). If the primary is very light (yellow, lime), the button text becomes black automatically; keep it.
5. **Fonts:** use the site's fonts only if they are available as files (add them to the spec's `fonts` map). Otherwise use the bundled Inter Display / Inter / JetBrains Mono, which look premium and are always available. Use at most two families plus mono.
6. **Logo:** prefer SVG, or else a PNG at least 512 px wide with a transparent background. On a dark theme, use the light version of the logo.
7. **Voice:** three adjectives from the source's own writing, banned words (the competitor's slogans plus generic hype), the CTA the site uses on its main button, and the short URL.

Validate with `anikit-video templates expand logo-reveal --brand brand.json` (it errors with "did you mean" on misspelt keys).
