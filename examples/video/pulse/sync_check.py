#!/usr/bin/env python3
"""List every cut and word reveal in spec.json against the detected beat grid.

    python3 examples/video/pulse/sync_check.py   (needs target/release/anikit-video)
"""
import json, os, re, subprocess

here = os.path.dirname(os.path.abspath(__file__))
exe = os.path.join(here, "../../../target/release/anikit-video")
run = lambda *a: json.loads(subprocess.run([exe, *a, "--json"], cwd=here, capture_output=True, text=True, check=True).stdout)
b = run("beats", "pulse.flac")
c = run("check", "spec.json")
beats, bars = b["beats"], b["downbeats"]
spec = json.load(open(os.path.join(here, "spec.json")))
starts = {s["id"]: s["start"] for s in c["scenes"]}
rows = [(f"cut -> {s['id']}", s["start"]) for s in c["scenes"]]


def walk(layers, origin):
    for l in layers:
        st = l.get("start", 0)
        if isinstance(st, str) and st.startswith("@"):
            k, n, off = re.match(r"@(beat|bar)(\d+)([+-][\d.]+)?", st).groups()
            t = (beats if k == "beat" else bars)[int(n) - 1] + float(off or 0)
        else:
            t = origin + st
        if "layers" in l:
            walk(l["layers"], t)
        kin = l.get("kinetic")
        if isinstance(kin, dict) and kin.get("stagger") == 0.5:
            rows.extend((f"word {w!r}", t + 0.5 * i) for i, w in enumerate(l["text"].split()))
        elif "text" in l and st:
            rows.append((f"text {l['text'][:14]!r}", t))


for s in spec["scenes"]:
    walk(s["layers"], starts[s["id"]])
rows.sort(key=lambda r: r[1])
print(f"bpm {b['bpm']:.2f}")
worst = 0.0
for name, t in rows:
    nb = min(beats, key=lambda x: abs(x - t))
    d = (t - nb) * 1000
    worst = max(worst, abs(d))
    print(f"{t:7.3f}  beat {nb:7.3f}  {d:+6.1f} ms  {name}")
print(f"{len(rows)} events, max |offset| {worst:.1f} ms (1 frame = 33.3 ms)")
