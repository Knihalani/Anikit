# Telemetry

A broadcast-style data recap of the last laps of a fictional race: the Kessra Grand Prix of the Meridian Formula Series, held on the Kessra Harbour Circuit (5.112 km, 58 laps). It runs 45.7 s at 1920×1080 and 60 fps, and every pixel comes from `spec.json` (anikit-video components and shapes only, no engine changes).

![poster](poster.png)

| Time | Scene | What's on screen |
|---|---|---|
| 0.0 | open | The circuit glows on; kinetic title; lap counter rolls 50 → 54 |
| 3.9 | map | Track drawn on with stroke trim, sector colours, ten cars racing round at ×12 time, a live timing tower with tyres, gaps and position swaps (IVE passes LIN at T7) |
| 12.5 | onboard | Okafor's lap 56: speed dial, gear, shift lights, throttle/brake, a speed trace drawing over the rival's ghost, live delta, minimap, lower third |
| 20.5 | overtake | Replay wipe, then a zoomed Turn 1: Marchetti locks up and Okafor goes past in slow motion, with motion trails, live speeds and the interval |
| 28.2 | fastest | Fastest-lap sting: 1:31.279, purple sectors |
| 31.3 | analysis | Sector comparison bars (purple / green / yellow) and a lap-time line chart for laps 48–58 |
| 38.7 | winner | Chrome 3D OKAFOR (`text-3d`), podium, confetti |

## The data is simulated, not typed in

`race.py` integrates a quasi-steady-state lap over the circuit's centre line, with grip-limited corner speeds and traction and braking passes. That gives a speed profile and a reference lap of 92.1 s. Each driver's laps are scaled from that same clock, plus tyre wear, fuel burn and a few scripted events (a lock-up, a tow). From one model the script derives:
- sector and lap times, which add up;
- gaps, taken from line-crossing times;
- car positions on the map;
- the speed, gear, throttle and brake shown on the gauges;
- the moment the pass happens (lap 57, 19.2 % round, the Turn 1 braking zone);
- the fastest lap and the final classification.

`spec.py` turns that into keyframes: car x/y at 30 Hz, trim of the speed trace by arc length, tower rows and counters.

## Commands

```bash
cargo build --release -p anikit-video
python3 examples/video/telemetry/spec.py          # runs the simulation → race.json + spec.json (~1 s)
target/release/anikit-video check  examples/video/telemetry/spec.json
target/release/anikit-video sheet  examples/video/telemetry/spec.json -o sheet.png --cols 4 --rows 4 --width 480
target/release/anikit-video still  examples/video/telemetry/spec.json --at 42.3 -o poster.png
target/release/anikit-video render examples/video/telemetry/spec.json -o telemetry.mp4
```

## Measured

On a 4-core cloud VM (16 GB), using the default ak264 encoder:

| | |
|---|---|
| frames | 2742 (185 deduped) |
| render wall time | 26.2 s (105 fps); other runs took 25.2–26.6 s (103–109 fps) |
| output | 16.4 MB MP4 (16 758 KB), 1920×1080, 60 fps, about 2.9 Mbit/s |
| spec | 527 KB JSON, mostly car-position and trail keyframes |

The film is silent. With an `audio` track, the render sometimes hangs in the ffmpeg mux step. Separately, about one full render in two hangs before the first frame, even without audio; re-running works. Both have been reported upstream.
