#!/bin/sh
# Bake every illo plate to plates/<name>.png (one still per plate scene, at 0.9 s into it), then build spec.json.
set -e
cd "$(dirname "$0")/.."
names=$(python3 src/film.py plates)
mkdir -p plates
i=0
for n in $names; do
  anikit-video still src/plates.json --at "$i.9" -o "plates/$n.png" >/dev/null
  i=$((i+1))
done
python3 src/film.py film
