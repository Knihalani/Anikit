# anikit (Python)

Python bindings for **Anikit**, a high-performance animation engine with a Rust
core. Animate plain Python objects — dicts, objects, lists of them — with
tweens, keyframes, timelines, springs, inertia and ballistic physics;
interpolate colours in OKLab and 3D CSS transforms like browsers do; measure,
edit, follow and morph SVG paths; simulate thousands of colliding bodies,
ropes and cloth in a 2D physics world; project 3D point clouds; generate
smooth noise; and export animations to CSS `@keyframes` or the Web Animations
API.

All public times are in **seconds**.

```python
import anikit

box = {"x": 0, "opacity": 0, "fill": "#ff0000", "width": "10px"}
anikit.animate(box, {"x": 100, "opacity": [0, 1], "fill": "royalblue", "width": "+=90"},
               duration=0.6, ease="outBack")

for frame, t in anikit.render(fps=30):   # advances the engine, writes values
    print(frame, round(t, 3), box)
```

Differences from the TypeScript API that trip people up:

- Options are keyword arguments in `snake_case` (`repeat_delay`, `on_complete`, `color_space`); a typo raises `TypeError` with a suggestion.
- `timeline.add(targets, props, position, **options)`: the **position is the third argument** (`"-=0.2"`, `"<"`, `"label+=0.1"`).
- `stagger(0.05, from_="center")` (`from` is a Python keyword); an `int` first argument is a count and returns a list: `stagger(5, 0.1)`.
- Nothing moves until you advance time: `anikit.tick(dt)`, `anikit.render(fps)` or `anikit.run_until_idle()`.
- Unknown easings raise `ValueError` with the nearest name; durations of 100 s or more warn (they are usually milliseconds).

Task-sized examples: the Python section of [docs/recipes.md](../../docs/recipes.md#python).

## Building

The package is a [maturin](https://www.maturin.rs) mixed Rust/Python project
(`abi3`, CPython ≥ 3.9).

```sh
cd crates/anikit-py
python -m venv .venv && . .venv/bin/activate
pip install maturin pytest
maturin develop --release      # build + install into the venv
pytest                         # tests + doctests
maturin build --release        # wheel in ../../target/wheels
```

## How it works

The Rust engine owns every animation. Each frame, `tick(dt)` makes **one** call
into the engine, which advances everything and reports which animations
changed; only those values are decoded and written to their targets. Paused,
finished or delayed animations cost nothing.

- Animations you keep a reference to stay replayable; fire-and-forget
  animations release their engine resources once finished and unreferenced.
  `kill()` releases immediately (`engine.node_count` shows live nodes).
- Starting an animation on a property that another animation (or spring)
  already drives takes it over (“auto overwrite”); tweens inside the same
  timeline never cancel each other.
- Callbacks receive the controller if they accept an argument:
  `on_complete=lambda anim: anim.reverse()` or `on_complete=lambda: ...`.

## Easing

```python
from anikit import Easing, ease

ease("outCubic", 0.5)                      # 0.875
Easing("power2.inOut")                     # GSAP names
Easing("back.out(1.7)"), Easing("elastic.out(1, 0.3)")
Easing("cubic-bezier(.2, .8, .2, 1)"), Easing("steps(4, jump-start)")
Easing("linear(0, 0.25 75%, 1)")           # CSS linear()
Easing("wiggle(5)"), Easing("anticipate"), Easing("slowmo(0.7, 0.7)")

s = Easing("spring(bounce: 0.3, duration: 0.6)")   # or "spring(200, 20)"
s.duration          # natural settle time in seconds
s.css()             # 'linear(0,0.0065 1.07%,...)' — a native CSS easing
s.sample(60)        # 60 samples over [0, 1]

Easing(lambda t: t ** 3)                   # any Python function (sampled to a table)
```

### Custom, composed and preset easings

```python
Easing("custom(M0,0 C0.5,0 0.5,1 1,1)")    # "CustomEase": draw the curve as SVG path data
Easing.from_path("M0,0 C0.2,0 0.3,1.2 0.6,1 0.8,0.9 1,1")   # may overshoot
# (y up from (0,0) to (1,1); paths in other ranges / y-down are normalised by their end points)

Easing("reverse(inQuad)")                  # 1 - f(1 - t) → same as outQuad
Easing("mirror(outBack)")                  # there and back within one iteration
Easing("blend(inQuad, outBounce, 0.3)")    # a·0.7 + b·0.3 (weight defaults to 0.5)
Easing("inCubic").reversed(), Easing("outBack").mirrored(), Easing.blend("linear", fn, 0.2)

Easing("rough(20, 1, 7, outQuad)")         # jitter: points, strength, seed, template
Easing.rough(points=30, strength=2, seed=7, template="outQuad")   # deterministic per seed

Easing("expoScale(1, 10)")                 # zoom 1 → 10 at a perceptually constant rate
Easing.expo_scale(1, 10)

Easing("spring.wobbly")                    # presets (also bare: "wobbly"):
# gentle, wobbly, stiff, slow, molasses, snappy, bouncy
```

All of these are plain easing strings, so they work anywhere an easing is
accepted (`animate(..., ease="mirror(outBack)")`, keyframes, timelines, CSS
export — non-native curves become `linear()`).

## Colours

```python
from anikit import color

color.parse("hsl(200 50% 40% / .5)")      # (r, g, b, a) — rgb 0–255, alpha 0–1
color.mix("red", "blue", 0.5)             # OKLab by default: 'rgb(140,83,162)'
color.mix("red", "blue", 0.5, space="srgb")
color.to_hex("rgb(255 128 0)")            # '#ff8000'
color.to_css((0, 0, 0, 0.25))             # 'rgba(0,0,0,0.25)'
```

Spaces: `oklab`, `oklch`, `srgb`, `linear-srgb`, `hsl`. Alpha is premultiplied,
so fading to `transparent` never passes through grey.

## Noise

```python
from anikit import Noise

n = Noise(seed=7)                         # same seed → same field
n.noise2(x, y), n.noise3(x, y, t)         # smooth simplex noise, ≈ [-1, 1]
n.fbm2(x, y, octaves=5, lacunarity=2, gain=0.5)   # fractal detail (also fbm3)

# Vectorised: one native call for a whole list; numbers broadcast.
xs = [i * 0.05 for i in range(1000)]
wobble = n.noise2_many(xs, 0.0)           # also noise3_many, fbm2_many, fbm3_many
field = n.grid(64, 64, step=0.05, z=t, octaves=3)   # 64 rows × 64 values

# Typical use: organic idle motion driven by time.
anikit.animate(leaf, {"x": 100}, duration=2, ease=lambda t: t + 0.05 * n.noise2(t * 4, 0))
```

## Paths, motion and morphing

```python
from anikit import Path, morph, motion_path

p = Path("M0 0 C50 -50 100 50 150 0")
p.length, p.bounds                  # arc length, exact bounding box
p.point_at(0.5)                     # (x, y, tangent angle in degrees)
p.to_svg()                          # normalised absolute M/C/Z path data
Path.circle(50, 50, 40), Path.rect(0, 0, 100, 50, rx=8), Path.star(0, 0, 5, 50, 20)

m = morph(Path.rect(0, 0, 100, 100), Path.circle(50, 50, 50))
m.at(0.5)                           # path string half-way
m.frames(30, ease="inOutCubic")     # 30 path strings

# Animate along a path / between shapes:
anikit.animate(sprite, {"motion": motion_path(p, rotate="angle")}, duration=2)
anikit.animate(icon, {"d": [Path.rect(0, 0, 10, 10), Path.circle(5, 5, 5)]})
```

### Path operations

```python
p = Path.through([(0, 0), (50, 80), (120, 20), (200, 60)], curviness=1)   # smooth spline
Path.through(points, closed=True)          # closed loop; curviness=0 → straight lines

p.trim(0.2, 0.6)                           # the part between 20% and 60% of the length
p.flatten(tolerance=0.25)                  # [[(x, y), ...], ...] one polyline per subpath
p.simplify(1.0)                            # Ramer–Douglas–Peucker → lines
p.simplify(1.0, smooth=True)               # … → smooth spline through the kept points
Path.rect(0, 0, 30, 20).area               # 600.0 (signed: clockwise on screen > 0)

p.point_at_length(42)                      # (x, y, angle) 42 units along the path
p.closest_point(60, 70)                    # (progress, x, y, distance) — e.g. drag along a path
p.sample_points(20)                        # 20 (x, y) points evenly spaced by distance

# "Draw-on" effect: grow the visible part of a path.
o = {"end": 0.0}
anikit.animate(o, {"end": 1}, duration=1,
               on_update=lambda: setattr(svg, "d", p.trim(0, o["end"]).d))
```

`closest_point` and the distance queries use the path's lookup table, so they
are accurate to a small fraction of a unit (sharp corners are cut by less than
one table step).

### Morphing through several shapes

```python
from anikit import morph, morph_many

seq = morph(square, circle, star)          # 3+ shapes → MorphSequence (2 → Morph)
seq = morph_many([square, circle, star])
seq.at(0.5)                                # path data at the circle
seq.frames(60, ease="inOutSine")           # 60 path strings through all shapes

anikit.animate(icon, {"d": [square, circle, star]}, duration=2)         # evenly spaced
anikit.animate(icon, {"d": seq}, duration=2)
anikit.animate(icon, {"d": [None, {"value": circle, "offset": 0.3},     # None = current shape
                            {"value": star, "ease": "outBack"}]})
```

All shapes are normalised to one structure (subpaths padded, segments
subdivided, closed shapes rotated to line up with the previous one), so a
multi-shape morph is a single numeric track whose keyframes are the shapes —
per-keyframe `offset` and `ease` work as for any other property.

## Stagger

```python
from anikit import stagger

stagger(5, 0.1)                            # [0, .1, .2, .3, .4]
stagger(5, 0.1, from_="center")            # [.2, .1, 0, .1, .2]
stagger(9, 0.05, grid=(3, 3), from_="center", ease="inQuad")
stagger(10, total=1.0, from_="random", seed=3)

anikit.animate(dots, {"y": -20}, delay=stagger(len(dots), 0.05))
anikit.animate(dots, {"y": -20}, delay=stagger(each=0.05))   # sized lazily
```

## `animate`

```python
anim = anikit.animate(
    targets,            # dict, object, or a list of them
    props,              # {"name": value, ...}
    duration=0.5, delay=0, ease="outQuad",
    repeat=0,           # -1 = forever
    yoyo=False, repeat_delay=0, speed=1, autoplay=True,
    color_space="oklab",
    on_start=None, on_update=None, on_complete=None, on_repeat=None,
)
```

Property values:

| value | example | result written |
| --- | --- | --- |
| number | `100` | float (from the current value) |
| unit string | `"100px"`, `"45deg"`, `"50%"` | string with the unit |
| relative | `"+=10"`, `"-=5px"`, `"*=2"`, `"/=2"` | relative to the current value |
| colour | `"#0af"`, `"rebeccapurple"`, `"oklch(70% .1 200)"` | CSS colour string (hex if the target value is hex) |
| string with numbers | `"0px 2px 10px rgba(0,0,0,.5)"` | every number interpolates |
| keyframes | `[0, 100]`, `[None, 120, 80]`, `[{"value": 1, "ease": "inQuad", "offset": .3}]` | `None` first = current value |
| from/to dict | `{"from": 0, "to": 1, "ease": "linear"}` | |
| callable | `lambda target, index, total: index * 10` | evaluated per target |
| `"transform"`: CSS transform lists | `["rotateY(0deg)", "rotateY(180deg) translateZ(100px)"]` | `matrix3d(...)` string |
| `transform.Matrix` (any key) | `Matrix.rotate_z(90)` | `Matrix` object |
| `Path` / `motion_path(...)` | | `(x, y, angle)` / separate keys |
| two `Path`s / `morph(a, b)` | | path data string |
| anything else | `True`, `"hidden"` | switches half-way (discrete) |

`duration`, `delay` and `repeat_delay` accept per-target lists (e.g. a stagger)
or callables. A keyframe's `ease` applies to the segment arriving at it; the
animation's `ease` applies to every other segment.

Controller: `play()`, `pause()`, `resume()`, `reverse()`, `restart()`,
`seek(t)`, `complete()`, `kill()`, and `progress` (settable), `time`
(settable), `duration`, `speed`, `state` (`"idle" | "playing" | "paused" |
"finished"`), `finished`, `is_active`, `reversed`.

## Timelines

```python
tl = anikit.timeline(defaults={"duration": 0.4, "ease": "outCubic"}, repeat=1, yoyo=True)
(tl.to(card, {"y": 0, "opacity": [0, 1]})
   .to(card, {"scale": 1.1}, "-=0.1")        # overlap the end of the timeline
   .label("shine")
   .to(glow, {"opacity": 1}, "shine+=0.2")
   .set(card, {"visible": True}, "<")          # zero-duration
   .call(print, ">", "done"))
```

Positions: absolute seconds; `">"` end of the previous child (**the default**);
`"<"` its start; `">0.2"`, `">-0.1"`, `"<0.2"`, `"<+=0.2"`, `"<50%"`;
`"+=0.5"` / `"-=0.2"` relative to the end of the timeline; `"label"`,
`"label+=0.3"` (unknown labels are created at the end). Consecutive tweens of
the same target property chain: when `from` is omitted, a tween starts where the
previous one in the timeline ends. Animations and timelines can be nested with
`tl.add(other, position)`.

## Springs and inertia

```python
s = anikit.spring(ball, {"x": 200, "y": 40}, stiffness=170, damping=26)
anikit.spring(ball, {"x": 0})           # retargets the running spring, keeping its velocity
anikit.spring(ball, {"x": "+=50"}, bounce=0.4, duration=0.5)
s.velocity("x"), s.finished

anikit.inertia(card, {"x": {"velocity": 900, "min": 0, "max": 500}})   # bounces off bounds
anikit.inertia(card, {"x": {"velocity": 900, "end": [0, 250, 500]}})    # snaps to the nearest
```

Springs use the exact closed-form solution, so they are stable at any frame rate.

## Physics (ballistic motion)

```python
p = anikit.physics(
    ball,
    {
        "x": {"velocity": 300, "min": 0, "max": 800},                    # walls
        "y": {"velocity": -600, "acceleration": 1500, "max": 400},       # gravity + floor
    },
    friction=0.2,          # exponential drag (1/s); terminal velocity = acceleration / friction
    restitution=0.5,       # bounce energy kept: 0 = stop dead … 1 = perfectly elastic
    life=None,             # seconds; None = until at rest
    on_update=None, on_complete=None,
)
p.velocity("y"), p.acceleration("y"), p.finished
p.set_acceleration({"x": 50})              # wind / change gravity while running
p.stop()

anikit.physics(confetti, {"y": lambda t, i, n: {"velocity": -400 - 40 * i, "acceleration": 900}},
               life=3)                     # per-target values, fixed lifetime
```

A plain number is the initial velocity (units/s); a dict accepts `velocity`,
`acceleration`, `min`, `max` and per-property `friction` / `restitution`.
Without `velocity`, a simulation already driving the property (spring,
inertia or physics) hands over its velocity — and `spring()`/`inertia()`
likewise continue from a physics simulation's velocity. Integration is exact,
so results do not depend on the frame rate.

A simulation finishes when it is at rest (slower than `rest_speed`, default
0.05 units/s, with no acceleration or lying on a bound it is pushed against)
or when `life` runs out. Motion that is accelerated with no bound in its way,
or bounces elastically without friction, never rests — give it a `life`.

## Physics world (2D)

A native, position-based 2D physics world: circles, static boxes, distance
constraints (ropes, cloth, soft bodies), attractors/repulsors and bounds.
Collisions use a grid broadphase and sub-stepping, so tens of thousands of
bodies run in real time; stepping releases the GIL.

```python
from anikit import World

world = World(gravity=(0, 980), bounds=(0, 0, 800, 600),   # all keyword-only:
              damping=0.05, substeps=4, iterations=2,      # drag/s, solver quality
              restitution=0.2, wall_restitution=0.3, friction=0.05)

ball = world.add_body(400, 50, radius=12, mass=1, velocity=(200, 0))   # -> Body handle
ball.position, ball.velocity, ball.rotation      # rotation: rolling angle (degrees)
ball.push(0, -500); ball.move_to(100, 100); ball.pinned = True
world.add_body(400, 500, radius=40, mass=0)      # mass <= 0: static
world.add_box(100, 400, 300, 420)                # static platform

# Bulk: one native call, no per-body Python objects.
ids = world.add_bodies([(x, 0, 4) for x in range(0, 800, 10)])  # (x, y[, r[, mass]]) -> range
world.add_bodies(array("f", flat_xyrm))          # or flat [x, y, r, mass] * n
world.step(1 / 60)                               # dt > 1/20 s is split into equal steps
world.run(2.0, fps=60)                           # fixed-step simulation, one native call
pos = world.positions()                          # array('f') [x0, y0, x1, y1, ...]
out = world.output()                             # array('f') [x, y, angle_rad] * n
world.settle()                                   # run until everything is at rest

# Constraints: ropes, cloth, custom links.
rope = world.rope([(400 + 10 * i, 100) for i in range(30)])   # first body pinned
rope.end.push(300, 0); rope.length, rope.points(), rope.broken
cloth = world.cloth(20, 15, spacing=12, origin=(200, 50), pin_top=True, tear=2.5)
world.connect(a, b, length=None, stiffness=1, tear=0)        # None: current distance

# Forces.
mouse = world.attract(x, y, strength=3000, radius=200)       # live: mouse.move_to(x, y)
world.repel(400, 300, strength=5000, radius=150)
world.impulse(400, 300, radius=200, strength=800)            # explosion
world.pick(x, y)                                             # Body under the pointer or None

# Drive Python objects: written after every step.
world.bind(ball, sprite)                                     # sprite.x / .y / .rotation
world.bind(ball, el, x="left", y="top", rotation=None, offset=(-12, -12))
world.bind_many(ids, sprites)
for frame, t in world.render(fps=30, duration=3):            # frame 0 = current state
    draw(sprites)
```

Configuration attributes (`world.gravity`, `world.bounds`, ...) can be
changed while the world runs. Bodies resting on bounds keep a tiny residual
velocity from the wall restitution, so rest checks should use a small speed
threshold (`settle(speed=5)`) rather than zero energy.

## 3D transforms and projection

`anikit.transform` works with 4×4 matrices in CSS `matrix3d()` order
(column-major, px, degrees):

```python
from anikit.transform import Matrix, Quaternion, Camera, mix_transforms, project_points

m = Matrix.from_css("perspective(800px) translateZ(-50px) rotateY(30deg)")
m.to_css()                       # 'matrix3d(...)'  (to_css(prefer_2d=True) → 'matrix(...)' when 2D)
m @ Matrix.rotate_x(10)          # multiply: like writing "m rotateX(10deg)" in CSS
m.invert(), m.transform_point((10, 20, 0)), m.determinant
m.decompose()                    # {'translate', 'scale', 'skew', 'perspective', 'quaternion', 'rotate'}
Matrix.recompose(parts)
Matrix.translate(x, y, z), Matrix.scale(2), Matrix.rotate((1, 1, 0), 45), Matrix.skew(10, 0)

mix_transforms("rotateY(0deg)", "rotateY(180deg) translateZ(100px)", 0.5)   # 'matrix3d(...)'
Matrix.from_css(a).mix(b, t)     # browser interpolation: decompose → lerp + slerp → recompose

q = Quaternion.from_euler(30, 45, 0)          # rotateX(30deg) rotateY(45deg)
q.slerp(Quaternion.from_axis_angle((0, 1, 0), 90), 0.5).to_matrix()
q.axis_angle(), q.to_css()                    # (x, y, z, deg), 'rotate3d(...)'
```

Bulk projection of point clouds (native, radix depth sort, GIL released):

```python
cam = Camera(position=(0, 0, 800), target=(0, 0, 0), fov=50)
cam.orbit(yaw=t * 30, pitch=20)
proj = project_points(points, camera=cam, size=(640, 480), model="rotateY(20deg)")
proj.points                      # array('f') [screen_x, screen_y, depth, scale] * n
proj.order                       # array('I') indices far → near (painter's order)
for i, x, y, depth, scale in proj.sorted():   # visible points, far → near
    draw_circle(x, y, radius * scale)
```

`points` may be `(x, y, z)` tuples, a flat list / `array('f')` (with `stride`
and channel `offsets`), or raw `f32` bytes. Points behind the camera get
`scale == 0`. Use a `Projector()` to reuse buffers between frames.

### Animating 3D transforms

The `transform` property accepts CSS transform lists; they are interpolated
like browsers interpolate mismatched transform lists (decomposition + quaternion
slerp) and written back as `matrix3d(...)` strings. `Matrix` keyframes work on
any property and write `Matrix` objects.

```python
card = {"transform": "none"}
flip = anikit.animate(card, {"transform": ["rotateY(0deg)", "rotateY(180deg) translateZ(100px)"]},
               duration=0.8, ease="inOutCubic")
anikit.animate(card, {"transform": "rotateX(60deg) scale(1.2)"})      # from the current value
anikit.animate(node, {"matrix": [Matrix(), Matrix.rotate_y(90)]})     # Matrix objects

flip.to_css_keyframes("flip", ".card")   # transform: matrix3d(...) keyframes (+ x/y shorthands first)
```

Because the interpolation goes through matrices, rotations take the shortest
arc between keyframes: `rotate(0deg)` → `rotate(360deg)` does not spin — add
intermediate keyframes (`0deg`, `120deg`, `240deg`, `360deg`) for full turns.
Strings that are not transform lists keep the generic number-template
interpolation.

## Point clouds and glow rendering

`PointCloud` keeps its particles natively; each one spring-follows a blend
between two target shapes with its own timing (so transitions ripple) plus a
turbulence field. `Splat` renders projected points as additive light (sub-pixel
splats, bokeh discs, fog, lens, trails, tone mapping) into RGBA bytes.

```python
import math, anikit
from anikit.transform import Camera

def sphere(i, n, r=200):
    y = 1 - 2 * (i + 0.5) / n
    a = i * 2.39996
    return (r * math.cos(a) * math.sqrt(1 - y * y), r * y, r * math.sin(a) * math.sqrt(1 - y * y))

cloud = anikit.PointCloud(20_000, shapes=[sphere, [(x, 0, 0) for x in range(-10_000, 10_000)]],
                          stiffness=60, damping=9, spread=0.6, noise=2, swirl=40)  # all live properties
cloud.shape(2, flat_xyz_array)          # (x, y, z) tuples, flat list / array('f'), bytes or fn(i, n)
cloud.morph(1, duration=2.0, ease="inOutCubic")   # step() advances the blend...
cloud.mix = scroll_progress             # ...or drive it yourself (cloud.from_, cloud.to, cloud.mix)
cloud.step(1 / 60)                      # also step(dt, from_, to, mix), run(seconds), snap(k)
cloud.explode(0, 0, 0, strength=400)    # radial kick; the springs bring particles home
cloud.positions, cloud.speed            # array('f') [x, y, z] * n, mean speed

glow = anikit.Splat(640, 360, background="#05060a", exposure=1.0, trail=0.6)
glow.colors(lambda i: (0.5, 0.6, 1.0), count=len(cloud))     # or (r, g, b) tuples / CSS strings
frame = glow.render(cloud, camera=Camera(position=(0, 0, 900), fov=40),
                    size=1.4, intensity=0.25, fog=(600, 1300), lens=(320, 180))  # RGBA bytes
```

`render()` is project → draw → resolve; for control, call
`glow.draw(project_points(...))` (a `Projection` or flat `[sx, sy, depth,
scale] * n`) any number of times, then `glow.resolve()`. Frames are row-major
RGBA `bytes` of `width * height * 4`; with Pillow, `Image.frombytes("RGBA",
glow.size, frame)` turns one into an image (see the GIF recipe below).

## Solid 3D, camera flights and the film grade (numpy)

`anikit.raster` is the same CPU renderer as the TypeScript `renderer3d`:
z-buffered, instanced, per-pixel lit triangles (chrome reflections,
iridescence, glowing UV lines and pulses, fog), mirror floors and light
points. `anikit.video` is the composition's film grade. Both need numpy
(`pip install anikit[numpy]`); frames are `(height, width, 4)` uint8 arrays.

```python
import numpy as np
import anikit
from anikit import raster, video
from anikit.transform import Camera

r3 = raster.Renderer3D(1280, 720)
knot = raster.torus_knot(p=2, q=3, radius=300, tube=58, segments=600, sides=28)
towers = raster.instances(900, x=xs, z=zs, sy=heights, sx=80, sz=80, color=(0.04, 0.045, 0.06))  # vectorised
towers.positions[:, 1] = heights / 2                       # (n, 15) float32 rows, edit in place

cam = Camera((0, 300, 2000), (0, 0, 0), fov=45)            # Camera(..., roll=deg) banks the view
frame = r3.render(cam, [
    raster.Item(knot, material=raster.Material(metal=0.9, iridescence=0.25, specular=1.4, shininess=120,
                                               lines=1.2, line_color=(0.3, 0.8, 1.6), grid=(0, 6),
                                               pulse=2.5, pulse_count=3, pulse_phase=t)),
    raster.Item(raster.cube(), towers, {"lines": 1.6, "grid": (1, 1), "line_color": (1.4, 0.35, 0.9)}),
    raster.Item(raster.plane(), raster.instances(1, y=-400, scale=20000), mirror=True),   # the floor
    raster.Points(stars, colors=star_rgb, size=3, intensity=2.5, streak=1.5),            # (n, 3) arrays
    raster.Item(beam, beams, {"emissive": 1}, blend="additive"),
], background="#04050a", mirror=-400, fog=(1800, 5200),
   sky=(0.05, 0.07, 0.16), horizon=(0.9, 0.5, 0.95), strips=(2.2, 2.3, 2.6))

video.post(frame, bloom=0.8, bloom_threshold=0.55, streaks=0.5, vignette=0.45, grain=4, seed=i)  # in place
```

- Meshes: `cube()`, `sphere(rings, segments)`, `torus(...)`, `torus_knot(p, q, radius, spread, tube, segments, sides)`, `plane(cells)`, `cylinder(segments, caps)`, `tube(points, radius=10, sides=8, radii=None, taper=None)` (light trails along a path; `taper=(0, 1)` for a comet), `heightfield(w, d, heights | f(x, z), skip=)`, `custom(positions, normals, indices, uvs)`; `mesh.positions` / `.indices` read them back.
- `Material` has every core field (`ambient` … `rim`, `emissive`, `metal`, `iridescence`, `hue`, `lines`, `line_color`, `grid`, `line_width`, `pulse*`, `texture`, `texture_mode`, `texture_repeat`, `texture_gain`, `pattern` = `"grid"` | `"hex"` | `"dots"` | `"circuit"` for what `lines` draws); `raster.texture(image)` uploads an `(h, w, 4)` array. Lighting is `Lighting(...)` or keyword arguments of `render` (including `ground_fog=(bottom, top, range)`: fog in `fog_color` hugging the floor).
- `mirror=y` (or `Mirror(y, strength)`) draws the scene reflected about `y`, then items with `mirror=True` glossily over it, then the scene — exactly the TypeScript pass. `reflect=False` keeps an item out of the reflection.
- Render passes, in the TypeScript order: `shadows=raster.Shadows(extent, center=, size=2048, strength=0.8, bias=0.0015)` (or an extent / dict; key-light shadow map from opaque items, `Item(cast_shadow=False)` opts out), `mirror`, opaque items, `skybox=True | sun_size` (environment gradient + sun behind the geometry), then additive items and points (light goes last, onto the sky too), then `dof=focus | dict(focus=, aperture=16, max=)`, then `motion_blur=True | shutter | dict(shutter=0.5, max=)` against the previous render's camera (`r3.reset_motion()` after a cut).
- `raster.shatter(mesh, transform=dict(rx=20, scale=1.2), force=900, spin=540)` breaks a mesh into shards; `shards.update(t)` poses them (numpy-vectorised, same numbers as TypeScript); draw `shards.item()`.
- `anikit.flight_path(points, look_ahead=, bank=, max_bank=, closed=)`: `.point(s)`, `.at(s)` (eye, target, roll, forward), `.apply(camera, s, extra_roll)`, `.timing([(sec, fraction), …])`, `.points(n, from_=0, to=None)` (evenly spaced, e.g. for `raster.tube`), `.control_points` — a pure-Python port with the TypeScript numbers.
- `video.post(frame, aa=1)` adds FXAA-style edge antialiasing (recommended for raster frames), alongside bloom, streaks, `rays`, zoom blur and the grade. `video.PostFx(**options)` keeps scratch buffers; options may be functions of time (`grade(frame, t=..., seed=i)`). `video.rgba_to_i420(frame)` feeds encoders and `video.VideoWriter(path, w, h, fps)` writes MP4s through ffmpeg (`write(frame)`, `close()`, or `with`); `video.MotionBlur(w, h)` averages sub-frames. `flight_path(...).apply(cam, s, shake=4)` adds a handheld wobble.
- `render()` returns a new array backed by one copy of the colour buffer; grading such a frame is zero-copy. Instance arrays are bytearray-backed, so a render passes them to the core without an extra Python-side copy. See `examples/knot_mirror.py`.

## Driving the engine

```python
anikit.tick(1 / 60)                         # advance the default engine
anikit.run_until_idle(fps=60)               # simulate to the end (realtime=True paces with sleep)
for i, t in anikit.render(fps=30, duration=2.0):   # frames for t = 0, 1/30, ..., 2.0
    ...

engine = anikit.Engine()                    # independent world
engine.animate(obj, {"x": 1}); engine.tick(0.016)   # also engine.spring/inertia/physics
engine.speed = 0.5                          # global time scale
```

### Rendering a GIF with Pillow

Pillow is not a dependency; install it separately.

```python
import anikit
from PIL import Image, ImageDraw

ball = {"x": 20.0, "y": 20.0, "r": 10.0, "fill": "#ff3366"}
tl = anikit.timeline(defaults={"duration": 0.6})
tl.to(ball, {"x": 280, "fill": "#33aaff"}, ease="outBounce")
tl.to(ball, {"y": 130, "r": 20}, "<", ease="spring(bounce: 0.4, duration: 0.6)")

frames = []
for _, _t in anikit.render(fps=30):
    im = Image.new("RGB", (300, 150), "white")
    x, y, r = ball["x"], ball["y"], ball["r"]
    ImageDraw.Draw(im).ellipse((x - r, y - r, x + r, y + r), fill=ball["fill"])
    frames.append(im)
frames[0].save("ball.gif", save_all=True, append_images=frames[1:], duration=33, loop=0)
```

## Export to the web

```python
a = anikit.animate({"opacity": 0, "y": 20}, {"opacity": 1, "y": 0},
                   duration=0.5, ease="spring(bounce: 0.2, duration: 0.5)", autoplay=False)
print(a.to_css_keyframes("enter", ".card"))
# @keyframes enter {
#   0% { opacity: 0; transform: translateY(20px); animation-timing-function: linear(0,...); }
#   100% { opacity: 1; transform: translateY(0px); }
# }
# .card {
#   animation: enter 0.5s linear 0s 1 normal both;
# }
keyframes, timing = a.to_waapi()            # for element.animate(keyframes, timing)
```

Keyframes are emitted at the animation's own offsets with each segment's easing
as a native CSS timing function (springs and bounces become `linear()`); when
properties use different keyframes or easings, values are sampled densely
instead (`samples=` forces this). `x`/`y`/`rotate`/`scale`/... are composed into
`transform` (followed by an animated `transform` property's `matrix3d(...)`);
`background_color` becomes `background-color`.

## Low-level API

`anikit._anikit` exposes the engine directly (milliseconds, integer handles,
flat float lists) — see `_anikit.pyi`. `Engine.core` gives access to the native
engine behind a high-level `Engine`.

```python
from anikit import _anikit as ak
core = ak.Engine()
h = core.create_anim([(ak.TRACK_NUMBER, 2, [0, 0, 100, 50], [], [core.easing("outQuad")], 0, None)],
                     duration=1000)
core.play(h)
updated, values, events = core.step(16.0)   # tick + changed handles + their values + events
```

Track kinds: `TRACK_NUMBER`, `TRACK_COLOR`, `TRACK_DISCRETE`, `TRACK_PATH` and
`TRACK_MATRIX` (16 `matrix3d` channels per keyframe). The physics world and
projector are `NativeWorld` / `NativeProjector`, point clouds and splatting
`NativeCloud` / `NativeSplat` (indices, flat native-endian `f32` bytes); matrix helpers are `transform_*`, `mat4_*` and `quat_*`.
