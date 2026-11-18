#!/usr/bin/env python3
"""Renders final.mp4 frame by frame with `anikit-video still` (a fresh renderer per frame),
then encodes with ffmpeg and takes the soundtrack from a normal `anikit-video render`.

Why: in this build the long-lived renderer used by `render` reuses cached illo layers at
stale positions while a camera (or group) transform animates, so still shapes drift
against moving ones (a picture card slides off its drawing, the pillow detaches). Each
`still` starts clean and matches the spec exactly.

Usage: python3 render.py [--jobs 4]   (writes final.mp4 and contact.png next to spec.json)
"""
import os, subprocess, sys, time, resource, shutil
from concurrent.futures import ThreadPoolExecutor

HERE = os.path.dirname(os.path.abspath(__file__))
BIN = os.path.join(HERE, "../../../target/release/anikit-video")
FPS, N = 24, 480
jobs = int(sys.argv[sys.argv.index("--jobs") + 1]) if "--jobs" in sys.argv else 4
frames = os.path.join(HERE, ".frames")
os.makedirs(frames, exist_ok=True)
env = dict(os.environ, ANIKIT_RENDER_CACHE="0")
t0 = time.time()

# 1. soundtrack from the normal pipeline
subprocess.run([BIN, "render", "spec.json", "--scale", "0.25", "-o", os.path.join(frames, "audio.mp4")],
               cwd=HERE, env=env, check=True, capture_output=True)


def one(i):
    out = os.path.join(frames, f"{i:04d}.png")
    subprocess.run([BIN, "still", "spec.json", "--at", f"{i / FPS:.6f}", "-o", out], cwd=HERE, env=env, check=True,
                   capture_output=True)


with ThreadPoolExecutor(jobs) as ex:
    list(ex.map(one, range(N)))

# 2. encode + mux
subprocess.run(["ffmpeg", "-v", "error", "-y", "-framerate", str(FPS), "-i", os.path.join(frames, "%04d.png"),
                "-i", os.path.join(frames, "audio.mp4"), "-map", "0:v", "-map", "1:a", "-c:v", "libx264", "-preset", "slow",
                "-crf", "21", "-tune", "animation", "-pix_fmt", "yuv420p", "-c:a", "copy", "-movflags", "+faststart",
                os.path.join(HERE, "final.mp4")], check=True)
# 3. contact sheet: one frame a second from the final file
subprocess.run(["ffmpeg", "-v", "error", "-y", "-ss", "0.5", "-i", os.path.join(HERE, "final.mp4"), "-vf", "fps=1,scale=480:-1,tile=5x4",
                "-frames:v", "1", os.path.join(HERE, "contact.png")], check=True)
shutil.rmtree(frames)
r = resource.getrusage(resource.RUSAGE_CHILDREN)
print(f"wall_s {time.time() - t0:.1f} peak_rss_mb {r.ru_maxrss / 1024:.0f}")
