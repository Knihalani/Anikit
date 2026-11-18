# Sound design for the film: cabin-roar noise, a pure tone for the physics shots,
# and the piano bed heard "through" the noise until the cancellation, then clean.
import numpy as np, wave
from scipy.signal import butter, sosfilt
sr = 48000; T = 30.0; n = int(sr * T); t = np.arange(n) / sr
rng = np.random.default_rng(3)

def read(p):
    from scipy.io import wavfile
    r, a = wavfile.read(p); a = a.astype(np.float32)
    if a.ndim == 1: a = a[:, None].repeat(2, 1)
    return a[:n] if len(a) >= n else np.pad(a, ((0, n - len(a)), (0, 0)))
def env(points):  # piecewise-linear gain curve [(time, gain)]
    ts, gs = zip(*points); return np.interp(t, ts, gs)
def lp(x, f): return sosfilt(butter(2, f, 'low', fs=sr, output='sos'), x, axis=0)
def bp(x, a, b): return sosfilt(butter(2, [a, b], 'band', fs=sr, output='sos'), x, axis=0)

# noise: brown rumble + engine hum harmonics + airy hiss, slightly decorrelated L/R
def roar(seed):
    r = np.random.default_rng(seed)
    brown = np.cumsum(r.standard_normal(n)); brown -= lp(brown, 8); brown /= np.abs(brown).max()
    hum = sum(a * np.sin(2 * np.pi * f * t * (1 + 0.004 * np.sin(2 * np.pi * 0.3 * t)) + ph)
              for f, a, ph in [(58, 0.5, 0), (116, 0.32, 1), (174, 0.18, 2), (232, 0.1, 0.5)])
    hiss = bp(r.standard_normal(n), 500, 4000) * 0.35
    return 0.9 * lp(brown, 400) + 0.35 * hum + hiss
noise = np.stack([roar(1), roar(2)], 1); noise /= np.abs(noise).max()
CUT = 18.15
ng = env([(0, 0), (0.05, 0.9), (3.5, 0.9), (4.3, 0.12), (7.4, 0.12), (7.6, 0.6), (11.9, 0.6), (12.1, 0.18), (16.3, 0.18),
          (16.45, 0.95), (CUT, 0.95), (CUT + 0.5, 0.0), (21.7, 0.0), (21.75, 0.05), (25.6, 0.05), (25.8, 0.0), (30, 0)])
# inside shot: outside world still faintly audible (ANC is not perfect), muffled
noise = noise * ng[:, None]

# pure tone for the "one simple tone" + flip shots (110 Hz with a soft octave)
tone = (np.sin(2 * np.pi * 110 * t) + 0.25 * np.sin(2 * np.pi * 220 * t))
tg = env([(0, 0), (3.6, 0), (4.4, 0.32), (7.3, 0.32), (7.6, 0), (12.0, 0), (12.3, 0.28), (13.0, 0.28), (13.3, 0.05),
          (13.55, 0.28), (16.3, 0.28), (16.45, 0)])
tone = np.stack([tone * tg] * 2, 1)

# music: muffled and low under the noise, then clean once the noise cancels
mus = read('music.wav')
muff = lp(lp(mus, 500), 500)
mix_w = env([(0, 0), (CUT + 0.1, 0), (CUT + 0.7, 1), (30, 1)])[:, None]
mg = env([(0, 0.18), (CUT, 0.22), (CUT + 0.7, 1.0), (28.5, 1.0), (30, 0.0)])[:, None]
music = (muff * (1 - mix_w) + mus * mix_w) * mg

out = 0.55 * noise + 0.5 * tone + 0.85 * music
out /= np.abs(out).max()
out = 0.88 * np.tanh(out * 1.5) / np.tanh(1.5)  # gentle limiter: louder, ceiling ~-1 dBFS
w = wave.open('soundtrack.wav', 'wb'); w.setnchannels(2); w.setsampwidth(2); w.setframerate(sr)
w.writeframes((out * 32767).astype(np.int16).tobytes()); w.close()
print('peak', np.abs(out).max())
