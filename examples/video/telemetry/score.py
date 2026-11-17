"""Synthesised score bed for the "Telemetry" film (needs numpy and ffmpeg).

    python3 examples/video/telemetry/score.py      # writes score.mp3 next to this file

A 124 BPM A-minor drive: four-on-the-floor kick, offbeat hats, a pulsing sub
and a 16th-note "telemetry" arpeggio, laid out on the film's scene times:
intro swell → groove (map, onboard) → cut to a half-time drone for the replay
with a hit on the pass → bright lift for the fastest lap → groove → major-key
resolve on the winner. Sound effects on cuts and events live in spec.json.
"""

import os
import subprocess
import wave

import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__))
SR = 44100
DUR = 45.7
BPM = 124.0
BEAT = 60.0 / BPM
T0 = 3.9  # first downbeat: the map scene
# scene times (s) from `anikit-video check spec.json`
REPLAY, PASS, FASTEST, ANALYSIS, WINNER = 20.45, 24.05, 28.2, 31.3, 38.7

n = int(DUR * SR)
t = np.arange(n) / SR
L = np.zeros(n)
R = np.zeros(n)
rng = np.random.default_rng(7)


def add(sig, at, gain=1.0, pan=0.0):
    i = int(at * SR)
    if i >= n:
        return
    sig = sig[: n - i]
    L[i:i + len(sig)] += sig * gain * (1 - max(0.0, pan))
    R[i:i + len(sig)] += sig * gain * (1 + min(0.0, pan))


def env(length, attack, decay):
    k = np.arange(int(length * SR)) / SR
    return np.minimum(1.0, k / max(attack, 1e-4)) * np.exp(-k / decay)


def kick(length=0.45):
    k = np.arange(int(length * SR)) / SR
    f = 45 + 95 * np.exp(-k * 28)
    ph = 2 * np.pi * np.cumsum(f) / SR
    return np.sin(ph) * np.exp(-k * 7) + 0.3 * np.sin(ph * 2) * np.exp(-k * 30)


def hat(length=0.06):
    x = rng.standard_normal(int(length * SR))
    x = np.diff(x, prepend=0.0)
    return x * env(length, 0.001, 0.018) * 0.35


def tone(freq, length, harmonics=(1.0,), attack=0.005, decay=0.2, detune=0.0):
    k = np.arange(int(length * SR)) / SR
    s = sum(a * np.sin(2 * np.pi * freq * (h + 1) * k * (1 + detune)) for h, a in enumerate(harmonics))
    return s * env(length, attack, decay)


def note(m):  # midi → Hz
    return 440.0 * 2 ** ((m - 69) / 12)


# Am – F – C – G, two bars each (midi roots)
PROG = [(57, [57, 60, 64]), (53, [53, 57, 60]), (48, [48, 52, 55]), (55, [55, 59, 62])]
BAR = 4 * BEAT


def chord_at(time):
    i = int((time - T0) // (2 * BAR)) % len(PROG)
    return PROG[i]


def groove(start, end, arp=True, full=True):
    b = start
    k = 0
    while b < end - 1e-6:
        root, triad = chord_at(b)
        add(kick(), b, 0.9)
        add(hat(), b + BEAT / 2, 0.8, pan=0.3)
        if full:
            add(hat(0.03), b + BEAT * 0.75, 0.35, pan=-0.3)
        # sub bass: 8th pulses
        for e in range(2):
            add(tone(note(root - 24), BEAT / 2, (1.0, 0.35, 0.12), 0.004, 0.16), b + e * BEAT / 2, 0.45)
        if arp:
            for s in range(4):
                m = triad[(k * 4 + s) % 3] + 12 * (1 + (s == 3))
                add(tone(note(m), 0.16, (1.0, 0.0, 0.25), 0.002, 0.07), b + s * BEAT / 4, 0.11, pan=0.4 * np.sin(k + s))
        b += BEAT
        k += 1


def pad(start, end, triad, gain=0.08, lift=0):
    length = end - start
    for m in triad:
        for dt in (-0.004, 0.004):
            s = tone(note(m + 12 + lift), length, (1.0, 0.5, 0.25, 0.12), attack=min(1.2, length / 3), decay=60, detune=dt)
            fade = np.minimum(1.0, (length - np.arange(len(s)) / SR) / 0.8).clip(0, 1)
            add(s * fade, start, gain, pan=dt * 80)


def riser(start, end, gain=0.25):
    length = end - start
    k = np.arange(int(length * SR)) / SR
    x = rng.standard_normal(len(k))
    # brighten over time: blend of smoothed and raw noise
    sm = np.convolve(x, np.ones(24) / 24, mode="same")
    u = k / length
    sweep = np.sin(2 * np.pi * np.cumsum(200 + 1400 * u ** 2) / SR) * 0.3
    add((sm * (1 - u) + x * u * 0.5 + sweep) * u ** 2, start, gain)


def hit(at, gain=1.0):
    add(kick(1.2), at, gain)
    x = rng.standard_normal(int(1.6 * SR))
    add(np.convolve(x, np.ones(10) / 10, mode="same") * env(1.6, 0.002, 0.35), at, 0.5 * gain)


# ---- arrangement ------------------------------------------------------------------
pad(0.0, T0 + 0.2, [57, 60, 64], 0.07)
riser(0.6, T0, 0.12)
hit(T0, 0.9)
groove(T0, REPLAY - 0.3)
riser(REPLAY - 2.2, REPLAY - 0.3, 0.2)
# replay: half-time drone, heartbeat kicks, the hit on the pass
add(tone(note(33), FASTEST - REPLAY, (1.0, 0.4, 0.2), attack=0.3, decay=40), REPLAY, 0.22)
pad(REPLAY, FASTEST, [57, 60, 64], 0.05)
b = REPLAY
while b < FASTEST - 0.2:
    add(kick(0.6), b, 0.6)
    b += 2 * BEAT
hit(PASS, 1.0)
# fastest-lap lift
hit(FASTEST, 0.8)
pad(FASTEST, ANALYSIS, [64, 67, 71], 0.06)
groove(FASTEST, ANALYSIS, arp=True, full=False)
groove(ANALYSIS, WINNER - 0.3)
riser(WINNER - 2.0, WINNER - 0.2, 0.18)
# winner: resolve to C major, then ring out
hit(WINNER + 0.1, 1.1)
pad(WINNER + 0.1, DUR, [60, 64, 67], 0.1)
add(tone(note(36), DUR - WINNER, (1.0, 0.3), attack=0.01, decay=2.5), WINNER + 0.1, 0.35)
for k in range(8):
    add(tone(note([72, 76, 79, 84][k % 4]), 0.5, (1.0, 0.0, 0.2), 0.002, 0.25), WINNER + 0.1 + k * BEAT / 2, 0.08)

# ---- master ------------------------------------------------------------------------
mix = np.stack([L, R], axis=1)
mix *= np.minimum(1.0, (DUR - t) / 1.5)[:, None]  # fade out
mix = np.tanh(mix * 2.2) / np.tanh(2.2)
mix *= 0.7 / max(1e-9, np.abs(mix).max())
pcm = (mix * 32767).astype("<i2")
wav = os.path.join(HERE, "score.wav")
with wave.open(wav, "wb") as w:
    w.setnchannels(2)
    w.setsampwidth(2)
    w.setframerate(SR)
    w.writeframes(pcm.tobytes())
out = os.path.join(HERE, "score.mp3")
subprocess.run(["ffmpeg", "-hide_banner", "-loglevel", "error", "-y", "-i", wav, "-c:a", "libmp3lame", "-b:a", "128k", out], check=True)
os.remove(wav)
print("wrote", out, os.path.getsize(out) // 1024, "KB")
