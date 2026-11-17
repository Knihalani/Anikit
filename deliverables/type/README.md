# TYPE proofs: four type-led pieces in different typographic styles

Every face here comes from the font library (`anikit-video fonts`); none of them is bundled. Stills are taken at t = 3.8 s and were checked at 100% crop.

| Piece | Faces | Type features shown |
|---|---|---|
| 1-swiss | Archivo (wdth 62→125, wght 900→100), IBM Plex Mono | width/weight morph with the left edge held, flush-left grid, `tnum`, auto opsz |
| 2-editorial | Bodoni Moda (opsz auto → 96 at 58 vh), Jost | didone hairlines at display size, italic crop, weight drift, tracking-out on caps, `onum` |
| 3-sport | Big Shoulders 900, Saira (wdth 50→125, outlined), Martian Mono (wdth 112.5) | compressed stack at 0.86 leading, width-slam with the right edge held, typed metadata |
| 5-weight-wave | Fraunces (wght 150 base, animator wght 900 + SOFT 100) | per-glyph axis animator with advance compensation, looping via an expression offset |
| 6-seventies | Shrikhand + Fraunces (SOFT 100, WONK) caps | hard offset shadow under a per-glyph drop-in animator, flush-left stack |
| 4-terminal | Sixtyfour (SCAN/BLED axes), VT323, Silkscreen | CRT turn-on driven by custom axes, glow |

Render times (1080p30, 4 threads): 0.37 s, 0.71 s, 0.44 s and 0.61 s for 4–5 s clips (210–407 fps).
