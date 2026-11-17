# Shader layers

Write a fragment shader the way you would on Shadertoy, and Anikit Video runs it on the CPU at full 1080p, compiled to native SIMD code. Use one as a layer, as a filter over everything below it, or as a scene transition.

```json
{ "shader": "aurora", "uniforms": { "intensity": 1.2 } }
```

```json
{ "shader": "void mainImage(out vec4 fragColor, in vec2 fragCoord) {\n  vec2 uv = fragCoord / iResolution.xy;\n  fragColor = vec4(0.5 + 0.5 * cos(iTime + uv.xyx + vec3(0, 2, 4)), 1.0);\n}" }
```

`anikit-video info --shaders` lists the built-ins. Showcase spec: [`examples/shaders.json`](examples/shaders.json).

## How it works

1. **Front end.** The source is wrapped in a small prelude (the Shadertoy inputs, four channel textures), parsed and validated by [naga](https://github.com/gfx-rs/wgpu/tree/trunk/naga) (GLSL 450 or WGSL). Errors point at your line and column, with a "did you mean" for misspelt builtins (`smoothStep` → `smoothstep`, `lerp` → `mix`, `iGlobalTime` → `iTime`).
2. **JIT.** naga IR is compiled with [Cranelift](https://cranelift.dev) into one native kernel per shader, once per spec (5–35 ms). Each kernel call shades a row of **2×2 pixel quads**. Every shader value is a 4-lane SIMD vector, with one lane per pixel of the quad, exactly like a GPU quad. That is why `dFdx`, `dFdy` and `fwidth` work: they are lane shuffles.
3. **Control flow** runs SPMD-style under an execution mask. An `if` runs a branch only when some lane takes it, and loops run while any lane is active. `break`, `continue`, `return` and `discard` switch lanes off. Uniform control flow therefore costs only a mask test. Calls are inlined, and maths that depends only on uniforms is hoisted out of the pixel loop.
4. **Threads.** Rows of quads run on rayon across all cores. Output is premultiplied RGBA8 straight into the display list as an `Op::Image`. Pixels are made only when the renderer asks for them, so occluded or duplicate frames cost nothing. A shader that never reads `iTime` renders once.

The JIT was built directly; there is no interpreter step. The masking optimisation is worth noting: when a store's lanes equal the lanes its function was entered with, it skips the mask blend. That took the SDF scene from 1190 to 358 ms per frame, because it lets Cranelift hoist uniform maths.

**Accuracy and determinism.** `sin cos tan exp exp2 log log2 asin acos atan sinh cosh tanh` use Cephes-style range reduction and minimax polynomials. The tests check them against `f64` at under 4 ulp over the ranges shaders use (sin and cos to ±100, exp to ±80, log over 10⁻³⁰…10³⁰). `sqrt`, `floor`, division and the like are IEEE-exact. There are no fused multiply-adds, so x86-64 and aarch64 agree, and frames are bit-identical between runs and thread counts.

## Authoring

### Inputs

| Name | Type | Value |
|---|---|---|
| `fragCoord` | `vec2` | pixel centre, **y up** (0.5 … height − 0.5), as on Shadertoy |
| `iResolution` | `vec3` | layer size in device pixels, `z = 1` |
| `iTime` | `float` | seconds since the **layer** started |
| `iFrame`, `iTimeDelta`, `iFrameRate` | | from the video's fps |
| `iChannel0..3` | `sampler2D` | see Channels |
| `iChannelResolution[4]` | `vec3[4]` | channel sizes |
| `iMouse`, `iDate` | `vec4` | zero / fixed (deterministic) |
| `progress`, `ratio` | `float` | transitions: 0 → 1, and width / height |

### Uniforms

Declare a uniform and set it from the spec. A `// = value` default comment (the gl-transitions convention) supplies the value when the spec doesn't give one:

```glsl
uniform float speed;  // = 1.0
uniform vec3  tint;   // = vec3(1.0, 0.5, 0.2)
```

```json
{ "shader": "…", "uniforms": { "speed": 2, "tint": "#ff8800" },
  "animate": [ { "to": { "speed": 0.5 }, "at": 1, "duration": 2 } ] }
```

- **Values:** numbers, `[x, y(, z, w)]`, colours (`vec3`, or `vec4` when the alpha is below 1), or `"=expr"` expressions (`"=sin(t)"`).
- **Undeclared uniforms:** a uniform the source doesn't declare is declared for it, as `float` or `vecN`.
- **Animation:** uniforms animate by name in `animate` (`to`, `from`, `keyframes`) like any property. A vector animates per component: `{"to": {"tint": [1, 0, 0]}}`. Each shader layer has 12 animatable floats; any uniforms past that are fixed and produce a warning.

### Channels

```json
"channels": [ "below", "noise", "photo.jpg", { "layers": [ { "text": "LOGO", "size": 300 } ], "wrap": "repeat", "mipmaps": true } ]
```

| Channel | Texture |
|---|---|
| `"below"` | everything drawn before this layer, composited (the default `iChannel0` of a filter) |
| `"noise"` | 256×256 RGBA white noise, repeat wrap, mipmapped (like Shadertoy's) |
| `"clip.mp4"` / `"photo.png"` | the image or the current video frame, stretched over uv 0..1 |
| `{ "layers": [...] }` | a 2D sub-composition (any layers, animated) rendered at the shader's size |

Options per channel:

- `wrap`: `clamp` (default), `repeat` or `mirror`.
- `filter`: `linear` (default) or `nearest`.
- `mipmaps: true` makes `textureLod` and minified `texture` work.

`texture`, `textureLod`, `textureGrad`, `texelFetch` and `textureSize` are supported, with uv `(0, 0)` at the bottom-left as in GL. You can also declare `uniform sampler2D name;`: it takes the next free channel.

### Modes

| Mode | Spec | Notes |
|---|---|---|
| layer | `{ "shader": … }` | full frame by default; `x y width height` place it; `opacity`, `blend` and masks work as for images |
| filter | `{ "shader": …, "filter": true }` | `iChannel0` = the frame below; the output replaces the whole frame |
| transition | `"transition": { "type": "shader", "shader": "vec4 transition(vec2 uv) { … }" }` or `"transition": "burn"` | [gl-transitions](https://gl-transitions.com) API: `getFromColor(uv)`, `getToColor(uv)`, `progress`, `ratio`; uniforms via `"uniforms"` |

More keys:

- `"supersample": true` renders 2×2 sub-samples per pixel (4× cost). Use it for thin bright detail; most shaders are smooth enough without it, and `fwidth`-based edges are already anti-aliased.
- `"alpha": true` uses `fragColor.a` as coverage. By default the output is opaque, as on Shadertoy.
- `src: "fx.glsl"` (or `.wgsl`) loads the code from a file.

### WGSL

Write `fn mainImage(fragCoord: vec2f) -> vec4f` (or `fn transition(uv: vec2f) -> vec4f`). The same inputs are private globals. Channels are `iChannel0..3` (`texture_2d<f32>`) with samplers `iSampler0..3`.

## Built-ins

| Name | Use | What it is |
|---|---|---|
| `sdf-scene` | layer | raymarched still life: smooth-blended forms, soft shadows, AO, fog. Uniforms: `speed`, `tint` |
| `liquid-chrome` | layer | chrome metaballs reflecting a procedural studio. `speed`, `warmth` |
| `aurora` | layer | aurora curtains over stars, mirrored in a lake. `speed`, `intensity` |
| `caustics` | layer | pool-floor caustics; ray density from the refraction map's derivatives. `speed`, `depth`, `tint` |
| `flow-ink` | layer | two inks marbled by a curl-noise flow field. `speed`, `ink`, `ink2`, `paper` |
| `turing` | layer | reaction–diffusion spots and labyrinths. `scale`, `balance` (−1 spots … 0 maze … 1 holes), `speed`, `colorA`, `colorB` |
| `film-burn` | filter | the frame below scorches from hot spots. Animate `amount` 0 → 1; `seed` |
| `glitch` | filter | codec-style block slides, row smears, RGB split, scanlines in bursts. `amount`, `rate` |
| `burn` | transition | the old scene burns away along a noisy ember edge. `glow`, `color` |
| `ripple` | transition | a ring wave carries the new scene out from the centre. `amplitude`, `waves` |
| `pixelate` | transition | pixel blocks grow, cross-fade, resolve. `blocks` |
| `swirl` | transition | a vortex twists and unwinds. `turns`, `radius` |
| `ink-bleed` | transition | the new scene soaks in like ink. `softness`, `edge` |
| `warp-wipe` | transition | a soft angled wipe that smears pixels. `direction` (degrees), `smear` |
| `luma-melt` | transition | the old scene melts away darkest-first. `softness`, `invert` |
| `zoom-blur` | transition | a punch-in zoom blur with chromatic fringes. `strength` |

The sources live in `crates/anikit-shader/library/*.glsl`. Copy one as a starting point: they are short and commented.

## Performance

Milliseconds per frame at **1920×1080**, on **4 cores** (Xeon @ 2.1 GHz, AVX2), release build. Compile time is the one-off parse + validate + JIT. Filters and transitions are timed with full-HD channel textures already bound.
Generated by `cargo run --release -p anikit-shader --example library_bench`:

| shader | use | compile ms | ms/frame | ms/frame 2×2 SS |
|---|---|---|---|---|
| `sdf-scene` | layer | 21 | 347 | 1441 |
| `liquid-chrome` | layer | 19 | 416 | 1935 |
| `aurora` | layer | 20 | 931 | 3849 |
| `caustics` | layer | 12 | 62 | 198 |
| `flow-ink` | layer | 12 | 507 | 1871 |
| `turing` | layer | 4 | 166 | 576 |
| `film-burn` | filter | 7 | 104 | 354 |
| `glitch` | filter | 3 | 49 | 212 |
| `burn` | transition | 5 | 77 | 331 |
| `ripple` | transition | 4 | 61 | 200 |
| `pixelate` | transition | 2 | 34 | 108 |
| `swirl` | transition | 2 | 59 | 192 |
| `ink-bleed` | transition | 3 | 86 | 294 |
| `warp-wipe` | transition | 2 | 42 | 125 |
| `luma-melt` | transition | 1 | 31 | 134 |
| `zoom-blur` | transition | 3 | 497 | 1867 |

End to end through the video pipeline (evaluate, render, composite text, H.264 encode): the 18 s [`shaders.json`](examples/shaders.json) showcase, with every built-in and seven shader transitions, is 537 frames in 215 s (400 ms per frame). `{"shader": "sdf-scene"}` alone is 90 frames in 32.2 s.

What costs time is pixels × work per pixel:

- **Raymarch steps and noise octaves** dominate. `aurora` and `flow-ink` evaluate around 100 noise lookups per pixel. Halving octaves or steps halves the time.
- **Texture reads** go through a sampler call: about 120 ns of one core per read per 4-pixel quad. `zoom-blur` reads 32 per pixel, which is why it costs 0.5 s per frame.
- **Maths on uniforms** (`sin(iTime)` and rotation matrices from time) is free: it is hoisted. Divergent branches cost both sides only in quads that actually diverge.
- **Supersampling** costs about 4×. A layer smaller than the frame costs proportionally less.
- **Static shaders** (no `iTime`) render once per distinct uniform set.

## Limits

- One pass per shader: no Shadertoy Buffer A–D feedback, keyboard or audio channels, or cube maps. Chain effects by stacking filters instead.
- `sampler2D` can't be passed to user functions. Sample `iChannelN` directly (inside helpers, refer to `iChannelN` itself).
- `double` is computed as `float`. These are unsupported: `textureOffset`, `textureGather`, shadow samplers, `bitfieldExtract`/`Insert`/`Reverse`, `findLSB`/`MSB`, and the pack/unpack built-ins (`bitCount` works).
- `pow(x, y)` with `x < 0` is exact for integral `y` (as GPU compilers effectively do) and NaN otherwise. NaN or out-of-range colours are written as 0 / clamped.
- A loop runs at most 2²⁰ iterations per quad, so a runaway `while (true)` ends instead of hanging the render.
- A filter always covers the full frame; its `x`, `y`, `width` and `height` are ignored.
