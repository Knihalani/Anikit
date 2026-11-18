# Graphic design proofs

**Reel:** `graphic-reel.mp4` (59.7 s) runs all five pieces back to back with their scores, re-encoded with x264 CRF 21.

Six short pieces, each in a different graphic-design tradition. Every one is built from the `graphic` features
(`anikit-video docs graphic`) plus standard layers, with no bitmaps. Specs are in `examples/video/graphic/`, and the fonts
(OFL) are in `examples/video/graphic/fonts/`. Render times are wall clock on this container (`render`, release build, 1920×1080).

| Piece | Tradition | Features doing the work | Length | Render |
|---|---|---|---|---|
| `pieces/konkrete.mp4` | Swiss / International poster system (Archivo, black/red/paper) | 12×8 grid with named areas, arc rings, dot-ramp pattern, line screen, truchet/arc tiles, halftone sphere | 10.0 s | 1.9 s (155 fps) |
| `pieces/paper-weekend.mp4` | Risograph zine (3 inks: fluo pink, medium blue, yellow) | multiply overprint, misregistered plates, halftone blob and type, posterised gradient-map over a reaction-diffusion shader, pattern pages | 9.2 s | 16 s (17 fps: the turing shader + grain) |
| `pieces/fold-identity.mp4` | Corporate identity system (Space Grotesk, pine/signal) | squircle + arc mark, `grid.show: "under"` construction, arcs pattern from the mark, colour columns on the grid, signage | 12.5 s | 1.1 s (343 fps) |
| `pieces/tempest-collage.mp4` | Collage theatre poster, Polish-poster spirit (Anton, navy/red/yellow on grey board) | `cutout` torn and scissor pieces with paper rims and printed shadows, posterised gradient-map ocean (shader), halftoned gem, `inkTexture` red sun, hard camera reframes on the beat inside one poster | 11.0 s | 86 s (4 fps: ocean + hero shaders every frame) |
| `pieces/salone-memphis.mp4` | Memphis, 9:16 (Bricolage Grotesque, pink/mint/lemon/cobalt/red) | Patterns matted inside shapes (terrazzo `tile` confetti in a squircle slab, stripes in a ball), a squiggle `tile` field, `blob`, `arc` half-discs, spring drops, and a cell shuffle on a 6×10 vertical grid | 9.0 s | 1.5 s (183 fps) |
| `pieces/street-ledger.mp4` | Editorial data story told in the street's own language: road markings | Asphalt built from aggregate (two jittered dot patterns plus `inkTexture`), worn paint (`inkTexture` with fine chips) on road-stencil type (Anton, `scaleY` 1.9). The line chart is painted lane lines (dashed context, a solid yellow story line drawing on), the unit chart is 100 parking bays with 38 bike racks, and the bar chart is a street cross-section with lanes as wide as their share. | 17.0 s | 14 s (37 fps) |

`paper-weekend.mp4` and `tempest-collage.mp4` are re-encoded with x264 CRF 22 for repo size, because grain and halftone are expensive to encode. The others are the engine's own MP4s.

Frame sheets (`*-frames.jpg`) are taken from the encoded MP4s. Feature stills are in this folder: `shapes-patterns.jpg`, `treatments.jpg`
and `grid.jpg`, with their specs.

The data in *The Street Ledger* is illustrative (a fictional city) and is labelled so on screen. Its first version (charcoal ground, editorial serif, one amber accent, a tracked-caps eyebrow) was replaced: the judge's round-1 review names exactly that kit as the "tasteful AI" formula (`docs/video/craft/judge-r1-review.md`).

More feature proofs: `tiles-animated.jpg` (custom `tile` motifs, `turns`, `stagger`, `ramp`, and a halftone whose cell and angle animate), `op-art.jpg` / `op-art.mp4` (pattern `field`s driven by distance and time, live at 46 fps with three regenerated patterns).
`brush.jpg` / `brush.mp4`: `brush` strokes (ensō with dry bristles, a pressure swash and a tapered rule) painting on with `trimEnd`, at 415 fps.
`photo-treatments.jpg`: the treatments on a real photo (`polyhaven:hdri/venice_sunset` backplate): the original, a duotone with a pre-curve, a 5 px line screen (engraving), and a two-plate riso of the same photo (blue duotone plus pink halftone, multiply, offset 6 px).
`palettes.jpg`: `theme.palette` from five seeds and schemes (`anikit-video palette "<seed> <scheme>"`): value-structured ink, paper, mid, accent, accent2, tint, muted and surface.
`grid-choreography.{jpg,mp4}`: layers tweening between grid fields (`"to": {"cell": …}`) over `show: "under"` guides, at 257 fps.
`editorial-charts.jpg`: `line-chart` and `bar-chart` with `"look": "editorial"`: a highlight series against grey context, direct labels, the unit on the top gridline, a leader-line note, and before/after ticks on ranked bars.
`print-presets.jpg`: the five `print` presets on one photo, with the original.
`cmyk.jpg`: `"halftone": {"ink": "cmyk"}` on a photo: four screens at the press angles, skeleton black, rosettes.
