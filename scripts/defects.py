#!/usr/bin/env python3
"""Scan a rendered film for frame defects: flat/blank frames and frozen stretches.

    python3 scripts/defects.py film.mp4 [more.mp4 ...] [--json]

- flat: a frame with almost no variation (luma std < 3 at 160 px), outside the first/last 0.3 s (fades are fine there);
- frozen: >= 1.0 s where consecutive frames are pixel-identical at 160 px (mean abs diff < 0.05/255),
  i.e. nothing at all moves; a held card with grain, a drift or a flicker does not count;
- dup bursts: runs of >= 3 identical frames inside otherwise moving footage (stutter, a cache returning a stale frame).
"""
import json, subprocess, sys
import numpy as np


def scan(path, w=160):
    pr = json.loads(subprocess.run(["ffprobe", "-v", "error", "-select_streams", "v", "-show_entries",
                                    "stream=width,height,r_frame_rate:format=duration", "-of", "json", path],
                                   capture_output=True, text=True).stdout)
    st = pr["streams"][0]
    n_, d_ = st["r_frame_rate"].split("/")
    fps = float(n_) / float(d_)
    h = round(w * st["height"] / st["width"]) // 2 * 2
    raw = subprocess.run(["ffmpeg", "-v", "error", "-i", path, "-vf", f"scale={w}:{h}", "-f", "rawvideo", "-pix_fmt", "gray", "-"],
                         capture_output=True).stdout
    fr = np.frombuffer(raw, np.uint8).reshape(-1, h, w).astype(np.float32)
    n = len(fr)
    edge = int(0.3 * fps)
    std = fr.reshape(n, -1).std(1)
    flat = [i for i in range(edge, n - edge) if std[i] < 3]
    diff = np.abs(np.diff(fr, axis=0)).mean(axis=(1, 2))
    same = diff < 0.05
    runs, i = [], 0
    while i < len(same):
        if same[i]:
            j = i
            while j < len(same) and same[j]:
                j += 1
            runs.append((i, j - i + 1))
            i = j
        else:
            i += 1
    frozen = [(round(s / fps, 2), round(L / fps, 2)) for s, L in runs if L / fps >= 1.0]
    moving = diff > 0.5
    dups = [(round(s / fps, 2), L) for s, L in runs if 3 <= L and L / fps < 1.0
            and s > 0 and moving[max(0, s - 2):s].any() and moving[s + L - 1:s + L + 1].any()]
    def spans(ix):
        out = []
        for k in ix:
            if out and k == out[-1][1] + 1:
                out[-1][1] = k
            else:
                out.append([k, k])
        return [(round(a / fps, 2), round((b - a + 1) / fps, 2)) for a, b in out]
    return {"file": path, "frames": n, "fps": fps, "flat": spans(flat), "frozen": frozen, "dup_bursts": dups,
            "ok": not flat and not frozen and not dups}


if __name__ == "__main__":
    paths = [p for p in sys.argv[1:] if not p.startswith("--")]
    res = [scan(p) for p in paths]
    if "--json" in sys.argv:
        print(json.dumps(res, indent=1))
    else:
        for r in res:
            tag = "OK " if r["ok"] else "FLAG"
            print(f"{tag} {r['file']}: flat {r['flat'] or '-'} · frozen(start s, len s) {r['frozen'] or '-'} · dup bursts {r['dup_bursts'] or '-'}")
