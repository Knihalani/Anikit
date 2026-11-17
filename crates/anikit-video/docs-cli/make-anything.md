# Make anything: escape hatches

Nothing is limited to the prebuilt pieces. The medium comes from the concept: 2D typography, illustration and paths are first-class, and 3D is for ideas that are physical or material (never a default studio/cyc set with chrome or glass; vary looks between pieces). Combine:

1. **Draw it**: SVG `path` + `trimEnd` draw-on, gradients, masks (`mask`, `clip`), blend modes. Any icon, map, chart or diagram.
2. **Move it**: `"=expressions"` of `t`, `beat`, `level` on any number or colour (`docs expressions`); `script` loops that generate layers (`docs script`); raw `draw` paths and points (`docs draw`); `repeat` + `$i` for fields of objects; keyframes for choreography.
3. **Nest it**: `compositions` with their own clocks, looped, sped up, or mapped onto 3D surfaces and shader channels.
4. **Shade it**: a GLSL `shader` layer for worlds (raymarched scenes, fluids, light), `filter: true` to treat everything below, shader transitions.
5. **Build it in 3D**: `scene3d` objects (primitives, text, glTF), PBR materials, instancing with `=expr` placement, physics, particles; compositions as textures.
6. **Score it**: `music` (styles, keys, hits; acoustic or orchestral unless the concept is electronic) and `sfx` on `@beat` markers.
7. **Finish it**: `post` grade and `motionBlur`.

Components (`info --components`), recipes (`anikit-video recipes`), looks and templates are shortcuts made from exactly these; read any recipe as a starting point, then change it.
