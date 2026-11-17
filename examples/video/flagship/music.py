"""EVERY FRAME score: 72 s, 120 BPM, A minor -> C major, synthesised with numpy.

Every sync point comes from world.py, so the music follows the picture.

    python3 examples/video/flagship/music.py      # writes music.mp3 (and music.wav in the scratch dir if given)
"""

from __future__ import annotations

import subprocess
import sys
import wave
from pathlib import Path

import numpy as np

HERE = Path(__file__).parent
sys.path.insert(0, str(HERE))
import world as Wd  # noqa: E402

SR = 48000
DUR = Wd.DURATION
N = int(DUR * SR)
T = Wd.T
B = Wd.BEAT
rng = np.random.default_rng(7)

L = np.zeros(N)
R = np.zeros(N)
REV = np.zeros((2, N))   # send to reverb


def midi(m):
    return 440.0 * 2 ** ((m - 69) / 12)


def at(t):
    return int(round(t * SR))


def env_adsr(n, a, d, s, r, sustain_len=None):
    a_n, d_n, r_n = int(a * SR), int(d * SR), int(r * SR)
    sus = n - a_n - d_n - r_n if sustain_len is None else int(sustain_len * SR)
    sus = max(0, sus)
    e = np.concatenate([np.linspace(0, 1, max(1, a_n)), np.linspace(1, s, max(1, d_n)), np.full(sus, s),
                        np.linspace(s, 0, max(1, r_n))])
    return e[:n] if len(e) >= n else np.pad(e, (0, n - len(e)))


def add(sig, t, gain=1.0, pan=0.0, rev=0.0):
    i = at(t)
    if i >= N:
        return
    sig = sig[: N - i]
    gl, gr = gain * np.sqrt((1 - pan) / 2), gain * np.sqrt((1 + pan) / 2)
    L[i:i + len(sig)] += sig * gl
    R[i:i + len(sig)] += sig * gr
    if rev:
        REV[0, i:i + len(sig)] += sig * gl * rev
        REV[1, i:i + len(sig)] += sig * gr * rev


def tvec(sec):
    return np.arange(int(sec * SR)) / SR


# --- instruments -------------------------------------------------------------

def piano(m, dur=2.5, vel=1.0):
    """Felt piano: a few inharmonic partials, soft attack, long decay."""
    f = midi(m)
    t = tvec(dur)
    s = np.zeros_like(t)
    for k, (amp, dec) in enumerate([(1, 1.6), (0.45, 1.0), (0.22, 0.7), (0.1, 0.45), (0.05, 0.3)], start=1):
        fk = f * k * (1 + 0.0004 * k * k)
        s += amp * np.sin(2 * np.pi * fk * t) * np.exp(-t / (dec * (1.2 if m < 60 else 1.0)))
    att = np.minimum(1, t / 0.008)
    felt = 1 - np.exp(-t / 0.002) * 0.6
    return vel * 0.22 * s * att * felt


def pad(freqs, dur, a=1.5, r=2.0, bright=0.4):
    t = tvec(dur)
    s = np.zeros_like(t)
    for f in freqs:
        for det in (-0.12, 0.0, 0.11):
            ff = f * 2 ** (det / 12)
            ph = rng.uniform(0, 2 * np.pi)
            s += np.sin(2 * np.pi * ff * t + ph) + bright * 0.3 * np.sin(4 * np.pi * ff * t + ph)
    s /= max(1, len(freqs) * 3)
    lfo = 0.85 + 0.15 * np.sin(2 * np.pi * 0.23 * t)
    return 0.16 * s * lfo * env_adsr(len(t), a, 0.5, 0.9, r)


def sub(m, dur):
    t = tvec(dur)
    return 0.32 * np.sin(2 * np.pi * midi(m) * t) * env_adsr(len(t), 0.02, 0.1, 0.9, 0.15)


def pluck_bass(m, dur=0.22):
    t = tvec(dur)
    f = midi(m)
    saw = 2 * ((f * t) % 1) - 1
    # one-pole low-pass with a decaying cutoff
    cut = 200 + 1600 * np.exp(-t / 0.05)
    y = np.zeros_like(saw)
    acc = 0.0
    for i in range(len(saw)):
        a = 1 - np.exp(-2 * np.pi * cut[i] / SR)
        acc += a * (saw[i] - acc)
        y[i] = acc
    return 0.22 * y * np.exp(-t / 0.18) + 0.18 * np.sin(2 * np.pi * f * t) * np.exp(-t / 0.2)


def kick(vel=1.0):
    t = tvec(0.45)
    f = 45 + 110 * np.exp(-t / 0.035)
    ph = 2 * np.pi * np.cumsum(f) / SR
    return vel * 0.9 * np.sin(ph) * np.exp(-t / 0.16) + vel * 0.15 * rng.standard_normal(len(t)) * np.exp(-t / 0.004)


def snare(vel=1.0):
    t = tvec(0.3)
    n = rng.standard_normal(len(t))
    n = n - np.concatenate([[0], n[:-1]]) * 0.6       # brighten
    return vel * (0.3 * n * np.exp(-t / 0.07) + 0.25 * np.sin(2 * np.pi * 190 * t) * np.exp(-t / 0.05))


def hat(vel=1.0, dec=0.025):
    t = tvec(0.12)
    n = rng.standard_normal(len(t))
    n = n - np.concatenate([[0], n[:-1]])
    return vel * 0.12 * n * np.exp(-t / dec)


def tink(f0=3200.0, vel=1.0):
    """Glass: inharmonic high partials with a long ring."""
    t = tvec(2.5)
    s = sum(a * np.sin(2 * np.pi * f0 * r * t) * np.exp(-t / d)
            for a, r, d in [(1, 1, 0.9), (0.6, 1.59, 0.6), (0.4, 2.33, 0.4), (0.25, 3.11, 0.25)])
    return vel * 0.08 * s


def clunk(vel=1.0):
    """Mechanical: a dull thump with a short metal transient."""
    t = tvec(0.6)
    thump = np.sin(2 * np.pi * (60 + 60 * np.exp(-t / 0.03)) * t) * np.exp(-t / 0.12)
    metal = sum(np.sin(2 * np.pi * f * t) for f in (820, 1330, 2210)) * np.exp(-t / 0.03)
    return vel * (0.7 * thump + 0.08 * metal)


def click(vel=1.0, f=2500):
    t = tvec(0.03)
    return vel * 0.2 * np.sin(2 * np.pi * f * t) * np.exp(-t / 0.003)


def shutter(vel=1.0):
    t = tvec(0.07)
    n = rng.standard_normal(len(t))
    e = np.exp(-t / 0.006) + 0.6 * np.exp(-np.maximum(0, t - 0.035) / 0.006) * (t > 0.035)
    return vel * 0.12 * n * e


def chime(m, vel=1.0):
    t = tvec(3.0)
    f = midi(m)
    s = np.sin(2 * np.pi * f * t) * np.exp(-t / 1.4) + 0.5 * np.sin(2 * np.pi * f * 2.0 * t) * np.exp(-t / 0.8) \
        + 0.2 * np.sin(2 * np.pi * f * 3.01 * t) * np.exp(-t / 0.4)
    return vel * 0.12 * s


def noise_swell(dur, up=True, lo=400, hi=6000):
    t = tvec(dur)
    n = rng.standard_normal(len(t))
    u = t / dur if up else 1 - t / dur
    cut = lo * (hi / lo) ** u
    y = np.zeros_like(n)
    acc = 0.0
    for i in range(len(n)):
        a = 1 - np.exp(-2 * np.pi * cut[i] / SR)
        acc += a * (n[i] - acc)
        y[i] = acc
    return 0.25 * y * (u ** 2)


def lead(m, dur, vel=1.0):
    t = tvec(dur)
    f = midi(m) * (1 + 0.003 * np.sin(2 * np.pi * 5.2 * t) * np.minimum(1, t / 0.3))
    ph = 2 * np.pi * np.cumsum(f) / SR
    s = np.sin(ph) + 0.35 * np.sin(2 * ph) + 0.12 * np.sin(3 * ph)
    return vel * 0.1 * s * env_adsr(len(t), 0.02, 0.15, 0.7, 0.25)


# --- the score ----------------------------------------------------------------
A_MIN = [57, 60, 64]      # A C E
F_MAJ = [53, 57, 60]
C_MAJ = [48, 52, 55]
G_MAJ = [55, 59, 62]
PROG = [A_MIN, F_MAJ, C_MAJ, G_MAJ]      # one chord per bar


def chord_at(bar):
    return PROG[bar % 4]


def build():
    # Room tone: very soft filtered noise through the whole film
    room = rng.standard_normal(N) * 0.004
    room = np.convolve(room, np.ones(40) / 40, mode="same")
    L[:] += room
    R[:] += np.roll(room, 997)

    # Act 1 (0-12): felt piano, shimmer
    add(pad([midi(81), midi(88)], 12.5, a=2.5, r=3), 0.0, 0.8, rev=0.6)
    mel1 = [(0.0, 57), (1.0, 64), (2.0, 60), (3.0, 64), (4.0, 57), (5.0, 62), (5.5, 64)]
    for tt, m in mel1:
        add(piano(m, 3.0, 0.9), tt, pan=-0.15, rev=0.5)
    # pane flip at 6.0: reverse swell into a glass tink
    add(noise_swell(0.9, True, 800, 9000) * 0.5, T["flip"] - 0.9, rev=0.3)
    add(tink(3200), T["flip"], 0.9, pan=0.2, rev=0.7)
    for bar in range(3, 6):                       # 6-12 s: two voices + sub
        ch = chord_at(bar)
        add(sub(ch[0] - 24, 2.0), bar * Wd.BAR, 0.8)
        for k, m in enumerate(ch):
            add(piano(m, 2.5, 0.6), bar * Wd.BAR + k * B * 0.5, pan=-0.3 + 0.3 * k, rev=0.5)
        add(piano(ch[2] + 12, 2.0, 0.55), bar * Wd.BAR + 1.5, pan=0.3, rev=0.6)
    for k in range(12):                           # spec lines light on 8ths
        add(click(0.35, 3800), 6.5 + k * B / 2 * 0.75, pan=0.4, rev=0.2)

    # Act 2a (12-20): pulse bass, the machine
    for k in range(int((20 - 12) / (B / 2))):
        tt = 12 + k * B / 2
        bar = int(tt / Wd.BAR)
        add(pluck_bass(chord_at(bar)[0] - 12), tt, 0.8)
    add(pad([midi(m) for m in A_MIN], 8.5, a=1.0, r=1.5), 12.0, 1.3, rev=0.4)
    add(sub(45 - 12, 6.0), 12.0, 0.5)
    add(clunk(1.0), T["enter1"], 0.9, rev=0.3)                 # pane drops home
    add(click(0.8, 2400), T["led"] + 0.05, pan=0.3)           # LED
    add(noise_swell(1.0, True, 200, 2000) * 0.4, T["frame1_0"])   # machine rolls
    add(chime(76), T["frame1_1"], 0.9, rev=0.7)                # frame 1 revealed
    add(chime(83), T["frame1_1"] + 0.25, 0.5, rev=0.7)

    # Act 2b (18-28.5): kick on 1 & 3; ticks per frame, accelerating into a whir
    for k in range(int((28.5 - 18) / B)):
        tt = 18 + k * B
        if k % 2 == 0:
            add(kick(0.9), tt)
        bar = int(tt / Wd.BAR)
        add(pluck_bass(chord_at(bar)[0] - 12), tt, 0.8)
        add(pluck_bass(chord_at(bar)[0] - 12), tt + B / 2, 0.6)
    # one tick per emitted frame while sparse; above ~30/s, a pitched whir that follows the rate
    t_whir0 = None
    prev = Wd.emitted(T["stream0"])
    tt = T["stream0"]
    dt = 1 / 600
    while tt < T["stream1"]:
        n = Wd.emitted(tt)
        rate = (n - prev) / dt
        if int(n) != int(prev) and rate < 30:
            add(hat(0.9, 0.02), tt, pan=0.25)
            add(click(0.4, 5200), tt, pan=0.25)
        if rate >= 30 and t_whir0 is None:
            t_whir0 = tt
        prev = n
        tt += dt
    if t_whir0 is not None:
        t = tvec(T["stream1"] - t_whir0)
        ts = t_whir0 + t
        rates = np.array([(Wd.emitted(x + 0.005) - Wd.emitted(x)) / 0.005 for x in ts[::48]])
        rates = np.interp(np.arange(len(t)), np.arange(0, len(t), 48)[:len(rates)], rates)
        f = 220 * 2 ** (np.log2(np.maximum(rates, 30) / 30) / np.log2(600 / 30) * 2)   # up two octaves
        ph = 2 * np.pi * np.cumsum(f) / SR
        g = np.minimum(1, t / 1.5) * (0.6 + 0.4 * (t / t[-1]))
        whir = (np.sin(ph) + 0.3 * np.sin(2 * ph) + 0.15 * np.sin(3 * ph) + 0.2 * np.sin(1.5 * ph)) * g
        add(0.09 * whir, t_whir0, pan=0.1, rev=0.3)
    # 3,600 lands: sub drop and a cymbal-ish wash
    add(sub(33, 1.8) * 1.4, T["stream1"], 1.0)
    add(kick(1.2), T["stream1"])
    add(noise_swell(1.5, False, 8000, 800) * 0.8, T["stream1"], rev=0.5)

    # Act 2c (30-40): half-time groove; the browser cart's shutter ratchet
    for bar in range(15, 20):
        b0 = bar * Wd.BAR
        ch = chord_at(bar)
        add(kick(0.9), b0)
        add(snare(0.8), b0 + 2 * B, rev=0.3)
        for k in range(8):
            add(hat(0.5 if k % 2 else 0.8), b0 + k * B / 2, pan=0.35)
        add(sub(ch[0] - 24, 2.0), b0, 0.9)
        add(pad([midi(m + 12) for m in ch], 2.3, a=0.3, r=0.6), b0, 0.5, rev=0.4)
    m_prev = Wd.browser_emitted(29.0)
    tt = 29.0
    while tt < 40.5:
        m = Wd.browser_emitted(tt)
        if int(m) != int(m_prev):
            near = np.clip(1 - abs(tt - 35) / 7, 0.25, 1)
            add(shutter(0.55 * near), tt, pan=-0.45)
        m_prev = m
        tt += 1 / 600
    add(tink(2600, 1.0), 36.0, 0.8, rev=0.6)      # "21x faster"
    add(kick(1.0), 36.0)

    # Act 3a (39-49.5): riser into the drop at lock0; drop = full groove + lead
    add(noise_swell(T["lock0"] - 38.5, True, 300, 12000), 38.5, 1.1, rev=0.4)
    for k in range(8):
        add(snare(0.25 + 0.08 * k), T["lock0"] - 1.0 + k * 0.125, rev=0.2)
    lead_line = [(0, 76, 1.0), (1.0, 79, 0.5), (1.5, 81, 0.5), (2.0, 79, 1.0), (3.0, 76, 0.5), (3.5, 74, 0.5),
                 (4.0, 72, 1.5), (5.5, 74, 0.5)]
    for bar in range(21, 24 + 1):
        b0 = bar * Wd.BAR
        ch = [C_MAJ, G_MAJ, A_MIN, F_MAJ][(bar - 21) % 4]
        for k in range(4):
            add(kick(1.0), b0 + k * B)
        for k in range(8):
            add(hat(0.9 if k % 2 else 0.5, 0.03), b0 + k * B / 2, pan=0.3)
        add(snare(0.9), b0 + B, rev=0.25)
        add(snare(0.9), b0 + 3 * B, rev=0.25)
        for k in range(8):
            add(pluck_bass(ch[0] - 12), b0 + k * B / 2, 0.85)
        add(pad([midi(m + 12) for m in ch], 2.2, a=0.05, r=0.4, bright=1.0), b0, 0.7, rev=0.4)
    for rep in range(2):
        for tt, m, d in lead_line:
            add(lead(m, d * 0.95), T["lock0"] + rep * 3.0 * 2 * 0 + rep * 3.0 + tt * 0.5, 1.0, pan=-0.1, rev=0.45)
    # brake: tape-stop style downward sweep
    t = tvec(T["brake1"] - T["lock1"])
    f = 110 * (1 - t / t[-1]) ** 2 + 30
    add(0.25 * np.sin(2 * np.pi * np.cumsum(f) / SR) * (1 - t / t[-1]), T["lock1"], rev=0.2)

    # Act 3b (50-58): fold -> stack lands -> crush -> shrink
    add(noise_swell(T["fold1"] - T["fold0"], True, 200, 6000) * 1.2, T["fold0"], rev=0.3)
    add(sub(28, 2.5) * 1.6, T["fold1"], 1.0)
    add(kick(1.4), T["fold1"])
    add(tink(1800, 1.2), T["fold1"], 0.9, rev=0.9)
    t = tvec(T["crush1"] - T["crush0"])
    crunch = rng.standard_normal(len(t)) * np.exp(-((t / t[-1] - 0.5) ** 2) / 0.08)
    crunch = np.round(crunch * 6) / 6
    add(0.08 * crunch, T["crush0"], pan=0.0, rev=0.2)
    add(clunk(1.0), T["crush1"], 0.9)
    t = tvec(T["shrink1"] - T["shrink0"])
    f = 600 * 2 ** (2 * t / t[-1])
    add(0.05 * np.sin(2 * np.pi * np.cumsum(f) / SR) * np.sin(np.pi * t / t[-1]), T["shrink0"], rev=0.6)
    add(pad([midi(m) for m in C_MAJ] + [midi(67)], 6.5, a=1.5, r=1.5, bright=0.8), 52.0, 1.8, rev=0.6)
    add(sub(36, 6.0), 52.0, 0.6)
    for k in range(12):
        add(piano([72, 76, 79, 84][k % 4], 1.5, 0.35), 52.0 + 3 + k * B / 2, pan=0.4 - 0.07 * k, rev=0.7)

    # Act 4 (58-72): tile clicks home, screen lights, melody resolves; cart callback; logo chord
    add(click(1.0, 1900), T["insert1"], 1.0)
    add(chime(84), T["screen"], 0.8, rev=0.7)
    res = [(58.0, 72), (59.0, 76), (60.0, 79), (61.0, 77), (62.0, 76), (63.0, 74), (64.0, 72), (65.0, 79)]
    for tt, m in res:
        add(piano(m, 3.0, 0.8), tt, pan=0.1, rev=0.6)
    for bar in range(29, 33):
        ch = [C_MAJ, F_MAJ, A_MIN, G_MAJ][(bar - 29) % 4]
        add(pad([midi(m) for m in ch] + [midi(ch[0] + 12)], 2.3, a=0.4, r=0.8), bar * Wd.BAR, 0.6, rev=0.5)
        add(sub(ch[0] - 24, 2.0), bar * Wd.BAR, 0.7)
    m_prev = Wd.browser_emitted(61.0)
    tt = 61.0
    while tt < 64.5:
        m = Wd.browser_emitted(tt)
        if int(m) != int(m_prev):
            add(shutter(0.35), tt, pan=-0.6, rev=0.3)
        m_prev = m
        tt += 1 / 600
    add(piano(60, 4, 0.9), 64.0, rev=0.6)              # "This film was one spec."
    add(piano(67, 4, 0.7), 64.0, rev=0.6)
    final = [48, 55, 60, 64, 67, 72, 76]
    add(pad([midi(m) for m in final], 6.0, a=0.8, r=3.0, bright=0.6), T["logo0"], 1.0, rev=0.8)
    for k, m in enumerate(final):
        add(piano(m, 6.0, 0.7), T["logo0"] + 0.04 * k, pan=-0.4 + 0.13 * k, rev=0.7)
    add(tink(4200, 0.8), T["logo1"], 0.7, rev=0.9)
    add(sub(36, 4.0), T["logo0"], 0.7)


def reverb():
    n = int(2.2 * SR)
    t = np.arange(n) / SR
    out = []
    for ch in range(2):
        ir = rng.standard_normal(n) * np.exp(-t / 0.55)
        ir = np.convolve(ir, np.ones(6) / 6, mode="same")
        ir[: int(0.02 * SR)] = 0
        ir /= np.sqrt((ir ** 2).sum())
        nfft = 1 << int(np.ceil(np.log2(N + n)))
        y = np.fft.irfft(np.fft.rfft(REV[ch], nfft) * np.fft.rfft(ir, nfft), nfft)[:N]
        out.append(y * 0.6)
    return out


def main():
    build()
    rl, rr = reverb()
    mix = np.stack([L + rl, R + rr])
    # gentle bus compression + limiter
    peak = np.max(np.abs(mix))
    mix = np.tanh(mix / peak * 1.6) / np.tanh(1.6) * 0.9
    fade = np.ones(N)
    fade[-int(1.5 * SR):] = np.linspace(1, 0, int(1.5 * SR)) ** 2
    mix *= fade
    pcm = (mix.T * 32767).astype(np.int16)
    wav = Path(sys.argv[1]) if len(sys.argv) > 1 else HERE / "music.wav"
    with wave.open(str(wav), "wb") as w:
        w.setnchannels(2)
        w.setsampwidth(2)
        w.setframerate(SR)
        w.writeframes(pcm.tobytes())
    subprocess.run(["ffmpeg", "-loglevel", "error", "-y", "-i", str(wav), "-c:a", "libmp3lame", "-b:a", "192k",
                    str(HERE / "music.mp3")], check=True)
    if len(sys.argv) <= 1:
        wav.unlink()


if __name__ == "__main__":
    main()
