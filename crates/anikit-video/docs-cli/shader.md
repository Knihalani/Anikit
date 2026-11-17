# Shaders: your own GLSL as a layer, filter or transition

```json
{ "scenes": [ { "duration": 3, "layers": [
  { "shader": "void mainImage(out vec4 o, in vec2 f) {\n  vec2 uv = (f - 0.5*iResolution.xy) / iResolution.y;\n  float r = length(uv), a = atan(uv.y, uv.x);\n  float rings = sin(18.0*r - iTime*6.0 + 3.0*sin(a*5.0 + iTime));\n  vec3 c = mix(vec3(0.05,0.02,0.1), vec3(1.0,0.45,0.1), smoothstep(0.6,1.0,rings)) * exp(-1.5*r);\n  o = vec4(c, 1.0);\n}",
    "uniforms": { "speed": 1 } },
  { "text": "RESONANCE", "size": "12vh", "weight": 900, "color": "#fff", "letterSpacing": "0.2em" } ] } ] }
```

- Shadertoy-style `mainImage(out vec4, in vec2)`; inputs `iTime iResolution iFrame iChannel0..3`, plus your `uniform float x; // = 1.0` (animatable by name in `animate`).
- `src: "fx.glsl"` loads a file; `"filter": true` processes everything below (`iChannel0`); transitions: `"transition": { "type": "shader", "shader": "vec4 transition(vec2 uv) {…}" }` (gl-transitions API).
- Channels: `"below"`, `"noise"`, an image/video, `"#composition"`, or `{ "layers": [...] }`.
- JIT-compiled to SIMD on the CPU at full resolution: raymarching, volumetrics and 100-octave noise run every frame (cost: `still` prints ms/frame; keep ≤ 200 ms).
- Built-in library: `anikit-video recipes` lists every shader with its uniforms; full guide: docs/video/shader.md.
