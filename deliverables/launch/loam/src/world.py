"""Shared world for the Loam film: one cutaway-pot illustration kit, one light, one palette.

Everything is built as `illo` shapes in a 1080x1920 design space. Each shot calls `pot()` with its
own placement and scale, so the material, light and hand stay identical while the framing changes.
"""
import math

W, H = 1080, 1920

# One palette family: earth umbers, window amber, leaf greens; water blue reserved for the device.
PAL = {
    "wall": "#3A2A22", "wallLo": "#1E1614", "sun": "#F2B566", "sunDeep": "#D98E45",
    "shelf": "#5C3B27", "shelfLo": "#2C1A12", "shelfHi": "#7A5034",
    "glaze": "#2F5D57", "glazeHi": "#4C8A7E", "clay": "#CDB89B", "clayLo": "#9E8466",
    "dry": "#D2AF7C", "dryHi": "#E6CC9C", "dryLo": "#AC895C",
    "wet": "#3E2719", "wetLo": "#24160F", "wetHi": "#5A3A24",
    "root": "#EBDDBF", "rootLo": "#B9A27C",
    "leaf": "#2F7D52", "leafHi": "#62B06C", "leafLo": "#1C4E3E", "stem": "#3E7A45",
    "water": "#57B6E8", "waterHi": "#A9DDF5", "waterLo": "#2C7FB8",
    "sensor": "#ECE7DD", "sensorLo": "#A9A296", "ink": "#140E0C",
}

LIGHTS = {
    # The chosen key: low morning window, top-left, warm, umber shadows.
    "window": {"angle": 215, "rim": 30, "color": "#FFE0A8", "shadow": "#2A1636", "strength": 1.0},
}


def wall(sun=True, sun_pts=None, sun_opacity=0.9):
    s = [{"shape": "rect", "x": 0, "y": 0, "w": W, "h": H,
          "fill": {"stops": [[0, "$wall"], [1, "$wallLo"]], "angle": 80, "steps": 4, "dither": 0.18},
          "wobble": 0, "rough": 0, "grain": 0.07, "corners": "sharp", "shade": 0}]
    if sun:
        pts = sun_pts or [[-40, 180], [700, 260], [640, 1500], [-40, 1460]]
        s += [
            {"shape": "polygon", "name": "mullV", "points": [[300, 150], [350, 156], [330, 1560], [282, 1560]], "visible": False},
            {"shape": "polygon", "name": "mullH", "points": [[-60, 760], [720, 800], [718, 846], [-60, 806]], "visible": False},
            {"shape": "polygon", "points": pts, "fill": {"stops": [[0, "#FFD48A"], [0.45, "$sun"], [1, "$sunDeep"]], "angle": 300, "steps": 5, "dither": 0.2}, "opacity": sun_opacity, "cut": ["mullV", "mullH"],
             "grain": 0.09, "texture": {"style": "paper", "amount": 0.6}, "corners": 8, "shade": 0},
        ]
    return s


def shelf(y=1560, h=None):
    h = h or (H - y + 40)
    return [
        {"shape": "rect", "x": -20, "y": y, "w": W + 40, "h": h,
         "fill": {"stops": [[0, "$shelfHi"], [0.08, "$shelf"], [1, "$shelfLo"]], "angle": 90, "steps": 5, "dither": 0.15},
         "grain": 0.08, "texture": {"style": "contour", "amount": 0.25}, "corners": "sharp", "shade": 0},
        {"shape": "rect", "x": -20, "y": y - 10, "w": W + 40, "h": 18, "fill": "$shelfHi", "shade": 0, "corners": "sharp"},
    ]


def wt(x):
    return [x * 0.45, x]


def _lerp(a, b, t):
    return a + (b - a) * t


def pot(cx=540, top=820, w=760, h=740, s=1.0, name="p", wet=None, flood=None, dry_cracks=False,
        line=None, sensor=None, plant="monstera", roots=True, sway=1.0, seed=1):
    """A glazed pot cut away along its front, so the soil, roots and water are visible.

    wet:   (t0, t1, depth_frac) - wet earth soaks down from the surface to depth_frac between t0 and t1.
    flood: (t0, t1) - too much water: a blue pool rises from the bottom and the soil goes black.
    line:  depth_frac of the "this much" fill line (drawn on the cut edge), or None.
    sensor: (t_in,) - time the sensor clips in, or None for no sensor.
    """
    w, h = w * s, h * s
    wall_t = 34 * s
    tl, tr = cx - w / 2, cx + w / 2
    bl, br = cx - w * 0.40, cx + w * 0.40
    bot = top + h
    # soil interior polygon (inset)
    it, ib = top + 40 * s, bot - wall_t
    il, ir = tl + wall_t, tr - wall_t
    ibl, ibr = bl + wall_t * 0.9, br - wall_t * 0.9
    soil_pts = [[il, it], [ir, it], [ibr, ib], [ibl, ib]]
    out = []
    # cast shadow of the pot on the shelf/wall, falling right (light from top-left)
    out.append({"shape": "polygon", "points": [[tl - 6 * s, top - 26 * s], [tr + 6 * s, top - 26 * s], [br, bot], [bl, bot]],
                "fill": "$glaze", "name": f"{name}_body",
                "shade": {"amount": 0.7, "offset": 0.1}, "shadow": {"dx": 70 * s, "dy": 18 * s, "flatten": 0.15, "opacity": 0.5},
                "texture": {"style": "halftone", "amount": 0.35, "scale": 0.9}, "grain": 0.05})
    # cut face of the pot wall (clay body) - a band around the soil
    out.append({"shape": "polygon", "points": [[tl, top], [tr, top], [br - 4 * s, bot - 4 * s], [bl + 4 * s, bot - 4 * s]],
                "fill": "$clay", "shade": {"amount": 0.35, "offset": 0.05}, "grain": 0.08,
                "texture": {"style": "stipple", "amount": 0.35}})
    # rim lip
    out.append({"shape": "rect", "x": tl - 16 * s, "y": top - 34 * s, "w": w + 32 * s, "h": 58 * s, "radius": 10 * s,
                "fill": "$glazeHi", "shade": {"amount": 0.6, "offset": 0.2}, "grain": 0.05})
    out.append({"shape": "rect", "x": tl - 4 * s, "y": top - 8 * s, "w": w + 8 * s, "h": 40 * s, "fill": "$clay",
                "shade": 0.2, "grain": 0.06})
    # soil
    sname = f"{name}_soil"
    out.append({"shape": "polygon", "name": sname, "points": soil_pts, "fill":
                {"stops": [[0, "$dryHi"], [0.4, "$dry"], [1, "$dryLo"]], "angle": 90, "steps": 4, "dither": 0.2},
                "texture": {"style": "stipple", "amount": 0.9, "scale": 1.1}, "grain": 0.12, "shade": 0})
    # pebbles / grit in the soil, clipped
    import random
    rnd = random.Random(seed)
    for _ in range(int(26 * s + 6)):
        px = _lerp(il, ir, rnd.random())
        py = _lerp(it + 20 * s, ib - 10 * s, rnd.random())
        r = (4 + rnd.random() * 9) * s
        out.append({"shape": "blob", "cx": px, "cy": py, "rx": r * 1.3, "ry": r, "fill": rnd.choice(["$dryLo", "$dryHi", "$clayLo"]),
                    "clipTo": sname, "shade": 0.5, "lumpy": 0.4, "seed": rnd.randint(1, 999), "opacity": 0.9})
    if dry_cracks:
        for i, (x0, len_) in enumerate([(0.3, 0.32), (0.55, 0.22), (0.72, 0.4), (0.15, 0.18)]):
            x = _lerp(il, ir, x0)
            pts = [[x, it]]
            for k in range(1, 6):
                pts.append([x + (rnd.random() - 0.5) * 50 * s, it + (ib - it) * len_ * k / 5])
            out.append({"shape": "path", "points": pts, "smooth": False, "stroke": "$wetLo", "weight": wt(5 * s),
                        "clipTo": sname, "taper": 0.9})
    # wet earth soaking down: a shape anchored at the soil surface that grows downward (sy) with a ragged front
    if wet is not None:
        t0, t1, depth = wet
        d_y = it + (ib - it) * depth
        # ragged front path
        n = 9
        front = []
        for k in range(n + 1):
            x = _lerp(il - 20, ir + 20, k / n)
            front.append(f"{x:.0f} {d_y + (rnd.random() - 0.3) * 46 * s + 22 * s * math.sin(k * 1.7):.0f}")
        d = f"M{il - 20:.0f} {it - 10:.0f} L{ir + 20:.0f} {it - 10:.0f} L" + " L".join(reversed(front)) + " Z"
        keys = [{"t": 0, "sy": 0.001}, {"t": t0, "sy": 0.001}, {"t": t1, "sy": 1, "ease": "out"}]
        out.append({"shape": "path", "d": d, "fill": {"stops": [[0, "$wetLo"], [0.85, "$wet"], [1, "$wetHi"]], "angle": 90, "steps": 3},
                    "clipTo": sname, "origin": [cx, it - 10], "animate": {"keys": keys}, "grain": 0.1,
                    "texture": {"style": "stipple", "amount": 0.6}, "shade": 0})
        # glistening drops at the front
    if flood is not None:
        t0, t1 = flood
        keys = [{"t": 0, "sy": 0.001}, {"t": t0, "sy": 0.001}, {"t": t1, "sy": 1, "ease": "out"}]
        out.append({"shape": "rect", "x": il - 20, "y": it - 10, "w": ir - il + 40, "h": ib - it + 20, "fill": "$wetLo",
                    "clipTo": sname, "origin": [cx, it - 10], "animate": {"keys": keys}, "grain": 0.1, "shade": 0})
        pool_top = it + (ib - it) * 0.42
        keysp = [{"t": 0, "sy": 0.001}, {"t": t0 + 0.2, "sy": 0.001}, {"t": t1 + 0.3, "sy": 1, "ease": "out"}]
        out.append({"shape": "path", "d": f"M{il-20:.0f} {ib+10:.0f} L{il-20:.0f} {pool_top:.0f} C{cx-150*s:.0f} {pool_top-26*s:.0f} {cx+150*s:.0f} {pool_top+26*s:.0f} {ir+20:.0f} {pool_top:.0f} L{ir+20:.0f} {ib+10:.0f} Z",
                    "fill": {"stops": [[0, "$waterHi"], [0.25, "$water"], [1, "$waterLo"]], "angle": 90, "steps": 4},
                    "opacity": 0.82, "clipTo": sname, "origin": [cx, ib], "animate": {"keys": keysp}, "shade": 0, "grain": 0.05})
        for k in range(7):
            bx = _lerp(il + 40, ir - 40, rnd.random())
            by0 = ib - 30 * s
            out.append({"shape": "circle", "cx": bx, "cy": by0, "r": (6 + rnd.random() * 8) * s, "fill": "none",
                        "stroke": "$waterHi", "weight": wt(3 * s), "outline": True, "clipTo": sname,
                        "animate": {"keys": [{"t": 0, "y": 0, "opacity": 0}, {"t": t0 + 0.3 + k * 0.12, "y": 0, "opacity": 0},
                                             {"t": t0 + 0.35 + k * 0.12, "y": 0, "opacity": 1},
                                             {"t": t0 + 1.5 + k * 0.12, "y": -(ib - pool_top) * 0.9, "opacity": 0.9}]}})
    if roots:
        rx0 = cx - 10 * s
        depth_root = 0.62
        for k, (dx, dep, bend) in enumerate([(-200, 0.55, -1), (-90, 0.7, 1), (0, depth_root, -1), (80, 0.66, 1), (190, 0.5, 1), (-30, 0.45, 1)]):
            ex = rx0 + dx * s
            ey = it + (ib - it) * dep
            pts = [[rx0 + dx * 0.1 * s, it + 6], [rx0 + dx * 0.45 * s + bend * 30 * s, it + (ey - it) * 0.4],
                   [ex - bend * 20 * s, it + (ey - it) * 0.75], [ex, ey]]
            out.append({"shape": "path", "points": pts, "smooth": True, "stroke": "$root", "weight": wt((7 - k * 0.5) * s),
                        "clipTo": sname, "taper": 1, "fill": "none"})
            # side rootlets
            mx, my = pts[2]
            out.append({"shape": "path", "points": [[mx, my], [mx + bend * 50 * s, my + 40 * s], [mx + bend * 70 * s, my + 90 * s]],
                        "smooth": True, "stroke": "$root", "weight": wt(3 * s), "clipTo": sname, "fill": "none", "taper": 1})
    if line is not None:
        ly = it + (ib - it) * line
        out.append({"shape": "rect", "x": il - 2, "y": ly - 3 * s, "w": ir - il + 4, "h": 6 * s, "fill": "$water",
                    "shade": 0, "corners": "sharp", "name": f"{name}_line"})
        # ticks on the clay cut face
        for side in (-1, 1):
            xx = (tl + (bl - tl) * ((ly - top) / h)) if side < 0 else (tr + (br - tr) * ((ly - top) / h))
            out.append({"shape": "rect", "cx": xx + side * -4 * s, "cy": ly, "w": 40 * s, "h": 10 * s, "fill": "$water", "shade": 0})
    if sensor is not None:
        t_in = sensor[0]
        sx = tr - 110 * s
        # stake and clip head; slides down into the soil at t_in
        keys = [{"t": 0, "y": -260 * s}, {"t": t_in, "y": -260 * s}, {"t": t_in + 0.35, "y": 0, "ease": "outBack"}]
        out.append({"shape": "rect", "x": sx - 9 * s, "y": top - 40 * s, "w": 18 * s, "h": 300 * s, "radius": 8 * s,
                    "fill": "$sensorLo", "group": f"{name}_sens", "shade": 0.4})
        out.append({"shape": "rect", "x": sx - 46 * s, "y": top - 120 * s, "w": 92 * s, "h": 110 * s, "radius": 34 * s,
                    "fill": "$sensor", "group": f"{name}_sens", "shade": {"amount": 0.7, "offset": 0.18},
                    "shadow": {"dx": 18 * s, "dy": 10 * s, "opacity": 0.45}})
        out.append({"shape": "circle", "cx": sx, "cy": top - 74 * s, "r": 12 * s, "fill": "$water", "group": f"{name}_sens", "shade": 0,
                    "animate": {"keys": [{"t": 0, "opacity": 0}, {"t": t_in + 0.4, "opacity": 0}, {"t": t_in + 0.5, "opacity": 1}]}})
        out.append({"__group__": {"name": f"{name}_sens", "animate": {"keys": keys}}})
    if plant:
        out += plant_shapes(plant, cx, top - 30 * s, s, sway, name, seed)
    return out


def leaf_path(x, y, ang, L, Wd):
    """A pointed leaf from base (x,y) pointing at angle ang (deg), length L, half-width Wd."""
    a = math.radians(ang)
    ux, uy = math.cos(a), math.sin(a)
    px, py = -uy, ux
    tip = (x + ux * L, y + uy * L)
    c1 = (x + ux * L * 0.25 + px * Wd * 1.3, y + uy * L * 0.25 + py * Wd * 1.3)
    c2 = (x + ux * L * 0.8 + px * Wd * 0.9, y + uy * L * 0.8 + py * Wd * 0.9)
    c3 = (x + ux * L * 0.8 - px * Wd * 0.9, y + uy * L * 0.8 - py * Wd * 0.9)
    c4 = (x + ux * L * 0.25 - px * Wd * 1.3, y + uy * L * 0.25 - py * Wd * 1.3)
    f = lambda p: f"{p[0]:.0f} {p[1]:.0f}"
    return f"M{f((x, y))} C{f(c1)} {f(c2)} {f(tip)} C{f(c3)} {f(c4)} {f((x, y))} Z", tip


def plant_shapes(kind, cx, base, s, sway, name, seed):
    out = []
    g = f"{name}_plant"
    out.append({"__group__": {"name": g, "pivot": [cx, base], "sway": {"rotate": 1.2 * sway, "period": 3.2, "phase": seed * 0.7}}})
    if kind == "monstera":
        spec = [(-150, 520, 290, 120), (-110, 600, 260, 110), (-60, 640, 300, 130), (-25, 700, 270, 120), (20, 560, 280, 120),
                (60, 620, 260, 115), (-80, 420, 240, 100), (-125, 360, 220, 95)]
    elif kind == "snake":
        spec = [(-100, 520, 60, 20), (-92, 640, 50, 18), (-84, 560, 50, 18), (-96, 470, 55, 18), (-80, 420, 45, 16)]
    elif kind == "fern":
        spec = [(-160, 380, 120, 50), (-140, 420, 120, 50), (-115, 440, 120, 50), (-90, 460, 120, 50), (-65, 440, 120, 50),
                (-40, 420, 120, 50), (-20, 380, 120, 50)]
    elif kind == "cactus":
        spec = []
    else:
        spec = []
    if kind == "monstera":
        for k, (ang, L, Lleaf, Wd) in enumerate(spec):
            a = math.radians(ang)
            ex, ey = cx + math.cos(a) * L * 0.55 * s, base + math.sin(a) * L * 0.55 * s
            out.append({"shape": "path", "points": [[cx + (k - 4) * 6 * s, base], [cx + math.cos(a) * L * 0.3 * s, base + math.sin(a) * L * 0.3 * s], [ex, ey]],
                        "smooth": True, "stroke": "$stem", "weight": wt(9 * s), "fill": "none", "group": g, "taper": 0.4})
            d, tip = leaf_path(ex, ey, ang + (12 if ang > -90 else -12), Lleaf * s, Wd * s)
            out.append({"shape": "path", "d": d, "fill": ["$leaf", "$leafLo", "$leaf", "$leafHi"][k % 4], "group": g,
                        "shade": {"amount": 0.75, "offset": 0.14}, "outline": True, "texture": {"style": "contour", "amount": 0.25},
                        "lag": 0.04 * k})
            out.append({"shape": "path", "points": [[ex, ey], [tip[0] * 0.5 + ex * 0.5 + 4, tip[1] * 0.5 + ey * 0.5 - 4], tip], "smooth": True,
                        "stroke": "$leafLo", "weight": wt(3 * s), "fill": "none", "group": g, "lag": 0.04 * k})
    elif kind in ("snake", "fern"):
        for k, (ang, L, Lleaf, Wd) in enumerate(spec):
            if kind == "snake":
                x0 = cx + (k - 2) * 34 * s
                d, tip = leaf_path(x0, base + 10 * s, ang, L * s, Wd * s * 1.5)
                out.append({"shape": "path", "d": d, "fill": ["$leaf", "$leafLo", "$leafHi"][k % 3], "group": g,
                            "shade": {"amount": 0.7, "offset": 0.15}, "texture": {"style": "hatch", "amount": 0.3}, "lag": 0.05 * k})
            else:
                a = math.radians(ang)
                pts = [[cx, base]]
                for j in range(1, 6):
                    pts.append([cx + math.cos(a) * L * s * j / 5, base + math.sin(a) * L * s * j / 5 + (j * j) * 6 * s])
                out.append({"shape": "path", "points": pts, "smooth": True, "stroke": "$stem", "weight": wt(6 * s), "fill": "none", "group": g, "taper": 0.8})
                for j in range(1, 6):
                    px, py = pts[j]
                    for side in (-1, 1):
                        d, _ = leaf_path(px, py, ang + side * 70, (70 - j * 6) * s, 14 * s)
                        out.append({"shape": "path", "d": d, "fill": "$leafHi" if side > 0 else "$leaf", "group": g, "shade": 0.5, "lag": 0.03 * j})
    elif kind == "cactus":
        out.append({"shape": "rect", "cx": cx, "cy": base - 170 * s, "w": 150 * s, "h": 340 * s, "radius": 75 * s, "fill": "$leaf", "group": g,
                    "shade": {"amount": 0.8, "offset": 0.18}, "texture": {"style": "contour", "amount": 0.5}, "outline": True})
        out.append({"shape": "rect", "cx": cx + 110 * s, "cy": base - 220 * s, "w": 70 * s, "h": 160 * s, "radius": 35 * s, "fill": "$leafHi", "group": g,
                    "shade": {"amount": 0.8, "offset": 0.18}, "outline": True})
    return out


def finalize(shapes):
    """Pull out the pseudo `__group__` entries into the illo `groups` list."""
    groups, real = [], []
    for sh in shapes:
        if "__group__" in sh:
            groups.append(sh["__group__"])
        else:
            real.append(sh)
    return groups, real


def illo(shapes, light="window", extra=None):
    groups, real = finalize(shapes)
    props = {"view": [W, H], "palette": PAL, "light": LIGHTS[light] if isinstance(light, str) else light,
             "hand": {"weight": [0.6, 3.6], "wobble": 1.2, "rough": 0.6, "boil": 8, "gaps": 0.2},
             "corners": "soft", "misregister": [1.5, 1], "grain": 0.05, "groups": groups, "shapes": real}
    if extra:
        props.update(extra)
    return {"component": "illo", "props": props}
