"""Score for the Offset 2027 opening titles. 120 BPM, A minor -> F -> C, 15 s.
Every accent is tied to a picture event in index.html (times in seconds)."""
import numpy as np
from scipy import signal
from scipy.io import wavfile

SR = 48000
DUR = 15.0
N = int(SR * DUR)
rng = np.random.default_rng(1405)
L = np.zeros(N); R = np.zeros(N)
REV_L = np.zeros(N); REV_R = np.zeros(N)  # reverb send


def midi(m):
    return 440.0 * 2 ** ((m - 69) / 12)


def place(buf, x, t, gain=1.0):
    i = int(t * SR)
    if i >= N:
        return
    n = min(len(x), N - i)
    buf[i:i + n] += x[:n] * gain


def put(x, t, gain=1.0, pan=0.0, send=0.0):
    gl = gain * np.cos((pan + 1) * np.pi / 4)
    gr = gain * np.sin((pan + 1) * np.pi / 4)
    place(L, x, t, gl); place(R, x, t, gr)
    if send:
        place(REV_L, x, t, gl * send); place(REV_R, x, t, gr * send)


def lp(x, fc, order=2):
    return signal.sosfilt(signal.butter(order, fc, "low", fs=SR, output="sos"), x)


def hp(x, fc, order=2):
    return signal.sosfilt(signal.butter(order, fc, "high", fs=SR, output="sos"), x)


def bp(x, lo, hi, order=2):
    return signal.sosfilt(signal.butter(order, [lo, hi], "band", fs=SR, output="sos"), x)


def env_adsr(n, a, d, s, r):
    a, d, r = int(a * SR), int(d * SR), int(r * SR)
    e = np.full(n, s)
    e[:a] = np.linspace(0, 1, a)
    e[a:a + d] = np.linspace(1, s, len(e[a:a + d]))
    if r:
        e[-r:] *= np.linspace(1, 0, r)
    return e


# ── pad: detuned saws through a slowly opening low-pass ─────────────────────
def pad_note(m, dur, bright=1400, a=0.6, r=0.9):
    n = int(dur * SR); t = np.arange(n) / SR
    x = np.zeros(n)
    for det in (-0.09, -0.03, 0.04, 0.1):
        f = midi(m + det)
        x += signal.sawtooth(2 * np.pi * f * t + rng.uniform(0, 6.28))
    x = lp(x / 4, bright, 2)
    return x * env_adsr(n, a, 0.4, 0.8, r)


CHORDS = [  # (start, dur, notes, brightness)
    (0.0, 2.1, [45, 57, 60, 64, 71], 900),       # Am9
    (2.0, 2.1, [41, 57, 60, 64, 69], 1200),      # Fmaj7
    (4.0, 2.1, [48, 55, 59, 64, 67], 1500),      # Cmaj7
    (6.0, 2.0, [43, 55, 62, 65, 69], 1900),      # G9sus-ish, opening into the riser
    (8.0, 2.1, [41, 53, 60, 64, 67, 69], 2600),  # Fmaj9 on the mark
    (10.0, 2.1, [45, 57, 60, 64, 71], 2000),     # Am9
    (12.0, 2.1, [41, 57, 60, 64, 69], 1700),     # Fmaj7
    (14.0, 1.0, [48, 55, 59, 64, 71], 1200),     # Cmaj7 (fades with picture)
]
for st, du, notes, br in CHORDS:
    for k, m in enumerate(notes):
        a = 0.9 if st == 0 else (0.02 if st == 8.0 else 0.25)
        put(pad_note(m, du + 0.6, br, a=a, r=0.7), st, 0.055 if m > 50 else 0.07,
            pan=(k / max(1, len(notes) - 1) - 0.5) * 0.7, send=0.6)


# ── plucks: Karplus-Strong, a nod to the guitarra portuguesa ───────────────
def pluck(m, dur=1.4, damp=0.996, bright=0.5):
    f = midi(m); p = int(SR / f); n = int(dur * SR)
    buf = rng.uniform(-1, 1, p)
    buf = lp(buf, 1500 + 6000 * bright, 1)
    out = np.zeros(n)
    for i in range(n):
        j = i % p
        out[i] = buf[j]
        buf[j] = damp * 0.5 * (buf[j] + buf[(j + 1) % p])
    out = hp(out, 120)
    return out * np.exp(-np.arange(n) / SR * 1.8)


# hero tile draws: one note per shape (disc 0.55, ring 0.75, dot 0.9)
put(pluck(76, 2.0), 0.57, 0.38, pan=-0.2, send=0.5)
put(pluck(81, 2.0), 0.77, 0.34, pan=0.15, send=0.5)
put(pluck(84, 2.0), 0.92, 0.30, pan=0.3, send=0.6)

BEAT = 0.5
arps = {  # bar start -> notes for 8th-note arpeggio
    2.0: [69, 72, 76, 72, 81, 76, 72, 76],
    4.0: [67, 71, 72, 76, 79, 76, 72, 71],
    6.0: [67, 69, 74, 77, 79, 81, 86, 88],
    8.0: [72, 76, 81, 84, 81, 76, 77, 76],
    10.0: [69, 72, 76, 79, 81, 79, 76, 72],
    12.0: [69, 72, 77, 81, 84, 81, 77, 72],
}
for bar, notes in arps.items():
    for k, m in enumerate(notes):
        t = bar + k * BEAT / 2 * 2  # 8ths across the bar? bar is 2 s = 4 beats = 8 eighths
        t = bar + k * 0.25
        if bar == 6.0 and t >= 7.25:
            continue  # leave air before the hit
        if t < 1.75:
            continue
        g = 0.16 if bar < 8 else 0.13
        if bar == 2.0:
            g *= 0.6 + 0.4 * k / 8  # grows with the tile wave
        put(pluck(m, 1.0, damp=0.993, bright=0.35), t, g, pan=((k % 4) - 1.5) * 0.25, send=0.45)
# the grid begins: a low pluck at the swap
put(pluck(57, 1.8, bright=0.6), 1.75, 0.3, send=0.4)


# ── drums ───────────────────────────────────────────────────────────────────
def kick(g=1.0):
    n = int(0.45 * SR); t = np.arange(n) / SR
    f = 46 + 110 * np.exp(-t * 32)
    ph = 2 * np.pi * np.cumsum(f) / SR
    x = np.sin(ph) * np.exp(-t * 7.5)
    x += 0.25 * hp(rng.standard_normal(n), 2000) * np.exp(-t * 300)
    return np.tanh(1.6 * x) * g


def hat(g=1.0, dec=60):
    n = int(0.08 * SR); t = np.arange(n) / SR
    return hp(rng.standard_normal(n), 7000, 2) * np.exp(-t * dec) * g


def clap():
    n = int(0.3 * SR); t = np.arange(n) / SR
    x = bp(rng.standard_normal(n), 900, 3000)
    e = np.exp(-t * 18)
    for off in (0.0, 0.011, 0.022):
        e += 0.6 * np.exp(-np.clip(t - off, 0, None) * 120) * (t >= off)
    return x * e * 0.35


kick_times = [3.5, 4.0, 4.5, 5.0, 5.5, 6.0] + [8.0 + 0.5 * k for k in range(12)]
for t in kick_times:
    put(kick(0.75 if t >= 8 else 0.6), t, 1.0)
for k in range(int((6.5 - 3.5) / 0.25)):
    t = 3.5 + k * 0.25
    put(hat(0.10 if k % 2 else 0.05), t, 1.0, pan=0.3)
for k in range(int((13.75 - 8.0) / 0.25)):
    t = 8.0 + k * 0.25
    put(hat(0.09 if k % 2 else 0.045), t, 1.0, pan=0.3)
for t in [9.0, 10.0, 11.0, 12.0, 13.0]:
    put(clap(), t, 0.8, pan=-0.1, send=0.5)


# ── bass: sine + a little saturation, pulsing 8ths under the groove ────────
def bass_note(m, dur):
    n = int(dur * SR); t = np.arange(n) / SR
    x = np.sin(2 * np.pi * midi(m) * t) + 0.25 * np.sin(4 * np.pi * midi(m) * t)
    return np.tanh(1.3 * x) * env_adsr(n, 0.005, 0.08, 0.7, 0.04)


roots = {3.5: 41, 4.0: 36, 6.0: 43, 8.0: 41, 10.0: 45, 12.0: 41}
for k in range(int((6.5 - 3.5) / 0.25)):
    t = 3.5 + k * 0.25
    m = 41 if t < 4 else (36 if t < 6 else 43)
    put(bass_note(m, 0.22), t, 0.16)
for k in range(int((13.75 - 8.0) / 0.25)):
    t = 8.0 + k * 0.25
    m = 41 if t < 10 else (45 if t < 12 else 41)
    put(bass_note(m, 0.22), t, 0.15 if k % 2 else 0.19)
put(bass_note(36, 1.4), 13.75, 0.18)


# ── foley-ish: ceramic tile ticks that follow the tile waves ───────────────
def tick(f, g):
    n = int(0.05 * SR); t = np.arange(n) / SR
    x = np.sin(2 * np.pi * f * t) * np.exp(-t * 120)
    x += 0.4 * hp(rng.standard_normal(n), 3000) * np.exp(-t * 400)
    return x * g


# grid wave (1.72 + d * 0.16), 209 tiles — sample a sparse subset
for _ in range(70):
    d = rng.uniform(0, 10.5)
    t = 1.72 + d * 0.16 + 0.03
    put(tick(rng.uniform(2600, 4600), 0.05 * (1 - d / 14)), t, 1.0, pan=rng.uniform(-0.8, 0.8), send=0.3)
# rotation wave 5.5 + (i+j)*0.018 (+ about 0.21 to mid-move)
for s in range(0, 29):
    t = 5.5 + s * 0.018 + 0.21
    put(tick(rng.uniform(2400, 4200), 0.05), t, 1.0, pan=(s / 28 - 0.5) * 1.4, send=0.3)
# band build 9.9 + k*0.022, rotations at 11.9 and 12.9
for k in range(0, 40, 2):
    put(tick(rng.uniform(3200, 5200), 0.025), 9.93 + k * 0.022, 1.0, pan=(k / 39 - 0.5) * 1.2, send=0.2)
    put(tick(rng.uniform(3200, 5200), 0.022), 12.08 + k * 0.02, 1.0, pan=(k / 39 - 0.5) * 1.2, send=0.2)
    put(tick(rng.uniform(3200, 5200), 0.022), 13.08 + (40 - k) * 0.02, 1.0, pan=(k / 39 - 0.5) * 1.2, send=0.2)


# ── whooshes for the offsets (row shift has its own sample) ────────────────
def whoosh(dur, f0, f1, g):
    n = int(dur * SR); t = np.arange(n) / SR
    noise = rng.standard_normal(n)
    out = np.zeros(n)
    seg = 512
    for s in range(0, n, seg):
        u = s / n
        fc = f0 * (f1 / f0) ** u
        out[s:s + seg] = bp(noise[max(0, s - 2048):s + seg], fc * 0.7, fc * 1.4)[-len(out[s:s + seg]):]
    e = np.sin(np.pi * np.clip(t / dur, 0, 1)) ** 2
    return out * e * g


put(whoosh(0.5, 400, 2500, 0.5), 4.48, 1.0, pan=0.0, send=0.3)           # columns down
put(whoosh(0.45, 2500, 600, 0.35), 6.22, 1.0, pan=0.0, send=0.3)         # back into register
put(whoosh(0.55, 3000, 500, 0.45), 6.85, 1.0, pan=0.0, send=0.4)         # pattern opens
put(tick(1800, 0.25), 6.66, 1.0, send=0.4)                                # registered: a single snap

# ── riser into the mark at 8.0 ─────────────────────────────────────────────
n = int(1.5 * SR); t = np.arange(n) / SR
nz = rng.standard_normal(n)
riser = np.zeros(n)
seg = 1024
for s in range(0, n, seg):
    u = s / n
    fc = 300 * (6000 / 300) ** u
    riser[s:s + seg] = bp(nz[max(0, s - 4096):s + seg], fc * 0.8, min(fc * 1.3, 20000))[-len(riser[s:s + seg]):]
riser *= (t / 1.5) ** 2.2 * 0.6
tone = np.sin(2 * np.pi * np.cumsum(110 * (4 ** (t / 1.5))) / SR) * (t / 1.5) ** 3 * 0.08
put(riser + tone, 6.5, 1.0, send=0.5)

# the mark: sub drop + bright overprint shimmer
n = int(2.2 * SR); t = np.arange(n) / SR
sub = np.sin(2 * np.pi * np.cumsum(38 + 50 * np.exp(-t * 6)) / SR) * np.exp(-t * 1.8) * 0.5
put(np.tanh(sub * 1.4), 8.0, 1.0)
shimmer = hp(rng.standard_normal(n), 6000) * np.exp(-t * 2.5) * 0.06
put(shimmer, 8.0, 1.0, send=1.0)


# ── reverb: stereo decaying-noise convolution ──────────────────────────────
def ir(sec, seed):
    r = np.random.default_rng(seed)
    n = int(sec * SR); t = np.arange(n) / SR
    x = r.standard_normal(n) * np.exp(-t * 3.2)
    return lp(x, 6000, 1) * 0.06


L += signal.fftconvolve(REV_L, ir(2.4, 1), mode="full")[:N]
R += signal.fftconvolve(REV_R, ir(2.4, 2), mode="full")[:N]

# master: gentle glue, fade with the picture (13.85 → 15)
mix = np.stack([L, R], axis=1)
tt = np.arange(N) / SR
fade = np.clip(1 - (tt - 13.9) / 1.1, 0, 1) ** 1.5
fade *= np.clip(tt / 0.02, 0, 1)
mix *= fade[:, None]
mix = np.tanh(mix * 1.1) / 1.1
peak = np.abs(mix).max()
mix *= 0.85 / peak
wavfile.write("assets/score.wav", SR, (mix * 32767).astype(np.int16))
print("peak before norm", peak)
