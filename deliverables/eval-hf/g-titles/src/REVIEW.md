# Offset 2027 — opening titles (15 s, 1920×1080, 30 fps)

**Idea.** A design system is one component used with rules. Lisbon's azulejos are exactly that: one tile, rotated, becomes a wall.
The film shows one tile being specced (dimension lines, "tile / 01"), then repeated into a cobalt-on-cream azulejo field.
After that the field is literally *offset*: rows slide half a tile, columns drop, every tile turns. The pattern opens from the centre,
and the wordmark arrives as two misregistered print plates (offset printing). They land on the downbeat at a deliberate
14 px offset, which is the brand's token, labelled as one. Then the spec row: what / where / when.

**Beats (120 BPM, every change on a beat):** 0–1.2 the tile is drawn · 1.25 it shrinks into the grid · 1.75–3.3 the wall builds outward ·
3.5 rows offset · 4.5 columns offset · 5.5 rotation wave · 6.25 back into register · 6.85 the pattern opens · 8.0 the mark lands (impact) ·
8.4–9.6 spec · 9.9 tile band · 11.9 / 12.9 band turns · 13.85–14.9 fade to black for the keynote.

**Sound.** The score is synthesised in `score.py` (numpy/scipy, deterministic): an Am9–F–C pad, Karplus-Strong plucks (a nod to the guitarra portuguesa) that land on the
tile's three shapes, a groove that starts on the first offset, ceramic ticks that follow each tile wave, a riser into the mark, and a sub drop.
Bundled HyperFrames SFX (Pixabay licence, see `assets/sfx/CREDITS.md`) add the swap click, the row whoosh and the impact.
I measured the sound but could not listen to it: −14.4 LUFS integrated, −1.1 dBTP, no clipping, and the loudest moment is on the mark.

**Checks.** `hyperframes check` passes. I kept one warning on purpose: `dead_stretch 2.0–6.75 s` is a false positive, because the 209 tiles inside
the `#tiles` clip move the whole time (the snapshots confirm this). I reviewed the beat, seam and phone sheets.

**Weakest moment / with more time:** while the plates are misregistered (7.2–7.9 s), the overprint reads near-black before it resolves to cobalt.
That is intended as the overprint, but a softer multiply colour would read better. A bespoke wordmark face instead of Inter Black would also help.

Rebuild: `node gen.mjs && python3 score.py && npx hyperframes render -o final.mp4`
