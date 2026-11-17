# Make anything: escape hatches

Nothing is limited to the prebuilt pieces. Combine:

1. **Draw it**: SVG `path` + `trimEnd` draw-on, gradients, masks (`mask`, `clip`), blend modes. Any icon, map, chart or diagram.
2. **Move it**: `"=expressions"` of `t` on any property; `repeat` + `$i` for fields of objects; keyframes for choreography.
3. **Nest it**: `compositions` with their own clocks, looped, sped up, or mapped onto 3D surfaces and shader channels.
4. **Shade it**: a GLSL `shader` layer for worlds (raymarched scenes, fluids, light), `filter: true` to treat everything below, shader transitions.
5. **Build it in 3D**: `scene3d` objects (primitives, text, glTF), PBR materials, instancing with `=expr` placement, physics, particles; compositions as textures.
6. **Score it**: `music` (styles, keys, hits) and `sfx` on `@beat` markers.
7. **Finish it**: `post` grade and `motionBlur`.

Components (`info --components`), recipes (`anikit-video recipes`), looks and templates are shortcuts made from exactly these; read any recipe as a starting point, then change it.
