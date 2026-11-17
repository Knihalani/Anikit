"""Score for the fast cut (fast.py): 130 BPM, an impact on every cut, a riser into the run,
a whoosh per whip, a final chord under the logo. Reuses music.py's instruments.

    python3 examples/video/flagship/fast_music.py   # writes fast_music.mp3
"""

from __future__ import annotations

import json
import subprocess
import sys
import wave
from pathlib import Path

import numpy as np

HERE = Path(__file__).parent
sys.path.insert(0, str(HERE))
import music as M  # noqa: E402

BPM = 130
B = 60 / BPM


def main():
    spec = json.loads((HERE / "fast-proxy.json").read_text() if (HERE / "fast-proxy.json").exists()
                      else (HERE / "fast.json").read_text())
    cuts = [0.0]
    for s in spec["scenes"]:
        cuts.append(cuts[-1] + s["duration"])
    dur = cuts[-1]
    M.N = int((dur + 1.0) * M.SR)
    M.L = np.zeros(M.N)
    M.R = np.zeros(M.N)
    M.REV = np.zeros((2, M.N))
    add = M.add
    # groove: kick on every beat, hats on the off-beats, a pulse bass in A minor
    roots = [45, 41, 48, 43]
    n_beats = int(dur / B)
    for k in range(n_beats):
        t = k * B
        add(M.kick(1.0), t)
        add(M.hat(0.7, 0.02), t + B / 2, pan=0.3)
        add(M.pluck_bass(roots[(k // 4) % 4] - 12), t, 0.8)
        add(M.pluck_bass(roots[(k // 4) % 4] - 12), t + B / 2, 0.55)
        if k % 2 == 1:
            add(M.clap(0.9), t, rev=0.2)
    # every cut: an impact and a glass hit; whooshes into the whips
    for i, c in enumerate(cuts[1:-1], start=1):
        add(M.sub(33, 0.5) * 0.8, c, 0.9)
        add(M.tink(2600 + 220 * (i % 5), 0.7), c, 0.6, pan=-0.4 + 0.08 * i, rev=0.5)
        add(M.noise_swell(0.35, True, 600, 9000) * 0.5, c - 0.35, pan=0.1)
    # stabs that change outfit across the run shots
    for i, c in enumerate(cuts[4:7]):
        for k in range(4):
            add(M.acid(57 + [0, 3, 7][i] + (12 if k % 2 else 0), 0.14, 0.8), c + k * B / 2, pan=0.15)
    # riser into the logo, then the chord
    logo_t = cuts[-2]
    add(M.noise_swell(1.6, True, 300, 12000), logo_t - 1.6, 1.0, rev=0.4)
    final = [48, 55, 60, 64, 67, 72, 76]
    add(M.pad([M.midi(m) for m in final], dur - logo_t + 1.0, a=0.05, r=1.2, bright=0.8), logo_t, 1.0, rev=0.8)
    for k, m in enumerate(final):
        add(M.piano(m, 3.0, 0.8), logo_t + 0.03 * k, pan=-0.4 + 0.13 * k, rev=0.7)
    add(M.kick(1.5), logo_t)
    add(M.sub(36, 2.5) * 1.4, logo_t, 1.0)
    rl, rr = M.reverb()
    mix = np.stack([M.L + rl, M.R + rr])[:, : int(dur * M.SR)]
    mix = np.tanh(mix / np.max(np.abs(mix)) * 1.8) / np.tanh(1.8) * 0.92
    fade = np.ones(mix.shape[1])
    fade[-int(0.4 * M.SR):] = np.linspace(1, 0, int(0.4 * M.SR))
    mix *= fade
    tmp = Path(sys.argv[1]) if len(sys.argv) > 1 else HERE / "fast_music.wav"
    with wave.open(str(tmp), "wb") as w:
        w.setnchannels(2)
        w.setsampwidth(2)
        w.setframerate(M.SR)
        w.writeframes((mix.T * 32767).astype(np.int16).tobytes())
    subprocess.run(["ffmpeg", "-loglevel", "error", "-y", "-i", str(tmp), "-c:a", "libmp3lame", "-b:a", "192k",
                    str(HERE / "fast_music.mp3")], check=True)
    if len(sys.argv) <= 1:
        tmp.unlink()
    print(f"fast_music.mp3: {dur:.2f} s, {len(cuts) - 2} cuts")


if __name__ == "__main__":
    main()
