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
| `wave.json` | A heightfield and a tube animated by `t` (rebuilt per frame). |
| `geometry.json` | Open geometry: extrude, lathe, tube, surface, heightfield, mesh. |
| `chrome.json` | The dark-studio lookdev: `{ "text": "ANIKIT" }` with no `objects`. |

## The spec in one screen

- **`env`:**
  - presets: `cyc` (bright warm-white studio, the default), `studio` (dark, glossy floor), `night`, `neon`, `sunset`;
  - or an object: `{ "preset": "cyc", "color": "#EEEBE5", "key": [-0.55, 1, 0.45], "haze": 0.0015 }`.
- **`lights`:** `[{ "dir": [x, y, z], "size": [w°, h°], "color", "intensity": 4, "roll", "key": true }]`. These are extra softboxes or strips. They light the scene and show as crisp shapes in chrome. `key` casts the traced shadows.
- **Local lights** (in `lights`, given a `pos`): a point light `{ "pos": [x, y, z], "color", "intensity": 10 }`, or a spot when it also has a `target`, `angle` (degrees) and `penumbra` (0–1). They fall off with the inverse square and cast hard traced shadows (`"shadow": false` turns them off). Use them for light pools on the floor, LEDs and practicals.
  - `pos`, `target` and `intensity` take `"=expressions"` of `t`: a searchlight that sweeps (`"pos": "=[2.2*sin(t*0.9), 3.2, 1+1.2*cos(t*0.9)]"`), a spot that tracks, a sign that flickers (`"intensity": "=1.2+max(0,sin(t*7))"`).
  - Proof: `docs/video/shots/3d-lights/searchlight.jpg` (two moments of the same shot).
  - Texture prints (compositions on panes, planes and text) are lit by local lights too, like matte paper: a sodium lamp pools on a brick-textured wall and a sign spills its colour onto a textured facade. Each shadowed light costs one shadow ray per textured pixel (a street with 20 shadowed lamps and many textured panes roughly doubles the trace time); set `"shadow": false` on fill lights.
- **`fog`:** a density per metre, or `{ "density", "color", "shafts": true, "height", "base" }`. `height` makes it height fog: dense at `base` (default 0, the floor) and thinning by e every `height` metres. That gives ground mist, a valley fog, or a set that dissolves into haze. It's integrated exactly along each camera ray (one `exp`), and the layer meets the sky softly. Proof: `docs/video/shots/3d-fog/ground-mist.jpg` (`{ "density": 0.9, "height": 0.25 }`). `shafts` adds volumetric light: spot cones and god rays from the key, with object shadows cut into them. Use density 0.02–0.08 with a dark fog colour at night.
- **`camera`:**
  - presets: `push`, `reveal`, `flyby`, `orbit` (all 5 s by default), `dolly`, `crane`, `static`, plus product-film moves: `slide` (a lateral parallax pass), `arc` (a slow 30° arc), `rise` (a pedestal up from below the subject) and `topdown` (an overhead flat-lay turning 20°); or `{ "preset", "duration", "degrees", "height", "shake" }`;
  - keys: `{ "interp": "smooth"|"linear", "keys": [ { "t", "pos", "target", "focal": 35, "fstop": 2.8, "focus", "shutter": 0.5 } ] }`. A dense per-frame list with linear interpolation works too. `shutter: 0` turns motion blur off.
  - rigs: `"target": "machine"` aims at the object with that `name` (key `target`s become offsets from it); add `"follow": true` to move with it too (key `pos` become offsets).
- **`objects`:**
  - `type`: `text`, `box`, `slab`/`pane`, `plane`, `card`, `sphere`, `cylinder`, `torus`, `phone`, `extrude`, `lathe`, `tube`, `surface`, `heightfield`, `mesh` (see *Make your own geometry*), `model` (glTF/GLB via `src`: meshes, PBR factors and node animation, which loops at `speed`);
  - placement: `pos`, `rot` (degrees), `scale` (a number or `[sx, sy, sz]`; 0 hides). Each can be an `"=expr"` of `t`, `i`, `lane` or `n`, e.g. `"pos": "=[1.7*i, 0.47, -4*lane]"`;
  - animation: `keys: [{ t, pos, rot, scale, ease }]`, `spin`, `start`/`end`;
  - instances: `count` and `lanes`;
  - `texture`: `"#comp"`, `{ "composition", "frame": "=i", "face": "front|back|both" }`, or a list of these (one per face);
    compositions can contain shader layers (including ones that read text channels), so a 3D sign, screen or poster can carry an animated GLSL treatment (proof: `deliverables/ceiling1/proof_shader_texture_3d.png`, top row as 2D layers, bottom row on 3D panes);
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
- **`lathe`:** a profile `[[radius, y], ...]` revolved about Y. Use it for vases, bottles, lenses, columns and chess pieces. `segments` defaults to 96. The profile is smoothed through the points (Catmull-Rom); `"smooth": false` keeps hard corners.
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

Proof of animated open geometry in an abstract piece: `docs/video/shots/3d-abstract/ribbon.mp4` (5 s, 1080p30, rendered in about 105 s on 4 cores; a flowing surface ribbon, a growing tube and a bouncing sphere on a yellow sweep). Shapes whose expressions use `t` (seconds) are rebuilt every frame: waves, growing tubes, morphing surfaces (`examples/video/lookdev/wave.json`). This costs about 44 ms per frame for a 160² heightfield plus a 300-segment tube, and turns off the baked floor. Shapes without `t` are built once. Motion blur follows object motion, not deformation. To generate many objects, put `{ "script": "..." }` in `objects`; it expands to the items it returns. Cost: tracing a mesh costs about the same as the preset shapes of similar screen size. A 160×32 surface or a 128² heightfield adds under 5 ms to scene build, and render time is set by pixels covered, not by triangle count.

## Decals: print on any surface

Dial numerals, a logo on a can, a label on a bottle: put a `plane` just above the surface with `"material": "decal"` and a composition `texture`. A decal is a clear, index-1 layer: no reflection, no refraction and no shadow. Only its print shows, lit like the surface under it.

`{ "type": "plane", "size": [0.6, 0.6], "pos": [0, 0.0805, 0], "rot": [-90, 0, 0], "texture": "#dial", "material": "decal", "thin": true, "shadow": false }`

- Leave the composition's `background` out, so it's transparent.
- Lift the plane about 0.5 mm off the surface.

Proof: `docs/video/shots/3d-decals/dial.jpg` (type and an index printed on a guilloché dial; spec beside it).

## Imperfections

Perfect CG surfaces read as fake. Real products have fingerprints, wipes, fine scratches that show only in a highlight, uneven sheen and dust. Add them on any material:
- `"imperfections": 0.4`: one believable mix (roughness variation, smudges, scratches, a little dust).
- Or each, 0–1: `roughnessVariation`, `smudges`, `scratches`, `dust` (on up-facing surfaces; even metal goes grey there).

They stick to the object (object space), are deterministic, and show mostly where a highlight crosses them. That's why they pair well with `rig: "product"` and `animate: "sweep"`. Use 0.2–0.4 for hero products and 0.6+ for worn props.

`{ "extends": "lacquer", "imperfections": 0.4 }` · `{ "extends": "glass", "smudges": 0.5 }`

Cost: measured on the comparison scene (3 objects, 1080p, 4 cores), trace went from 0.76 s to 1.02 s at 0.6, because uneven highlights get more edge samples. Proof: `docs/video/shots/3d-imperfections/compare.jpg` (clean vs `imperfections: 0.6`, with 100% crops).

## Machined finishes

Watches, cameras, audio gear and tools are defined by their machined surfaces. `pattern` adds one as bump on any material (`patternScale` is the period in metres, `patternStrength` defaults to 1):
- `brushed`: fine parallel grooves (along the object's X);
- `sunburst`: radial lines from the object's Y axis (dials);
- `guilloche`: engine-turned waves (rings whose radius ripples with angle);
- `clous-de-paris`: a hobnail grid of small pyramids;
- `knurl`: crossed helical grooves (grips and crowns, on the sides of cylinders).

Grooves need about six pixels per period to resolve. Below that they fade into extra roughness (sheen) instead of aliasing, so frame them close: a 100 mm `macro` at f/8–11.

`{ "extends": "aluminium", "base": "#E4E6E9", "roughness": 0.15, "pattern": "guilloche", "patternScale": 0.012 }`

Recipe: `anikit-video recipes precision-macro` (a knurled knob with a sunburst cap on a brushed plate, a strip sweep and a rack focus; still `docs/video/shots/3d-patterns/precision-macro.jpg`). Proof: `docs/video/shots/3d-patterns/dials.jpg` (a 100 mm macro on two dials, guilloché and clous de Paris; trace about 0.7 s at 1080p).

## Sets

Build a stage, not a void. `set` adds a designed set around the subject as ordinary objects, so materials, keys and expressions still apply. It takes a name, `{ "preset", "color", "material", "size", "shape", "density" }`, or a list of these.

| `set` | What it builds |
|---|---|
| `plinth` / `podium` | A two-tier round (or square) stand. The floor drops to its base, so objects at y = 0 sit on top. |
| `steps` | Four stepped blocks rising behind the subject. |
| `sweep` | A seamless cyc of any `color`: floor, curve and wall in one piece, with no horizon line. |
| `field` | An abstract field of columns with noise heights, rising with distance, with a clearing for the subject (`density` 0.25–3). |
| `arches` | A receding corridor of half-buried rings. |
| `terrain` | Rolling fBm ground that flattens near the subject (replaces the floor). |
| `landscape` | Terrain plus about 70 boulders and 90 stylised trees standing on it (placed with the same height function), outside a clearing. Pair it with `rig: { "preset": "golden", "gobo": false }` and a light height fog. |

`{ "env": "cyc", "rig": "rembrandt", "look": "clay", "set": { "preset": "sweep", "color": "#C8664A" }, "objects": [ ... ] }`

Stills (each with its spec): `docs/video/shots/3d-sets/` (`landscape.jpg`: 1080p trace 0.8 s and object build 0.3 s, on 4 cores). Measured 1080p traces on 4 cores: 0.52–0.82 s (`field` has 784 instanced columns).

## Camera language

- **`lens`** on the camera: `wide24` (f/8), `normal35`, `normal50` (f/4), `portrait85` (f/2.8), `macro100` (f/5.6), `tele135` (f/4), `tele200`. It sets the focal length and working aperture for keys that set neither. On a preset it also moves the camera to keep the framing, so a `tele135` push compresses the background instead of cropping in.
  Use 85–135 for hero product shots, 100 macro for details, and 24 only for scale.
- **Rack focus:** a key's `focus` can name an object, e.g. `{ "t": 1.8, ..., "focus": "logo" }`. Focus pulls between the keys' objects, with the same easing as the move. Hold about 0.5 s either side of a 0.6–1.2 s pull.
- **`shake`:** rotational camera noise, sized as a fraction of the frame so it reads the same at any lens. It is three bands of smooth noise plus roll. All presets are deterministic.
  - `breath`: a locked-off camera that's alive, 0.15% of the frame at 0.3 Hz. Use it on every "static" shot.
  - `handheld`: 0.6% at 0.8 Hz, 0.35° roll.
  - `shoulder`: 0.4% at 0.55 Hz.
  - `run`: 1.8% at 1.9 Hz with a vertical bob.
  - Custom: `{ "frame": 0.004, "frequency": 0.7, "roll": 0.2, "bob": 0 }`. A plain number is still the old positional wobble, in metres.

`{ "lens": "tele135", "shake": "breath", "keys": [ { "t": 0, "pos": [-0.2, 0.85, 10.5], "target": [-0.1, 0.4, 0], "focus": "bottle", "fstop": 2.8 }, { "t": 1.8, "pos": [-0.1, 0.85, 10.1], "target": [-0.05, 0.4, 0], "focus": "logo", "fstop": 2.8 } ] }`

Proof: `docs/video/shots/3d-camera/rack-focus.mp4` (3 s, 1080p30, rendered in 51 s on 4 cores) and `.jpg`.

## Lighting rigs

Light like a gaffer, not with an HDRI default. `rig` places a named setup around the camera's view, at set illuminance ratios. It replaces the env's softboxes; the env's sky stays as dim ambient.

| `rig` | Setup | Ratio |
|---|---|---|
| `rembrandt` | key 45° side, 40° up; soft fill opposite; cool rim behind | 4:1 |
| `loop` | key 30° side; fill; soft back light | 3:1 |
| `clamshell` (`beauty`) | big key above the lens, fill below, two edge strips | 2:1 |
| `butterfly` | high frontal key, low fill | 3:1 |
| `product` (`edge`) | big top key, two tall strips at ±115° for edge highlights on metal and glass, black flags at ±75°, front fill | 3:1 |
| `high-key` | huge soft key and fill, back light | 1.5:1 |
| `low-key` (`noir`) | hard side key, cool hard rim | 16:1 |
| `split` | key at 90°, no fill | — |
| `rim` (`silhouette`) | two kickers from behind, faint front fill | 12:1 |
| `golden` | low warm sun through foliage (gobo), cool sky fill | 5:1 |
| `window` | crisp daylight through window panes (gobo) | 6:1 |
| `top` | one overhead source (pendant, skylight) | 10:1 |

Tune it with an object: `{ "preset": "rembrandt", "ratio": 6, "rim": 1.5, "key": 1.2, "angle": 60, "height": 25, "warmth": 0.4, "gobo": "blinds", "goboScale": 0.4, "softness": 0.1, "beam": true }`.
- `angle`/`height`: the key's azimuth from the camera (+ = camera left) and elevation, in degrees.
- `ratio`: (key + fill) : fill. `rim` and `key` scale those lights.
- `warmth`: -1 cool to +1 warm; individual lights keep their own warm/cool contrast.
- `gobo`: `blinds`, `window` or `leaves` (a cookie in front of the key); `goboScale` is the pattern period in metres.
- `beam`: haze plus volumetric shafts, so the key reads as a beam.

- `animate`: light that moves, which is how studios make chrome come alive. `start` and `duration` (default 2.5 s) set the timing.
  - `sweep`: the strips travel 130° across the back, so a highlight runs along every edge.
  - `reveal`: rim first, then the key, then the fill (from silhouette to fully lit).

  `{ "preset": "product", "animate": "sweep", "duration": 3 }`

  Animated rigs rebuild the lights each frame (about 2 ms) and skip the baked floor.

Stills (one scene, six rigs) and `sweep.mp4` (3 s, 1080p30, rendered in 43 s on 4 cores): `docs/video/shots/3d-rigs/`. Rigs cost nothing extra: a rig is a few softboxes, the same as an env. Gobos add one pattern lookup per shaded point. Measured 1080p traces on 4 cores: 0.62–0.79 s.
Why and how studios light: `docs/video/craft/3d-design.md`.

All of it together: `docs/video/shots/3d-showcase/showcase.mp4` (15.5 s, five shots: a macro reveal, an abstract field, a clay sweep, window light and the hero logo; `contact.jpg`; spec beside it).

## Looks and worlds beyond the studio

Not every shot should be a chrome product on a white cyc. One word changes the whole style:

- **`look`** (top level) sets the material of every object that has none. An object's `color` tints it. Stills (JPEG, spec beside each): `docs/video/shots/3d-looks/`.
  - `toon`: three crisp light bands, a hard highlight, a rim, ink outlines.
  - `ink`/`hatch`: pen hatching on paper (lines cross in the shadows), with outlines.
  - `clay`: wrapped soft light, warm subsurface fill, faint fingerprints and sheen.
  - `paper`/`cardboard`: fibrous matte card; `paper` draws cut-edge outlines.
  - `gouache`: flat colour fields, a soft two-tone turn, cool shadows, brush strokes.
  - `photo`: the default PBR look.

  `{ "look": "toon", "objects": [ { "type": "sphere", "color": "#E4572E" }, { "type": "torus", "color": "#29335C" } ] }`
- **Style materials** work per object too: `"material": "toon"`, or `{ "extends": "clay", "base": "#C96" }`. Mix a clay character with a chrome prop.
- **`outline`:** `true`, a width in px at 1080p, or `{ "width": 2, "color": "#111" }`. Draws ink lines on silhouettes, creases and depth steps, thicker for near objects. `false` turns off a look's outlines.
- **New `env` worlds:**
  - `overcast`: an open landscape under a soft grey sky; grass floor, distant haze.
  - `interior`: a room lit by one window: a crisp side key, warm bounce, wood floor and a wall.
  - `city`: night city. Deep blue ambient, a sodium key, neon signage strips and four coloured practicals around the subject (unless you give `lights`), on a wet reflective floor.
  - `underwater`: teal water, light from the surface, moving caustics on the sand and objects, depth haze.

Cost: the styles shade in the same pass as PBR, so they cost the same or less. Outlines add about 30 ms per 1080p frame (measured: toon 1.03–1.07 s with outlines, 1.00 s without). Example 1080p traces on 4 cores: toon 0.70 s, ink 0.97 s, clay/paper/gouache about 0.66 s, city 1.44 s (four shadowed lights).

## Real-world light, materials and models (CC0 library)

Photographed light and scanned surfaces, by name (details: `anikit-video docs assets`; find them with `anikit-video assets search <words> --type hdri|texture|model|lut`):

- **`env: {"hdri": "polyhaven:hdri/<slug>", "rotate": 0, "exposure": 0, "background": true}`**: the HDRI lights the scene, appears in reflections and behind the subject (`"blur"` softens it like a long lens; `false` keeps the scene's own backdrop). The sun, or a studio's brightest compact lamp, is lifted out of the map and becomes the traced key, so it casts real shadows; the rest is pre-filtered for rough reflections and projected to 9 SH terms for diffuse. The scene is exposed for the light around the horizon, and the default floor takes the albedo that matches the HDRI's own ground, so it melts into the photographed horizon. `exposure` offsets in stops; `key: false` keeps the sun in the map.
- **`material: {"texture": "ambientcg:Wood049", "scale": 1}`**: a PBR set (albedo, normal, roughness, AO, metalness) on any object, triplanar in object space (no UVs, nothing swims as it moves), mip-mapped by ray footprint. `scale` 1 is the texture's real-world size; `color` tints; an explicit `roughness`/`metallic` overrides the maps; `"mapping": "uv"` uses mesh UVs; `textureRotate`, `normalStrength`.
- **`floor: {"texture": "ambientcg:WoodFloor051", "scale": 1}`**: a textured ground (albedo, AO, roughness-varied reflection, normal-shaded key); `color` tints it.
- **`{"type": "model", "src": "polyhaven:model/<slug>"}`**: textured glTF (base colour, normal, metal-roughness, occlusion; alpha cutouts for leaves). Any `.glb`/`.gltf` path works the same way.

Stills (spec beside each in `examples/video/asset-library/`): `docs/video/shots/asset-library/` holds one scene under four HDRIs (`hdri-sheet.jpg`), six materials, three models and six film LUTs. 1080p traces: 1.3 s per HDRI shot and 2.2 s for the textured models, on 4 cores. The first use downloads (1.5 MB for a 1k HDRI, 16–25 MB for a 2k material); later uses read the cache.

## Rules that keep it looking expensive

1. **Chrome needs something dark to reflect.** On `cyc`, add a black `card` in front of the subject or slightly above the camera. On `studio`, it already has contrast.
2. **Clear glass means `transmission` near 1.** Prints on glass should have transparent margins: alpha 0 shows the glass itself.
3. **Use `focal` 35–85 with `fstop` 2–4 for product depth of field.** Focus defaults to the camera target.
4. **Stand objects on the floor:** text and models stand on y = 0. Lift thick glass by 1 mm.
5. **Keep instances in the thousands, not millions.** Use `scale` 0 to hide them; hidden instances cost nothing.

## Cost and debugging

- 1080p on 4 cores runs at about 0.6–1.2 s per frame at `standard` (lookdev shots; glass-heavy shots are the slowest). Use `"quality": "draft"` for previews.
- Static layouts bake a noise-free floor once per spec (1.5–2.5 s). The bake is cached in `$ANIKIT_CACHE/sim` (`ANIKIT_CACHE=off` disables it), so re-renders and parallel chunks of the same spec skip it.
- `ANIKIT_SCENE3D_PROFILE=1` prints per-stage ms and ray counts; `=2` adds micro-benchmarks of the building blocks.
- `ANIKIT_SCENE3D_DEBUG=refl|col|diffuse` shows one buffer.
- `anikit-video check spec.json` also flags the craft tells that make CG read cheap (`docs/video/craft/3d-design.md` §6), from the spec alone:
  - default lighting with no `rig`;
  - an orbit over 20°/s;
  - a linear camera that starts and stops dead;
  - a locked-off camera with no `shake`;
  - f/1.4 or wider, or a lens under 20 mm;
  - bloom over 0.15;
  - a floating sphere;
  - more than three hues;
  - chrome on a white cyc with no flag;
  - warm metals under sunset or neon.
- Errors name the exact field with a "did you mean" suggestion: ``objects[2].material: unknown material `crome`; did you mean `chrome`?``.

Reference: `docs/video/components.md#scene3d`. Code: `crates/anikit-video/src/scene3d/`.

## Performance internals (render kernels)

The hot parts live in `scene3d/kernel/`:

- `wide.rs`: 8-wide BVHs (binary SAH collapsed to eight children,
  triangles in SoA blocks of four), traversal with explicit SSE2 / AVX2 slab
  and Möller–Trumbore kernels picked at run time (`ANIKIT_NO_AVX2=1` forces
  the baseline). Both paths compute the same bits as the portable reference
  (unit tested).
- `sbvh.rs`: the mesh hierarchy build. Binned SAH over all three axes with
  spatial splits: a node straddled by long thin triangles (extruded text,
  slabs, bevel strips) is cut at a plane, and those triangles go to both
  sides with clipped boxes, so siblings stop overlapping. Duplicated
  references are capped at 1× the triangle count. Same hits, about 1.8×
  faster rays on text and slabs (`bvh_bench`: text 238 → 128 ns, slab
  151 → 84 ns). Large meshes build in parallel (650k triangles: 1.3 s).
- `post.rs`: the post stack on structure-of-arrays planes, row-parallel,
  hot loops in `simd_fn!` kernels (AVX2 + baseline, same bits).
- Volumetric shafts (`Scene::volumetrics`) march at half resolution with an
  R2 low-discrepancy offset per pixel, so the separable 5×5 binomial blur
  cancels the banding (white-noise offsets left sandy grain in bright
  beams), and each local light's scattering fades out over the outer 40% of
  its reach (no visible sphere where the march stops). Before/after:
  `docs/video/shots/rtk/shafts-before-after.jpg`.
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

Tried and rejected: temporal reuse of refined (edge) pixels. Reprojected
history with an id/depth check and a 3×3 luminance clamp doubled ΔE p99.9 on
`cyc` (14.7 → 31) and raised flicker (42 → 74), because sub-pixel motion
changes an edge pixel's coverage and a clamp built from centre samples
accepts any mix. Limiting reuse to whole-pixel motion removes the error but
also the reuse: every lookdev shot has a moving camera. Speed gain at best
~3%.

Also measured and rejected (scene3d_bench, 4 frames vs the 25-spp references):

- Checkerboarding the reflection ray on glossy floors (roughness 0.03–0.2,
  e.g. the default studio floor at 0.09): chrome ΔE p99.9 4.7 → 21–27, and
  slower once sharp contact reflections are re-traced. Its reflections are
  mostly too sharp for post's gap fill.
- Edge sub-samples with one bounce less (max depth 3 → 2): cyc ΔE 14.8 →
  24, panes flicker 10 → 36, no faster.
- Bounding the next camera-ray traversal by a thin pane's exit face: no
  gain (traversal from inside the pane still enters the nearby boxes).

Where the time goes (`ANIKIT_SCENE3D_PROFILE=1`, 1080p, 4 cores): closest-hit
traversal is 45–60% of trace CPU (camera rays 40–65% of that; glass panes
re-trace per interface), shadow/AO rays are negligible, refinement covers
3–6% of pixels but edge samples cost ~2.6× a centre sample, and post is
140–175 ms (floor reflection, denoise, depth of field, motion blur, bloom
~20–45 ms each). The cheapest quality lever is edge samples: standard's 6
→ 9 costs ~6% CPU on cyc and takes ΔE p99.9 14.8 → 10.3, flicker 42 → 29
(chrome flicker 10 → 5.3); 12 costs ~22% for ΔE 7.4, flicker 21.

Measure every change:

- `cargo run --release -p anikit-video --example scene3d_bench [-- --reference]`:
  ms and CPU ms per frame for the lookdev shots, and SSIM / ΔE p99.9 /
  flicker against converged references (`--reference` writes them:
  `ANIKIT_SCENE3D_REFERENCE=1`, 25 samples per pixel, 4 shadow/AO rays,
  no shortcuts).
- `cargo run --release -p anikit-video --example bvh_bench`: ns per ray.
- `ANIKIT_SCENE3D_DUMP=f.px` + `examples/post_bench`: the post stack alone.
