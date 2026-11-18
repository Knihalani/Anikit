# 3 · One world: a sunlit counter at the roastery, 9 a.m.

- **Material:** one counter top, seen from directly above (the Instagram flat-lay is native to the platform and to coffee), with one tumbler of the brewed Guji on it. The counter is the hero surface: procedurally built at full resolution with texture locked to the world, never to the screen (see style frames for which stone/wood). Glass and liquid: a heavy-based tumbler, the brew a clear red-amber (a washed Ethiopian brews light). At the end, the roaster's bag lies on the same counter.
- **Key light:** low morning sun through the shop window, top of frame, slightly right (light travels down-left, ≈ 20° off vertical), warm ≈ 3500 K, hard. Every object throws a long shadow down the tall 9:16 frame; shadows are filled by cool sky (blue-slate), never grey or black. A window mullion throws one soft bar across some shots. Arc: the sun goes behind a cloud for the hard stop (cool, flat, silent) and comes back for the payoff.
- **Palette family:** counter whites/greys, cool slate shadow (#55657A family), sun warm white; **one accent reserved for the device: the caustic amber (#FFB347 → #E8742A)**, which only ever appears as light that has passed through the coffee, plus the vermilion stamp at the end in the same red-amber.
- **Type voice:** Bricolage Grotesque — ink-trapped, punchy at display opsz and calm at small sizes, so one family does the bag's whole label (name, origin, notes, stamp) the way a small roaster's own label would be set. Type lives in the scene twice: the tasting notes are **written in the light** (the caustic, with its filaments), and the name is **printed on the bag** and lit by the caustic. Nothing is laid over the image.
- **Sound world:** a quiet room: the clunk of a glass set on stone, the pour, a spoon tap; nylon-string guitar and soft mallets (kalimba/vibes) at 96 bpm — morning, small shop, warm. One silence when the cloud passes.
- **Anikit means:** one GLSL world shader (`src/counter.glsl`) renders every shot (counter, glass, shadow, caustic, bag), so the material cannot drift; text is fed in through channels (light words as a caustic mask, label ink as albedo), so type is lit by the same sun.

## Changes shot to shot
subject (light alone → glass → peach → bergamot → tea leaf → bag) · scale (macro filaments at 8× → wide flat lay → close) · camera (rotation of the top-down frame, push, slow drift).

## Style frames (`frames/f0–f2.png`, side by side in `frames/compare.png`)
Same key moment (the light draws the peach), one decision varied: the counter material — f0 terrazzo, f1 ash wood, f2 honed blue-grey stone.
- f0 terrazzo: the chips fight the caustic; the light's filaments get lost among the flecks, and it reads "café décor".
- f1 ash wood: warm on warm — the amber light loses its contrast against honey wood, and the grain streaks read soft.
- **f2 honed stone — picked.** Cool stone in sun, blue-slate in shadow, and the only warm thing in the frame is light that went through the coffee: the device owns the accent colour, so the eye goes straight to it.

---

# World v2 (pass 2): a tea table in the roastery window, 9 a.m.

- **Material:** white linen (woven, slubbed, soft folds) under bone china — a cup and saucer with a blue floral transfer band and gold rim — and a silver teaspoon; the brew is red-amber like tea. At the end the roaster's matte black bag lies on the same cloth. One shader (`src/teatable.glsl`) renders every shot, top-down.
- **Key light:** low sun through the shop window (top of frame, slightly right; light travels down-left). It lands as one slanted patch with a mullion cross; everything outside it is cool shade. The window's lettering casts **shadow type** inside the patch. Arc: cloud (stop) → sun back for the payoff.
- **Palette:** linen white / cool slate shade / bone china with cobalt-blue transfer / tea red-amber / gold rim; the black bag is the only dark object and arrives last.
- **Type voice:** two voices on purpose. The window lettering is **Libre Caslon Display** — sign-painter Caslon, the voice of an old English tea room, which sells the misdirection. The bag is **Bricolage Grotesque** — the roaster's own modern label — which answers it.
- **Sound:** a teaspoon on china, a cup set on a saucer, room tone; nylon guitar + kalimba at 96 bpm; silence for the cloud.
- **Change per shot:** macro tea surface → wide table → rolled close on saucer → tea close → dim (cloud) → wide reveal with bag → bag close.

## Style frames v2 (`frames2/f0–f2.png`, `frames2/compare.png`): type treatment varied
- f0 Bricolage lowercase heavy: readable, but it is the roaster's modern voice too early — gives the twist away.
- **f1 Libre Caslon Display — picked:** reads as a tea room's window, so the set-up is convincing and the grotesk bag at the end lands as an answer.
- f2 tracked caps: the "tiny tracked caps" kit look the jury rejects.
