# Animation: keyframes and expressions on any property

```json
{ "text": "SLAM", "size": "30vh", "weight": 900, "color": "#fff",
  "animate": [
    { "from": { "scale": 3, "opacity": 0, "blur": 20 }, "duration": 0.25, "ease": "outExpo" },
    { "keyframes": { "rotation": [0, -4, 3, 0] }, "at": 0.25, "duration": 0.3 },
    { "to": { "x": "+400", "opacity": 0 }, "at": "@beat9", "duration": 0.2, "ease": "inExpo" } ] }
```

- `from` holds until `at`; `to` animates from the current value; `keyframes` for multi-step; relative values `"+60" "-20" "*1.5"`.
- `at`: seconds from the layer start, `"+0.2"` after the previous tween, `"@marker"`, `"@beat17"`, `"@bar5+0.25"`.
- Easings: `outExpo outCubic inOutQuart inOutExpo outBack`, `cubicBezier(…)`, `steps(4)`, springs `snappy bouncy gentle wobbly stiff`.
- **Expressions** anywhere a number goes: `"x": "=960 + 300*sin(t*2)"`, `"rotation": "=t*90"`. Vars `t T gt p i n w h fps frame`; fns `sin cos abs min max clamp mix smoothstep noise rand`.
- Animatable: `x y width height scale scaleX scaleY rotation skewX skewY opacity blur radius fill color stroke strokeWidth trimStart trimEnd reveal value wipe depth`; shader uniforms by name.
- Presets: `enter: "rise"|"pop"|"blur"|"wipe"|"type"|…`; kinetic text: `"mask by line up"`, `"pop by char from center"`, `"scramble"`, `"wave by word"`.
- Copies: `"repeat": 12` with `"$i"` and `"=$i*0.08"` (stagger), or `stagger` on `enter`.
