# Railyard launch film: notes
1. Concept: the payments API shown as a railway signal-box panel. The payment is a lit train carrying a "$12,400.00" describer; each state change is a signal that fires a webhook down a spur.
2. Medium: precise 2D graphic design (tile-seamed panel, rails, lamps, engraved plates, amber LED type). It's one continuous camera move through one world. No 3D or stock looks.
3. 0–1.5 s: a macro on Bank A. The LED types `POST /v1/transfers` and the amber route lights along the line ("one API call").
4. 1.5–4 s: the train departs. Signals go red→green, and a pulse runs up each spur to its plate: `transfer.created`, then `transfer.pending` (200).
5. 4–6 s: a tense push-in at the pending signal, then the snap. The train covers the whole run in 0.5 s and hits Bank B on the bar-4 downbeat (shake + impact). The route turns green: `transfer.settled`, "Settled instantly.", +$12,400.00.
6. 7.7–11.4 s: a speed-ramp pull-out reveals the whole yard: dozens of trains, with lamps and webhook pings timed to when each train passes. Headline plate: "A webhook at every state change."
7. 11.4–15 s: the camera pushes up to the panel's cream nameplate, which is the logo. Its mark lights (amber segment + green lamp), and the line "One API call moves money between bank accounts." types in.
8. Sound: a hand-written score at 120 bpm in D minor. A cello 8th-note ostinato carries the train rhythm, with marimba, strings, horns and timpani on the settle, then an orchestral kit. Relay ticks fall on each webhook, with a riser → impact at settlement and whooshes on the camera moves. −14.2 LUFS.
9. Source: `build.py` generates `spec.json` (462 layers; the yard is procedural with seed 7). Rendered with `anikit-video render spec.json -o final.mp4 --crf 18`.
10. Copy uses only the brief's claims (one API call, bank-to-bank, instant settlement, a webhook per state change). The amounts and account numbers are illustrative.
