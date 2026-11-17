# scene3d: ray-traced 3D for agents

`scene3d` is a layer component that renders a real 3D scene with a software ray tracer, inside any Anikit Video spec. Use it when a shot needs:
- chrome, aluminium or glass;
- objects reflecting each other;
- soft shadows;
- a studio look;
- many copies of an object;
- a 2D composition (UI, a chart, a film) on a 3D surface.

## What it does that a rasteriser (three.js, WebGL) can't

- **True reflections:** chrome and glass mirror the other objects and the floor (up to 3 bounces), not just a fake environment map.
- **Real glass:** Fresnel, refraction and Beer–Lambert tint, so thick edges go green. You can see through layer after layer of panes, each with its own print.
- **Soft area-light shadows and contact occlusion:** traced, then denoised edge-aware.
- **A seamless lit cyclorama:** the floor turns into the backdrop with no horizon line.
- **Thousands of textured instances:** each can show a different frame of a composition, with LOD and culling.

## Start here (copy, then edit)

The shortest form is a scene key. It renders full-frame behind the scene's other layers:

```json
{ "scenes": [ { "duration": 4, "scene3d": { "env": "studio", "camera": "push", "objects": [ { "text": "ANIKIT", "material": "chrome" } ] },
                "layers": [ { "text": "Ray-traced", "y": "85%", "style": "subtitle" } ] } ] }
```

A layer can also be `{ "scene3d": {...}, "opacity": 0.8, ... }`, or the long form below:

```json
{ "component": "scene3d", "props": {
  "env": "cyc",
  "camera": "push",
  "objects": [
    { "text": "ANIKIT", "height": 0.7, "material": "chrome" },
    { "type": "card", "size": [4, 0.5], "pos": [0, 0.45, 2.6], "rot": [0, 180, 0], "color": "#000", "intensity": 0 }
  ] } }
```

The card is a black flag that only shows in reflections; it gives chrome contrast on the white cyc. Full examples are in `examples/video/lookdev/`:

| File | Shows |
|---|---|
| `cyc.json` | White cyc, chrome type, textured glass panes, aluminium, emissive, reflector cards. |
| `panes.json` | 3,600 glass panes, each showing frame `i` of a composition. |
| `glass.json` | Thin and thick glass, a glass sphere, aluminium type. |
| `chrome.json` | The dark-studio lookdev: `{ "text": "ANIKIT" }` with no `objects`. |

## The spec in one screen

- **`env`:**
  - presets: `cyc` (bright warm-white studio, the default), `studio` (dark, glossy floor), `night`, `neon`, `sunset`;
  - or an object: `{ "preset": "cyc", "color": "#EEEBE5", "key": [-0.55, 1, 0.45], "haze": 0.0015 }`.
- **`lights`:** `[{ "dir": [x, y, z], "size": [w°, h°], "color", "intensity": 4, "roll", "key": true }]`. These are extra softboxes or strips. They light the scene and show as crisp shapes in chrome. `key` casts the traced shadows.
- **Local lights** (in `lights`, given a `pos`): a point light `{ "pos": [x, y, z], "color", "intensity": 10 }`, or a spot when it also has a `target`, `angle` (degrees) and `penumbra` (0–1). They fall off with the inverse square and cast hard traced shadows (`"shadow": false` turns them off). Use them for light pools on the floor, LEDs and practicals.
- **`fog`:** a density per metre, or `{ "density", "color", "shafts": true }`. `shafts` adds volumetric light: spot cones and god rays from the key, with object shadows cut into them. Use density 0.02–0.08 with a dark fog colour at night.
- **`camera`:**
  - presets: `push`, `reveal`, `flyby`, `orbit` (all 5 s by default), `dolly`, `crane`, `static`; or `{ "preset", "duration", "degrees", "height", "shake" }`;
  - keys: `{ "interp": "smooth"|"linear", "keys": [ { "t", "pos", "target", "focal": 35, "fstop": 2.8, "focus", "shutter": 0.5 } ] }`. A dense per-frame list with linear interpolation works too. `shutter: 0` turns motion blur off.
  - rigs: `"target": "machine"` aims at the object with that `name` (key `target`s become offsets from it); add `"follow": true` to move with it too (key `pos` become offsets).
- **`objects`:**
  - `type`: `text`, `box`, `slab`/`pane`, `plane`, `card`, `sphere`, `cylinder`, `torus`, `phone`, `model` (glTF/GLB via `src`: meshes, PBR factors and node animation, which loops at `speed`);
  - placement: `pos`, `rot` (degrees), `scale` (a number or `[sx, sy, sz]`; 0 hides). Each can be an `"=expr"` of `t`, `i`, `lane` or `n`, e.g. `"pos": "=[1.7*i, 0.47, -4*lane]"`;
  - animation: `keys: [{ t, pos, rot, scale, ease }]`, `spin`, `start`/`end`;
  - instances: `count` and `lanes`;
  - `texture`: `"#comp"`, `{ "composition", "frame": "=i", "face": "front|back|both" }`, or a list of these (one per face);
  - `material`: a name or an object;
  - flags: `camera: false` (a reflector card), `shadow`, `thin`.
- **`materials`:**
  - named custom materials, e.g. `{ "ink": { "extends": "matte", "base": "#15171A" } }`;
  - built-ins: `chrome`, `mirror`, `glass`, `aluminium`, `aluminium-bead`, `titanium`, `copper`, `gold`, `matte`/`white`, `ceramic`, `plastic`, `rubber`, `black`, `car-paint`, `lacquer`, `emissive`;
  - keys: `base`, `metallic`, `roughness`, `specular`, `clearcoat`, `emissive`, `emissiveStrength`, `transmission`, `ior`, `tint` + `tintDistance`, `glow` (how much a texture glows like a screen).
- **Look:** `quality` (`draft`, `standard`, `high`), `bloom`, `exposure`, `tonemap` (`neutral`, `aces`), `vignette`, `floor` (`false` or `{ color, roughness, reflect, mirror, y }`).

## Rules that keep it looking expensive

1. **Chrome needs something dark to reflect.** On `cyc`, add a black `card` in front of the subject or slightly above the camera. On `studio`, it already has contrast.
2. **Clear glass means `transmission` near 1.** Prints on glass should have transparent margins: alpha 0 shows the glass itself.
3. **Use `focal` 35–85 with `fstop` 2–4 for product depth of field.** Focus defaults to the camera target.
4. **Stand objects on the floor:** text and models stand on y = 0. Lift thick glass by 1 mm.
5. **Keep instances in the thousands, not millions.** Use `scale` 0 to hide them; hidden instances cost nothing.

## Cost and debugging

- 1080p on 4 cores runs at about 0.6–1.2 s per frame at `standard` (lookdev shots; glass-heavy shots are the slowest). Use `"quality": "draft"` for previews.
- `ANIKIT_SCENE3D_PROFILE=1` prints per-stage ms and ray counts; `=2` adds micro-benchmarks of the building blocks.
- `ANIKIT_SCENE3D_DEBUG=refl|col|diffuse` shows one buffer.
- Errors name the exact field with a "did you mean" suggestion: ``objects[2].material: unknown material `crome`; did you mean `chrome`?``.

Reference: `docs/video/components.md#scene3d`. Code: `crates/anikit-video/src/scene3d/`.

## Performance internals (render kernels)

The hot parts live in `scene3d/kernel/`:

- `wide.rs`: 8-wide BVHs (binary SAH collapsed to eight children,
  triangles in SoA blocks of four), traversal with explicit SSE2 / AVX2 slab
  and Möller–Trumbore kernels picked at run time (`ANIKIT_NO_AVX2=1` forces
  the baseline). Both paths compute the same bits as the portable reference
  (unit tested).
- `post.rs`: the post stack on structure-of-arrays planes, row-parallel,
  hot loops in `simd_fn!` kernels (AVX2 + baseline, same bits).
- `envk.rs`: environment softboxes evaluated eight lanes at a time (same
  bits as `Softbox::eval`).
- `pool.rs`: frame-sized buffers reused across frames (fresh 100+ MB
  allocations cost ~0.4 ms/MB in page faults).
- `par.rs`: row-parallel helpers (serial inside a rayon worker).

The primary pass (`Scene::render_frame`) runs in two whole-frame phases: one
centre sample per pixel, then supersampling where neighbours disagree in
object id or in tone-mapped luminance with *unshadowed* diffuse light (AO
noise is denoised in post and must not trigger supersampling). Secondary
rays whose whole-path weight is below 0.3% take the pre-filtered
environment instead (`W_MIN`).

Measure every change:

- `cargo run --release -p anikit-video --example scene3d_bench [-- --reference]`:
  ms and CPU ms per frame for the lookdev shots, and SSIM / ΔE p99.9 /
  flicker against converged references (`--reference` writes them:
  `ANIKIT_SCENE3D_REFERENCE=1`, 25 samples per pixel, 4 shadow/AO rays,
  no shortcuts).
- `cargo run --release -p anikit-video --example bvh_bench`: ns per ray.
- `ANIKIT_SCENE3D_DUMP=f.px` + `examples/post_bench`: the post stack alone.
