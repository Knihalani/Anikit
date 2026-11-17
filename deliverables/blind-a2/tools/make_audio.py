"""Synthesise the 15 s soundtrack for the film (deterministic, seeded).

120 BPM (one beat = 0.5 s). Hits are locked to the picture:
  0.0-3.0  riser + accelerating ticks into the singularity
  3.0      impact, then a stab on every manifesto card (3.0, 3.75, 4.5, 5.25)
  5.5      "<html>" hit, reverse swell into the drop
  6.5      impact + driving groove through the render tunnel
  10.5     final impact, pad swell, shimmer, tail to silence at 15.0
"""
import sys
import wave

import numpy as np

SR = 48000
DUR = 15.0
N = int(SR * DUR)
rng = np.random.default_rng(7)
t_all = np.arange(N) / SR
L = np.zeros(N)
R = np.zeros(N)
send = np.zeros(N)  # reverb send (mono)


def at(t0):
    return int(round(t0 * SR))


def add(sig, t0, gain=1.0, pan=0.0, rev=0.0):
    i = at(t0)
    if i >= N:
        return
    sig = sig[: N - i]
    gl = np.cos((pan + 1) * np.pi / 4) * np.sqrt(2)
    gr = np.sin((pan + 1) * np.pi / 4) * np.sqrt(2)
    L[i : i + len(sig)] += sig * gain * gl
    R[i : i + len(sig)] += sig * gain * gr
    if rev:
        send[i : i + len(sig)] += sig * gain * rev


def onepole_lp(x, fc):
    """One-pole low-pass; fc may be a scalar or a per-sample array."""
    fc = np.broadcast_to(np.asarray(fc, dtype=float), x.shape)
    a = 1.0 - np.exp(-2 * np.pi * fc / SR)
    y = np.empty_like(x)
    s = 0.0
    for n in range(len(x)):
        s += a[n] * (x[n] - s)
        y[n] = s
    return y


def lp_static(x, fc):
    # cheap static low-pass via FFT brickwall-ish rolloff (smooth)
    X = np.fft.rfft(x)
    f = np.fft.rfftfreq(len(x), 1 / SR)
    X *= 1 / np.sqrt(1 + (f / fc) ** 4)
    return np.fft.irfft(X, len(x))


def hp_static(x, fc):
    X = np.fft.rfft(x)
    f = np.fft.rfftfreq(len(x), 1 / SR)
    X *= 1 / np.sqrt(1 + (fc / np.maximum(f, 1e-3)) ** 4)
    return np.fft.irfft(X, len(x))


def saw(freq, n, detune=0.0):
    tt = np.arange(n) / SR
    out = np.zeros(n)
    for k in range(1, 24):
        if freq * k * (1 + detune) > SR / 2.2:
            break
        out += np.sin(2 * np.pi * freq * (1 + detune) * k * tt) / k
    return out * 0.6


def midi(m):
    return 440.0 * 2 ** ((m - 69) / 12)


# ---------------------------------------------------------------- voices
def kick(dur=0.45, f0=160, f1=42):
    n = int(dur * SR)
    tt = np.arange(n) / SR
    f = f1 + (f0 - f1) * np.exp(-tt / 0.035)
    ph = 2 * np.pi * np.cumsum(f) / SR
    body = np.sin(ph) * np.exp(-tt / 0.16)
    click = rng.standard_normal(n) * np.exp(-tt / 0.003) * 0.4
    return np.tanh((body + click) * 1.6)


def hat(dur=0.06, open_=False):
    n = int((0.25 if open_ else dur) * SR)
    tt = np.arange(n) / SR
    x = hp_static(rng.standard_normal(n), 7000)
    return x * np.exp(-tt / (0.09 if open_ else 0.018))


def clap():
    n = int(0.35 * SR)
    tt = np.arange(n) / SR
    x = hp_static(lp_static(rng.standard_normal(n), 3500), 900)
    env = np.exp(-tt / 0.09)
    for d in (0.0, 0.011, 0.022):
        env += (tt >= d) * np.exp(-np.maximum(tt - d, 0) / 0.006) * 0.8
    return x * env * 0.8


def impact(size=1.0):
    n = int(2.8 * SR)
    tt = np.arange(n) / SR
    f = 34 + 70 * np.exp(-tt / 0.08)
    sub = np.sin(2 * np.pi * np.cumsum(f) / SR) * np.exp(-tt / (0.9 * size))
    nz = lp_static(rng.standard_normal(n), 1800) * np.exp(-tt / 0.35) * 0.7
    crack = hp_static(rng.standard_normal(n), 2500) * np.exp(-tt / 0.04) * 0.6
    return np.tanh((sub * 1.3 + nz + crack) * 1.2)


def stab(notes, dur=0.42, bright=4000):
    n = int(dur * SR)
    tt = np.arange(n) / SR
    out_l = np.zeros(n)
    out_r = np.zeros(n)
    for m in notes:
        out_l += saw(midi(m), n, -0.004)
        out_r += saw(midi(m), n, 0.004)
    env = np.exp(-tt / 0.12) * (1 - np.exp(-tt / 0.002))
    fc = 300 + bright * np.exp(-tt / 0.07)
    return onepole_lp(out_l * env, fc), onepole_lp(out_r * env, fc)


def pluck(m, dur=0.6):
    n = int(dur * SR)
    tt = np.arange(n) / SR
    f = midi(m)
    x = (np.sin(2 * np.pi * f * tt) + 0.3 * np.sin(4 * np.pi * f * tt)) * np.exp(-tt / 0.18)
    return x * (1 - np.exp(-tt / 0.001))


# ---------------------------------------------------------------- 0-3 s: riser
n3 = at(3.0)
tt = t_all[:n3]
# drone
drone = saw(55, n3) * 0.5 + saw(55 * 1.5, n3, 0.003) * 0.2
drone = onepole_lp(drone, 120 + 900 * (tt / 3.0) ** 2) * (0.25 + 0.5 * tt / 3.0)
add(drone * 0.55, 0.0, pan=0.0, rev=0.25)
# filtered noise riser
nz = rng.standard_normal(n3)
riser = onepole_lp(nz, 200 + 9000 * (tt / 3.0) ** 3) * (tt / 3.0) ** 2.2
add(riser * 0.55, 0.0, rev=0.4)
# sine sweep
f = 70 * (900 / 70) ** ((tt / 3.0) ** 1.6)
sweep = np.sin(2 * np.pi * np.cumsum(f) / SR) * (tt / 3.0) ** 3 * 0.35
add(sweep, 0.0, rev=0.3)
# accelerating ticks (the frame counter)
tk = 0.15
gap = 0.26
while tk < 2.92:
    n = int(0.02 * SR)
    x = np.sin(2 * np.pi * 3200 * np.arange(n) / SR) * np.exp(-np.arange(n) / SR / 0.004)
    add(x * (0.12 + 0.2 * tk / 3), tk, pan=0.3 * np.sin(tk * 9), rev=0.2)
    tk += gap
    gap = max(0.028, gap * 0.86)
# sub-drop "inhale" just before the flash
n = int(0.25 * SR)
x = rng.standard_normal(n) * np.linspace(0, 1, n) ** 3
add(hp_static(x, 3000) * 0.3, 2.75, rev=0.2)

# ---------------------------------------------------------------- 3-6.5 s: manifesto
add(impact(1.0), 3.0, gain=0.95, rev=0.35)
chords = [[45, 57, 60, 64], [41, 53, 57, 60], [48, 55, 60, 64], [43, 55, 59, 62]]
for k, t0 in enumerate([3.0, 3.75, 4.5, 5.25]):
    sl, sr_ = stab(chords[k])
    i = at(t0)
    m = min(len(sl), N - i)
    L[i : i + m] += sl[:m] * 0.32
    R[i : i + m] += sr_[:m] * 0.32
    send[i : i + m] += (sl[:m] + sr_[:m]) * 0.12
for b in range(6):  # kicks on the beat 3.0..5.5
    add(kick(), 3.0 + 0.5 * b, gain=0.8)
for b in range(12):
    add(hat(), 3.25 + 0.25 * b if b % 2 == 0 else 3.0 + 0.25 * b, gain=0.15 + 0.1 * (b % 2), pan=0.25)
# "<html>" hit at 5.5
add(impact(0.6), 5.5, gain=0.6, rev=0.5)
# reverse swell 5.6 -> 6.5
n = at(6.5) - at(5.6)
tt = np.arange(n) / SR
x = rng.standard_normal(n) * (tt / tt[-1]) ** 3
x = onepole_lp(x, 400 + 12000 * (tt / tt[-1]) ** 2)
add(x * 0.6, 5.6, rev=0.3)
# snare roll 6.0 -> 6.5
for k in range(8):
    add(clap(), 6.0 + k * 0.0625, gain=0.15 + 0.06 * k, pan=0.0)

# ---------------------------------------------------------------- 6.5-10.5 s: groove
add(impact(1.2), 6.5, gain=1.0, rev=0.4)
for b in range(8):
    tb = 6.5 + 0.5 * b
    add(kick(), tb, gain=0.95)
    if b % 2 == 1:
        add(clap(), tb, gain=0.5, rev=0.25)
for s in range(16 * 2):
    ts = 6.5 + 0.125 * s
    add(hat(open_=(s % 4 == 2)), ts, gain=0.12 if s % 2 == 0 else 0.22, pan=-0.3 if s % 2 else 0.3)
# bass: 16th-note arpeggio, side-chained to the kick
bass_notes = [33, 33, 45, 33, 36, 33, 45, 40] * 4
nb = at(10.5) - at(6.5)
bass = np.zeros(nb)
for s, m in enumerate(bass_notes):
    i = at(0.125 * s)
    n = at(0.125)
    seg = saw(midi(m), n) * np.exp(-np.arange(n) / SR / 0.09)
    bass[i : i + n] += seg[: nb - i]
tt = np.arange(nb) / SR
duck = 1 - 0.85 * np.exp(-((tt % 0.5) / 0.06))
cut = 250 + 1800 * (0.5 + 0.5 * np.sin(2 * np.pi * tt / 4.0 - np.pi / 2)) ** 2
bass = onepole_lp(onepole_lp(bass, cut), cut * 1.5) * duck
add(np.tanh(bass * 2.2) * 0.42, 6.5)
# pad
pad_chords = [[57, 60, 64, 71], [53, 57, 60, 67], [48, 55, 60, 64], [55, 59, 62, 67]]
for k, ch in enumerate(pad_chords):
    n = at(1.0)
    tp = np.arange(n) / SR
    padl = sum(saw(midi(m), n, -0.006) for m in ch)
    padr = sum(saw(midi(m), n, 0.006) for m in ch)
    env = np.minimum(1, tp / 0.15) * np.minimum(1, (1.0 - tp) / 0.2) * duck[at(k) : at(k) + n]
    i = at(6.5 + k)
    L[i : i + n] += lp_static(padl, 1400) * env * 0.07
    R[i : i + n] += lp_static(padr, 1400) * env * 0.07
    send[i : i + n] += lp_static(padl + padr, 1400) * env * 0.03
# riser into 10.5
n = at(10.5) - at(9.5)
tt = np.arange(n) / SR
x = onepole_lp(rng.standard_normal(n), 500 + 10000 * (tt / tt[-1]) ** 2) * (tt / tt[-1]) ** 2.5
add(x * 0.4, 9.5, rev=0.3)

# ---------------------------------------------------------------- 10.5-15 s: lockup
add(impact(1.6), 10.5, gain=1.0, rev=0.6)
n = at(15.0) - at(10.5)
tp = np.arange(n) / SR
final = [45, 57, 59, 60, 64, 71, 76]
padl = sum(saw(midi(m), n, -0.005) for m in final)
padr = sum(saw(midi(m), n, 0.005) for m in final)
env = np.minimum(1, tp / 0.8) * np.clip((4.4 - tp) / 1.2, 0, 1)
fc = 500 + 2500 * np.minimum(1, tp / 2.0)
L[at(10.5) :] += onepole_lp(padl, fc) * env * 0.06
R[at(10.5) :] += onepole_lp(padr, fc) * env * 0.06
send[at(10.5) :] += onepole_lp(padl + padr, fc) * env * 0.04
# shimmer plucks under the title words
for k, (t0, m) in enumerate(
    [(10.62, 81), (10.79, 84), (10.96, 88), (11.13, 83), (11.55, 86), (11.72, 88), (11.89, 91), (12.06, 93)]
):
    add(pluck(m), t0, gain=0.16, pan=0.5 * np.sin(k * 1.7), rev=0.6)
add(pluck(69, 2.5), 12.6, gain=0.12, rev=0.8)

# ---------------------------------------------------------------- reverb + master
ir_n = int(2.6 * SR)
ti = np.arange(ir_n) / SR
irl = rng.standard_normal(ir_n) * np.exp(-ti / 0.55)
irr = rng.standard_normal(ir_n) * np.exp(-ti / 0.55)
irl = lp_static(irl, 6000)
irr = lp_static(irr, 6000)
size = 1 << int(np.ceil(np.log2(N + ir_n)))
S = np.fft.rfft(send, size)
L += np.fft.irfft(S * np.fft.rfft(irl, size), size)[:N] * 0.035
R += np.fft.irfft(S * np.fft.rfft(irr, size), size)[:N] * 0.035

# tail to silence
fade = np.clip((15.0 - t_all) / 0.5, 0, 1) ** 2
L *= fade
R *= fade
mix = np.stack([L, R], 1)
mix = np.tanh(mix * 0.6) / 0.6
mix *= 0.89 / np.max(np.abs(mix))
pcm = (mix * 32767).astype("<i2")

out = sys.argv[1] if len(sys.argv) > 1 else "assets/music.wav"
with wave.open(out, "wb") as w:
    w.setnchannels(2)
    w.setsampwidth(2)
    w.setframerate(SR)
    w.writeframes(pcm.tobytes())
print("wrote", out)
