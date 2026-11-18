# Camera: moves, whips, shake, depth, motion blur

```json
{ "motionBlur": true, "scenes": [ { "duration": 2.4,
  "camera": { "aperture": 10, "animate": [
    { "from": { "zoom": 1.25, "x": -600 }, "to": { "zoom": 1, "x": 0 }, "duration": 0.35, "ease": "outExpo" },
    { "to": { "focus": 2 }, "at": 1.2, "duration": 0.6, "ease": "inOutSine" },
    { "to": { "x": 1400 }, "at": 2.2, "duration": 0.2, "ease": "inExpo" } ],
    "shake": { "amount": 10, "at": 0.35, "duration": 0.3 } },
  "layers": [
    { "shape": "rect", "width": "120%", "height": "120%", "fill": "linear-gradient(90deg, #111, #333)", "depth": 3 },
    { "text": "Near", "size": "18vh", "color": "#fff", "x": "30%" },
    { "text": "Far", "size": "10vh", "color": "#aaa", "x": "70%", "depth": 2 } ] } ] }
```

- Camera keys: `zoom x y rotation focus aperture dolly`, animated like layers; `shake { amount, at, duration }`.
- **Whip:** move `x` across a frame in 0.15–0.25 s with `inExpo`/`outExpo`, cut at its fastest point; `motionBlur: true` (or `{ "shutter": 270 }`) smears it for real.
- **Snap zoom:** `zoom` 1.3 → 1 in 0.2 s `outExpo` on a beat. **Speed ramp:** one long move with `inOutExpo`.
- **Depth:** `depth` on layers (+ farther) gives parallax when the camera pans; `aperture` + animated `focus` = rack focus.
- **Dolly (2.5D push):** `"dolly": "=t*0.3"` moves the camera *through* the depth layers: near ones grow faster than far ones and fly past the lens (unlike `zoom`, which scales everything alike). Layer depth 0 sits 1 unit away.
- 3D cameras (dolly, crane, orbit, focal length, f-stop) live in `scene3d` (`docs 3d`).
