# Motion graphics: repeaters, trails, motion paths, mattes, shape modifiers, text animators

```json
{ "scenes": [ { "duration": 4, "layers": [
  { "shape": "rect", "x": 960, "y": 420, "width": 10, "height": 110, "radius": 5, "fill": "$accent", "anchor": [0.5, 2.2], "enter": "popIn",
    "repeat": { "count": 24, "offset": { "rotate": 15 }, "stagger": 0.02 } },
  { "shape": "circle", "width": 36, "height": 36, "fill": "$primary", "path": "M200 900 C600 500 1300 1100 1720 700", "orient": true,
    "echo": { "count": 8, "delay": 0.03, "decay": 0.7, "blend": "add" } },
  { "shape": "star", "x": 360, "y": 300, "width": 200, "height": 200, "fill": "$secondary", "roundCorners": 14, "twist": 25 },
  { "shape": "circle", "x": 1560, "y": 300, "width": 200, "height": 200, "fill": "$primary", "zigzag": { "size": 8, "ridges": 16, "smooth": true } },
  { "shape": "circle", "x": 1560, "y": 820, "width": 180, "height": 180, "fill": "$accent", "wigglePath": { "amount": 12, "detail": 16 } },
  { "shape": "circle", "x": 360, "y": 820, "width": 160, "height": 160, "fill": "none", "stroke": { "color": "$text", "dash": [12, 10] }, "strokeWidth": 3, "dashOffset": "=t*-60" },
  { "text": "TEXT ON A PATH", "size": 44, "x": 960, "y": 760, "width": 900, "height": 300,
    "textPath": "M100 250 Q450 0 800 250", "pathOffset": "=t*60" } ] } ] }
```

- **Repeater** `"repeat": {"count", "offset": {x, y, rotate, scale, opacity}, "stagger", "center": [x, y]}` on any layer or group:
  copy *i* is moved by i×(x, y), then rotated i×rotate and scaled scale^i about `center` (default: the layer's position);
  opacity changes by `opacity` per copy (−0.1 fades), and copy *i* plays i×stagger s later (`"order": "center|reverse|random"`, `seed`). Radial burst: `anchor` off-centre +
  `rotate: 360/count`. Grid: repeat a repeated group. Tunnel: `scale: 0.85` + `opacity: -0.08`. Spiral: `x` + `rotate`.
  (`"repeat": 3`, a plain number, is still the spec-level copy with `$i`.)
- **Echo / trails** `"echo": {"count": 5, "delay": 0.05, "decay": 0.6, "blend": "add"}`: earlier-time copies under the layer.
- **Motion path** `"path": "<SVG d in parent px>"`: x/y follow the path at constant speed; `progress` 0..1 (animatable,
  `"=expr"`, keyframes; default linear over the layer's life); `"orient": true` (or extra degrees) turns along the tangent.
- **Text on a path** `"textPath": "<d in the text box's px>"`, slide it with `pathOffset` (px, animatable).
- **Shape modifiers** (any shape; applied offset → round → pucker → zigzag → twist): `roundCorners` px, `offsetPath` px
  (±), `puckerBloat` −1..1, `zigzag {size, ridges, smooth}`, `twist` deg, `wigglePath {amount, detail, speed, seed}` (animated, deterministic).
- **Track matte** `"matte": "logo"` or `{"layer": "logo", "mode": "alpha|alphaInverted|luma|lumaInverted"}`: the layer shows only through
  the sibling with that `id` (which is then not drawn itself). Gradient inside text: a gradient rect with `"matte": "word"`.
- **Follow-through** `"inertia": true | "soft" | "bouncy" | "tight" | "heavy"` or `{"stiffness", "damping", "mass", "props": ["x", "y", "rotation", "scale"]}`:
  a spring chases the layer's own motion (tweens, presets, paths, expressions), so moves overshoot and settle. Stagger it across children for overlapping action.
- **Drag** `"drag": 0.04` on a moving group: child i follows the group's motion i × 0.04 s later (overlapping action; add `inertia` for settle).
- **Smear** `"smear": true` (or a strength): fast moves stretch along their travel, volume kept; still layers are untouched.
- **Idea transitions** (on the incoming scene): `{"type": "zoomThrough", "layer": "logo"}` flies into a layer of the outgoing
  scene and the new scene shows through its shape (a letter, a window, a logo); `{"type": "reveal", "shape": "circle|diamond|square|star|blob",
  "amount": 90 (spin°), "origin": [x, y]}`; `"carry": ["logo"]` on any transition moves layers with the same id from the old layout to the new one (a match move); `{"type": "blinds", "count": 8, "direction"}`; `{"type": "split", "direction": "up"}`; `"whip"` (speed-blurred pan).
- **Posterize time** `"posterizeTime": 12` on a layer or group: its motion steps at 12 poses/s (on twos; with `wigglePath` → boiling, handmade lines).
- **Named curves**: `glide` (expo out 0.16,1,0.3,1), `snap` (0.87,0,0.13,1), `land` (long settle), `launch` (expo in, exits), `material`,
  `emphasizedDecelerate`/`emphasizedAccelerate`, `productive`, `expressive`, `expressiveEntrance`/`expressiveExit`. Why and when: docs/video/craft/motion-design.md.
- **Gradient along a stroke** `"stroke": {"width": 12, "along": ["#f36", "#fb0", "#1cb"]}`: colours flow start → end along the path (write-ons included).
- **Dashes**: `stroke.dash` + `dashOffset` (animatable: `"=t*-60"` marches) — combine with `trimEnd` for write-ons.
- **Easing**: any name (`outExpo`, `inOutBack`, `elastic.out(1, 0.3)`, `steps(4)`), `"ease": [x1, y1, x2, y2]` (cubic-bezier),
  `"spring": {"stiffness": 300, "damping": 18}`; per keyframe: `"keyframes": {"x": [[0, 200], [0.5, 960, "outBack"], [1, 1700, [0.9, 0, 0.1, 1]]]}` (the ease *into* that key).
- **Text animator** (AE range selector) on plain text: `"animator": {"by": "char|word|line", "y": 60, "opacity": 0, "rotation": 20, "scale": 0.6,
  "blur": 6, "color": "#f36", "smooth": 0.3, "order": "forward|reverse|center|random", "seed": 7, "duration": 1.2, "delay": 0.2, "ease": "outExpo"}`.
  Units in the range take the props; with no `offset` the range sweeps away (a reveal). Drive it yourself with `start`/`end`/`offset`
  (0..1, or animate `rangeStart`/`rangeEnd`/`rangeOffset`): `"offset": "=sin(t*2)"` makes a travelling wave.

Full pieces: `examples/video/mograph/` (pulse-ident, kinetic-poster, night-line).
