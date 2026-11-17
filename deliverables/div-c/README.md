# Substation — line-up reveal (15 s, 1080×1920)

- `build.py` generates `substation.json` (the Anikit Video spec); `fonts/` holds Big Shoulders Bold (OFL).
- `substation.mp4`: the final video. `contact-sheet.jpg`: one frame per second (at 0.5 s, 1.5 s, … 14.5 s).

Rebuild:

    python3 build.py
    anikit-video render substation.json --from 0 --to 7.5 -o a.mp4 --no-audio
    anikit-video render substation.json --from 7.5 --to 15 -o b.mp4 --no-audio
    anikit-video music substation.json -o score.wav
    # concat a.mp4 + b.mp4 (ffmpeg concat demuxer, -c:v copy) and mux score.wav as AAC

The video is rendered in two halves because a full-range `render` of this spec panics in
the render-time estimate (`render/mod.rs:678`, slice index out of range) in anikit-video 0.1.0.
Each half renders cleanly, and the score is generated once for the whole 15 s, so the audio has no seam.
