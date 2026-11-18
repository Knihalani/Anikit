# Generates roar.wav: cabin/engine noise that is cancelled (by literally adding its inverse) at t=16-17 s.
import numpy as np, wave
sr = 48000; T = 30.0; n = int(sr*T); t = np.arange(n)/sr
rng = np.random.default_rng(7)
# brown noise via cumulative sum with leak
w = rng.standard_normal(n)
b = np.cumsum(w); b -= np.convolve(b, np.ones(4801)/4801, mode='same'); b /= np.abs(b).max()
# engine hum with slow wobble
f0 = 74 + 1.5*np.sin(2*np.pi*0.23*t)
ph = 2*np.pi*np.cumsum(f0)/sr
hum = 0.55*np.sin(ph) + 0.35*np.sin(2*ph+0.4) + 0.2*np.sin(3*ph+1.1) + 0.1*np.sin(5*ph)
hum *= 1 + 0.15*np.sin(2*np.pi*3.1*t)
hiss = rng.standard_normal(n); hiss = np.convolve(hiss, np.ones(6)/6, mode='same')
roar = 0.62*b + 0.42*hum + 0.06*hiss
roar /= np.abs(roar).max()
# anti-noise: the inverse, faded in during the cancellation (15.9 -> 16.9 s)
k = np.clip((t-15.9)/1.0, 0, 1); k = k*k*(3-2*k)
anti = -roar * k * 0.995
env = np.clip(t/0.08, 0, 1)
sig = (roar + anti) * env * 0.8
# stereo, slight decorrelation of hiss
L = sig; R = sig.copy()
st = np.stack([L, R], 1)
pcm = (np.clip(st, -1, 1)*32767).astype('<i2')
with wave.open('roar.wav', 'wb') as f:
    f.setnchannels(2); f.setsampwidth(2); f.setframerate(sr); f.writeframes(pcm.tobytes())
print('ok', np.abs(sig[int(17.5*sr):]).max())
