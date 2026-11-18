# Round r1: frame-defect scan (`scripts/defects.py`)

Checks: **flat** frames (luma std < 3 at 160 px, not counting the first or last 0.3 s); **frozen** stretches (≥ 1 s of pixel-identical frames, so nothing moves at all); **dup bursts** (≥ 3 identical frames in otherwise moving footage, i.e. stutter or a stale cached frame).

**No cache-bug garbage (blank, black or corrupt frames) in any Anikit r1 output.** Every flag was checked by eye (`defects.jpg` contact rows):

| Output | Flag | Verdict |
|---|---|---|
| h-ident | flat frame at 3.5 s | deliberate hard cut to black before the K reveal (the jury also calls this a "cut to black and fade-up") |
| k-sports | flat frame at 0.77 s | false positive: a dark floodlight shot with low contrast at 160 px |
| l-generative | flat 0.3–0.4 s | deliberate near-black opening, a single point that becomes a line |
| j-music | frozen 1.07–1.23 s at 5.0, 13.8 and 18.8 s | **real dead holds**: the harbour plate is pixel-identical, so not even the water or lights move. Either an unanimated plate or cached layers that should animate; worth checking against the spec |
| d-illustration | frozen 1.23 s at 18.8 s (end card) | **dead end hold**: the Burrow end card is pixel-identical for its last 1.2 s |
| a, b, c, e, f, g, i | none | clean |
| hf-b-launch | frozen 1.1–2.0 s ×3; dup bursts of 7, 7 and 19 frames | HyperFrames output stutters (repeated frames mid-motion) |
| hf-e-data | frozen 1.3–3.1 s ×4; dup burst of 4 | HyperFrames: long frozen stretches |
| hf-g-titles | frozen 1.2 s; dup bursts of 19 and 9 | HyperFrames stutter |
