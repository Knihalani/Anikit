# Pacing: intent, beats, cuts

Declare what you mean, and the reviewer judges against it:
`"intent": { "pace": "calm|steady|fast|frantic", "genre": "launch|brand-film|explainer|music|social|data" }`

| pace | longest shot | median | holds |
|---|---|---|---|
| frantic | 1.2 s | 0.7 s | none |
| fast | 2.5 s | 1.5 s | cut or major change every 0.5–1.5 s, on the beat |
| steady | 6 s | 3.5 s | each hold has a change |
| calm | any | any | long holds are fine, lifeless ones are not (parallax, light, drift) |

- Cut on the music: scene `duration: "@bar2"`, `at: "@beat17"`; 120–140 BPM for fast work.
- Energy: whips, snap zooms, speed ramps (`timeScale` in physics, `inOutExpo` moves), impacts (`shake` + `sfx`), match cuts (one shape carried across a cut).
- Text: 1–4 words per shot; a sentence can run across cuts.
- Build an arc: start sparse, peak at about two-thirds, land on a held end frame.
