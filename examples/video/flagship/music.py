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


def lowpass(x, cut):
    """One-pole low-pass with a per-sample cutoff (array or scalar)."""
    cut = np.broadcast_to(np.asarray(cut, float), x.shape)
    a = 1 - np.exp(-2 * np.pi * cut / SR)
    y = np.zeros_like(x)
    acc = 0.0
    for i in range(len(x)):
        acc += a[i] * (x[i] - acc)
        y[i] = acc
    return y


def saw(f, t):
    return 2 * ((f * t) % 1) - 1


def marimba(m, vel=1.0):
    t = tvec(0.8)
    f = midi(m)
    return vel * 0.18 * (np.sin(2 * np.pi * f * t) * np.exp(-t / 0.25) + 0.3 * np.sin(2 * np.pi * f * 3.9 * t)
                         * np.exp(-t / 0.05))


def string_stab(ms, dur=0.18, vel=1.0):
    t = tvec(dur + 0.15)
    s = sum(saw(midi(m) * (1 + d), t) for m in ms for d in (-0.003, 0.003))
    s = lowpass(s / (2 * len(ms)), 2600)
    return vel * 0.22 * s * env_adsr(len(t), 0.01, 0.05, 0.7, 0.12, sustain_len=dur - 0.06)


def acid(m, dur=0.22, vel=1.0, sweep=1.0):
    t = tvec(dur)
    x = saw(midi(m), t)
    cut = 300 + 4200 * sweep * np.exp(-t / 0.07)
    y = lowpass(lowpass(x, cut), cut * 1.2)
    return vel * 0.35 * np.tanh(3 * y) * np.exp(-t / 0.16)


def clap(vel=1.0):
    t = tvec(0.25)
    n = rng.standard_normal(len(t))
    n = n - np.concatenate([[0], n[:-1]])
    e = sum(np.exp(-np.maximum(0, t - o) / 0.008) * (t >= o) for o in (0, 0.01, 0.02)) + np.exp(-t / 0.08) * 0.6
    return vel * 0.13 * n * e


def brass(ms, dur=0.35, vel=1.0):
    t = tvec(dur + 0.2)
    s = sum(saw(midi(m) * (1 + d), t) for m in ms for d in (-0.004, 0.0, 0.004))
    cut = 800 + 3000 * np.minimum(1, t / 0.06) * np.exp(-t / 0.4)
    y = lowpass(s / (3 * len(ms)), cut)
    return vel * 0.3 * y * env_adsr(len(t), 0.03, 0.1, 0.8, 0.2, sustain_len=dur - 0.13)


def tom(f=110, vel=1.0):
    t = tvec(0.4)
    ff = f * (1 + 0.5 * np.exp(-t / 0.03))
    return vel * 0.5 * np.sin(2 * np.pi * np.cumsum(ff) / SR) * np.exp(-t / 0.18)


def reverse_swell(m, dur=1.2):
    s = piano(m, dur, 0.8)[::-1] + pad([midi(m), midi(m + 7)], dur, a=0.05, r=0.05)[::-1] * 0.6
    return s


def build():
    W = Wd
    # Room tone under everything
    room = rng.standard_normal(N) * 0.004
    room = np.convolve(room, np.ones(40) / 40, mode="same")
    L[:] += room
    R[:] += np.roll(room, 997)

    # --- Act 1 (0-12): felt piano, shimmer; a glass tone as each request appears; flips ripple
    add(pad([midi(81), midi(88)], 12.5, a=2.5, r=3), 0.0, 0.8, rev=0.6)
    for tt, m in [(0.0, 57), (1.0, 64), (2.0, 60), (3.0, 64)]:
        add(piano(m, 3.0, 0.9), tt, pan=-0.15, rev=0.5)
    for k in range(W.LANES):
        add(tink(2400 * 2 ** (k / 12 * 2), 0.5), W.T["reveal0"] + 0.55 * k, 0.7, pan=-0.5 + 0.2 * k, rev=0.7)
    add(noise_swell(0.8, True, 800, 9000) * 0.5, W.T["flip"] - 0.8, rev=0.3)
    for k in range(W.LANES):
        add(tink(3200 * 2 ** (k * 2 / 12)), W.T["flip"] + k * W.T["flip_step"], 0.75, pan=-0.5 + 0.2 * k, rev=0.7)
    for bar in range(3, 6):
        ch = chord_at(bar)
        add(sub(ch[0] - 24, 2.0), bar * W.BAR, 0.8)
        for k, m in enumerate(ch):
            add(piano(m, 2.5, 0.6), bar * W.BAR + k * B * 0.5, pan=-0.3 + 0.3 * k, rev=0.5)
    for k in range(12):
        add(click(0.35, 3800), 7.8 + k * B * 0.6, pan=0.4, rev=0.2)

    # --- Act 2a (10-18): pulse bass; six drops into the machine; LEDs; the roll
    for k in range(int((18 - 10) / (B / 2))):
        tt = 10 + k * B / 2
        add(pluck_bass(chord_at(int(tt / W.BAR))[0] - 12), tt, 0.8)
    add(pad([midi(m) for m in A_MIN], 8.5, a=1.0, r=1.5), 10.0, 1.3, rev=0.4)
    add(sub(33, 8.0), 10.0, 0.5)
    for k in range(W.LANES):
        add(clunk(0.9), W.drop_time(k), 0.85, pan=-0.4 + 0.16 * k, rev=0.3)
    for k in range(W.LANES):
        add(click(0.7, 2400 + 120 * k), W.T["led"] + 0.04 * k, pan=-0.4 + 0.16 * k)
    add(noise_swell(1.0, True, 200, 2000) * 0.4, W.T["frame1_0"])
    for k, m in enumerate([72, 76, 79, 84]):
        add(chime(m), W.T["frame1_1"] + 0.06 * k, 0.6, pan=-0.3 + 0.2 * k, rev=0.7)

    # --- Act 2b (18-26): kick on 1 & 3, a tick per frame accelerating into a whir, riser into the run
    for k in range(int((26 - 18) / B)):
        tt = 18 + k * B
        if k % 2 == 0:
            add(kick(0.9), tt)
        ch = chord_at(int(tt / W.BAR))
        add(pluck_bass(ch[0] - 12), tt, 0.8)
        add(pluck_bass(ch[0] - 12), tt + B / 2, 0.6)
    prev, tt, dt, t_whir0 = W.emitted(W.T["stream0"]), W.T["stream0"], 1 / 600, None
    while tt < W.T["stream1"]:
        n = W.emitted(tt)
        rate = (n - prev) / dt
        if int(n) != int(prev) and rate < 30:
            add(hat(0.9, 0.02), tt, pan=0.25)
            add(click(0.4, 5200), tt, pan=0.25)
        if rate >= 30 and t_whir0 is None:
            t_whir0 = tt
        prev = n
        tt += dt
    if t_whir0 is not None:
        t = tvec(W.T["stream1"] - t_whir0)
        ts = t_whir0 + t
        rates = np.array([(W.emitted(x + 0.005) - W.emitted(x)) / 0.005 for x in ts[::48]])
        rates = np.interp(np.arange(len(t)), np.arange(0, len(t), 48)[:len(rates)], rates)
        f = 220 * 2 ** (np.log2(np.maximum(rates, 30) / 30) / np.log2(400 / 30) * 2)
        ph = 2 * np.pi * np.cumsum(f) / SR
        g = np.minimum(1, t / 1.5) * (0.6 + 0.4 * (t / t[-1]))
        whir = (np.sin(ph) + 0.3 * np.sin(2 * ph) + 0.15 * np.sin(3 * ph) + 0.2 * np.sin(1.5 * ph)) * g
        add(0.09 * whir, t_whir0, pan=0.1, rev=0.3)
    add(sub(33, 1.0) * 1.2, W.T["stream1"], 1.0)
    add(noise_swell(W.T["run0"] - 24.0, True, 300, 12000), 24.0, 1.0, rev=0.4)
    for k in range(8):
        add(snare(0.25 + 0.08 * k), W.T["run0"] - 1.0 + k * 0.125, rev=0.2)

    # --- Act 3 (26-50): the run. One kick, six outfits, a hop whoosh into every lane.
    run0 = W.T["run0"]
    progs = [[C_MAJ, G_MAJ], [A_MIN, F_MAJ], [F_MAJ, C_MAJ], [A_MIN, G_MAJ], [F_MAJ, A_MIN], [C_MAJ, G_MAJ]]
    for lane in range(W.LANES):
        t0 = run0 + lane * W.RUN_LANE_SECONDS
        for bar in range(2):
            b0 = t0 + bar * W.BAR
            ch = progs[lane][bar]
            root = ch[0] - 12
            soft = lane == 4
            for k in range(4):
                add(kick(0.6 if soft else 1.0), b0 + k * B)
            add(sub(root - 12, W.BAR), b0, 0.8 if not soft else 1.0)
            if lane == 0:      # Sundial: warm piano house
                for k in range(4):
                    for m in ch:
                        add(piano(m + 12, 0.6, 0.55), b0 + k * B + B / 2, rev=0.4)
                    add(hat(0.6), b0 + k * B + B / 2, pan=0.3)
                for k in range(8):
                    add(pluck_bass(root), b0 + k * B / 2, 0.7)
            elif lane == 1:    # Maison Lune: string staccato, minor, elegant
                for k in range(8):
                    add(string_stab([m + 12 for m in ch], 0.16, 0.9 if k % 2 == 0 else 0.6), b0 + k * B / 2,
                        pan=-0.2 + 0.4 * (k % 2), rev=0.5)
                add(snare(0.5), b0 + 2 * B, rev=0.5)
            elif lane == 2:    # Rain: marimba arpeggio and clicks
                arp = [ch[0] + 12, ch[1] + 12, ch[2] + 12, ch[1] + 24]
                for k in range(16):
                    add(marimba(arp[k % 4], 0.9), b0 + k * B / 4, pan=-0.4 + 0.8 * ((k * 3) % 8) / 7, rev=0.35)
                    add(click(0.25, 6000), b0 + k * B / 4 + 0.06, pan=0.5)
            elif lane == 3:    # No Sleep: acid stabs and claps
                pat = [0, 0, 12, 0, 3, 0, 12, 7, 0, 0, 12, 0, 3, 15, 12, 7]
                for k in range(16):
                    add(acid(root + 12 + pat[k], 0.12, 0.8, sweep=0.5 + 0.5 * ((k % 4) == 0)), b0 + k * B / 4,
                        pan=0.15)
                add(clap(1.0), b0 + B, rev=0.3)
                add(clap(1.0), b0 + 3 * B, rev=0.3)
                for k in range(8):
                    add(hat(0.8, 0.015), b0 + k * B / 2 + B / 4, pan=0.4)
            elif lane == 4:    # Fold: bloom and reversed swells
                add(pad([midi(m + 12) for m in ch] + [midi(ch[0] + 24)], W.BAR + 0.5, a=0.6, r=0.8, bright=0.9),
                    b0, 1.0, rev=0.9)
                rs = reverse_swell(ch[2] + 24, 1.0)
                add(rs, b0 + W.BAR - 1.0, 0.6, pan=0.3, rev=0.6)
            elif lane == 5:    # Harbour Cup: brass hits and stadium drums
                hits = [0, 1.5, 2.5, 3.0] if bar == 0 else [0, 1.0, 2.0, 3.5]
                for hb in hits:
                    add(brass([m + 12 for m in ch] + [ch[0] + 24], 0.3, 1.0), b0 + hb * B, rev=0.45)
                for k in range(4):
                    add(tom(90 + 20 * (k % 2), 0.9), b0 + k * B + B / 2, pan=-0.3 + 0.6 * (k % 2), rev=0.3)
                add(snare(1.0), b0 + B, rev=0.5)
                add(snare(1.0), b0 + 3 * B, rev=0.5)
        if lane < W.LANES - 1:  # the hop into the next lane
            t_land = t0 + W.RUN_LANE_SECONDS
            add(noise_swell(W.T["hop"], True, 500, 9000) * 0.9, t_land - W.T["hop"], pan=-0.1, rev=0.2)
            add(tink(2600, 0.6), t_land, 0.6, rev=0.5)
    # a lead line over the last two lanes ties it together
    for k, (tt, m, d) in enumerate([(0, 76, 1.0), (1.0, 79, 0.5), (1.5, 81, 0.5), (2.0, 79, 1.0), (3.0, 76, 1.0)]):
        add(lead(m, d * 0.95), run0 + 20.0 + tt * 0.5 * 2, 0.9, pan=-0.1, rev=0.45)

    # --- Act 4 (50-58): brake, fold, land, crush, shrink, six tiles
    t = tvec(W.T["brake1"] - W.T["run1"])
    f = 110 * (1 - t / t[-1]) ** 2 + 30
    add(0.25 * np.sin(2 * np.pi * np.cumsum(f) / SR) * (1 - t / t[-1]), W.T["run1"], rev=0.2)
    add(noise_swell(W.T["fold1"] - W.T["fold0"], True, 200, 6000) * 1.2, W.T["fold0"], rev=0.3)
    add(sub(28, 2.5) * 1.6, W.T["fold1"], 1.0)
    add(kick(1.4), W.T["fold1"])
    add(tink(1800, 1.2), W.T["fold1"], 0.9, rev=0.9)
    t = tvec(W.T["crush1"] - W.T["crush0"])
    crunch = np.round(rng.standard_normal(len(t)) * np.exp(-((t / t[-1] - 0.5) ** 2) / 0.08) * 6) / 6
    add(0.08 * crunch, W.T["crush0"], rev=0.2)
    add(clunk(1.0), W.T["crush1"], 0.9)
    t = tvec(W.T["shrink1"] - W.T["shrink0"])
    f = 600 * 2 ** (2 * t / t[-1])
    add(0.05 * np.sin(2 * np.pi * np.cumsum(f) / SR) * np.sin(np.pi * t / t[-1]), W.T["shrink0"], rev=0.6)
    for k in range(W.LANES):
        add(tink(3600 + 300 * k, 0.5), W.T["shrink1"] + 0.08 * k, 0.6, pan=-0.5 + 0.2 * k, rev=0.6)
    add(pad([midi(m) for m in C_MAJ] + [midi(67)], 6.5, a=1.5, r=1.5, bright=0.8), 52.0, 1.8, rev=0.6)
    add(sub(36, 6.0), 52.0, 0.6)

    # --- Act 5 (57-72): six screens light; the melody resolves; the logo chord
    for k in range(W.LANES):
        add(chime([72, 74, 76, 79, 81, 84][k], 0.8), W.T["rise0"] + 0.15 * k + 1.5, 0.7, pan=-0.5 + 0.2 * k, rev=0.7)
    for tt, m in [(59.0, 76), (60.0, 79), (61.0, 77), (62.0, 76), (63.0, 74), (64.0, 72), (65.0, 79)]:
        add(piano(m, 3.0, 0.8), tt, pan=0.1, rev=0.6)
    for bar in range(29, 33):
        ch = [C_MAJ, F_MAJ, A_MIN, G_MAJ][(bar - 29) % 4]
        add(pad([midi(m) for m in ch] + [midi(ch[0] + 12)], 2.3, a=0.4, r=0.8), bar * W.BAR, 0.6, rev=0.5)
        add(sub(ch[0] - 24, 2.0), bar * W.BAR, 0.7)
        for k in range(4):
            add(kick(0.55), bar * W.BAR + k * B)
            add(hat(0.4), bar * W.BAR + k * B + B / 2, pan=0.3)
    add(noise_swell(1.0, True, 400, 8000) * 0.6, W.T["part0"] - 0.9, rev=0.4)
    final = [48, 55, 60, 64, 67, 72, 76]
    add(pad([midi(m) for m in final], 6.0, a=0.8, r=3.0, bright=0.6), W.T["logo0"], 1.0, rev=0.8)
    for k, m in enumerate(final):
        add(piano(m, 6.0, 0.7), W.T["logo0"] + 0.04 * k, pan=-0.4 + 0.13 * k, rev=0.7)
    add(tink(4200, 0.8), W.T["logo1"], 0.7, rev=0.9)
    add(sub(36, 4.0), W.T["logo0"], 0.7)


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
