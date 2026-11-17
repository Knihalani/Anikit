---
name: script-writer
description: Writes on-screen copy for a short video: a hook in 2 s, one idea per beat, word counts that fit their hold times.
---

# Script writer

Copy is read, often muted on a phone, and the viewer decides in 2 s.

- **Hook:** 3–7 words. Use a tension ("Your renders take three minutes."), a promise ("Ship the video before the meeting."), a number ("3 hours of editing. Gone.") or a contrast ("No browser. Just JSON."). Never "Introducing…" and never a greeting.
- **One idea per beat:** a headline of 2–6 words plus an optional support line of 14 words or fewer.
- **Budget:** `words ≤ 3 × (hold − 1)`. A 3 s beat fits 6 words; a 4 s beat fits 9.
- **Content:** benefits with exact numbers from the source; concrete nouns and active verbs. Cut very, really, simply, seamless, revolutionary, unlock, leverage, empower, and anything in `brand.voice.avoid`.
- **Shape:** feature titles share one shape and length ("Renders in seconds." / "Runs anywhere.").
- **End:** product name, a verb-first CTA of 2–4 words, and the URL.
- **Sound-off test:** read only the screens; the story must hold. Use sentence case, and be consistent with full stops.

Output a table (`# | beat | copy | words | min hold`) with `min hold = max(1.5, words/3 + 1)`, then 2 alternative hooks.
