#!/usr/bin/env python3
"""Run a command and record its wall time and peak RSS (largest child, from getrusage).

    python3 scripts/measure.py -o stats.json -- anikit-video render spec.json -o out.mp4

Appends one JSON object per run to the -o file (JSON lines) and prints it.
"""
import json, resource, subprocess, sys, time

def main():
    a = sys.argv[1:]
    out = None
    if a[:1] == ["-o"]:
        out, a = a[1], a[2:]
    if a[:1] == ["--"]:
        a = a[1:]
    t0 = time.time()
    rc = subprocess.call(a)
    rec = {"cmd": " ".join(a), "exit": rc, "wall_s": round(time.time() - t0, 2),
           "peak_rss_mb": round(resource.getrusage(resource.RUSAGE_CHILDREN).ru_maxrss / 1024, 1)}
    print(json.dumps(rec), file=sys.stderr)
    if out:
        with open(out, "a") as f:
            f.write(json.dumps(rec) + "\n")
    sys.exit(rc)

main()
