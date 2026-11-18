import math, random
def loop(cx, cy, rx, ry, seed=1, turns=1.12, start=-110, tilt=-4, n=48):
    """A hand-drawn ballpoint loop: starts at the top-left, goes clockwise past its start and overshoots."""
    r = random.Random(seed)
    pts = []
    total = 360 * turns
    t0 = math.radians(tilt)
    wob = [r.uniform(-0.04, 0.04) for _ in range(6)]
    for i in range(n + 1):
        f = i / n
        a = math.radians(start + total * f)
        k = 1 + wob[int(f * 5)] * math.sin(f * math.pi) + 0.06 * f  # spirals slightly outward
        x, y = rx * k * math.cos(a), ry * k * math.sin(a)
        xr = x * math.cos(t0) - y * math.sin(t0)
        yr = x * math.sin(t0) + y * math.cos(t0)
        pts.append((cx + xr, cy + yr))
    d = "M%.1f %.1f" % pts[0]
    for i in range(1, len(pts) - 1, 2):
        d += " Q%.1f %.1f %.1f %.1f" % (*pts[i], *pts[i + 1])
    return d
