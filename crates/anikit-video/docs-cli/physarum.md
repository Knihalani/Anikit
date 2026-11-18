# Physarum: slime-mould networks (`physarum` component)

```json
{ "scenes": [ { "duration": 8, "layers": [ { "component": "physarum", "props": {
  "agents": 300000, "resolution": 960, "spawn": "random", "facing": "random",
  "sensor_angle": 22.5, "sensor_distance": 9, "turn": 45, "jitter": 8, "decay": 0.9,
  "colors": ["#1b2a5a", "#3d7bff", "#9fe6ff", "#ffffff"], "exposure": 0.04 } } ] } ] }
```

Agents deposit into a 2D trail field and read it back (Jones 2010). Each
agent senses the field at three points ahead, turns towards the strongest,
steps forward and deposits; the field blurs and decays. Veins form, merge and
re-route. It's deterministic (identical at any thread count) and cached;
frames can render in any order.

- Agents: `agents` (count, up to 8 M) on a field of `resolution` cells along the long side.
- Sensing: `sensor_angle` (deg), `sensor_distance` (cells), `turn` (deg per step), `step` (cells per step), `jitter` (random wobble per step, deg).
  - Sensor distance sets the mesh size: 5 gives fine lace, 20+ gives a few thick trunks.
  - A wider sensor angle than turn makes the network stable; a narrower one keeps it wandering.
- Field: `deposit` per agent per step, `decay` (fraction kept per step: 0.8 thin and fast, 0.95 lingering), `diffuse` (0–1 of a 3×3 blur per step).
- Start: `spawn` is `disc`, `ring`, `random`, `point` or `text`, with `radius` and `center`; `facing` is `out`, `in` or `random`. Symmetric starts stay symmetric, so add `jitter` to break them.
- Text: `text: { text, font, weight, pos, size }`.
  - `spawn: "text"` grows the network from the letters.
  - `attract` (trail added per step inside the letters) pulls the network onto them, so the word appears out of the mesh.
- Edges: `wrap` (true: a torus; false: agents turn back).
- Look: `background`, `colors` (gradient from faint to dense trail), `exposure` (v = 1 − exp(−trail · exposure)).
  - Keep the densest veins below white; 0.03–0.08 for ~1 M agent-steps per frame.
- `speed`: simulation steps per second (60). Cost is about 18 ms per step for 300 k agents at 960 cells on 4 cores (a 6 s still from scratch: 6.7 s).
