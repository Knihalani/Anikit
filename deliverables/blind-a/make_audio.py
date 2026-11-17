"""Synthesise the 15 s soundtrack for the f(t) film (deterministic, numpy only).

120 BPM, beat = 0.5 s. Cues line up with index.html:
  0.0-2.0  oscilloscope tone + accelerating ticks, reverse swell
  2.0      impact (light becomes matter)
  2.0-6.5  pulse + pad, word hits at 2.3 / 3.3 / 4.3
  6.5-7.0  whoosh into the object
  7.0-10.5 driving section, hit on every word cut
  10.5     white-out impact, then final chord to 15.0
"""
import wave
import numpy as np

SR = 48000
DUR = 15.0
N = int(SR * DUR)
rng = np.random.default_rng(7)
t_all = np.arange(N) / SR
L = np.zeros(N)
R = np.zeros(N)


def add(sig, start, gain=1.0, pan=0.0):
    i = int(start * SR)
    if i >= N:
        return
    sig = sig[: N - i]
    lg = np.cos((pan + 1) * np.pi / 4) * np.sqrt(2)
    rg = np.sin((pan + 1) * np.pi / 4) * np.sqrt(2)
    L[i : i + len(sig)] += sig * gain * lg
    R[i : i + len(sig)] += sig * gain * rg


def env_exp(n, tau):
    return np.exp(-np.arange(n) / SR / tau)


def fft_filter(x, lo=None, hi=None):
    X = np.fft.rfft(x)
    f = np.fft.rfftfreq(len(x), 1 / SR)
    m = np.ones_like(f)
    if hi:
        m *= 1 / np.sqrt(1 + (f / hi) ** 4)
    if lo:
        m *= 1 / np.sqrt(1 + (lo / np.maximum(f, 1)) ** 4)
    return np.fft.irfft(X * m, len(x))


def sweep_noise(dur, f0, f1, lo=None):
    """Noise whose lowpass cutoff glides f0->f1 (block-wise overlap-add)."""
    n = int(dur * SR)
    blk, hop = 2048, 1024
    out = np.zeros(n + blk)
    win = np.hanning(blk)
    noise = rng.standard_normal(n + blk)
    for s in range(0, n, hop):
        k = s / max(n - 1, 1)
        fc = f0 * (f1 / f0) ** k
        out[s : s + blk] += fft_filter(noise[s : s + blk] * win, lo=lo, hi=fc)
    return out[:n]


def kick(gain=1.0):
    n = int(0.45 * SR)
    tt = np.arange(n) / SR
    f = 42 + 110 * np.exp(-tt * 28)
    ph = 2 * np.pi * np.cumsum(f) / SR
    body = np.sin(ph) * np.exp(-tt * 7.5)
    click = rng.standard_normal(n) * np.exp(-tt * 300) * 0.25
    return np.tanh((body + click) * 1.6) * gain


def boom(dur=2.4):
    n = int(dur * SR)
    tt = np.arange(n) / SR
    f = 28 + 70 * np.exp(-tt * 6)
    ph = 2 * np.pi * np.cumsum(f) / SR
    sub = np.sin(ph) * np.exp(-tt * 1.6)
    crack = fft_filter(rng.standard_normal(n), lo=200, hi=6000) * np.exp(-tt * 9) * 0.5
    return np.tanh((sub * 1.2 + crack) * 1.3)


def tick(freq=3200, dur=0.05):
    n = int(dur * SR)
    tt = np.arange(n) / SR
    return np.sin(2 * np.pi * freq * tt) * np.exp(-tt * 90)


def hat(dur=0.07):
    n = int(dur * SR)
    return fft_filter(rng.standard_normal(n), lo=7000) * env_exp(n, 0.018)


def snare():
    n = int(0.25 * SR)
    tt = np.arange(n) / SR
    tone = np.sin(2 * np.pi * 190 * tt) * np.exp(-tt * 30)
    nz = fft_filter(rng.standard_normal(n), lo=1500, hi=9000) * np.exp(-tt * 16)
    return tone * 0.5 + nz * 0.8


def note(freq, dur, harmonics=6, bright=0.55, attack=0.02, release=0.6, detune=0.0):
    n = int(dur * SR)
    tt = np.arange(n) / SR
    s = np.zeros(n)
    for h in range(1, harmonics + 1):
        for d in (-detune, detune) if detune else (0.0,):
            s += np.sin(2 * np.pi * freq * h * (1 + d) * tt + h * 1.3) * (bright ** (h - 1)) / h
    a = np.clip(tt / attack, 0, 1)
    r = np.clip((dur - tt) / release, 0, 1)
    return s * a * r


def hz(midi):
    return 440.0 * 2 ** ((midi - 69) / 12)


# --- 0-2 s: oscilloscope tone and accelerating ticks -------------------------
n0 = int(2.0 * SR)
tt = np.arange(n0) / SR
f_osc = 110 * 2 ** (tt / 2.0 * 1.5)
osc = np.sin(2 * np.pi * np.cumsum(f_osc) / SR) * np.clip(tt / 0.3, 0, 1) * (0.25 + 0.2 * tt)
osc += 0.3 * np.sin(2 * np.pi * np.cumsum(f_osc * 2.003) / SR) * np.clip(tt / 0.3, 0, 1) * 0.2
add(osc * 0.22, 0.0)
# ticks accelerate: intervals shrink towards t=2
tk = 0.12
k = 0
while tk < 1.95:
    add(tick(2400 + 900 * tk), tk, 0.18 + 0.1 * tk, pan=0.5 * np.sin(k * 1.7))
    tk += 0.25 * (1 - tk / 2.3)
    k += 1
swell = sweep_noise(1.4, 400, 9000) * np.linspace(0, 1, int(1.4 * SR)) ** 3
add(swell * 0.35, 0.6)

# --- 2.0 impact -------------------------------------------------------------
add(boom(), 2.0, 0.95)
shimmer = fft_filter(rng.standard_normal(int(2.5 * SR)), lo=5000) * env_exp(int(2.5 * SR), 0.5)
add(shimmer * 0.12, 2.0, pan=-0.3)

# --- 2.0-6.5 pulse + pad -----------------------------------------------------
BEAT = 0.5
for b in range(9):  # 2.0 .. 6.0
    st = 2.0 + b * BEAT
    if b > 0:
        add(kick(0.75), st)
    add(hat(), st + 0.25, 0.10, pan=0.4)
# pad: A minor 9 (A2 E3 G3 B3 C4)
for i, m in enumerate([45, 52, 55, 59, 60]):
    add(note(hz(m), 4.6, harmonics=5, bright=0.45, attack=0.6, release=1.4, detune=0.004), 2.0, 0.07, pan=(i - 2) * 0.3)
# bass on beats
for b in range(1, 9):
    add(note(hz(33), 0.45, harmonics=3, bright=0.4, attack=0.005, release=0.2), 2.0 + b * BEAT, 0.28)
# word hits
for st, m in ((2.3, 76), (3.3, 79), (4.3, 83)):
    add(note(hz(m), 0.9, harmonics=3, bright=0.3, attack=0.003, release=0.8), st, 0.12, pan=-0.3)
    add(tick(5200, 0.03), st, 0.25)

# --- 6.3-7.0 whoosh into the object ------------------------------------------
wh = sweep_noise(0.75, 300, 12000, lo=150)
wh *= np.sin(np.linspace(0, np.pi, len(wh))) ** 1.5
add(wh * 0.55, 6.3)
add(boom(1.2) * 0.55, 7.0)

# --- 7.0-10.5 driving section -------------------------------------------------
arp = [45, 57, 52, 57, 48, 57, 55, 57]
for b in range(7):
    st = 7.0 + b * BEAT
    add(kick(0.9), st)
    for s in range(2):
        add(hat(), st + 0.25 * s + 0.125, 0.09 + 0.03 * s, pan=0.5 if s else -0.5)
    if b % 2 == 1:
        add(snare(), st, 0.35)
    # word hit
    add(tick(4200, 0.04), st, 0.22)
for s16 in range(28):
    st = 7.0 + s16 * 0.125
    m = arp[s16 % 8] + (12 if s16 >= 16 else 0)
    add(note(hz(m), 0.14, harmonics=7, bright=0.5 + 0.01 * s16, attack=0.002, release=0.08), st, 0.10, pan=0.35 * np.sin(s16))
# snare roll + riser into the white-out
for i in range(16):
    st = 9.5 + i * (1.0 / 16)
    add(snare(), st, 0.06 + 0.2 * i / 16)
riser = sweep_noise(1.6, 500, 16000, lo=300) * np.linspace(0, 1, int(1.6 * SR)) ** 2.5
add(riser * 0.45, 9.0)

# --- 10.5 white-out impact and final chord -----------------------------------
add(boom(3.5), 10.5, 1.0)
add(shimmer * 0.2, 10.5, pan=0.3)
# F maj9 -> resolves the A minor colour (F2 C3 E3 G3 A3 + E5)
for i, m in enumerate([41, 48, 52, 55, 57, 76]):
    add(note(hz(m), 4.5, harmonics=5, bright=0.4, attack=0.9 if m < 70 else 1.5, release=2.2, detune=0.003), 10.55, 0.075 if m < 70 else 0.04, pan=(i - 2.5) * 0.25)
# title hits at 11.1 and 12.4
add(note(hz(88), 2.2, harmonics=2, bright=0.2, attack=0.002, release=2.0), 11.1, 0.06, pan=0.2)
add(note(hz(81), 2.5, harmonics=2, bright=0.2, attack=0.002, release=2.4), 12.4, 0.05, pan=-0.2)
add(tick(3000, 0.05), 14.75, 0.12)

# --- reverb (convolution with decaying stereo noise) ---------------------------
ir_n = int(2.2 * SR)
irt = np.arange(ir_n) / SR
irL = rng.standard_normal(ir_n) * np.exp(-irt / 0.45)
irR = rng.standard_normal(ir_n) * np.exp(-irt / 0.45)
irL = fft_filter(irL, hi=7000)
irR = fft_filter(irR, hi=7000)


def conv(x, ir):
    m = len(x) + len(ir)
    nfft = 1 << (m - 1).bit_length()
    y = np.fft.irfft(np.fft.rfft(x, nfft) * np.fft.rfft(ir, nfft), nfft)[: len(x)]
    return y / np.sqrt(np.sum(ir ** 2))


wetL, wetR = conv(L, irL), conv(R, irR)
L2 = L + 0.28 * wetL
R2 = R + 0.28 * wetR

# master: gentle limiter, fade out last 0.35 s
mix = np.stack([L2, R2], 1)
mix /= np.max(np.abs(mix)) / 1.6
mix = np.tanh(mix) * 0.9
fade = np.clip((DUR - t_all) / 0.35, 0, 1)[:, None]
mix *= fade
pcm = (mix * 32767).astype(np.int16)
with wave.open("assets/score.wav", "wb") as w:
    w.setnchannels(2)
    w.setsampwidth(2)
    w.setframerate(SR)
    w.writeframes(pcm.tobytes())
print("wrote assets/score.wav", pcm.shape)
