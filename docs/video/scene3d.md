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
| `geometry.json` | Open geometry: extrude, lathe, tube, surface, heightfield, mesh. |
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
  - `type`: `text`, `box`, `slab`/`pane`, `plane`, `card`, `sphere`, `cylinder`, `torus`, `phone`, `extrude`, `lathe`, `tube`, `surface`, `heightfield`, `mesh` (see *Make your own geometry*), `model` (glTF/GLB via `src`: meshes, PBR factors and node animation, which loops at `speed`);
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
- **`glassLayers`:** how many panes a camera ray may pass through (default 5; 3 at `draft`). Lower it for stacks of overlapping glass; each extra pane costs a full trace.
- **Look:** `quality` (`draft`, `standard`, `high`), `bloom`, `exposure`, `tonemap` (`neutral`, `aces`), `vignette`, `floor` (`false` or `{ color, roughness, reflect, mirror, y }`).

## Make your own geometry

You are not limited to the preset shapes. These types build a mesh from your description. Materials, `pos`/`rot`/`scale`, `keys`, `count` and expressions all apply to them as usual.

- **`extrude`:** any SVG path, as a solid with a rounded bevel. It stands on y = 0, `height` tall. Use it for logos, icons and signs. Inner subpaths cut holes.
  `{ "type": "extrude", "d": "M0 0 L40 0 L40 10 L10 10 L10 30 L0 30 Z", "height": 0.6, "depth": 0.15, "bevel": 0.01, "material": "gold" }`
- **`lathe`:** a profile `[[radius, y], ...]` revolved about Y. Use it for vases, bottles, lenses, columns and chess pieces. `segments` defaults to 96.
  `{ "type": "lathe", "points": [[0, 0], [0.22, 0.02], [0.3, 0.25], [0.12, 0.6], [0.16, 0.85], [0, 0.86]], "material": "ceramic" }`
- **`tube`:** a circle swept along a path, for cables, neon, springs and paths through space.
  - The path is `points` `[[x, y, z], ...]` (smoothed) or `path`, an expression of `s` (0→1).
  - `radius` is a number or an expression of `s`, so tubes can taper.
  - `segments` (default 256) and `sides` (default 24) set the resolution.
  `{ "type": "tube", "path": "=[0.35*cos(s*18.85), s*1.2, 0.35*sin(s*18.85)]", "radius": "=0.03+0.02*s", "material": "copper" }`
- **`surface`:** a parametric surface. `x`, `y` and `z` are expressions of `u` and `v`; `"u": [from, to, steps]` and `"v"` set the ranges (default `[0, 1, 64]`). Use it for ribbons, shells, knots, waves and Möbius strips.
  `{ "type": "surface", "x": "=u", "y": "=0.2*sin(u*6+v*3)", "z": "=v", "u": [-1, 1, 128], "v": [-1, 1, 64], "material": "chrome" }`
- **`heightfield`:** `y = expr(x, z)` over a `size` `[w, d]` rectangle centred on the origin, at `res` steps per side (default 128). Use it for terrain, liquid surfaces and data landscapes.
  `{ "type": "heightfield", "expr": "=0.06*sin(x*9)*cos(z*9)", "size": [4, 4], "res": 128, "material": "matte" }`
- **`mesh`:** raw `vertices` `[[x, y, z], ...]` and `faces` (triangles, polygons or a flat index list). Normals are smoothed unless `"flat": true`.
  `{ "type": "mesh", "vertices": [[0, 0.8, 0], [-0.25, 0.1, -0.25], [0.25, 0.1, -0.25], [0, 0.1, 0.25]], "faces": [[0, 2, 1], [0, 3, 2], [0, 1, 3], [1, 2, 3]], "flat": true }`

Shapes are built once when the spec loads, so their expressions use `u`/`v`/`s`/`x`/`z` (not `t`). To animate them, animate the object (`keys`, `"rot": "=[0, t*40, 0]"`), or deform with `scale`. To generate many objects, put `{ "script": "..." }` in `objects`; it expands to the items it returns. Cost: tracing a mesh costs about the same as the preset shapes of similar screen size. A 160×32 surface or a 128² heightfield adds under 5 ms to scene build, and render time is set by pixels covered, not by triangle count.

## Rules that keep it looking expensive

1. **Chrome needs something dark to reflect.** On `cyc`, add a black `card` in front of the subject or slightly above the camera. On `studio`, it already has contrast.
2. **Clear glass means `transmission` near 1.** Prints on glass should have transparent margins: alpha 0 shows the glass itself.
3. **Use `focal` 35–85 with `fstop` 2–4 for product depth of field.** Focus defaults to the camera target.
4. **Stand objects on the floor:** text and models stand on y = 0. Lift thick glass by 1 mm.
5. **Keep instances in the thousands, not millions.** Use `scale` 0 to hide them; hidden instances cost nothing.

## Cost and debugging

- 1080p on 4 cores runs at about 1.3 s per frame at `standard`. Use `"quality": "draft"` for previews.
- `ANIKIT_SCENE3D_PROFILE=1` prints per-stage ms and ray counts.
- `ANIKIT_SCENE3D_DEBUG=refl|col|diffuse` shows one buffer.
- Errors name the exact field with a "did you mean" suggestion: ``objects[2].material: unknown material `crome`; did you mean `chrome`?``.

Reference: `docs/video/components.md#scene3d`. Code: `crates/anikit-video/src/scene3d/`.
