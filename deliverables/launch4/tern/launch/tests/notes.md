# Tests: notes (`tests.json`, `sheet.png`, frames at 3.9 / 7.9 / 9 / 12 s)

1. **2D form + Reenie Beanie ballpoint + loop circle (frame 1)** — *the one that made me stop.* It reads instantly as a real carbonless report; the scrawl is believable because it's slightly uneven and blue on pink. The circle drawn on with `trimEnd` and a spiral overshoot looks like a hand, not a vector ellipse. Fix: Reenie's word space is narrow ("thetop"): use double spaces.
2. **Same in Caveat at 2.6× macro (frame 2)** — legible but **looks stock**: Caveat is the default "handwritten font" of every explainer. Dropped.
3. **Ray-traced 3D plane with the form as a live composition texture (frame 3)** — new to me. My plane orientation was wrong (it rendered edge-on); even fixed, a CG sheet in a macro lens risks the "simulated object" look, and at ~1 s a frame it would eat the budget. **Dropped: looks artificial and costs most.** Limit learned: composition textures work, but plane axes need care.
4. **Type as image: giant printed "NG" with a brush strike (frame 4)** — the scale contrast is strong and it suits a hard-stop beat. But `inkTexture` 0.2 on 70vh type turns into camouflage blotches (**artificial**). Use ≤ 0.05 at that size, or none. The brush strike also reads as a paint stroke, not a ballpoint: use a plain round-cap ballpoint line.
5. **Light:** the multiply radial "desk light" works as falloff, but at full strength it muddies the pink into mauve-brown. Keep it, but go no darker than about 80% on the far side.

**Conclusion:** the world is the 2D document. The light is a single lamp falloff, the hand is Reenie Beanie in Bic blue, and the gesture is the loop. Depth comes from stacked sheets (white/yellow/pink offset like the real 3-part set), not from 3D.
