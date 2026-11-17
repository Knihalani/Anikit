#!/usr/bin/env python3
"""Synthesise the original soundtrack for "Pulse": 120 BPM, 18 bars (36 s).

    python3 examples/video/pulse/soundtrack.py            # writes pulse.flac (and pulse.wav with --wav)

Pure numpy + stdlib; ffmpeg (if present) converts the WAV to FLAC.
Arrangement (bar = 2 s, beat = 0.5 s):
  bars 1-2   intro    pad + filtered kick + hats, lead teaser
  bars 3-6   verse    kick, bass, hats, claps on 2 and 4
  bars 7-8   build    snare roll, noise riser, kick drops out on the last beat
  bars 9-14  DROP     sidechained saw bass, lead hook, open hats
  bars 15-16 break    pad + hook echo, half-time kick
  bars 17-18 outro    final hit on bar 17 beat 1, pad tail
"""
import os
import shutil
import subprocess
import sys
import wave

import numpy as np

SR = 44100
BPM = 120.0
BEAT = 60.0 / BPM
BAR = 4 * BEAT
BARS = 18
LEN = BARS * BAR
N = int(LEN * SR)
rng = np.random.default_rng(7)

L = np.zeros(N)
R = np.zeros(N)


def t_of(n):
    return np.arange(n) / SR


def add(sig, at, gain=1.0, pan=0.0):
    i = int(round(at * SR))
    if i >= N:
        return
    sig = np.asarray(sig, float)[: N - i]
    lg = gain * np.sqrt(0.5 * (1 - pan))
    rg = gain * np.sqrt(0.5 * (1 + pan))
    L[i : i + len(sig)] += sig * lg
    R[i : i + len(sig)] += sig * rg


def env_ad(n, a, d):
    t = t_of(n)
    return np.minimum(t / max(a, 1e-4), 1.0) * np.exp(-np.maximum(t - a, 0) / d)


def kick(gain=1.0):
    n = int(0.45 * SR)
    t = t_of(n)
    f = 45 + 110 * np.exp(-t / 0.035)
    ph = 2 * np.pi * np.cumsum(f) / SR
    body = np.sin(ph) * np.exp(-t / 0.22)
    click = rng.standard_normal(n) * np.exp(-t / 0.004) * 0.4
    return np.tanh(1.6 * (body + click)) * gain


def clap():
    n = int(0.3 * SR)
    t = t_of(n)
    noise = rng.standard_normal(n)
    # band-ish: differentiate to brighten
    noise = np.diff(noise, prepend=0)
    e = np.zeros(n)
    for k, off in enumerate((0.0, 0.011, 0.022)):
        i = int(off * SR)
        e[i:] += np.exp(-(t[: n - i]) / (0.012 if k < 2 else 0.11))
    return noise * e * 0.35


def hat(open_=False):
    n = int((0.25 if open_ else 0.06) * SR)
    t = t_of(n)
    noise = np.diff(rng.standard_normal(n), prepend=0)
    noise = np.diff(noise, prepend=0)
    return noise * np.exp(-t / (0.08 if open_ else 0.015)) * 0.18


def snare():
    n = int(0.18 * SR)
    t = t_of(n)
    tone = np.sin(2 * np.pi * 190 * t) * np.exp(-t / 0.04)
    noise = np.diff(rng.standard_normal(n), prepend=0) * np.exp(-t / 0.06)
    return (0.5 * tone + 0.5 * noise) * 0.5


def saw(freq, n, detune=0.0):
    t = t_of(n)
    out = np.zeros(n)
    for d in (-detune, 0.0, detune) if detune else (0.0,):
        ph = (t * freq * (1 + d) + rng.random()) % 1.0
        out += 2 * ph - 1
    return out / (3 if detune else 1)


def lowpass(x, cutoff):
    """One-pole low-pass; `cutoff` may be an array (per-sample Hz)."""
    cutoff = np.broadcast_to(np.asarray(cutoff, float), x.shape)
    a = 1 - np.exp(-2 * np.pi * cutoff / SR)
    y = np.empty_like(x)
    acc = 0.0
    for i in range(len(x)):
        acc += a[i] * (x[i] - acc)
        y[i] = acc
    return y


def midi(m):
    return 440.0 * 2 ** ((m - 69) / 12)


# ---- sections -------------------------------------------------------------
def bar_t(b, beat=0.0):
    return (b - 1) * BAR + beat * BEAT


# Chords (A minor-ish): Am - F - C - G, one per bar, cycling.
CHORDS = [[57, 60, 64], [53, 57, 60], [48, 55, 60], [55, 59, 62]]
ROOTS = [45, 41, 36, 43]

# Pad: whole song except the drop is brighter.
for b in range(1, BARS + 1):
    ch = CHORDS[(b - 1) % 4]
    n = int(BAR * SR)
    t = t_of(n)
    sig = sum(saw(midi(m), n, detune=0.004) for m in ch) / 3
    bright = 2200 if 9 <= b <= 14 else (900 if b <= 2 else 1400)
    sig = lowpass(sig, bright)
    e = np.minimum(t / 0.25, 1) * np.minimum((BAR - t) / 0.08, 1)
    g = 0.16 if 9 <= b <= 14 else 0.22
    if b >= 17:
        g = 0.22 * (1 - (b - 17 + t / BAR) / 2) ** 2
        e = np.minimum(t / 0.25, 1)
    add(sig * e * g, bar_t(b), 1.0, pan=-0.2)
    add(sig * e * g, bar_t(b) + 0.012, 1.0, pan=0.2)

# Drums.
for b in range(1, BARS + 1):
    for k in range(4):
        at = bar_t(b, k)
        if b <= 2:
            add(lowpass(kick(), 600), at, 0.7)
        elif 3 <= b <= 6 or 9 <= b <= 14:
            add(kick(), at, 1.0)
        elif b in (7, 8):
            if not (b == 8 and k == 3):
                add(kick(), at, 0.9)
        elif b in (15, 16):
            if k in (0, 2):
                add(kick(), at, 0.85)
        elif b == 17 and k == 0:
            add(kick(1.2), at, 1.0)
        # claps on 2 and 4
        if (3 <= b <= 6 or 9 <= b <= 16) and k in (1, 3):
            add(clap(), at, 1.0 if b >= 9 else 0.8)
        # hats on off-beats (16ths in the drop)
        if 1 <= b <= 16 and not (b == 8 and k == 3):
            if 9 <= b <= 14:
                for s in range(4):
                    add(hat(open_=(s == 2)), at + s * BEAT / 4, 0.9 if s == 2 else 0.55, pan=0.3)
            else:
                add(hat(), at + BEAT / 2, 0.8, pan=0.3)

# Build: snare roll accelerating over bars 7-8, plus a noise riser.
roll = []
tt = bar_t(7)
while tt < bar_t(8, 3) - 1e-6:
    roll.append(tt)
    prog = (tt - bar_t(7)) / (bar_t(8, 3) - bar_t(7))
    tt += BEAT / (2 if prog < 0.5 else (4 if prog < 0.85 else 8))
for i, tt in enumerate(roll):
    add(snare(), tt, 0.35 + 0.65 * i / len(roll))
n = int((bar_t(9) - bar_t(7)) * SR)
t = t_of(n)
riser = np.diff(rng.standard_normal(n), prepend=0)
riser = lowpass(riser, 300 + 9000 * (t / t[-1]) ** 2) * (t / t[-1]) ** 2 * 0.6
add(riser * (t < (bar_t(8, 3) - bar_t(7))), bar_t(7), 1.0)

# Bass: pulsing eighths, sidechained in the drop.
for b in range(3, 17):
    if b in (7, 8):
        root = ROOTS[(b - 1) % 4]
    else:
        root = ROOTS[(b - 1) % 4]
    for e8 in range(8):
        if b == 8 and e8 >= 6:
            continue
        at = bar_t(b) + e8 * BEAT / 2
        n = int(BEAT / 2 * SR)
        t = t_of(n)
        f = midi(root + (12 if (e8 % 2 == 1 and b >= 9) else 0))
        sig = saw(f, n, detune=0.006 if b >= 9 else 0.0)
        cut = (2400 if 9 <= b <= 14 else 700) * np.exp(-t / 0.09) + 120
        sig = lowpass(sig, cut)
        duck = 1 - 0.85 * np.exp(-((at - np.floor(at / BEAT) * BEAT) + t) / 0.08) if b >= 9 else 1.0
        add(np.tanh(2 * sig) * duck * np.minimum((n - np.arange(n)) / 200, 1), at, 0.45 if b >= 9 else 0.38)

# Lead hook (drop and break): 2-bar phrase, square-ish pluck with echo.
HOOK = [  # (beat offset in the 2-bar phrase, midi, length in beats)
    (0, 76, 0.5), (0.5, 79, 0.5), (1, 81, 1), (2.5, 79, 0.5), (3, 76, 0.5),
    (4, 74, 0.5), (4.5, 76, 0.5), (5, 72, 1), (6.5, 74, 0.5), (7, 71, 1),
]


def pluck(freq, dur):
    n = int((dur * BEAT + 0.25) * SR)
    t = t_of(n)
    sig = np.sign(np.sin(2 * np.pi * freq * t)) * 0.5 + saw(freq * 1.003, n) * 0.5
    sig = lowpass(sig, 1200 + 5000 * np.exp(-t / 0.08))
    return sig * env_ad(n, 0.004, 0.35)


def hook(start_bar, bars, gain):
    for p in range(0, bars, 2):
        for off, m, d in HOOK:
            at = bar_t(start_bar + p) + off * BEAT
            s = pluck(midi(m), d)
            add(s, at, gain, pan=-0.15)
            add(s, at + 3 * BEAT / 4, gain * 0.35, pan=0.5)  # dotted-8th echo


hook(9, 6, 0.30)
hook(15, 2, 0.18)
# Teaser: first two notes in the intro.
for off, m, d in HOOK[:3]:
    add(lowpass(pluck(midi(m), d), 1500), bar_t(2) + off * BEAT, 0.16)

# Final hit: impact + crash on bar 17.
n = int(2.5 * SR)
t = t_of(n)
crash = np.diff(rng.standard_normal(n), prepend=0) * np.exp(-t / 0.7) * 0.25
add(crash, bar_t(17), 1.0, pan=0.1)
add(lowpass(saw(midi(33), n, detune=0.01), 400) * np.exp(-t / 0.9), bar_t(17), 0.5)

# Master: gentle glue + limiter.
mix = np.stack([L, R], 1)
mix = np.tanh(mix * 1.25)
mix /= np.max(np.abs(mix)) + 1e-9
mix *= 0.66  # about -12 LUFS integrated
fade = np.minimum((N - np.arange(N)) / (0.05 * SR), 1)[:, None]
mix *= fade

here = os.path.dirname(os.path.abspath(__file__))
wav_path = os.path.join(here, "pulse.wav")
with wave.open(wav_path, "wb") as w:
    w.setnchannels(2)
    w.setsampwidth(2)
    w.setframerate(SR)
    w.writeframes((mix * 32767).astype("<i2").tobytes())

flac = os.path.join(here, "pulse.flac")
if shutil.which("ffmpeg"):
    subprocess.run(["ffmpeg", "-y", "-loglevel", "error", "-i", wav_path, "-c:a", "flac", flac], check=True)
    if "--wav" not in sys.argv:
        os.remove(wav_path)
    print(f"wrote {flac} ({os.path.getsize(flac) / 1e6:.2f} MB, {LEN:.0f} s)")
else:
    print(f"wrote {wav_path} ({LEN:.0f} s); install ffmpeg for FLAC")
