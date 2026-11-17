#!/usr/bin/env python3
"""Synthesises music.mp3, the 120 BPM score for "Spec to Screen".

Pure numpy (additive synthesis, no samples), then ffmpeg/libmp3lame to MP3.
Section times match build.py: every scene cut is on a beat.

    python3 examples/video/spec-to-screen/music.py
"""
import os, subprocess
import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__))
SR, BPM, LEN = 44100, 120, 55.0
BEAT = 60 / BPM
N = int(SR * LEN)
L, R = np.zeros(N), np.zeros(N)
rng = np.random.default_rng(7)

# sections (s) — keep in sync with build.py's printed scene starts
SPEC, SCREEN, KINETIC, DEVICES, SPEED, CHART, PARTICLES, LOGO = 4.5, 15.5, 19.5, 27.5, 35.0, 39.0, 43.5, 47.0


def add(sig, t, gain=1.0, pan=0.0):
    i = int(t * SR)
    if i >= N:
        return
    sig = sig[: N - i] * gain
    L[i:i + len(sig)] += sig * np.sqrt(0.5 * (1 - pan))
    R[i:i + len(sig)] += sig * np.sqrt(0.5 * (1 + pan))


def env(n, a, d):
    t = np.arange(n) / SR
    return np.minimum(t / max(a, 1e-4), 1) * np.exp(-t / d)


def smooth(x, k):
    return np.convolve(x, np.ones(k) / k, mode="same")


def kick(big=False):
    n = int(SR * (0.9 if big else 0.45))
    t = np.arange(n) / SR
    f = 45 + 95 * np.exp(-t / 0.045)
    s = np.sin(2 * np.pi * np.cumsum(f) / SR) * np.exp(-t / (0.32 if big else 0.16))
    click = rng.standard_normal(n) * np.exp(-t / 0.004) * 0.25
    return np.tanh(1.6 * (s + click))


def clap():
    n = int(SR * 0.3)
    t = np.arange(n) / SR
    no = np.diff(rng.standard_normal(n + 1))
    e = sum(np.exp(-np.maximum(t - o, 0) / 0.012) * (t >= o) for o in (0, 0.011, 0.023)) + 0.6 * np.exp(-t / 0.09)
    return no * e * 0.35


def hat(open_=False):
    n = int(SR * (0.25 if open_ else 0.06))
    no = np.diff(np.diff(rng.standard_normal(n + 2)))
    return no * env(n, 0.001, 0.08 if open_ else 0.018) * 0.18


def tone(freq, dur, harm=8, roll=1.6, a=0.01, d=None, det=0.0):
    n = int(SR * dur)
    t = np.arange(n) / SR
    s = np.zeros(n)
    for k in range(1, harm + 1):
        if freq * k > 16000:
            break
        s += np.sin(2 * np.pi * freq * (1 + det) * k * t + k) / k ** roll
    e = np.minimum(t / a, 1) * (np.exp(-t / d) if d else 1)
    return s * e


def pad(freqs, dur, bright=0.0):
    n = int(SR * dur)
    t = np.arange(n) / SR
    out = np.zeros((2, n))
    for f in freqs:
        for side, det in ((0, -0.004), (1, 0.004)):
            out[side] += tone(f, dur, harm=10, roll=2.2 - bright, a=0.6, det=det)
    fade = np.minimum(1, (dur - t) / 0.5)
    return out * fade * 0.06


# A minor: Am(add9) – F(maj7) – C – G(add6), one chord per bar (2 s)
CHORDS = [[110.0, 164.8, 220.0, 246.9, 261.6], [87.3, 130.8, 174.6, 220.0, 329.6],
          [130.8, 196.0, 261.6, 329.6, 392.0], [98.0, 146.8, 196.0, 246.9, 329.6]]
ROOTS = [55.0, 43.65, 65.4, 49.0]
ARP = [[440.0, 523.3, 659.3, 493.9], [349.2, 440.0, 523.3, 659.3], [392.0, 523.3, 659.3, 784.0],
       [392.0, 493.9, 587.3, 659.3]]

bars = int(LEN / 2) + 1
duck = np.ones(N)

# ---- pads: whole film, brighter as it builds
for b in range(bars):
    t0 = b * 2.0
    if t0 >= LOGO:
        break
    c = CHORDS[b % 4]
    bright = 0.0 if t0 < SCREEN else 0.4 if t0 < PARTICLES else 0.2
    p = pad(c, 2.15, bright)
    gain = 0.6 + 0.4 * min(t0 / 4.5, 1)
    i = int(t0 * SR)
    m = min(p.shape[1], N - i)
    L[i:i + m] += p[0, :m] * gain
    R[i:i + m] += p[1, :m] * gain

# ---- hook: soft hit on the second line
add(kick(True) * 0.5, 2.0)
add(tone(55, 2.5, harm=3, a=0.005, d=0.9), 2.0, 0.35)

# ---- spec: plucked 8th-note arpeggio, quiet; sub on every bar
for k in range(int((SCREEN - SPEC) / (BEAT / 2))):
    t = SPEC + k * BEAT / 2
    b = int(t / 2) % 4
    f = ARP[b][k % 4]
    add(tone(f, 0.4, harm=6, roll=1.4, a=0.003, d=0.12), t, 0.05 + 0.04 * (t - SPEC) / (SCREEN - SPEC),
        pan=0.4 * np.sin(k))
for t in np.arange(SPEC, SCREEN, 2.0):
    add(tone(ROOTS[int(t / 2) % 4], 2.0, harm=2, a=0.02, d=1.2), t, 0.25)

# ---- riser into the push-in
n = int(SR * 2.0)
tt = np.arange(n) / SR
sweep = np.sin(2 * np.pi * np.cumsum(200 + 1400 * (tt / 2.0) ** 2) / SR)
noise = smooth(rng.standard_normal(n), 6)
add((sweep * 0.15 + noise * 0.25) * (tt / 2.0) ** 2, SCREEN - 2.0, 0.6)
add((sweep * 0.15 + noise * 0.25) * (tt / 2.0) ** 2, LOGO - 2.0, 0.7)

# ---- drums
def beat_range(a, b):
    return np.arange(a, b - 1e-6, BEAT)


for t in [SCREEN]:
    add(kick(True), t, 0.9)
for t in beat_range(SCREEN + 2 * BEAT, KINETIC):          # half-time
    if round((t - SCREEN) / BEAT) % 2 == 0:
        add(kick(), t, 0.6)
for t in beat_range(KINETIC, PARTICLES):                    # four on the floor
    add(kick(), t, 0.75)
    i = int(t * SR)
    m = min(int(0.25 * SR), N - i)
    duck[i:i + m] = np.minimum(duck[i:i + m], 1 - 0.55 * np.exp(-np.arange(m) / SR / 0.09))
    k = round((t - KINETIC) / BEAT)
    if k % 2 == 1:
        add(clap(), t, 0.8)
    if t >= DEVICES:
        add(hat(), t + BEAT / 2, 1.0, pan=0.3)
        add(hat(), t + BEAT / 4, 0.4, pan=-0.3)
    if k % 8 == 7:
        add(hat(True), t + BEAT / 2, 0.8)
for t in beat_range(KINETIC, PARTICLES):                    # sub bass, 8ths
    for h in (0, BEAT / 2):
        add(tone(ROOTS[int((t + h) / 2) % 4], 0.24, harm=3, roll=1.2, a=0.004, d=0.14), t + h, 0.32)
for k in range(int((PARTICLES - DEVICES) / (BEAT / 4))):    # 16th arp once the product shows
    t = DEVICES + k * BEAT / 4
    f = ARP[int(t / 2) % 4][k % 4] * (2 if k % 8 == 6 else 1)
    add(tone(f, 0.25, harm=5, roll=1.5, a=0.002, d=0.07), t, 0.05, pan=0.5 * np.sin(k * 0.7))

# ---- breakdown (particles): pad + filtered arp only, then the logo hit
for k in range(int((LOGO - PARTICLES) / (BEAT / 2))):
    t = PARTICLES + k * BEAT / 2
    add(tone(ARP[int(t / 2) % 4][k % 4], 0.6, harm=3, roll=2, a=0.004, d=0.25), t, 0.06, pan=0.5 * np.sin(k))

# ---- logo: impact, crash, long chord
add(kick(True), LOGO, 1.0)
n = int(SR * 5)
add(np.diff(rng.standard_normal(n + 1)) * env(n, 0.001, 1.4) * 0.12, LOGO)
for f in [55.0, 110.0, 164.8, 220.0, 329.6, 493.9, 659.3]:
    for det, pan in ((-0.003, -0.6), (0.003, 0.6)):
        add(tone(f, 8.0, harm=8, roll=1.9, a=0.05, d=6.0, det=det), LOGO, 0.08, pan=pan)
add(tone(1318.5, 4, harm=2, a=0.002, d=0.9), LOGO + 1.7, 0.05)  # glint with the flare

# ---- mix
L *= duck ** 0.5
R *= duck ** 0.5
t = np.arange(N) / SR
fade = np.clip((LEN - t) / 3.0, 0, 1) * np.clip(t / 0.05, 0, 1)
mix = np.stack([L, R]) * fade
mix = np.tanh(mix / max(1e-9, np.percentile(np.abs(mix), 99.95)) * 1.1) * 0.93
pcm = (mix.T * 32767).astype("<i2").tobytes()
out = os.path.join(HERE, "music.mp3")
subprocess.run(["ffmpeg", "-y", "-loglevel", "error", "-f", "s16le", "-ar", str(SR), "-ac", "2", "-i", "-",
                "-c:a", "libmp3lame", "-b:a", "160k", out], input=pcm, check=True)
print("wrote", out, os.path.getsize(out), "bytes")
