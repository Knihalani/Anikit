"""Guilloché dial maps for the Calibre 9 (albedo, OpenGL normal, roughness).
The tile spans T scene units with the dial centre at the tile corner (triplanar
wraps), so the spec uses material scale = 1/T."""
import numpy as np, sys, os
from PIL import Image, ImageDraw, ImageFont
N, T = 2048, 2.0
out = sys.argv[1] if len(sys.argv) > 1 else "tex/dial"
c = (np.arange(N) + 0.5) / N
c = np.where(c < 0.5, c, c - 1.0) * T          # wrapped coords, centre at 0
X, Z = np.meshgrid(c, c)                         # u -> x (cols), v -> z (rows)
r = np.hypot(X, Z); th = np.arctan2(X, -Z)       # theta 0 at 12 o'clock (-z)
def ss(a, b, x): t = np.clip((x - a) / (b - a), 0, 1); return t * t * (3 - 2 * t)

# Centre: rose-engine wave sunburst (barleycorn-like), 120 rays wavering
h_c = 0.5 + 0.5 * np.sin(120 * th + 2.4 * np.sin(2 * np.pi * r / 0.055))
h_c *= ss(0.05, 0.10, r)                          # polished boss at the pinion
# Chapter ring: fine circular azurage
h_ring = 0.5 + 0.5 * np.sin(2 * np.pi * r / 0.011)
# Sub-dial (power reserve) at 12: snailing
sx, sz = X, Z + 0.33; sr = np.hypot(sx, sz)
h_sub = 0.5 + 0.5 * np.sin(2 * np.pi * sr / 0.009)
inner = 1 - ss(0.555, 0.565, r)
ring = ss(0.595, 0.605, r)
sub = 1 - ss(0.165, 0.172, sr)
h = h_c * inner * (1 - sub) + h_ring * ring + h_sub * sub * 0
h = np.where(sub > 0, h_sub * sub + h * (1 - sub), h)
h = h * 0.9
# normal from height (strength chosen for a crisp engraved read)
k = 3.2 / (T / N) * 0.004
dx = (np.roll(h, -1, 1) - np.roll(h, 1, 1)) * 0.5 * k
dz = (np.roll(h, -1, 0) - np.roll(h, 1, 0)) * 0.5 * k
n = np.stack([-dx, -dz, np.ones_like(h)], -1); n /= np.linalg.norm(n, axis=-1, keepdims=True)
Image.fromarray(((n * 0.5 + 0.5) * 255).astype(np.uint8)).save(os.path.join(out, "dial_normalgl.png"))

# Albedo: salmon, grooves a touch deeper; polished separation rings brighter
base = np.array([0.86, 0.62, 0.50])
cav = 0.80 + 0.20 * h
alb = base[None, None, :] * cav[..., None]
flat = (1 - inner) * (1 - ring)                  # polished band 0.56-0.60
alb = alb * (1 - flat[..., None]) + np.array([0.93, 0.74, 0.64]) * flat[..., None]
img = Image.new("L", (N, N), 0)
# Printing (dark brown-black ink), drawn centred then rolled so the dial centre is at the corner
d = ImageDraw.Draw(img)
px = N / T
cx = cy = N // 2
ink = 255
def P(x, z): return (cx + x * px, cy + z * px)
# minute track at r 0.82-0.85
for m in range(60):
    a = m / 60 * 2 * np.pi
    r0 = 0.815 if m % 5 else 0.80
    s, co = np.sin(a), -np.cos(a)
    d.line([P(r0 * s, r0 * co), P(0.845 * s, 0.845 * co)], fill=ink, width=3 if m % 5 == 0 else 2)
d.ellipse([P(-0.848, -0.848), P(0.848, 0.848)], outline=ink, width=2)
# power reserve scale on sub-dial: arc from -120 to +120 deg, 0..72
for i in range(13):
    a = np.radians(-120 + 240 * i / 12)
    s, co = np.sin(a), -np.cos(a)
    r0 = 0.115 if i % 4 else 0.10
    d.line([P(r0 * s, -0.33 + r0 * co), P(0.150 * s, -0.33 + 0.150 * co)], fill=ink, width=3 if i % 4 == 0 else 2)
F = "/mnt/skills/examples/canvas-design/canvas-fonts/"
def text(s, x, z, size, font="CrimsonPro-Regular.ttf", track=0):
    f = ImageFont.truetype(F + font, int(size * px))
    w = sum(f.getlength(ch) for ch in s) + track * px * (len(s) - 1)
    xx = cx + x * px - w / 2
    for ch in s:
        d.text((xx, cy + z * px), ch, font=f, fill=ink, anchor="lm")
        xx += f.getlength(ch) + track * px
for lab, a in (("0", -120), ("24", -40), ("48", 40), ("72", 120)):
    aa = np.radians(a)
    text(lab, 0.075 * np.sin(aa), -0.33 - 0.075 * np.cos(aa), 0.032)
text("RÉSERVE", 0, -0.33 + 0.07, 0.022, track=0.006)
text("VAUCLAIR", 0, 0.26, 0.06, track=0.022)

text("Calibre 9", 0, 0.335, 0.034, font="CrimsonPro-Italic.ttf")
m = np.roll(np.array(img), (-cy, -cx), (0, 1))[..., None] / 255.0
alb = alb * (1 - m) + np.array([0.13, 0.09, 0.085]) * m
Image.fromarray((np.clip(alb, 0, 1) * 255).astype(np.uint8)).save(os.path.join(out, "dial_color.png"))
rough = 0.32 - 0.06 * h; rough = np.where(flat > 0.5, 0.10, rough); rough = rough * (1 - m[..., 0]) + 0.45 * m[..., 0]
Image.fromarray((np.clip(rough, 0, 1) * 255).astype(np.uint8)).save(os.path.join(out, "dial_roughness.png"))
print("ok")
