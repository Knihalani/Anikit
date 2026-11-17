# Kestrel — 6 s logo ident
1. Concept: the kestrel is the bird that hovers dead still while its wings beat; cinema began with Muybridge's motion plates. The ident is a chronophotograph of a hovering kestrel that comes alive and becomes the mark.
2. 0–2.1 s: a backlit film strip of numbered 4:3 plates (measuring grid, plate numbers, edge code) slides through the gate, each plate a different wing phase.
3. ~1.8 s: the centre plate (PL. 07) starts to flap, one phase per frame; 180° motion blur smears the wings while the body stays locked — the hover.
4. 2.15–3.6 s: the camera pushes into that plate until the bird fills the frame; the wings lock open on the glide pose.
5. 3.6 s (beat 7 at 100 bpm): hard cut to the negative — cream bird on black — which settles into the mark inside its plate frame, then KESTREL (Instrument Serif, wide tracking) and a rust rule; fade to black at 6 s.
6. The mark: an original frontal kestrel silhouette (pointed wings, fanned tail with a cut-out subterminal band), generated as SVG paths by bird.py (8 wingbeat phases + glide).
7. Sound: projector clatter (shaker/rim) under a low string + cello drone, a harp flutter as the bird comes alive, timpani roll into the cut; on the cut the clatter stops and a D-add9 string chord, low piano, reverse cymbal and boom ring out, a glockenspiel tick on the lockup. −15.5 LUFS.
8. Look: warm tungsten palette (#0A0908 / #EEE4CF / rust #B4532A), grain, halation, vignette, projector flicker and gate weave; no 3D — a 2D graphic idea pushed to one material, the film plate.
9. Source: gen.py writes spec.json (python3 gen.py), bird.py draws the bird; render with `anikit-video render spec.json -o final.mp4` (≈20 s).
10. Weakest moment: the first ~0.4 s is a fade-up on a moving strip; acceptable for an ident that must sit before any film.
