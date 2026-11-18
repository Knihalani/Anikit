# Technique tests

| test | capability | result |
|---|---|---|
| t1-clock (`t1.png`) | GLSL shader ground driven by an `hour` uniform + script-generated per-frame clock text (Bricolage, tnum) | **Stopped me.** The clock as a huge low-contrast set reads as a design object, not a caption. Mid-night colour went muddy grey (linear mix): the hour→light curve must stay ink until 05:00, then warm. Text isn't animatable through `update`, so the clock is a script that emits one text layer per frame — cheap in 2D. |
| t2-row (`t2.png`) | defs/`use` groups as real UI rows (Host Grotesk) | Clean and believable at 1×; reads as product, not template. Group needs `anchor: topLeft` + full-frame width/height to use local coordinates. The amber read bar must sit inside the card radius. |
| t3-storm (`t3.png`) | script-ish list of 18 rows + camera zoom-out (new to me: camera on a shader ground) | **Looked broken, not chaotic**: overlapping rows read as a rendering error. Lesson: the storm must be a *clean, ever-taller list* (rows at a fixed pitch) where the scale is the joke. Camera zoom < 1 exposes the shader's frame edge: keep zoom ≥ 1 or oversize the ground. |
| t4 (style frames, `frames/`) | brief + draft + Send, light ground | see `frames/` |

**Looked stock:** nothing yet; the risk is the dark inbox card = "tasteful-AI kit". The clock-as-set and the colour arc are what keep it from being that.
**Looked artificial / simulated:** none (no simulation in this film). Dropped: overlapping card pile.
**Engine limits learned:** text content isn't per-frame animatable; generate it. Group coordinates need topLeft anchoring.
