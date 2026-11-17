# Shader layers

Write a fragment shader the way you would on Shadertoy, and Anikit Video runs it on the CPU at full 1080p, compiled to native SIMD code. Use one as a layer, as a filter over everything below it, or as a scene transition.

```json
{ "shader": "aurora", "uniforms": { "intensity": 1.2 } }
```

```json
{ "shader": "void mainImage(out vec4 fragColor, in vec2 fragCoord) {\n  vec2 uv = fragCoord / iResolution.xy;\n  fragColor = vec4(0.5 + 0.5 * cos(iTime + uv.xyx + vec3(0, 2, 4)), 1.0);\n}" }
```

`anikit-video info --shaders` lists the built-ins. Showcase spec: [`examples/shaders.json`](examples/shaders.json).

## Write your own

The built-ins are starting points. Anything a fragment shader can draw, you can write. Shadertoy code usually runs unchanged; see Limits for what doesn't.

**Loop:** keep the code in a file and watch it. Each save re-renders the still and reports the shader's cost; only the edited shader recompiles.

```json
{ "size": [1920, 1080], "scenes": [ { "duration": 8, "layers": [ { "src": "fx.glsl", "uniforms": { "hue": 0.6 } } ] } ] }
```

```sh
anikit-video still spec.json --at 2 --scale 0.5 --watch   # prints e.g. "shader scenes[0].layers[0] (fx.glsl): ≈ 85 ms/frame at full size"
```

**A fast raymarch skeleton.** Bound the scene, cap the steps, and stop rays early. These three habits keep a hero shot under 100 ms/frame at 1080p:

```glsl
uniform float hue;   // = 0.6 base hue 0..1

float map(vec3 p) {                         // the scene as a signed distance
    p.xz *= mat2(cos(iTime), -sin(iTime), sin(iTime), cos(iTime));
    vec3 q = abs(p) - 0.7;
    return length(max(q, 0.0)) + min(max(q.x, max(q.y, q.z)), 0.0) - 0.15;
}

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 uv = (2.0 * fragCoord - iResolution.xy) / iResolution.y;   // y up, −1..1 vertically
    vec3 ro = vec3(0.0, 0.4, 4.0), rd = normalize(vec3(uv, -2.2));
    vec3 col = vec3(0.04, 0.045, 0.06) * (1.2 - 0.4 * length(uv));  // backdrop
    // 1. Bound: rays that miss a sphere around the scene skip the march entirely.
    float b = dot(ro, rd), h = b * b - dot(ro, ro) + 1.6 * 1.6;
    if (h > 0.0) {
        float t = max(-b - sqrt(h), 0.0), tEnd = -b + sqrt(h);
        for (int i = 0; i < 96; i++) {                               // 2. cap the steps
            float d = map(ro + rd * t);
            if (d < 0.0005 * t) {
                vec3 p = ro + rd * t;
                vec2 e = vec2(0.001, 0.0);
                vec3 n = normalize(vec3(map(p + e.xyy), map(p + e.yxy), map(p + e.yyx)) - d);
                vec3 base = 0.5 + 0.5 * cos(6.2832 * (hue + vec3(0.0, 0.33, 0.67)));
                col = base * (0.15 + 0.85 * max(dot(n, normalize(vec3(-0.5, 0.8, 0.4))), 0.0))
                    + pow(1.0 - max(dot(n, -rd), 0.0), 4.0) * 0.6;   // rim
                break;
            }
            t += d;
            if (t > tEnd) break;                                     // 3. stop at the far side
        }
    }
    fragColor = vec4(pow(col, vec3(0.4545)), 1.0);
}
```

Habits that make shaders look finished:

- **Tone-map and gamma at the end.** Light in linear values that may exceed 1, then `col = 1.0 - exp(-col)` (or an ACES fit) and `pow(col, vec3(0.4545))`. Clamping straight to 8 bits looks flat.
- **Animate slowly and continuously.** Use `iTime` inside `sin`/noise, at rates of 0.05–0.5. Expose a `speed` uniform so the spec can pace it, and animate uniforms on the timeline for the beats (`"animate": [{"to": {"glow": 1}, "at": "@beat9"}]`).
- **React to music** with `iAudio` and `iBeat`, for example `float kick = iBeat.x * iAudio.y;` for a bass-weighted pulse on every beat.
- **Cut out and composite.** Set `"alpha": true` and write coverage to `fragColor.a` to put a shader object over video, text or other shaders. Use `"filter": true` to post-process everything below.
- **Edges.** `"supersample": true` costs about 1.5–2× and cleans up thin geometry and specular sparkle. Turn it on for the final render if `still` shows aliasing.
- **Cost.** It scales with pixels × steps × noise octaves. `still` prints it per layer. Keep each layer under about 200 ms/frame at 1080p and a 10 s, 30 fps shot renders in about a minute.

## How it works

1. **Front end.** The source is wrapped in a small prelude (the Shadertoy inputs, four channel textures), parsed and validated by [naga](https://github.com/gfx-rs/wgpu/tree/trunk/naga) (GLSL 450 or WGSL). Errors point at your line and column, with a "did you mean" for misspelt builtins (`smoothStep` → `smoothstep`, `lerp` → `mix`, `iGlobalTime` → `iTime`).
2. **JIT.** naga IR is compiled with [Cranelift](https://cranelift.dev) into one native kernel per shader, once per spec (5–35 ms). Each kernel call shades a row of **2×2 pixel quads**. Every shader value is a 4-lane SIMD vector, with one lane per pixel of the quad, exactly like a GPU quad. That is why `dFdx`, `dFdy` and `fwidth` work: they are lane shuffles.
3. **Control flow** runs SPMD-style under an execution mask. An `if` runs a branch only when some lane takes it, and loops run while any lane is active. `break`, `continue`, `return` and `discard` switch lanes off. Uniform control flow therefore costs only a mask test. Calls are inlined, and maths that depends only on uniforms is hoisted out of the pixel loop.
4. **Threads.** Rows of quads run on rayon across all cores. Output is premultiplied RGBA8 straight into the display list as an `Op::Image`. Pixels are made only when the renderer asks for them, so occluded or duplicate frames cost nothing. A shader that never reads `iTime` renders once.

**Loop unrolling.** Counted loops with a short constant trip count, such as `for (int i = 0; i < 6; i++)`, become straight-line copies with `i` a constant in each, as long as nothing but the guard leaves them. Everything that depends only on uniforms and `i`, like the `sin(iTime + float(i))` that places each sphere of a Shadertoy scene, then no longer varies per pixel and is hoisted out of the pixel loop. The arithmetic is the same, so the output is byte-identical. On a typical Shadertoy raymarch (six moving metaballs, shadows, AO, one reflection bounce) this went from 2867 to 1087 ms per 1080p frame. `turing` went from 144 to 26 ms.

**Two quads at a time.** Each kernel iteration shades two neighbouring 2×2 quads as one 8-lane SPMD program: one branch structure, with masks per quad. A raymarch spends its time waiting on a serial chain (each step's distance feeds the next), so two independent chains let the CPU overlap them. The scene above went from 971 to 691 ms per frame (1.41×), `hero` 58 → 48, `particles` 116 → 98, `liquid-chrome` 141 → 125. Every library shader's output is byte-identical to one quad at a time. Compiling takes about twice as long, so `anikit-video still` (one frame) uses one quad, and renders use two. `ANIKIT_SHADER_GROUPS=1|2` overrides this; in Rust, call `anikit_shader::set_lane_groups`.

**`pow` with a whole-number exponent** from 1 to 16 (Fresnel's `pow(1.0 - c, 5.0)`) compiles to repeated squaring: a few multiplies instead of `exp2(y·log2 x)`, and closer to exact. liquid-chrome went from 163 to 139 ms, caustics from 58 to 50.

**Division by a constant** compiles to a multiply by the reciprocal. That is exact for powers of two and within 1.5 ulp otherwise; GLSL allows 2.5. A multiply has about a third of a divide's latency, which matters in the serial `smin` chains of a raymarch: 1106 → 985 ms on the scene above, with 27 of 2 M pixels changing by 1–4/255.

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
| `iAudio` | `vec4` | the music now, 0..1: (level, bass, mid, treble), fast attack, 150 ms release |
| `iBeat` | `vec4` | (pulse: 1 on each beat decaying to 0, phase 0..1 through the beat, beat number from 1, bar pulse) |

`iAudio` and `iBeat` follow the spec's music: the first top-level `audio` cue with a `src` that isn't a voice-over, the same track `"@beat12"` markers use. It is analysed once, and only if a shader reads them. Without music they are 0 and a warning says so.

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

- `"supersample": true` gives 2×2 anti-aliasing, adaptively. Every pixel is shaded once, and only pixels that differ from one of their 8 neighbours by more than 1.5/255 (edges, highlights, fine detail) are re-shaded with 4 sub-samples. That is about 2.4× faster than full 2×2. Measured against full 2×2 at 1080p (liquid-chrome, sdf-scene, Fold, turing): PSNR 85–101 dB, 0.001% of samples off by more than 1/255, worst pixel 7/255.
- `"alpha": true` uses `fragColor.a` as coverage. By default the output is opaque, as on Shadertoy.
- `src: "fx.glsl"` (or `.wgsl`) loads the code from a file.

### Multi-pass buffers

Simulations, trails, blurs and anything that needs a previous frame or an intermediate image go in buffer passes, as on Shadertoy. Up to four passes, `"A"` to `"D"`, run in that order before the layer's own (image) pass. Each writes a float RGBA buffer at the layer's size, unclamped and exact, so it can hold positions, velocities or HDR light.

```json
{ "shader": "void mainImage(out vec4 c, in vec2 p) { vec3 v = texture(iChannel0, p / iResolution.xy).rgb; c = vec4(pow(1.0 - exp(-v * 1.5), vec3(0.4545)), 1.0); }",
  "channels": ["A"],
  "buffers": { "A": { "src": "trail.glsl", "channels": ["A"] } } }
```

```glsl
// trail.glsl: last frame, zoomed and turned a little and faded, plus a bright orbiting dot.
void mainImage(out vec4 c, in vec2 p) {
    vec2 ctr = p / iResolution.xy - 0.5;
    ctr = mat2(cos(0.01), -sin(0.01), sin(0.01), cos(0.01)) * ctr * 0.985;
    vec3 prev = texture(iChannel0, ctr + 0.5).rgb * 0.965;
    vec2 q = (p - 0.5 * iResolution.xy) / iResolution.y;
    vec2 dot_ = 0.3 * vec2(cos(iTime * 1.7), sin(iTime * 2.3));
    vec3 hue = 0.5 + 0.5 * cos(iTime * 0.6 + vec3(0.0, 2.1, 4.2));
    c = vec4(prev + hue * exp(-pow(length(q - dot_) / 0.018, 2.0)) * 2.0, 1.0);
}
```

- **Channels.** A channel `"A"` (in the layer's `channels` or a buffer's) reads buffer A. If A runs earlier in the order, it reads this frame's A; if A is the reading pass itself or a later one, it reads last frame's A (feedback). On the layer's first frame, previous buffers are zero. Buffer channels can also be `"noise"` or `"none"`, with `wrap` and `filter` options as usual.
- **Determinism.** Frame *n* is the same whether the render reaches it in order or a still jumps straight to it: a jump runs the buffers from the layer's first frame. Running from the start makes a still deep into a long feedback shot cost *n* buffer frames (about 6 ms each at 960×540). Float buffers and `texelFetch` are sampled inline, with no call per read. Renders step one frame at a time.
- **Uniforms.** Buffers see the spec's uniform values (or the declared defaults), not animated ones. Otherwise a frame would depend on how it was reached. Animate the image pass, or drive buffers from `iTime`, `iAudio` and `iBeat`.

### WGSL

Write `fn mainImage(fragCoord: vec2f) -> vec4f` (or `fn transition(uv: vec2f) -> vec4f`). The same inputs are private globals. Channels are `iChannel0..3` (`texture_2d<f32>`) with samplers `iSampler0..3`.

## Built-ins

| Name | Use | What it is |
|---|---|---|
| `sdf-scene` | layer | portals at golden hour: concrete frames receding down a reflecting pool, low backlit sun, soft shadows, AO, haze, a slow dolly. Uniforms: `speed`, `sun` (0 day … 1 sunset), `haze` |
| `liquid-chrome` | layer | a liquid-metal mass with drops that merge and tear away, mirror chrome in a studio or night sky. `speed`, `env` (0 studio … 1 night), `film` (iridescence), `background` (0 = cut out, with `"alpha": true`), `size` |
| `aurora` | layer | aurora curtains over a ridge, mirrored in a lake. Heavy (~1 s/frame); `"lake": 0` + `lake-mirror` is 1.33× faster. `speed`, `intensity`, `lake` |
| `hero` | layer | product shot: one object on a glossy studio floor, turning under softboxes. `shape` (0 torus, 1 rounded cube, 2 pill, 3 gem, 4 sphere, 5 twisted bar), `color`, `metal`, `rough`, `speed`, `size`, `background`, `floor_` |
| `mesh-gradient` | layer | soft animated mesh gradient: four colours drifting and folding into each other, film grain. The background for titles and product shots. `color1`…`color4` (hex strings work), `speed`, `warp`, `softness`, `grain` |
| `ocean` | layer | open sea to the horizon under a low sun: peaked swell and chop, Fresnel sky reflection, the sun's glitter path, light through crests. `speed`, `sun` (0 high … 1 on the horizon), `swell`, `chop`, `color`, `sky` |
| `caustics` | layer | pool-floor caustics; ray density from the refraction map's derivatives. `speed`, `depth`, `tint` |
| `flow-ink` | layer | two inks marbled by a curl-noise flow field. `speed`, `ink`, `ink2`, `paper` |
| `light-shafts` | layer | sun through a window into a hazy room: pane-shaped beams, mullion shadows, leaves outside, drifting haze, dust glinting in the light. `angle`, `spread` (0 parallel … 1 fanning), `panes`, `color`, `intensity`, `haze`, `dust`, `speed` |
| `particles` | layer | glowing particles drifting in depth with bokeh; `focus` racks between near and far. `color`, `color2`, `density`, `speed`, `size`, `focus`, `direction` |
| `turing` | layer | reaction–diffusion spots and labyrinths. `scale`, `balance` (−1 spots … 0 maze … 1 holes), `speed`, `colorA`, `colorB` |
| `film-burn` | filter | the frame below scorches from hot spots. Animate `amount` 0 → 1; `seed` |
| `lake-mirror` | filter | reflects everything drawn above a horizon in rippled dark water below it, including layers composited in between. It pairs with `aurora` (`"lake": 0`) for a 1.33× faster aurora. That path is not identical to aurora's traced lake: against it, SSIM 0.955 and ΔE p99.9 11.6, from different jitter noise, stars and the ridge rim. Uniforms: `horizon`, `focal`, `speed`, `ripple`, `gain` |
| `glitch` | filter | codec-style block slides, row smears, RGB split, scanlines in bursts. `amount`, `rate` |
| `burn` | transition | the old scene burns away along a noisy ember edge. `glow`, `color` |
| `ripple` | transition | a ring wave carries the new scene out from the centre. `amplitude`, `waves` |
| `pixelate` | transition | pixel blocks grow, cross-fade, resolve. `blocks` |
| `swirl` | transition | a vortex twists and unwinds. `turns`, `radius` |
| `ink-bleed` | transition | the new scene soaks in like ink. `softness`, `edge` |
| `warp-wipe` | transition | a soft angled wipe that smears pixels. `direction` (degrees), `smear` |
| `luma-melt` | transition | the old scene melts away darkest-first. `softness`, `invert` |
| `zoom-blur` | transition | a punch-in zoom blur with chromatic fringes. `strength` |

Reel of the newer recipes and a feedback buffer: [`shots/recipes.mp4`](shots/recipes.mp4) (spec [`examples/recipes.json`](examples/recipes.json); 13 s of 1080p rendered in 60 s on 4 cores). `anikit-video info --shaders` lists every built-in with its measured cost and each uniform's default and meaning. `info --json` has the same under `"shader_library"`. The sources live in `crates/anikit-shader/library/*.glsl`. Copy one as a starting point: they are short and commented.

## Performance

Milliseconds per frame at **1920×1080**, on **4 cores** (Xeon @ 2.1 GHz, AVX2), release build. Compile time is the one-off parse + validate + JIT, for the two-quad kernels renders use (`still` compiles one-quad kernels in about half the time). Filters and transitions are timed with full-HD channel textures already bound.
Generated by `cargo run --release -p anikit-shader --example library_bench`:

| shader | use | compile ms | ms/frame | ms/frame 2×2 SS |
|---|---|---|---|---|
| `sdf-scene` | layer | 317 | 265 | 549 |
| `liquid-chrome` | layer | 141 | 125 | 236 |
| `aurora` | layer | 92 | 708 | 2824 |
| `hero` | layer | 137 | 48 | 97 |
| `mesh-gradient` | layer | 12 | 32 | 172 |
| `ocean` | layer | 49 | 191 | 563 |
| `caustics` | layer | 14 | 51 | 215 |
| `flow-ink` | layer | 57 | 151 | 672 |
| `turing` | layer | 20 | 25 | 93 |
| `light-shafts` | layer | 28 | 91 | 167 |
| `particles` | layer | 64 | 98 | 191 |
| `film-burn` | filter | 19 | 77 | 147 |
| `lake-mirror` | filter | 12 | 31 | 73 |
| `glitch` | filter | 8 | 27 | 133 |
| `burn` | transition | 16 | 52 | 161 |
| `ripple` | transition | 8 | 34 | 83 |
| `pixelate` | transition | 5 | 22 | 63 |
| `swirl` | transition | 7 | 38 | 88 |
| `ink-bleed` | transition | 15 | 59 | 114 |
| `warp-wipe` | transition | 6 | 29 | 66 |
| `luma-melt` | transition | 5 | 20 | 52 |
| `zoom-blur` | transition | 14 | 129 | 216 |

Compiling is a one-off. Kernels are cached process-wide by source and uniform set, so a shader used by many layers, compositions or transitions compiles once. Every shader in a spec is JIT-compiled in parallel before the first frame is evaluated, which took the showcase spec's build from 1114 ms to 114 ms.

End to end through the video pipeline (evaluate, render, composite text, H.264 encode): the 18 s [`shaders.json`](examples/shaders.json) showcase, with every built-in and seven shader transitions, is 537 frames in 215 s (400 ms per frame). `{"shader": "sdf-scene"}` alone is 90 frames in 32.2 s.

**Know the cost before you render.** `anikit-video still spec.json` prints each shader layer's measured ms/frame, scaled to full size, and flags anything over 200 ms (`--json` adds a `"shaders"` list). `still --scale 0.5` renders shaders at half size too, about 4× cheaper, so quick looks stay quick. `still --watch` keeps compiled kernels between edits: only a changed shader recompiles.

What costs time is pixels × work per pixel:

- **Raymarch steps and noise octaves** dominate. `aurora` evaluates around 100 noise lookups per pixel; `flow-ink` got from 470 to 160 ms by taking its curl from analytic noise gradients instead of finite differences. Halving octaves or steps halves the time.
- **Texture reads**: plain bilinear or nearest reads with clamp or repeat wrap are inlined as SIMD gathers. Mipmapped and mirror-wrapped channels go through a sampler call, which is slower because the call spills the shader's registers. `zoom-blur` reads 32 texels per pixel, which is why it costs a third of a second per frame.
- **Maths on uniforms** (`sin(iTime)` and rotation matrices from time) is free: it is hoisted. Divergent branches cost both sides only in quads that actually diverge.
- **Supersampling** costs about 1.5–2× when adaptive (4× for full 2×2). A layer smaller than the frame costs proportionally less, and raymarchers should bound their scene: `liquid-chrome` skips every ray that misses its bounding sphere.
- **Static shaders** (no `iTime`) render once per distinct uniform set.

## Limits

- One pass per shader: no Shadertoy Buffer A–D feedback, keyboard or audio channels, or cube maps. Chain effects by stacking filters instead.
- `sampler2D` can't be passed to user functions. Sample `iChannelN` directly (inside helpers, refer to `iChannelN` itself).
- `double` is computed as `float`. These are unsupported: `textureOffset`, `textureGather`, shadow samplers, `bitfieldExtract`/`Insert`/`Reverse`, `findLSB`/`MSB`, and the pack/unpack built-ins (`bitCount` works).
- `pow(x, y)` with `x < 0` is exact for integral `y` (as GPU compilers effectively do) and NaN otherwise. NaN or out-of-range colours are written as 0 / clamped.
- A loop runs at most 2²⁰ iterations per quad, so a runaway `while (true)` ends instead of hanging the render.
- A filter always covers the full frame; its `x`, `y`, `width` and `height` are ignored.
