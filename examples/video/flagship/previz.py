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

SHOTS = [(0, 1), (7, 2), (12, 3), (18, 4), (26, 5), (50, 6), (58, 7), (66, 8)]


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


LANE_HUES = [(0.09, 0.55, 0.95), (0.0, 0.55, 0.55), (0.62, 0.75, 0.85), (0.18, 0.9, 0.95), (0.45, 0.6, 0.5),
             (0.98, 0.8, 0.8)]


def frame_colour(k, j):
    """Film k's frame j, as a colour (hue family per film; drifts with time so playback reads)."""
    import colorsys
    h0, s0, v0 = LANE_HUES[k]
    t = j / Wd.FPS
    h = (h0 + 0.06 * math.sin(t * 0.9)) % 1.0
    v = v0 * (0.75 + 0.25 * math.sin(t * 2.3))
    r, g, b = colorsys.hsv_to_rgb(h, s0, v)
    return (int(r * 255), int(g * 255), int(b * 255))


def render(t, w, h):
    cam = Cam(t, w, h)
    hz = h / 2 - cam.fpx * cam.f[1] / max(1e-3, math.hypot(cam.f[0], cam.f[2]))
    arr = np.zeros((h, w, 3), np.uint8)
    yy = np.arange(h)[:, None]
    sky = np.array([238, 235, 229])
    floor = np.array([226, 221, 213])
    m = np.clip((yy - hz) / 40.0 + 0.5, 0, 1)[..., None]
    arr[:] = (sky * (1 - m) + floor * m).astype(np.uint8)
    img = Image.fromarray(arr)
    d = ImageDraw.Draw(img)
    polys = []
    labels = []

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
        for f, nrm in box_faces(c, s, yaw):
            add_quad(np.array(f), col, outline, two_sided=False, normal=nrm)

    # floor grid
    gx, gz = cam.p[0], cam.p[2]
    R = 40 if cam.p[1] < 10 else 400
    step = 1 if cam.p[1] < 10 else 20
    for axis in (0, 2):
        c0 = gx if axis == 0 else gz
        for k in range(int(c0 - R) // step * step, int(c0 + R) + 1, step):
            if axis == 0:
                a, b = cam.view([(k, 0, gz - R), (k, 0, gz + R)])
            else:
                a, b = cam.view([(gx - R, 0, k), (gx + R, 0, k)])
            if a[2] < cam.near and b[2] < cam.near:
                continue
            sg = clip_near(np.array([a, b, b]), cam.near)
            if sg is not None:
                p2 = cam.proj(sg[:2])
                d.line([tuple(p2[0]), tuple(p2[1])], fill=(214, 209, 200))

    M = Wd.MACHINE
    n = Wd.emitted(t)
    mx = Wd.machine_x(n)
    if t < Wd.T["stream1"] + 0.5:
        add_box((mx, M["y1"] / 2, (M["z0"] + M["z1"]) / 2), ((M["x1"] - M["x0"]) / 2, M["y1"] / 2, (M["z1"] - M["z0"]) / 2),
                (176, 178, 182))
        for k in range(Wd.LANES):
            on = Wd.drop_time(k) <= t < Wd.drop_time(k) + 0.3 or Wd.T["stream0"] <= t
            add_box((mx + 0.6, M["y1"] + 0.005, Wd.lane_z(k) + 0.3), (0.03, 0.005, 0.03),
                    (255, 90, 31) if on else (90, 60, 50))

    # requests
    for k in range(Wd.LANES):
        pos, yaw, vis = Wd.request_pose(k, t)
        if vis:
            front = math.cos(math.radians(yaw)) > 0
            col = (150, 205, 235) if front else (40, 44, 52)
            add_box(pos, (Wd.PANE_W / 2, Wd.PANE_H / 2, Wd.PANE_T / 2), col, yaw=yaw, outline=(120, 180, 170))
            v = cam.view([pos])[0]
            if front and v[2] > 0 and cam.fpx * Wd.PANE_W / v[2] > 160:
                labels.append((pos, f"req {k + 1}: {Wd.FILMS[k]['name']}", 0.09))

    # strips
    xc = Wd.fold_centre()
    jc = Wd.fold_jc()
    if t < Wd.T["fold0"]:
        nn = int(math.ceil(n))
        if nn > 0:
            js = np.arange(nn)
            xs = js * Wd.PITCH - 0.15
            for k in range(Wd.LANES):
                z = Wd.lane_z(k)
                cx = cam.view(np.stack([xs, np.full_like(xs, Wd.PANE_Y), np.full_like(xs, z)], -1))
                keep = (cx[:, 2] > -2) & (np.abs(cx[:, 0]) < cx[:, 2] * 2 + 3)
                for j, x, v in zip(js[keep], xs[keep], cx[keep]):
                    if v[2] > 0 and cam.fpx * Wd.PANE_W / v[2] < 1.2:
                        d.point(tuple(cam.proj(v)), fill=frame_colour(k, j))
                        continue
                    hw, hh = Wd.PANE_W / 2, Wd.PANE_H / 2
                    quad = np.array([(x - hw, Wd.PANE_Y - hh, z), (x + hw, Wd.PANE_Y - hh, z),
                                     (x + hw, Wd.PANE_Y + hh, z), (x - hw, Wd.PANE_Y + hh, z)])
                    add_quad(quad, frame_colour(k, j), (60, 140, 120))
                    if v[2] > 0 and cam.fpx * Wd.PANE_W / v[2] > 140:
                        labels.append(((x, Wd.PANE_Y, z + 0.01), f"{Wd.FILMS[k]['name']} · {j + 1}", 0.08))
                x_far = min(mx - 1.0, xs[-1] + 0.8)
                add_quad(np.array([(-1.0, 0.002, z - 0.06), (x_far, 0.002, z - 0.06), (x_far, 0.002, z + 0.06),
                                   (-1.0, 0.002, z + 0.06)]), (255, 90, 31))
    elif t < Wd.T["shrink0"]:
        step = 1 if t < Wd.T["fold1"] else 4
        for j in range(0, Wd.FRAMES, step):
            x, yw = Wd.pane_pose_fold(j, t)
            if abs(x - cam.p[0]) > 300:
                continue
            for k in range(Wd.LANES):
                add_box((x, Wd.PANE_Y, Wd.lane_z(k)), (Wd.PANE_W / 2, Wd.PANE_H / 2, Wd.PANE_T / 2), frame_colour(k, j),
                        yaw=yw, outline=None)
    else:
        bs = Wd.block_scale(t)
        for k in range(Wd.LANES):
            if t < Wd.T["rise0"]:
                add_box((xc, Wd.PANE_Y * bs + 0.01, Wd.block_z(k, t)), (0.125 * bs, Wd.PANE_H / 2 * bs, Wd.PANE_W / 2 * bs),
                        (170, 220, 210))
            else:
                pos, yaw, sc = Wd.screen_pose(k, t)
                add_box(pos, (Wd.PANE_W / 2 * sc, Wd.PANE_H / 2 * sc, 0.01 + 0.03 * sc), frame_colour(k, int((t - 59) * 60)),
                        yaw=yaw, outline=(30, 30, 30))
                if sc > 0.8:
                    labels.append(((pos[0], pos[1], pos[2] + 0.05), Wd.FILMS[k]["name"], 0.12))

    # logo
    if t >= Wd.T["logo0"]:
        rise = Wd.smoother(Wd.seg(t, Wd.T["logo0"], Wd.T["logo1"]))
        lx, _, lz = Wd.LOGO_POS
        add_box((lx, 0.3 * rise, lz), (2.1, 0.3 * rise + 0.001, 0.08), (200, 205, 215))

    polys.sort(key=lambda q: -q[0])
    for _, pts, col, outline in polys:
        d.polygon(pts, fill=col, outline=outline)

    for P, txt, hh in labels:
        v = cam.view([P])[0]
        if v[2] <= cam.near:
            continue
        p = cam.proj(v)
        d.text(tuple(p), txt, fill=(20, 20, 20), font=font(cam.fpx * hh / v[2]), anchor="mm")

    for c in Wd.CAPTIONS:
        if not (c["t0"] <= t < c["t1"]):
            continue
        a = Wd.caption_anchor(c)
        v = cam.view([a])[0]
        if v[2] <= cam.near:
            continue
        p = cam.proj(v)
        size = cam.fpx * c["h"] / v[2]
        d.text(tuple(p), c["text"], fill=(21, 23, 26), font=font(min(size, h * 0.2)), anchor="ms")

    shot = max(s for s0, s in SHOTS if t >= s0)
    beat = int(t / Wd.BEAT)
    hud = (f"{int(t // 60)}:{t % 60:05.2f}  shot {shot}  bar {beat // 4 + 1}.{beat % 4 + 1}  "
           f"f={Wd.cam_focal(t):.0f}mm  shutter={Wd.shutter(t):.0f}")
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
