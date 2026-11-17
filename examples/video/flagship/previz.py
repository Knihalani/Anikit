"""Grey-box animatic for EVERY FRAME (not the film: a camera-and-timing check).

    python3 examples/video/flagship/previz.py [--fps 30] [--scale 0.333] [--t0 0 --t1 72] -o previz.mp4

Draws the world in world.py as flat-shaded boxes and quads with a pinhole camera,
pipes frames to ffmpeg, and overlays the time code, shot and beat.
"""

from __future__ import annotations

import argparse
import math
import subprocess
import sys
from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw, ImageFont

sys.path.insert(0, str(Path(__file__).parent))
import world as Wd  # noqa: E402

SHOTS = [(0, 1), (6, 2), (12, 3), (20, 4), (30, 5), (40, 6), (50, 7), (58, 8), (66, 9)]


def font(px):
    for p in ("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
              "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf"):
        if Path(p).exists():
            return ImageFont.truetype(p, max(6, int(px)))
    return ImageFont.load_default()


class Cam:
    def __init__(self, t, w, h):
        self.w, self.h = w, h
        p = np.array(Wd.cam_pos(t), float)
        g = np.array(Wd.cam_tgt(t), float)
        fwd = g - p
        fwd /= np.linalg.norm(fwd)
        right = np.cross(fwd, [0, 1, 0])
        right /= np.linalg.norm(right)
        up = np.cross(right, fwd)
        self.p, self.f, self.r, self.u = p, fwd, right, up
        self.fpx = Wd.cam_focal(t) / 36.0 * w
        self.near = 0.05

    def view(self, pts):
        d = np.asarray(pts, float) - self.p
        return np.stack([d @ self.r, d @ self.u, d @ self.f], axis=-1)

    def proj(self, v):
        z = v[..., 2]
        x = self.w / 2 + self.fpx * v[..., 0] / z
        y = self.h / 2 - self.fpx * v[..., 1] / z
        return np.stack([x, y], -1)


def clip_near(v, near):
    out = []
    n = len(v)
    for i in range(n):
        a, b = v[i], v[(i + 1) % n]
        ia, ib = a[2] >= near, b[2] >= near
        if ia:
            out.append(a)
        if ia != ib:
            s = (near - a[2]) / (b[2] - a[2])
            out.append(a + (b - a) * s)
    return np.array(out) if len(out) >= 3 else None


def shade(base, normal, cam_dir=None):
    L = np.array([-0.5, 0.8, 0.45])
    L /= np.linalg.norm(L)
    k = 0.55 + 0.45 * max(0.0, float(np.dot(normal, L)))
    return tuple(int(min(255, c * k)) for c in base)


def box_faces(c, s, yaw=0.0):
    """Six quads of a box centred at c with half-sizes s, rotated by yaw (deg) about Y."""
    cx, cy, cz = c
    hx, hy, hz = s
    a = math.radians(yaw)
    ca, sa = math.cos(a), math.sin(a)

    def P(x, y, z):
        return (cx + x * ca + z * sa, cy + y, cz - x * sa + z * ca)

    def N(x, y, z):
        return np.array((x * ca + z * sa, y, -x * sa + z * ca))
    corners = {(i, j, k): P(i * hx, j * hy, k * hz) for i in (-1, 1) for j in (-1, 1) for k in (-1, 1)}
    faces = [
        ([(1, -1, -1), (1, 1, -1), (1, 1, 1), (1, -1, 1)], N(1, 0, 0)),
        ([(-1, -1, 1), (-1, 1, 1), (-1, 1, -1), (-1, -1, -1)], N(-1, 0, 0)),
        ([(-1, 1, -1), (-1, 1, 1), (1, 1, 1), (1, 1, -1)], N(0, 1, 0)),
        ([(-1, -1, 1), (1, -1, 1), (1, 1, 1), (-1, 1, 1)], N(0, 0, 1)),
        ([(1, -1, -1), (-1, -1, -1), (-1, 1, -1), (1, 1, -1)], N(0, 0, -1)),
    ]
    return [([corners[k] for k in f], n) for f, n in faces]


def frame_colour(j):
    """The inner film, as a colour per frame (so playback reads as a smooth colour change)."""
    t = j / Wd.FPS
    h = (t / 60.0 + 0.55) % 1.0
    import colorsys
    r, g, b = colorsys.hsv_to_rgb(h, 0.45, 0.85)
    return (int(r * 255), int(g * 255), int(b * 255))


def render(t, w, h):
    cam = Cam(t, w, h)
    img = Image.new("RGB", (w, h))
    # cyc: warm white gradient, horizon where the floor meets the curve
    hz = h / 2 - cam.fpx * cam.f[1] / max(1e-3, math.hypot(cam.f[0], cam.f[2]))
    arr = np.zeros((h, w, 3), np.uint8)
    yy = np.arange(h)[:, None]
    sky = np.array([238, 235, 229])
    floor = np.array([226, 221, 213])
    m = np.clip((yy - hz) / 40.0 + 0.5, 0, 1)[..., None]
    arr[:] = (sky * (1 - m) + floor * m).astype(np.uint8)
    img = Image.fromarray(arr)
    d = ImageDraw.Draw(img)
    polys = []  # (depth, pts2d, colour, outline)

    def add_quad(pts, col, outline=None, two_sided=True, normal=None):
        v = cam.view(pts)
        if normal is not None and not two_sided:
            if np.dot(np.asarray(normal), cam.p - np.mean(pts, 0)) < 0:
                return
        if (v[:, 2] < cam.near).all():
            return
        v = clip_near(v, cam.near) if (v[:, 2] < cam.near).any() else v
        if v is None:
            return
        p2 = cam.proj(v)
        if p2[:, 0].max() < 0 or p2[:, 0].min() > w or p2[:, 1].max() < 0 or p2[:, 1].min() > h:
            return
        c = shade(col, normal) if normal is not None else col
        polys.append((float(v[:, 2].mean()), [tuple(q) for q in p2], c, outline))

    def add_box(c, s, col, yaw=0.0, outline=(90, 90, 90)):
        for f, n in box_faces(c, s, yaw):
            add_quad(np.array(f), col, outline, two_sided=False, normal=n)

    # floor grid (1 m near the camera)
    gx, gz = cam.p[0], cam.p[2]
    R = 40 if cam.p[1] < 10 else 400
    step = 1 if cam.p[1] < 10 else 20
    for k in range(int(gx - R) // step * step, int(gx + R) + 1, step):
        a, b = cam.view([(k, 0, gz - R), (k, 0, gz + R)])
        if a[2] < cam.near and b[2] < cam.near:
            continue
        seg = clip_near(np.array([a, b, b]), cam.near)
        if seg is not None:
            p2 = cam.proj(seg[:2])
            d.line([tuple(p2[0]), tuple(p2[1])], fill=(214, 209, 200))
    for k in range(int(gz - R) // step * step, int(gz + R) + 1, step):
        a, b = cam.view([(gx - R, 0, k), (gx + R, 0, k)])
        if a[2] < cam.near and b[2] < cam.near:
            continue
        seg = clip_near(np.array([a, b, b]), cam.near)
        if seg is not None:
            p2 = cam.proj(seg[:2])
            d.line([tuple(p2[0]), tuple(p2[1])], fill=(214, 209, 200))

    M = Wd.MACHINE
    n = Wd.emitted(t)
    mx = Wd.machine_x(n)
    add_box((mx, M["y1"] / 2, 0), ((M["x1"] - M["x0"]) / 2, M["y1"] / 2, M["z1"]), (176, 178, 182))
    led_on = Wd.T["led"] <= t < Wd.T["led"] + 0.25 or (Wd.T["stream0"] <= t <= Wd.T["stream1"])
    add_box((mx + 0.6, M["y1"] + 0.005, M["z1"] - 0.08), (0.03, 0.005, 0.03), (255, 90, 31) if led_on else (90, 60, 50))
    # browser machine: a standing white browser window on a cart
    m = Wd.browser_emitted(t)
    add_box((Wd.machine_x(m), 0.7, Wd.BROWSER_Z), (0.9, 0.7, 0.15), (205, 208, 213))

    # request pane
    pos, yaw, vis = Wd.request_pose(t)
    if vis:
        front = math.cos(math.radians(yaw)) > 0   # front (chat) faces +Z
        col = (150, 205, 235) if front else (40, 44, 52)
        add_box(pos, (Wd.PANE_W / 2, Wd.PANE_H / 2, Wd.PANE_T / 2), col, yaw=yaw, outline=(120, 180, 170))

    # strip panes
    in_fold = t >= Wd.T["fold0"]
    xc = Wd.fold_centre()
    jc = int(round((xc + 0.15) / Wd.PITCH))
    labels = []
    if not in_fold:
        js = np.arange(0, int(math.ceil(n)))
        xs = js * Wd.PITCH - 0.15
        # cull by view
        cx = cam.view(np.stack([xs, np.full_like(xs, Wd.PANE_Y), np.zeros_like(xs)], -1))
        keep = cx[:, 2] > -2
        js, xs, cx = js[keep], xs[keep], cx[keep]
        for j, x, v in zip(js, xs, cx):
            if v[2] > 0:
                sz = cam.fpx * Wd.PANE_W / v[2]
                if sz < 1.2:
                    p = cam.proj(v)
                    d.point(tuple(p), fill=(140, 150, 150))
                    continue
            hw, hh = Wd.PANE_W / 2, Wd.PANE_H / 2
            quad = np.array([(x - hw, Wd.PANE_Y - hh, 0), (x + hw, Wd.PANE_Y - hh, 0),
                             (x + hw, Wd.PANE_Y + hh, 0), (x - hw, Wd.PANE_Y + hh, 0)])
            add_quad(quad, frame_colour(j), (60, 140, 120))
            if v[2] > 0 and cam.fpx * Wd.PANE_W / v[2] > 120:
                labels.append((v[2], (x, Wd.PANE_Y, 0.01), f"{j + 1}"))
        # orange rail under the strip
        if n > 0:
            x_far = mx - 1.0
            add_quad(np.array([(-1.0, 0.002, -0.06), (x_far, 0.002, -0.06), (x_far, 0.002, 0.06), (-1.0, 0.002, 0.06)]),
                     (255, 90, 31))
    else:
        bs = Wd.block_scale(t)
        if t < Wd.T["crush1"]:
            for j in range(0, Wd.FRAMES, 3 if t > Wd.T["fold1"] else 1):
                x, y, z, yw = Wd.pane_pose_fold(j, t, xc, jc)
                if abs(x - cam.p[0]) > 400:
                    continue
                add_box((x, y, z), (Wd.PANE_W / 2, Wd.PANE_H / 2, Wd.PANE_T / 2), frame_colour(j), yaw=yw,
                        outline=None)
        else:
            # the block, shrinking into the tile; the tile slides into the phone
            ins = Wd.smoother(Wd.seg(t, Wd.T["insert0"], Wd.T["insert1"]))
            ph = np.array(Wd.PHONE_OFFSET) + (xc, 0, 0)
            base = np.array((xc, Wd.PANE_Y * bs + 0.01, 0.0))
            tgt = ph + (0, 0.09, 0)
            c = base + (tgt - base) * ins
            if ins < 1:
                add_box(tuple(c), (0.15 * bs, Wd.PANE_H / 2 * bs, Wd.PANE_W / 2 * bs), (170, 220, 210))
        # phone, standing on a little stand
        ph = np.array(Wd.PHONE_OFFSET) + (xc, 0, 0)
        on = t >= Wd.T["screen"]
        add_box((ph[0], 0.085, ph[2]), (0.0375, 0.08, 0.004), (30, 32, 36), yaw=-25)
        if on:
            add_box((ph[0] + 0.002, 0.085, ph[2] + 0.005), (0.034, 0.075, 0.001), frame_colour(int((t - 59) * 60)), yaw=-25)

    # browser strip
    for j in range(int(math.ceil(m))):
        x = Wd.pane_x(j)
        quad = np.array([(x - 0.8, 0.02, Wd.BROWSER_Z), (x + 0.8, 0.02, Wd.BROWSER_Z),
                         (x + 0.8, 0.92, Wd.BROWSER_Z), (x - 0.8, 0.92, Wd.BROWSER_Z)])
        add_quad(quad, (150, 156, 164), (110, 115, 120))

    # logo
    if t >= Wd.T["logo0"]:
        rise = Wd.smoother(Wd.seg(t, Wd.T["logo0"], Wd.T["logo1"]))
        lx = Wd.LOGO_POS[0]
        add_box((lx, 0.3 * rise, Wd.LOGO_Z), (2.1, 0.3 * rise + 0.001, 0.08), (200, 205, 215))

    polys.sort(key=lambda q: -q[0])
    for _, pts, col, outline in polys:
        d.polygon(pts, fill=col, outline=outline)

    for z, P, txt in labels:
        v = cam.view([P])[0]
        if v[2] <= cam.near:
            continue
        p = cam.proj(v)
        d.text(tuple(p), txt, fill=(20, 20, 20), font=font(cam.fpx * 0.25 / v[2]), anchor="mm")

    # captions
    for c in Wd.CAPTIONS:
        if not (c["t0"] <= t < c["t1"]):
            continue
        a = None if c["kind"] in ("counter", "cart", "cart2") else Wd.caption_anchor(c)
        if c["kind"] == "counter":
            txt = f"{int(min(n, Wd.FRAMES)):,} / 3,600 frames"
            d.text((w * 0.05, h * 0.9), txt, fill=(255, 90, 31), font=font(h * 0.06), anchor="ls")
            continue
        if c["kind"] == "cart2":
            a = (Wd.browser_x(t), 2.6, Wd.BROWSER_Z)
            txt = f"Still rendering · {int(m)} / 3,600"
        elif c["kind"] == "cart":
            a = (Wd.browser_x(t), 1.9, Wd.BROWSER_Z)
            txt = f"{c['text']}  ·  {int(m)} / 3,600"
        else:
            txt = c["text"]
        v = cam.view([a])[0]
        if v[2] <= cam.near:
            continue
        p = cam.proj(v)
        size = cam.fpx * c["h"] / v[2]
        col = (255, 90, 31) if c["kind"] == "counter" else (21, 23, 26)
        d.text(tuple(p), txt, fill=col, font=font(min(size, h * 0.2)), anchor="ms")

    # HUD
    shot = max(s for s0, s in SHOTS if t >= s0)
    beat = int(t / Wd.BEAT)
    hud = f"{int(t // 60)}:{t % 60:05.2f}  shot {shot}  bar {beat // 4 + 1}.{beat % 4 + 1}  f={Wd.cam_focal(t):.0f}mm"
    d.rectangle([0, h - 18, w, h], fill=(0, 0, 0))
    d.text((6, h - 16), hud, fill=(255, 255, 255), font=font(12))
    if abs(t / Wd.BEAT - round(t / Wd.BEAT)) < 0.02:
        d.ellipse([w - 16, h - 15, w - 6, h - 5], fill=(255, 90, 31))
    return img


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--fps", type=float, default=30)
    ap.add_argument("--scale", type=float, default=1 / 3)
    ap.add_argument("--t0", type=float, default=0.0)
    ap.add_argument("--t1", type=float, default=Wd.DURATION)
    ap.add_argument("--still", type=float, action="append")
    ap.add_argument("-o", "--out", default="previz.mp4")
    a = ap.parse_args()
    w, h = int(Wd.W * a.scale) // 2 * 2, int(Wd.H * a.scale) // 2 * 2
    if a.still:
        for s in a.still:
            render(s, w, h).save(f"{a.out.rsplit('.', 1)[0]}-{s:05.2f}.png")
        return
    ff = subprocess.Popen(["ffmpeg", "-loglevel", "error", "-y", "-f", "rawvideo", "-pix_fmt", "rgb24",
                           "-s", f"{w}x{h}", "-r", str(a.fps), "-i", "-", "-c:v", "libx264", "-preset", "veryfast",
                           "-crf", "20", "-pix_fmt", "yuv420p", a.out], stdin=subprocess.PIPE)
    nfr = int((a.t1 - a.t0) * a.fps)
    for i in range(nfr):
        t = a.t0 + i / a.fps
        ff.stdin.write(render(t, w, h).tobytes())
    ff.stdin.close()
    ff.wait()


if __name__ == "__main__":
    main()
