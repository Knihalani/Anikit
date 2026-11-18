# Graphic design proofs

Five short pieces, each in a different graphic-design tradition. Every one is built from the `graphic` features
(`anikit-video docs graphic`) plus standard layers, with no bitmaps. Specs are in `examples/video/graphic/`, and the fonts
(OFL) are in `examples/video/graphic/fonts/`. Render times are wall clock on this container (`render`, release build, 1920×1080).

| Piece | Tradition | Features doing the work | Length | Render |
|---|---|---|---|---|
| `pieces/konkrete.mp4` | Swiss / International poster system (Archivo, black/red/paper) | 12×8 grid with named areas, arc rings, dot-ramp pattern, line screen, truchet/arc tiles, halftone sphere | 10.0 s | 1.9 s (155 fps) |
| `pieces/paper-weekend.mp4` | Risograph zine (3 inks: fluo pink, medium blue, yellow) | multiply overprint, misregistered plates, halftone blob and type, posterised gradient-map over a reaction-diffusion shader, pattern pages | 9.2 s | 16 s (17 fps: the turing shader + grain) |
| `pieces/fold-identity.mp4` | Corporate identity system (Space Grotesk, pine/signal) | squircle + arc mark, `grid.show: "under"` construction, arcs pattern from the mark, colour columns on the grid, signage | 12.5 s | 1.1 s (343 fps) |
| `pieces/tempest-collage.mp4` | Collage theatre poster, Polish-poster spirit (Anton, navy/red/yellow on grey board) | `cutout` torn and scissor pieces with paper rims and printed shadows, posterised gradient-map ocean (shader), halftoned gem, `inkTexture` red sun, hard camera reframes on the beat inside one poster | 11.0 s | 86 s (4 fps: ocean + hero shaders every frame) |
| `pieces/street-ledger.mp4` | Editorial data story (Fraunces + Plex Mono, one highlight) | grid typography, direct-labelled line chart via `draw`, unit (dot) chart, paired bars with tabular labels, counter | 15.0 s | 1.3 s (361 fps) |

`paper-weekend.mp4` and `tempest-collage.mp4` are re-encoded with x264 CRF 22 for repo size, because grain and halftone are expensive to encode. The others are the engine's own MP4s.

Frame sheets (`*-frames.jpg`) are taken from the encoded MP4s. Feature stills are in this folder: `shapes-patterns.jpg`, `treatments.jpg`
and `grid.jpg`, with their specs.

The data in *The Street Ledger* is illustrative (a fictional city) and is labelled so on screen.

More feature proofs: `tiles-animated.jpg` (custom `tile` motifs, `turns`, `stagger`, `ramp`, and a halftone whose cell and angle animate), `op-art.jpg` / `op-art.mp4` (pattern `field`s driven by distance and time, live at 46 fps with three regenerated patterns).
`brush.jpg` / `brush.mp4`: `brush` strokes (ensō with dry bristles, a pressure swash and a tapered rule) painting on with `trimEnd`, at 415 fps.
