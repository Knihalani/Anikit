#!/usr/bin/env python3
"""Synthesises the film's sound effects from the subject (no samples): a summer-night
ambience (crickets + room tone), crayon scribbles, soft hop thumps, a pond splash and a
string pluck. Writes sfx/*.wav (mono, 22.05 kHz)."""
import os, wave
import numpy as np

SR = 22050
HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.join(HERE, "sfx")
rng = np.random.default_rng(7)


def onepole(x, a):
    """One-pole low-pass, a in (0,1): higher = darker."""
    y = np.empty_like(x)
    acc = 0.0
    b = 1.0 - a
    for i in range(len(x)):
        acc = b * x[i] + a * acc
        y[i] = acc
    return y


def bandnoise(n, lo_a, hi_a):
    w = rng.standard_normal(n)
    return onepole(w, lo_a) - onepole(w, hi_a)


def write(name, x, peak=0.8):
    x = x / (np.max(np.abs(x)) + 1e-9) * peak
    os.makedirs(OUT, exist_ok=True)
    with wave.open(os.path.join(OUT, name + ".wav"), "wb") as f:
        f.setnchannels(1)
        f.setsampwidth(2)
        f.setframerate(SR)
        f.writeframes((x * 32767).astype("<i2").tobytes())


def env(n, attack, release):
    t = np.arange(n) / SR
    return np.minimum(1, t / max(attack, 1e-4)) * np.exp(-t / release)


# night ambience: three crickets and a soft room tone, 20.5 s
n = int(20.5 * SR)
t = np.arange(n) / SR
amb = 0.05 * onepole(rng.standard_normal(n), 0.985)
for f0, period, gain, ph in ((4300, 0.62, 0.22, 0.0), (3900, 0.81, 0.15, 0.27), (4700, 1.07, 0.09, 0.5)):
    chirp = np.zeros(n)
    k = ph
    while k < 20.5:
        for p in range(3):
            s = int((k + p * 0.034) * SR)
            m = int(0.022 * SR)
            if s + m < n:
                chirp[s:s + m] += np.sin(np.pi * np.arange(m) / m) ** 2
        k += period * (0.6 + 1.2 * rng.random()) + (1.5 * rng.random() if rng.random() < 0.25 else 0)
    amb += gain * chirp * np.sin(2 * np.pi * f0 * t)
write("night", amb, 0.5)

# crayon scribble: bursts of band noise, one per stroke
n = int(0.7 * SR)
x = bandnoise(n, 0.82, 0.3)
strokes = np.zeros(n)
for s in np.arange(0, 0.62, 0.075):
    a = int((s + 0.01 * rng.random()) * SR)
    m = int(0.06 * SR)
    strokes[a:a + m] += np.sin(np.pi * np.arange(m) / m) ** 1.5 * (0.6 + 0.4 * rng.random())
fade = np.minimum(1, np.minimum(np.arange(n) / (0.06 * SR), (n - np.arange(n)) / (0.15 * SR)))
write("scribble", x * strokes * fade, 0.7)

# hop thump: low sine with a pitch drop and a little grass
n = int(0.25 * SR)
tt = np.arange(n) / SR
f = 120 * np.exp(-tt * 9) + 55
th = np.sin(2 * np.pi * np.cumsum(f) / SR) * env(n, 0.003, 0.06)
th += 0.25 * bandnoise(n, 0.6, 0.2) * env(n, 0.001, 0.03)
write("thump", th, 0.8)

# splash: noise burst that darkens, plus bubbles
n = int(1.4 * SR)
tt = np.arange(n) / SR
w = rng.standard_normal(n) * env(n, 0.005, 0.25)
sp = onepole(w, 0.35) * 0.7 + onepole(w, 0.8) * 0.6
for k in range(14):
    s = int((0.05 + rng.random() * 0.8) * SR)
    m = int(0.05 * SR)
    if s + m < n:
        fb = 500 + 900 * rng.random()
        bt = np.arange(m) / SR
        sp[s:s + m] += 0.25 * np.sin(2 * np.pi * (fb + 2500 * bt) * bt) * np.exp(-bt * 60)
write("splash", sp, 0.8)

# string pluck (the moon's string drawn tight)
n = int(1.6 * SR)
tt = np.arange(n) / SR
pl = sum(np.sin(2 * np.pi * 392 * h * tt) * np.exp(-tt * (2.5 + h)) / h for h in range(1, 6)) * env(n, 0.002, 1.0)
write("pluck", pl, 0.6)
print("ok")
