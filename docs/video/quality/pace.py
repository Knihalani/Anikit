#!/usr/bin/env python3
"""Pace check for a rendered clip: is it fast and dense, or slow and samey?

    python3 docs/video/quality/pace.py clip.mp4 [--strip strip.png]

Prints:
- shot lengths, from ffmpeg scene detection;
- mean frame-to-frame change per second;
- "same-composition" seconds, where the frame barely differs from one second earlier.

It can also write a 1-frame-per-second strip.

FAIL rules (see BAR.md, "Pace"):
- any shot longer than 2.5 s without a major change;
- median shot length over 1.5 s;
- more than 20% of seconds look like the second before.
"""
import json, subprocess, sys, tempfile, os
import numpy as np
from PIL import Image

def frames_1fps(path, w=320):
    out = tempfile.mkdtemp()
    subprocess.run(["ffmpeg", "-v", "error", "-i", path, "-vf", f"fps=1,scale={w}:-1", f"{out}/%04d.png"], check=True)
    return [Image.open(os.path.join(out, f)).convert("RGB") for f in sorted(os.listdir(out))]

def scene_cuts(path, thr=0.25):
    r = subprocess.run(["ffprobe", "-v", "error", "-f", "lavfi", "-i", f"movie={path},select=gt(scene\\,{thr})",
                        "-show_entries", "frame=pts_time", "-of", "json"], capture_output=True, text=True)
    try:
        return [float(f["pts_time"]) for f in json.loads(r.stdout).get("frames", [])]
    except Exception:
        return []

def duration(path):
    r = subprocess.run(["ffprobe", "-v", "error", "-show_entries", "format=duration", "-of", "csv=p=0", path], capture_output=True, text=True)
    return float(r.stdout.strip())

def main():
    path = sys.argv[1]
    dur = duration(path)
    cuts = scene_cuts(path)
    bounds = [0.0] + cuts + [dur]
    shots = [round(b - a, 2) for a, b in zip(bounds, bounds[1:]) if b - a > 0.05]
    fr = frames_1fps(path)
    arr = [np.asarray(f, dtype=np.float32) for f in fr]
    diffs = [float(np.abs(arr[i] - arr[i - 1]).mean()) for i in range(1, len(arr))]
    same = sum(1 for d in diffs if d < 6.0)  # under ~2.4% mean change from one second to the next counts as the same picture
    med = float(np.median(shots)) if shots else dur
    fail = []
    if max(shots) > 2.5:
        fail.append(f"longest shot {max(shots):.1f}s > 2.5s")
    if med > 1.5:
        fail.append(f"median shot {med:.1f}s > 1.5s")
    if diffs and same / len(diffs) > 0.2:
        fail.append(f"{same}/{len(diffs)} seconds look like the previous second")
    print(f"{os.path.basename(path)}: {dur:.1f}s, {len(shots)} shots, median {med:.2f}s, longest {max(shots):.2f}s")
    print("per-second change:", [round(d, 1) for d in diffs])
    print("PACE:", "FAIL — " + "; ".join(fail) if fail else "PASS")
    if "--strip" in sys.argv:
        out = sys.argv[sys.argv.index("--strip") + 1]
        w, h = fr[0].size
        strip = Image.new("RGB", (w * min(len(fr), 10), h * ((len(fr) + 9) // 10)))
        for i, f in enumerate(fr):
            strip.paste(f, ((i % 10) * w, (i // 10) * h))
        strip.save(out)
    sys.exit(1 if fail else 0)

if __name__ == "__main__":
    main()
