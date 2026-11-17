#!/usr/bin/env python3
"""Original score for "Halden": 128 BPM, 8 bars = 15.0 s. numpy only.

    python3 soundtrack.py   -> score.wav

Beat = 0.46875 s. Cue sheet (beats):
  b0-4   forge breath: sub drone, fire crackle, heartbeat kick on 1 and 3
  b4-12  four-on-the-floor; anvil clangs on the strikes (b7, b9, b10.5, b11.5)
  b11-12 riser
  b12-16 folding: pluck arpeggio climbing an octave every half beat, 16th hats
  b16-18 breath held (kick out), blade lifted
  b18    quench: sub boom + steam hiss sweep
  b22    glint bell
  b25    final impact + long metal tail; b28 second hit
"""
import subprocess, wave
import numpy as np

SR = 44100
BPM = 128.0
BEAT = 60.0 / BPM
LEN = 15.0
N = int(LEN * SR)
rng = np.random.default_rng(11)
L = np.zeros(N); R = np.zeros(N)
REV = np.zeros(N)  # reverb send (mono)

def t_of(n): return np.arange(n) / SR
def at(b): return b * BEAT

def add(sig, t0, gain=1.0, pan=0.0, rev=0.0):
    i = int(round(t0 * SR))
    if i >= N: return
    sig = np.asarray(sig, float)[: N - i]
    L[i:i+len(sig)] += sig * gain * np.sqrt(0.5 * (1 - pan))
    R[i:i+len(sig)] += sig * gain * np.sqrt(0.5 * (1 + pan))
    REV[i:i+len(sig)] += sig * gain * rev

def onepole(x, cut):
    """Low-pass; cut may be an array (Hz)."""
    cut = np.broadcast_to(np.asarray(cut, float), x.shape)
    a = np.exp(-2 * np.pi * cut / SR)
    y = np.empty_like(x); s = 0.0
    for i in range(len(x)):
        s = (1 - a[i]) * x[i] + a[i] * s
        y[i] = s
    return y

def kick(g=1.0):
    n = int(0.5 * SR); t = t_of(n)
    f = 42 + 120 * np.exp(-t / 0.03)
    s = np.sin(2 * np.pi * np.cumsum(f) / SR) * np.exp(-t / 0.22)
    s += 0.25 * rng.standard_normal(n) * np.exp(-t / 0.004)
    return np.tanh(1.6 * s) * g

def clang(f0=420.0, dec=1.4):
    n = int(2.5 * SR); t = t_of(n)
    s = np.zeros(n)
    for k, (r, a, d) in enumerate([(1, 1, 1), (2.76, .7, .6), (5.40, .5, .35), (8.93, .35, .2), (13.3, .25, .12), (1.51, .4, .8)]):
        s += a * np.sin(2 * np.pi * f0 * r * t + k) * np.exp(-t / (dec * d))
    s += 1.2 * rng.standard_normal(n) * np.exp(-t / 0.006)
    s += 0.8 * kick()[:n] if n <= len(kick()) else 0
    return np.tanh(s * 0.6)

def hat(open_=False):
    n = int((0.25 if open_ else 0.06) * SR); t = t_of(n)
    x = rng.standard_normal(n)
    x = x - onepole(x, 6000)
    return x * np.exp(-t / (0.08 if open_ else 0.015))

def pluck(f, d=0.35):
    n = int(d * 2 * SR); t = t_of(n)
    s = sum((1 / k) * np.sin(2 * np.pi * f * k * t) * np.exp(-t * k / d) for k in range(1, 7))
    return s * np.minimum(t / 0.003, 1)

def noise_sweep(dur, c0, c1, curve=2.0):
    n = int(dur * SR); t = t_of(n)
    cut = c0 + (c1 - c0) * (t / dur) ** curve
    return onepole(rng.standard_normal(n), cut)

# --- drone (whole piece), D minor-ish
t = t_of(N)
drone = 0.35 * np.sin(2 * np.pi * 36.71 * t) + 0.18 * np.sin(2 * np.pi * 73.42 * t + np.sin(2 * np.pi * 0.2 * t))
saw = (2 * ((55.0 * t) % 1) - 1) + (2 * ((55.3 * t) % 1) - 1)
drone += 0.12 * onepole(saw, 300 + 250 * np.sin(2 * np.pi * t / 15) ** 2)
drone *= np.minimum(t / 0.8, 1) * np.clip((15 - t) / 0.8, 0, 1)
drone[int(at(16) * SR):int(at(18) * SR)] *= 0.5
L += drone; R += drone

# --- fire crackle b0-4 and under the forge
cr = np.zeros(N)
for k in range(140):
    tt = rng.uniform(0, at(6)); i = int(tt * SR); n = int(0.01 * SR)
    if i + n < N: cr[i:i+n] += rng.standard_normal(n) * np.exp(-t_of(n) / 0.002) * rng.uniform(0.05, 0.3)
add(cr - onepole(cr, 1500), 0, 0.8, 0.2)

# --- kicks
for b in [0, 2]: add(kick(), at(b), 0.9)
for b in range(4, 16): add(kick(), at(b), 0.95)
for b in range(18, 25): add(kick(), at(b), 0.9)
for b in [25, 28, 29, 30, 31]: add(kick(), at(b), 1.0)

# --- anvil strikes
for b, f, p in [(7, 410, -0.2), (9, 440, 0.2), (10.5, 470, -0.1), (11.5, 520, 0.1)]:
    add(clang(f), at(b), 0.75, p, rev=0.5)

# --- riser into folding
r = noise_sweep(at(1), 400, 9000)
add(r * np.linspace(0, 1, len(r)) ** 2, at(11), 0.35, rev=0.2)

# --- hats
for b in np.arange(4, 12, 0.5): add(hat(), at(b + 0.25 if b % 1 == 0 else b), 0.18, 0.3)
for b in np.arange(12, 16, 0.25): add(hat(), at(b), 0.2, -0.3 if (b * 4) % 2 else 0.3)
for b in np.arange(18, 25, 0.5): add(hat(b % 1 != 0), at(b), 0.16, 0.25)

# --- folding arpeggio: octave climbs every half beat
base = 146.83  # D3
seq = [1, 1.2, 1.5, 1.782]
for k in range(8):
    for j in range(4):
        add(pluck(base * (2 ** (k / 4)) * seq[j]), at(12 + k * 0.5 + j * 0.125), 0.16, (j - 1.5) * 0.3, rev=0.4)

# --- held breath riser b16-18
r = noise_sweep(at(2), 200, 5000, 3)
add(r * np.linspace(0, 1, len(r)) ** 3, at(16), 0.4, rev=0.3)

# --- quench: boom + steam hiss
n = int(1.6 * SR); tt = t_of(n)
boom = np.sin(2 * np.pi * np.cumsum(30 + 60 * np.exp(-tt / 0.08)) / SR) * np.exp(-tt / 0.5)
add(np.tanh(2 * boom), at(18), 0.9)
st = rng.standard_normal(int(3.0 * SR)); tt = t_of(len(st))
st = (st - onepole(st, 1200 + 5000 * np.exp(-tt / 0.6))) * np.exp(-tt / 0.9) * np.minimum(tt / 0.02, 1)
add(st, at(18), 0.45, 0.0, rev=0.3)

# --- glint bells
for b, f in [(22, 1760), (23.5, 2349)]:
    n = int(2 * SR); tt = t_of(n)
    bell = sum(a * np.sin(2 * np.pi * f * r_ * tt) * np.exp(-tt / d) for r_, a, d in [(1, 1, .8), (2.01, .4, .4), (3.9, .2, .2)])
    add(bell, at(b), 0.12, 0.3 if b == 22 else -0.3, rev=0.6)

# --- finale
add(clang(330, 2.6), at(25), 0.9, 0, rev=0.8)
add(np.tanh(2 * boom), at(25), 0.8)
add(clang(392, 1.6), at(28), 0.6, 0.15, rev=0.6)
for k in range(4):
    add(pluck(293.66 * [1, 1.5, 2, 3][k], 0.8), at(28 + k * 0.5), 0.12, (k - 1.5) * 0.4, rev=0.6)

# --- reverb: exponential noise IR via FFT
irn = int(2.2 * SR); ir = rng.standard_normal(irn) * np.exp(-t_of(irn) / 0.55)
ir = onepole(ir, 4000); ir /= np.sqrt(np.sum(ir ** 2))
m = 1 << int(np.ceil(np.log2(N + irn)))
wet = np.fft.irfft(np.fft.rfft(REV, m) * np.fft.rfft(ir, m), m)[:N] * 0.35
L += wet; R += np.roll(wet, 300)

out = np.stack([L, R], 1)
out *= np.clip((LEN - t_of(N)) / 0.6, 0, 1)[:, None]
out = np.tanh(out / np.max(np.abs(out)) * 1.3) * 0.89
pcm = (out * 32767).astype("<i2")
with wave.open("score.wav", "wb") as w:
    w.setnchannels(2); w.setsampwidth(2); w.setframerate(SR); w.writeframes(pcm.tobytes())
print("score.wav", LEN, "s")
