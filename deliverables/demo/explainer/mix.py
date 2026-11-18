#!/usr/bin/env python3
"""Soundtrack for the explainer. Sound is the subject, so the audio does what the
picture does. The noise bed is cancelled by summing it with its own inverted copy
(19-22 s), exactly as the speaker does in the earcup; nothing is faded out.
Stems are rendered by `anikit-video sound render` (see stems/); the spec's
`"sound"` key masters the result to -14 LUFS / -1 dBTP."""
import wave, pathlib
import numpy as np

SR = 48000
D = 30.0
here = pathlib.Path(__file__).parent
N = int(D * SR)

def load(name):
    with wave.open(str(here / "stems" / name)) as w:
        x = np.frombuffer(w.readframes(w.getnframes()), dtype=np.int16).astype(np.float32) / 32768.0
        return x.reshape(-1, 2)

def ramp(t, pts):
    xs, ys = zip(*pts)
    return np.interp(t, xs, ys)

def place(out, x, at, gain_db=0.0, hit=0.0):
    s = int(round((at - hit) * SR))
    x = x * 10 ** (gain_db / 20)
    a, b = max(s, 0), min(s + len(x), len(out))
    if b > a:
        out[a:b] += x[a - s:b - s]

t = np.arange(N) / SR
out = np.zeros((N, 2), np.float32)

# Room tone throughout (very low).
out += load("room.wav")[:N] * 10 ** (-6 / 20)
# 1. One drop, one ring (the first ring is born at 0.7 s).
place(out, load("drop.wav"), 0.7, -4)
place(out, load("thump.wav"), 0.7, -14)
# 2. Noise: city and crowd rise with the churn (5.2-7 s).
noise = load("city.wav")[:N] + 0.7 * load("crowd.wav")[:N]
noise *= ramp(t, [(0, 0), (5.2, 0), (7.0, 1), (30, 1)])[:, None] * 10 ** (4 / 20)
# 4. The anti-noise: the same bed, inverted, ramped in as the cup goes still.
#    (Perspective: inside the earcup; outside the picture still churns until 23 s.)
anti = -noise * ramp(t, [(0, 0), (19.0, 0), (22.0, 1), (30, 1)])[:, None]
out += noise + anti
# 3. The earcup rises; the microphone listens.
place(out, load("rise.wav"), 12.2, -6, hit=1.4 * 0.62)
place(out, load("metal.wav"), 12.6, -10)
for i, at in enumerate([13.4, 14.6, 15.8]):
    place(out, load(f"blip{i + 1}.wav"), at, -10)
# 5. Silence, named: one glass note in the key of D as the word surfaces.
place(out, load("glass.wav"), 23.4, -6)

peak = np.abs(out).max()
out = out / max(peak, 1e-9) * 0.7
with wave.open(str(here / "soundtrack.wav"), "wb") as w:
    w.setnchannels(2); w.setsampwidth(2); w.setframerate(SR)
    w.writeframes((np.clip(out, -1, 1) * 32767).astype(np.int16).tobytes())
residual = np.abs((noise + anti)[int(22.5 * SR):int(29 * SR)]).max()
print(f"soundtrack.wav: peak {peak:.3f}; noise residual after cancellation {residual:.2e}")
