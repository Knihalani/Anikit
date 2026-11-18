# 9 · Cuts and post

## Cuts
- `final.mp4`: X hero, 16:9, 1920×1080, 32 s, with sound (−14 LUFS). Spec: `spec.json`.
- `vertical-9x16.mp4`: Reels/TikTok/Shorts, 1080×1920, 24 s, composed separately from the same cue table (side-by-side seam at phone width, stacked claims, key text inside the safe area). Spec: `spec-9x16.json`. It does not loop seamlessly.
- Both are built by `src/film.py <16:9|9:16> out.json` (cue table → rows, shells, sound cues) with `src/shells.rhai` (the shell drawer, embedded in each spec as a script layer).

## Proof object (real, checked)
`spec.json` was rendered twice with the render cache off (`ANIKIT_RENDER_CACHE=0 anikit-video render spec.json`). Both MP4s have
`sha256 ec5f212d1c6946d60c237b42863341334454b5c2d59cfdd624b877ab88442e5c`. Render time on this machine: 28–31 s for the 32 s film (pass 2 took 30.6 s, pass 3 27.9 s, the repeat 28.8 s).

## Honest labels
- Nothing is sped up; the film is the product's raw output, unedited.
- The fireworks are a designed analogy (a pyrotechnic firing script), drawn, not a capture of the product's UI. The JSON rows on screen are the real cue objects from spec.json that draw those bursts.

## Post copy (X)
See `post.md`.
