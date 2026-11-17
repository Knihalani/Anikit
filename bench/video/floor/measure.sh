#!/usr/bin/env bash
# Measurements behind docs/video/quality/ONLY_ANIKIT.md. Run from bench/video after `npm install` and
# `cargo build --release -p anikit-video`. Writes out/floor/*.mp4 and prints one line per measurement.
set -u
cd "$(dirname "$0")/.."
AV=${ANIKIT_VIDEO:-../../target/release/anikit-video}
HF=node_modules/.bin/hyperframes
export HYPERFRAMES_BROWSER_PATH=${HEADLESS_SHELL:-/opt/pw-browsers/chromium_headless_shell-1243/chrome-headless-shell-linux64/chrome-headless-shell}
mkdir -p out/floor
t() { local s e; s=$(date +%s.%N); "$@" > out/floor/last.log 2>&1; local rc=$?; e=$(date +%s.%N); printf "%8.2f s  rc=%d  %s\n" "$(echo "$e - $s" | bc)" "$rc" "$*"; }

echo "== 1. HyperFrames floor (15 s, 450 frames): fastest settings"
t $HF render floor/hyperframes -o out/floor/floor-hf-fast.mp4 --workers 4 --quality draft --quiet

echo "== 2. Same raymarch GLSL: HyperFrames (Chrome, software WebGL) vs Anikit (CPU JIT), 5 s = 150 frames"
t $HF render floor/hyperframes -o out/floor/raymarch-hf.mp4 --start 5 --end 10 --workers 4 --quality draft --quiet
t $AV render floor/anikit/raymarch.json -o out/floor/raymarch-anikit.mp4 --quiet

echo "== 3. 5,000 independently moving dots, 5 s = 150 frames"
t $AV render floor/scale/dots5000.json -o out/floor/dots-anikit.mp4 --quiet
t $HF render floor/scale/hf -o out/floor/dots-hf.mp4 --quiet
t $HF render floor/scale/hf -o out/floor/dots-hf-fast.mp4 --workers 4 --quality draft --quiet

echo "== 4. Determinism: the same render twice, and on 1 vs 4 threads (md5 of the MP4)"
RAYON_NUM_THREADS=4 $AV render anikit/kinetic.json -o out/floor/det-a.mp4 --quiet
RAYON_NUM_THREADS=4 $AV render anikit/kinetic.json -o out/floor/det-b.mp4 --quiet
RAYON_NUM_THREADS=1 $AV render anikit/kinetic.json -o out/floor/det-c.mp4 --quiet
md5sum out/floor/det-a.mp4 out/floor/det-b.mp4 out/floor/det-c.mp4
$HF render hyperframes/kinetic -o out/floor/det-hf-a.mp4 --workers 4 --quiet
$HF render hyperframes/kinetic -o out/floor/det-hf-b.mp4 --workers 4 --quiet
$HF render hyperframes/kinetic -o out/floor/det-hf-c.mp4 --workers 1 --quiet
md5sum out/floor/det-hf-a.mp4 out/floor/det-hf-b.mp4 out/floor/det-hf-c.mp4
# Pixel-level: decoded frames identical? (MP4 container bytes can differ by metadata alone.)
for f in det-b det-c; do echo "anikit det-a vs $f: $(ffmpeg -v error -i out/floor/det-a.mp4 -i out/floor/$f.mp4 -lavfi psnr -f null - 2>&1 | grep -o 'average:[^ ]*' || ffmpeg -hide_banner -nostats -i out/floor/det-a.mp4 -i out/floor/$f.mp4 -lavfi psnr -f null - 2>&1 | grep -o 'average:[^ ]*')"; done
for f in det-hf-b det-hf-c; do echo "hf det-hf-a vs $f: $(ffmpeg -hide_banner -nostats -i out/floor/det-hf-a.mp4 -i out/floor/$f.mp4 -lavfi psnr -f null - 2>&1 | grep -o 'average:[^ ]*')"; done
