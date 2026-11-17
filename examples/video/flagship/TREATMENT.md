# EVERY FRAME — flagship launch film for Anikit Video

72 s · 1920×1080 · 60 fps · one continuous camera move · H.264 + AAC from a single spec

## 1. Logline

A video is just thousands of still pictures. This film follows one short written request as a low aluminium
machine turns it into 3,600 of them, laid out across a bright studio floor, in the time the fastest browser
setup manages 175, then packs them into one MP4 that plays in your hand. You come away knowing what Anikit
Video does and feeling how fast it is.

## 2. Concept

**Video is a strip of glass frames, and we run alongside it.**

The whole film is one unbroken camera journey through a single bright, physical place: an infinite white
photo studio (a seamless cyclorama). Every frame of a video is a real object here, a thin pane of glass with
a picture printed on it. That one idea carries the whole story and needs no prior knowledge:

- **Beginning (the request).** A single pane floats in the empty studio. On it, an agent's request in plain
  words: *"Make a 60-second launch film for Sundial. 1080p, 60 fps."* The pane turns over: on the back is
  the plan the agent wrote, a short JSON spec. It drops, like bread into a toaster, into the top slot of a
  low aluminium machine on the floor.
- **Middle (the frames).** The machine's light blinks and it rolls forward, leaving a pane standing where the
  plan went in: frame 1, a finished picture. Then 2, 3, 4… then it accelerates away towards the horizon, laying
  frames behind it like track: a strip of 3,600 upright glass panes standing side by side, with a counter.
  The camera chases it, then drops down beside the finished strip and catches up with a second machine on
  a parallel lane: a browser window on a cart, the way other tools do it, flashing a screenshot per frame.
  It started at the same moment and is still crawling at a few hundred frames. Labels stand by the strip:
  *all 3,600 frames in 3.4 s* vs *fastest browser setup: 69.1 s*.
  Then the camera turns to face our strip and accelerates alongside it until it passes exactly one pane
  per frame of video. At that speed the panes lock still on screen and the Sundial film **plays**, like a
  film running through a projector gate. The frames *are* the video.
- **End (the file).** The camera brakes; the whole strip folds up like an accordion into a stack that
  rushes in from the horizon, then crushes down into one small glass tile, the MP4, etched with its name.
  It slides into a phone and plays. The camera pulls back until the whole studio fits in frame: *this film
  was written as one spec* (and, if the measured time is impressive, *rendered in N seconds*). Chrome logo
  on the studio floor.

Not a slide in sight: every caption is a physical object or etched type that lives in the studio and gets
passed by the camera. The film never cuts; transitions are camera moves.

## 3. Look

**"Daylight lab."** Bright, clean, physical: the opposite of every dark-purple glow film before it.

| | |
|---|---|
| **Set** | Infinite warm-white cyclorama: floor curving up into the wall with no horizon line. Faint 1 m floor grid etched at 6% contrast, visible only at grazing angles. Gentle exponential depth haze to warm white so the frame line dissolves into the distance. |
| **Palette** | Studio white `#EEEBE5` · shadow taupe `#B9B2A7` · ink `#15171A` (type, machine details) · **signal orange `#FF5A1F`** (the one accent: status LED, frame counter, Anikit's line) · glass-edge green `#9FD8C8` (the natural tint of thick glass edges) · browser line in cool neutral grey `#8C939C` so it reads as "the other one" without being villainous. No purple, no neon, no black backgrounds until the last 6 s. |
| **Materials** | Frames: 8 mm low-iron glass, rounded corners, picture printed on the back face (refraction shows through the thickness), polished green-tinted edges catching the key light. Machine: bead-blasted aluminium, chamfered edges, one orange LED, a black glass slot. Browser windows: matte white plastic with a printed browser bar. Final MP4 tile: thick optical glass with the 3,600 frames visible as faint strata inside. Logo: polished chrome on the floor, reflecting the cyc. |
| **Light** | One huge soft key from top-left (a 6 m softbox), a warm bounce from the floor, a thin rim from behind for glass edges. Soft contact shadows under every pane; caustic-like bright edges where glass meets light. Real studio reflection map in chrome and glass. |
| **Camera** | Real lens behaviour: 35 mm for travelling, 85 mm for the macro moments, shallow DOF (focus pulls are part of the storytelling), motion blur on fast moves only. Moves are slow and weighted, a motion-control rig, except one deliberate rush (shot 6). |
| **Type** | Inter Display, tight tracking, ink on white, very large and confident; JetBrains Mono for the spec and file label. Captions are lit 3D objects: thin extruded ink letters standing on the floor or etched into glass, never overlays. One exception: the small frame counter, flat orange, locked to the line. |
| **References** | Apple product films in white studios (weight, clean light); Teenage Engineering and Braun product photography (the machine); Muybridge and the zoetrope (frames becoming motion); Stripe Sessions stage design (bright, precise); tilt-shift photography for the crane-up reveal. |

## 4. Shot list

One continuous camera move; "shots" are beats of it. Beat = 0.5 s at 120 BPM (30 frames); bar = 2 s.

| # | Time (dur) | On screen | Camera | The viewer understands | Shows off |
|---|---|---|---|---|---|
| 1 | 0:00–0:06 (6 s) | Empty white studio. One glass pane hovers at eye height, soft shadow below. On it, a chat bubble: *"Make a 60-second launch film for Sundial. 1080p, 60 fps."* | 85 mm, slow push-in from 3 m; focus rack from the blurred floor grid to the words. | Someone has asked for a video, in plain words. | Glass PBR, studio env, soft shadow, DOF rack, UI as a texture on a 3D plane. |
| 2 | 0:06–0:12 (6 s) | The pane turns over on a spring. The back holds 18 lines of JSON (scenes, text, a 3D model, music); lines highlight in sequence. Standing type beside it: **"An agent writes the plan."** | 90° orbit round the pane while dollying back to 35 mm framing. | The request has become a short written plan the computer can follow. | Camera orbit, refraction through glass, code component on a 3D plane, 3D kinetic type. |
| 3 | 0:12–0:20 (8 s) | The pane rises and drops, edge first, into the top slot of a low aluminium machine (LED blinks orange). The machine rolls forward 1.7 m and leaves a pane standing where the plan went in: **frame 1** of the Sundial film, a finished, designed picture. Standing type: **"Anikit Video turns it into frames."** | Descending three-quarter shot onto the machine, then low and close on frame 1; focus hands off from the spec to frame 1. | A machine reads the plan and produces the video's pictures, one at a time. | Aluminium and emissive PBR, physical motion, UI-in-3D, focus hand-off. |
| 4 | 0:20–0:30 (10 s) | Frames 2, 3, 4 on the beat, then the machine accelerates away to the horizon laying a strip of upright panes side by side. Orange counter **0 → 3,600 / 3,600 frames**. A grey browser cart on a parallel lane starts at the same moment and falls far behind. | Rise and chase: the camera climbs and flies after the machine; the strip streams beneath. | A video is thousands of still pictures, and this machine makes them extremely fast. | 3,600 instanced glass panes, motion blur, depth haze, counter, chase move. |
| 5 | 0:30–0:40 (10 s) | The camera drops down beside our finished strip and catches the browser cart, still working: each frame it opens, draws and *flashes a screenshot*; its counter reads ~200 / 3,600. Type standing by our strip: **"Anikit Video: all 3,600 frames in 3.4 s"**, on the cart **"Fastest browser setup: 69.1 s"**, then **"No browser. 21× faster."** | Low tracking shot at the cart's speed, our strip in the foreground, the cart beyond. | Other tools screenshot a web browser frame by frame; Anikit doesn't need one and is far faster. | Browser UI on 3D planes, shutter flashes, standing 3D type, tracking camera. |
| 6 | 0:40–0:50 (10 s) | The camera turns to face our strip and accelerates alongside it. Panes sweep past, then at exactly one pane per video frame they lock still and the Sundial launch film plays in them (a chrome-and-glass device turning, its app UI, a chart rising, the word **Sundial**), while the floor streaks past. | 35 mm, 2.4 m from the strip at pane height, accelerating to 102 m/s (1.7 m per frame at 60 fps), shutter closing as it locks. | Those frames *are* the video: run past them at the right speed and it plays. | The signature move: the projector-gate lock, motion blur, a film within the film (3D product, UI, chart, kinetic type). |
| 7 | 0:50–0:58 (8 s) | The camera brakes. The strip folds like an accordion: every pane turns 90° and they rush in from the horizon into one long stack, which crushes down into a block and then a matchbox-sized glass tile with faint strata inside. Etched: **"sundial-launch.mp4 · H.264 · 1920×1080 · 60 fps"**; standing type: **"Its own H.264 encoder. One file."** | Hold as the stack arrives; slow arc down onto the tile. | All the frames get packed into one ordinary video file. | Animation of 3,600 objects, thick glass, etched type, arc. |
| 8 | 0:58–1:06 (8 s) | The tile slides into a phone standing on the floor; its screen lights with the Sundial film. Camera keeps pulling back, up and up, until the whole studio is a small bright island. Huge floor type: **"This film was one spec. Rendered in N s."** (N = this film's measured render time, filled in by the parent before the final render.) | Continuous pull-back and crane to a high wide; studio lights dim to warm dusk, the orange lines glow. | The film you just watched was made the same way, in N seconds. | Device model with a live screen texture, extreme pull-back, light change, scale. |
| 9 | 1:06–1:12 (6 s) | Chrome **Anikit Video** lettering rises out of the floor, reflecting the studio; below it in mono: `anikit-video render spec.json`. Tagline: **"Words in. Film out."** | Slow descending push-in to the logo, last 2 s locked off. | This is Anikit Video, and using it is one command. | Chrome extruded type, reflections, soft shadow, final hold. |

## 5. Sound

Synthesised score (numpy additive + FM, mixed to AAC), 120 BPM, A minor → C major. Every move lands on a beat.

| Time | Beat/bar | Music | Sync points |
|---|---|---|---|
| 0:00 | bar 1 | Room tone, soft felt-piano A, high glassy shimmer pad. | Focus rack on beat 3. |
| 0:06 | bar 4 | Second piano voice; low sub enters. | Pane flip = glass "tink" + reverse swell on 0:06; spec lines highlight on 8th notes. |
| 0:12 | bar 7 | Pulse bass (8ths) begins. | Pane enters slot on 0:14 with a heavy mechanical clunk; LED blink = hi click; frame 1 out on 0:16 with a bright chime. |
| 0:20 | bar 11 | Kick on 1 and 3; hat ticks follow pane stamps and accelerate (16ths → 32nds → a pitched whir rising one octave). | Counter digit ticks are the hats; whir peaks at 3,600 on 0:28, cut to sub drop. |
| 0:30 | bar 16 | Half-time groove; each browser frame gets a dry camera-shutter click (slow, audible, almost comic). | Shutter clicks land on beats; "21×" text lands on the bar at 0:36. |
| 0:40 | bar 21 | **Drop**: full groove, lead synth melody (C major), sidechained to the flicker of passing pane edges. | Camera launch on the downbeat; the Sundial word in the inner film hits 0:46. |
| 0:50 | bar 26 | Riser of compressed whoosh as panes stack; silence for 1 beat. | Stack completes on 0:52.0 = huge low thud + glass ring. |
| 0:58 | bar 30 | Melody resolves, pad opens; phone screen lights with a soft chime. | "Rendered in N s" lands on 1:02. |
| 1:06 | bar 34 | Final C-major chord, long reverb tail; logo shimmer is a high glass partial. | Logo settles 1:08; tail decays to silence at 1:12. |

## 6. Proof

Only measured numbers appear on screen:

| On screen | Source |
|---|---|
| "3,600 frames · 60 seconds of 1080p · 60 fps" | Composition (d) in `docs/video/BENCHMARKS.md` (3,600 frames, 1920×1080, 60 fps). |
| "Anikit Video 3.4 s" | `BENCHMARKS.md` (d), Anikit Video default: 3.37 s (median of 3). |
| "Fastest browser setup 69.1 s" | `BENCHMARKS.md` (d), HyperFrames fast (`--workers 4 --quality draft`): 69.14 s, the quickest browser row. |
| 175 panes in the browser line | 3,600 × 3.37 / 69.14 = 175 frames finished by the browser when Anikit finishes. |
| "21× faster" | `BENCHMARKS.md` (d): HyperFrames fast 69.14 s ÷ Anikit 3.37 s = 20.5×, the table's own "21×". |
| "Its own H.264 encoder" | `ak264`, the default encoder in every Anikit row of `BENCHMARKS.md`. |
| "Rendered in N s" | This film's own end-to-end wall time, measured and filled in by the parent (`RENDER_SECONDS` in `build.py`). |

The inner Sundial product is fictional; no other claims are made.

## 7. What the engine has to provide (for 3D and RENDER)

1. `scene3d` with instanced thin glass slabs (≈3,700), each with a **texture that is a 2D sub-composition
   sampled at a per-instance time offset** (frame *i* of the inner film). Needed for shots 4–7; distant panes
   can fall back to a flat average colour (LOD).
2. A camera along a spline with per-key lens (35/85 mm), focus distance and aperture: DOF and motion blur.
3. Glass (transmission + refraction through thickness, tinted edges), aluminium, chrome, emissive; cyc
   environment and soft contact shadows; exponential haze.
4. Extruded 3D type standing in the world, plus text/UI on planes (spec, browser bar, counter, labels).
5. glTF or primitive phone and device for the inner film and shot 8.
6. Post: light bloom on the LED and orange lines, subtle grain, no heavy glow.

## 8. Blind-read check

A fresh subagent was given only the shot list. It correctly said: "Someone types a plain request for a launch
video; an agent turns it into a JSON plan, and a machine renders that plan into frames and then into an MP4.
The ad is for Anikit Video… needs no browser… much faster… its own H.264 encoder. (Sundial is only the
made-up client in the demo.)" It got the beginning, middle and end right. It flagged four unclear points,
all now fixed: the speed numbers didn't match (shot 5 now compares like with like: 3.4 s vs the fastest
browser setup's 69.1 s, which is 21×, with 175 panes); which way the panes face for the fly-through (upright,
one behind another, facing the camera); how 3,600 panes fit in a phone (they crush down to a matchbox-sized
tile); and a hard-to-read "third line" reveal in shot 8 (removed).
