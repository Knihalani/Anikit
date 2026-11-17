# Kestrel ident: "THE GATE" (6 s, 16:9, 24 fps)
1. Idea (concept.md): a kestrel hovers with its head dead still, and a projector holds each frame still in the gate. So the bird lives on 35 mm film, and its stoop is the pull-down that brings in the title.
2. Process: proposition, 17 banned clichés, 25 facts, 31 one-liners (1–10 dropped), then a de Bono reversal ("the bird doesn't move, the world does"). That led to a treatment with 10 world rules.
3. Medium: hand-inked 2D on celluloid. The kestrel's poses are drawn procedurally in `kestrel.py` (6 wingbeats, 3 fold frames, the stoop mark) with ink boil and scratched-clear feather lines.
4. Material pushed: projected light. One lamp light field is multiplied over the whole strip (hot gate, chestnut spill), with halation, grain, flicker, gate weave, dust and a hair.
5. Arc: 0–2.8 s the film races and decelerates while the bird hovers; 2.8 s it registers and stops, silent, and the bird folds; 3.2–3.5 s pull-down/stoop; 3.5 s KESTREL slams into the gate.
6. The payoff holds 2.5 s. The button at 5.5 s is the changeover cue mark (4 frames) with a click, "kee-kee-kee" on flute and the next projector spinning up.
7. Sound is from the picture: one tick per perforation passing (30, timed from the scroll function) and 4 claw ticks in the pull-down. Wind and a cello drone stop dead at 2.8 s, then a whoosh and a low impact land on beat 8 with a D-major chord. −14.6 LUFS.
8. Re-watch details: the edge code reads KESTREL from frame 0, every frame keeps the bird's eye in the same spot, and the title frame is the stoop's next drawing.
9. Files: `gen.py` builds `spec.json` (`python3 gen.py && anikit-video render spec.json -o final.mp4`); style frames are `frame-hover.png`, `frame-stop.png` and `frame-title.png`.
10. Known trade-off: the reviewer flags the 18 px edge print for contrast and size. That is deliberate texture, legible only when the film stops.
