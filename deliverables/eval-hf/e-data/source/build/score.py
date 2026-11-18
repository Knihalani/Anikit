# Synthesised score for the film, timed to the scene clock in compositions/map.html (T = {...}).
import numpy as np, wave, sys
SR = 48000; DUR = 30.0; N = int(SR * DUR)
t = np.arange(N) / SR
rng = np.random.default_rng(1979)
L = np.zeros(N); R = np.zeros(N)

def env_adsr(n, a, d, peak=1.0):
    e = np.ones(n) * peak
    na = int(a * SR); nd = int(d * SR)
    e[:na] = np.linspace(0, peak, na) if na else e[:na]
    if nd: e[-nd:] = e[-nd:] * np.linspace(1, 0, nd) ** 2
    return e

def fft_filter(x, lo=None, hi=None, slope=6.0):
    X = np.fft.rfft(x); f = np.fft.rfftfreq(len(x), 1 / SR)
    g = np.ones_like(f)
    if hi: g *= 1 / (1 + (f / hi) ** slope)
    if lo: g *= 1 / (1 + (lo / np.maximum(f, 1e-3)) ** slope)
    return np.fft.irfft(X * g, len(x))

def place(sig, at, gain=1.0, pan=0.0):
    i = int(at * SR); j = min(N, i + len(sig))
    if j <= i: return
    l = np.cos((pan + 1) * np.pi / 4); r = np.sin((pan + 1) * np.pi / 4)
    L[i:j] += sig[: j - i] * gain * l * 1.414; R[i:j] += sig[: j - i] * gain * r * 1.414

# ---------- pad ----------
def saw(f, tt, harm=14, phase=0.0):
    out = np.zeros_like(tt)
    for n in range(1, harm + 1):
        if f * n > 9000: break
        out += np.sin(2 * np.pi * f * n * tt + phase * n) / n ** 1.7
    return out

def chord_layer(freqs, t0, t1, fade_in, fade_out, amp):
    i0 = int(t0 * SR); i1 = int(min(DUR, t1) * SR); tt = t[i0:i1]
    sigL = np.zeros_like(tt); sigR = np.zeros_like(tt)
    for k, f in enumerate(freqs):
        for d, side in ((-0.06, 0), (0.0, 2), (0.07, 1)):
            ph = rng.uniform(0, 6.28)
            lfo = 1 + 0.18 * np.sin(2 * np.pi * (0.07 + 0.03 * k) * tt + ph)
            v = saw(f * 2 ** (d / 12), tt, phase=ph) * lfo
            if side in (0, 2): sigL += v
            if side in (1, 2): sigR += v
    e = np.ones_like(tt)
    nfi = int(fade_in * SR); nfo = int(fade_out * SR)
    e[:nfi] = np.sin(np.linspace(0, np.pi / 2, nfi)) ** 2
    e[-nfo:] *= np.cos(np.linspace(0, np.pi / 2, nfo)) ** 2
    L[i0:i1] += sigL * e * amp; R[i0:i1] += sigR * e * amp

Dm = [73.42, 146.83, 220.0, 329.63, 349.23]           # D2 D3 A3 E4 F4
Bb = [58.27, 116.54, 174.61, 220.0, 293.66, 349.23]   # Bb1 Bb2 F3 A3 D4 F4
Dsus = [73.42, 146.83, 220.0, 293.66, 329.63, 440.0]  # D2 D3 A3 D4 E4 A4
chord_layer(Dm, 0.0, 22.6, 2.8, 1.4, 0.022)
chord_layer(Bb, 21.55, 26.4, 0.25, 1.6, 0.026)
chord_layer(Dsus, 25.0, 30.0, 1.4, 2.6, 0.022)
# high shimmer while ice grows / holds
shim_i0 = int(2.0 * SR); shim = np.zeros(N - shim_i0); tt = t[shim_i0:]
for f in (880.0, 1318.5, 1760.0):
    shim += np.sin(2 * np.pi * f * tt + rng.uniform(0, 6)) * (0.5 + 0.5 * np.sin(2 * np.pi * 0.21 * tt + f))
se = np.clip((tt - 2.0) / 2.0, 0, 1) * np.clip((18.8 - tt) / 2.0, 0, 1)
L[shim_i0:] += shim * se * 0.004; R[shim_i0:] += np.roll(shim, 900) * se * 0.004

# ---------- wind ----------
for ch, seed in ((L, 1), (R, 2)):
    w = np.random.default_rng(seed).standard_normal(N)
    w = fft_filter(w, lo=180, hi=900, slope=2)
    gust = 0.55 + 0.45 * np.sin(2 * np.pi * 0.09 * t + seed) * np.sin(2 * np.pi * 0.033 * t + 2 * seed)
    we = np.clip(t / 2.5, 0, 1) * np.clip((30 - t) / 3.0, 0, 1) * (1 - 0.5 * np.clip((t - 21.7) / 0.3, 0, 1) * np.clip((25.5 - t) / 1.0, 0, 1))
    ch += w * gust * we * 0.010

# ---------- one-shots ----------
def tick(seed, bright):
    n = int(0.03 * SR); x = np.random.default_rng(seed).standard_normal(n)
    x = fft_filter(x, lo=bright, hi=bright * 3, slope=4)
    return x * np.exp(-np.arange(n) / (0.0035 * SR))

def thump(f0=62, f1=38, dur=0.9, seed=0):
    n = int(dur * SR); tt = np.arange(n) / SR
    f = f1 + (f0 - f1) * np.exp(-tt / 0.08)
    ph = 2 * np.pi * np.cumsum(f) / SR
    return np.sin(ph) * np.exp(-tt / (dur / 4)) * np.clip(tt / 0.004, 0, 1)

def crack(seed, dur=0.45, n_imp=34):
    g = np.random.default_rng(seed); n = int(dur * SR); x = np.zeros(n)
    times = np.sort(g.exponential(dur / 6, n_imp)); times = times[times < dur * 0.9]
    for k, tm in enumerate(times):
        m = int(0.004 * SR); burst = g.standard_normal(m) * np.exp(-np.arange(m) / (0.0009 * SR))
        i = int(tm * SR); x[i:i + m] += burst[: n - i] * (0.4 + 0.6 * g.random()) * np.exp(-tm / (dur / 2.5))
    return fft_filter(x, lo=700, hi=6000, slope=3)

def whoosh(seed, dur=0.7, f0=300, f1=1800):
    n = int(dur * SR); x = np.random.default_rng(seed).standard_normal(n); out = np.zeros(n)
    K = 12
    for k in range(K):
        a, b = int(k * n / K), int((k + 1) * n / K)
        fc = f0 * (f1 / f0) ** (k / (K - 1))
        seg = fft_filter(x, lo=fc * 0.6, hi=fc * 1.6, slope=3)
        w = np.zeros(n); w[a:b] = 1
        out += seg * w
    e = np.sin(np.linspace(0, np.pi, n)) ** 1.6
    return fft_filter(out * e, hi=6000, slope=2)

def riser(seed, dur=1.0):
    n = int(dur * SR); tt = np.arange(n) / SR
    x = np.random.default_rng(seed).standard_normal(n); out = np.zeros(n); K = 16
    for k in range(K):
        a, b = int(k * n / K), int((k + 1) * n / K)
        fc = 400 * (5000 / 400) ** (k / (K - 1))
        w = np.zeros(n); w[a:b] = 1
        out += fft_filter(x, lo=fc * 0.5, hi=fc * 1.5, slope=3) * w
    tone = np.sin(2 * np.pi * np.cumsum(110 * 2 ** (2 * tt / dur)) / SR) * 0.25
    e = (tt / dur) ** 2.4
    return (out + tone) * e

# year ticks: same clock as yearAt() in map.html
def year_at(tt):
    u = np.clip((tt - 5.0) / 11.0, 0, 1); s = u * u * (3 - 2 * u)
    return 1979 + 47 * (0.55 * u + 0.45 * s)
ts = np.arange(4.9 * SR, 16.2 * SR) / SR
yi = np.round(year_at(ts)); ch_idx = np.nonzero(np.diff(yi))[0] + 1
TK = [tick(11, 2600), tick(12, 3200), tick(13, 2200)]
for k, i in enumerate(ch_idx):
    yr = int(yi[i])
    place(TK[k % 3], ts[i] + 1 / 30, gain=0.10 if yr != 2012 else 0.16, pan=-0.25 if k % 2 else 0.25)
print("ticks", len(ch_idx), file=sys.stderr)

place(thump(70, 40, 1.4), 2.22, 0.16)                  # ice cap forms
place(whoosh(21, 0.9, 250, 1400), 2.0, 0.09, -0.2)
place(whoosh(22, 0.6, 500, 2400), 4.0, 0.07, 0.3)      # headline -> panel
place(thump(55, 40, 0.8), 16.03, 0.09)                 # run lands on 2026
place(crack(31), 16.72, 0.22, 0.35)                    # 2012 record low
place(thump(90, 50, 0.4), 16.72, 0.06)
place(whoosh(23, 0.7, 350, 1600), 19.55, 0.07, 0.2)    # -36%
place(thump(60, 42, 0.7), 20.03, 0.10)
place(riser(41, 1.0), 20.72, 0.11)                     # into the loss
place(thump(52, 30, 2.4), 21.73, 0.32)                 # impact
place(crack(32, 0.8, 60), 21.73, 0.30, -0.3)
place(crack(33, 0.7, 40), 21.95, 0.18, 0.4)
place(whoosh(24, 1.2, 1600, 250), 25.1, 0.08, -0.1)    # pull back to the end card

# ---------- reverb ----------
def reverb(x, seed, secs=3.2, mix=0.28):
    n = int(secs * SR); g = np.random.default_rng(seed)
    ir = g.standard_normal(n) * np.exp(-np.arange(n) / (secs / 6.5 * SR))
    ir = fft_filter(ir, hi=5000, slope=2); ir /= np.sqrt(np.sum(ir ** 2))
    m = len(x) + n; sz = 1 << int(np.ceil(np.log2(m)))
    y = np.fft.irfft(np.fft.rfft(x, sz) * np.fft.rfft(ir, sz), sz)[: len(x)]
    return x * (1 - mix) + y * mix * 1.6
L = reverb(L, 5); R = reverb(R, 6)
L = fft_filter(L, lo=28, slope=4); R = fft_filter(R, lo=28, slope=4)
fade = np.clip((30 - t) / 1.2, 0, 1) ** 2
L *= fade; R *= fade
pk = max(np.abs(L).max(), np.abs(R).max()); L /= pk * 1.12; R /= pk * 1.12
pcm = (np.stack([L, R], 1) * 32767).astype(np.int16)
with wave.open(sys.argv[1], "wb") as w:
    w.setnchannels(2); w.setsampwidth(2); w.setframerate(SR); w.writeframes(pcm.tobytes())
