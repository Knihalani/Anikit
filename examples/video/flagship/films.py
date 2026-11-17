"""The six films inside EVERY FRAME, as anikit-video compositions.

Each film is 30 s at 1920x1080 and has its own art direction. In the flagship, lane
k's film is seen up close in three places, so that's where the craft goes:
  - frame 0 (all six first frames stand side by side in shot 3): a designed hero still;
  - film time 2 + 4k .. 6 + 4k s (its window in the locked run, shot 5): the peak;
  - 0 .. 8 s (on the floating screens, shot 7).

    python3 examples/video/flagship/films.py [film-id ...]   # writes films/<id>.json previews
"""

from __future__ import annotations

import json
import math
import sys
from pathlib import Path

HERE = Path(__file__).parent
W, H = 1920, 1080
SECONDS = 30.0


def window(k):
    return 2.0 + 4.0 * k, 6.0 + 4.0 * k


def comp(layers, background):
    return {"size": [W, H], "duration": SECONDS, "background": background, "layers": layers}


def rect(x, y, w, h, fill, **kw):
    return {"shape": "rect", "x": x, "y": y, "width": w, "height": h, "fill": fill, **kw}


def txt(text, x, y, size, color, font="Inter", weight=600, **kw):
    return {"text": text, "x": x, "y": y, "size": size, "color": color, "font": font, "weight": weight, **kw}


# =============================================================================
# 1. Sundial: chrome-and-glass product, golden hour (window 2-6 s)
# =============================================================================

def sundial():
    CREAM, GOLD, SUN, CORAL, INK = "#FFF4E3", "#FFC15E", "#FFD98A", "#FF7A59", "#10202A"
    # hour of the day from global time: dawn 0-2 s, the day sweeps 2-6.5 s, then dusk
    HOUR = "(5.9 + 0.8 * clamp(gt / 2, 0, 1) + 13.3 * smoothstep(2, 6.5, gt))"
    h = HOUR
    SUNRISE, SUNSET = 6.2, 19.97
    CX, CY, R = 1250, 545, 330

    def sky(top, bottom, op):
        return rect(960, 540, W, H, f"linear-gradient(180deg, {top}, {bottom})", opacity=op)

    def ang(e):
        return f"((({e}) - 12) / 24 * 2 * pi)"

    def card(i, time, title, sub, hour):
        near = f"(smoothstep({hour - 0.9}, {hour - 0.2}, {h}) * (1 - smoothstep({hour + 0.6}, {hour + 1.3}, {h})))"
        return {"type": "group", "x": 140, "y": 300 + i * 132, "width": 560, "height": 112, "anchor": "topLeft",
                "enter": {"preset": "fadeUp", "delay": 0.35 + 0.08 * i, "duration": 0.6},
                "layers": [
                    rect(280, 56, 560, 112, "#ffffff14", radius=22, stroke={"color": "#ffffff26", "width": 1.5}),
                    rect(280, 56, 560, 112, "#ffffff2e", radius=22, stroke={"color": GOLD, "width": 2.5},
                         opacity=f"={near}"),
                    rect(34, 56, 10, 54, GOLD if i != 3 else CORAL, radius=5),
                    txt(time, 68, 38, 22, "#ffffffb0", font="JetBrains Mono", weight=500, anchor="left"),
                    txt(title, 68, 76, 34, CREAM, font="Inter Display", weight=700, anchor="left"),
                    txt(sub, 530, 56, 22, "#ffffff99", weight=500, anchor="right"),
                ]}

    def tick(hour, label, color):
        a = math.radians((hour - 12) / 24 * 360)
        return [
            {"shape": "circle", "x": round(CX + (R + 16) * math.sin(a), 1), "y": round(CY - (R + 16) * math.cos(a), 1),
             "width": 12, "height": 12, "fill": color, "enter": {"preset": "popIn", "delay": 0.5 + (hour - 6) * 0.03}},
            txt(label, round(CX + (R + 66) * math.sin(a), 1), round(CY - (R + 66) * math.cos(a), 1), 20, "#ffffffb8",
                font="JetBrains Mono", weight=500, enter={"preset": "fadeIn", "delay": 0.6 + (hour - 6) * 0.03}),
        ]

    m = f"(({h}) * 60)"
    hh, mm = f"floor({m} / 60)", f"floor({m} - 60 * floor({m} / 60))"
    digits = [txt("{value:0}", CX + dx, CY - 6, 92, CREAM, font="Inter Display", weight=700, value=v)
              for v, dx in ((f"=floor({hh} / 10)", -96), (f"=({hh}) - 10 * floor({hh} / 10)", -48),
                            (f"=floor({mm} / 10)", 48), (f"=({mm}) - 10 * floor({mm} / 10)", 96))]
    digits.append(txt(":", CX, CY - 14, 92, CREAM, font="Inter Display", weight=700,
                      opacity="=0.6 + 0.4 * (floor(t * 2) - 2 * floor(t))"))
    a = ang(h)
    sun_x, sun_y = f"={CX} + {R} * sin({a})", f"={CY} - {R} * cos({a})"
    day0 = (SUNRISE - 12) / 24 * 360
    day_frac = (SUNSET - SUNRISE) / 24
    dial = {"type": "group", "start": 1.9, "end": 7.4, "exit": {"preset": "fadeOut", "duration": 0.4}, "layers": [
        {"shape": "circle", "x": CX, "y": CY, "width": 2 * R, "height": 2 * R, "fill": "none",
         "stroke": {"color": "#ffffff26", "width": 3}, "enter": {"preset": "drawOn", "duration": 0.9}},
        {"shape": "circle", "x": CX, "y": CY, "width": 2 * R, "height": 2 * R, "fill": "none", "rotation": day0 - 90,
         "stroke": {"color": GOLD, "width": 10, "cap": "round"}, "trimEnd": day_frac,
         "animate": [{"from": {"trimEnd": 0}, "at": 0.2, "duration": 1.0, "ease": "inOutCubic"}]},
        {"shape": "circle", "x": CX, "y": CY, "width": 2 * R, "height": 2 * R, "fill": "none", "rotation": day0 - 90,
         "stroke": {"color": SUN, "width": 18, "cap": "round"},
         "trimEnd": f"=clamp(({h} - {SUNRISE}) / 24, 0.001, {day_frac})", "glow": {"radius": 18, "color": GOLD},
         "opacity": "=smoothstep(0.4, 0.9, t)"},
        {"shape": "circle", "x": CX, "y": CY, "width": 2 * R - 80, "height": 2 * R - 80, "fill": "#ffffff0d",
         "stroke": {"color": "#ffffff1a", "width": 1.5}, "enter": {"preset": "scaleIn", "delay": 0.1}},
        *[l for hr, lab, col in ((6.67, "6:40", GOLD), (9, "9:00", GOLD), (12.5, "12:30", GOLD), (17.87, "17:52", CORAL))
          for l in tick(hr, lab, col)],
        {"shape": "circle", "x": sun_x, "y": sun_y, "width": 120, "height": 120,
         "fill": "radial-gradient(#FFE6A866, #FFC15E00)", "opacity": "=smoothstep(0.3, 0.8, t)"},
        {"shape": "circle", "x": sun_x, "y": sun_y, "width": 44, "height": 44, "fill": SUN,
         "glow": {"radius": 30, "intensity": 1.2, "color": GOLD}, "opacity": "=smoothstep(0.3, 0.8, t)"},
        *[dict(l, start=0.2, enter={"preset": "fadeIn", "duration": 0.5}) for l in digits],
        txt("SUNSET 19:58", CX, CY + 70, 22, "#ffffffa8", letterSpacing="0.14em",
            enter={"preset": "fadeIn", "delay": 0.4}),
    ]}
    ui = {"type": "group", "start": 1.9, "end": 7.4, "exit": {"preset": "fadeOut", "duration": 0.4}, "layers": [
        txt("Sundial", 140, 150, 54, CREAM, font="Inter Display", weight=800, anchor="left"),
        {"shape": "circle", "x": 112, "y": 150, "width": 26, "height": 26, "fill": SUN, "glow": {"radius": 14, "color": GOLD}},
        txt("TODAY · THURSDAY", 140, 222, 22, "#ffffff99", letterSpacing="0.14em", anchor="left"),
        card(0, "06:40", "Run by the river", "sunrise +28m", 6.67),
        card(1, "09:00", "Deep work", "bright, 3h", 9.0),
        card(2, "12:30", "Lunch outside", "sun high", 12.5),
        card(3, "17:52", "Golden hour walk", "45 min", 17.87),
        {"type": "group", "x": 140, "y": 860, "width": 560, "height": 96, "anchor": "topLeft", "start": 3.7, "end": 4.7,
         "enter": {"preset": "slideUp", "duration": 0.5}, "exit": {"preset": "fadeOut", "duration": 0.3}, "layers": [
             rect(280, 48, 560, 96, CREAM, radius=48),
             {"shape": "circle", "x": 52, "y": 48, "width": 30, "height": 30, "fill": CORAL},
             txt("Golden hour starts in 15 min", 86, 48, 30, INK, weight=650, anchor="left")]},
    ]}
    title = {"type": "group", "start": 0, "end": 2.15, "exit": {"preset": "fadeOut", "duration": 0.25}, "layers": [
        {"component": "scene3d", "width": W, "height": H,
         "props": {"text": "Sundial", "font": "Inter Display", "weight": 800, "env": "sunset", "material": "gold",
                   "primitives": False, "aperture": 18, "bloom": 0.12, "quality": "standard",
                   "camera": [{"at": 0.0, "pos": [-2.6, 0.9, 8.6], "target": [0.0, 0.45, 0.0], "fov": 30},
                              {"at": 2.2, "pos": [-1.5, 0.7, 7.4], "target": [0.0, 0.42, 0.0], "fov": 29}]}},
        txt("Plan your day by the light.", 960, 860, 44, "#FFF4E3e0", weight=500,
            exit={"preset": "fadeOut", "duration": 0.3}),
    ]}
    phone = {"type": "group", "start": 7.0, "end": 16.4, "enter": {"preset": "fadeIn", "duration": 0.5},
             "exit": {"preset": "fadeOut", "duration": 0.4}, "layers": [
        {"component": "model", "x": 1260, "y": 560, "width": 900, "height": 900,
         "props": {"shape": "rounded-box", "size": [0.5, 1.0, 0.06], "radius": 0.07, "color": "#E8B66A",
                   "metallic": 0.85, "roughness": 0.25, "spin": 22, "angle": -35, "tilt": 12, "float": 0.03}},
        txt("Your day,\nlit right.", 160, 470, 120, CREAM, font="Inter Display", weight=800, lineHeight=1.0,
            anchor="left", align="left", kinetic="mask by line up"),
        txt("Sundial plans around sunrise, sunset and golden hour.", 160, 660, 34, "#ffffffb0", weight=500,
            anchor="left", align="left", width=640, enter={"preset": "fadeUp", "delay": 0.5}),
    ]}
    months = ["J", "F", "M", "A", "M", "J", "J", "A", "S", "O", "N", "D"]
    year = {"type": "group", "start": 16.0, "end": 23.4, "enter": {"preset": "fadeIn", "duration": 0.5},
            "exit": {"preset": "fadeOut", "duration": 0.4}, "layers": [
        txt("Every season, its own light.", 960, 200, 72, CREAM, font="Inter Display", weight=800, kinetic="rise"),
        {"component": "area-chart", "x": 960, "y": 640, "width": 1400, "height": 640,
         "props": {"series": [[8.0, 9.8, 11.9, 13.9, 15.6, 16.6, 16.1, 14.5, 12.5, 10.5, 8.6, 7.7]],
                   "names": ["Hours of daylight"], "labels": months, "colors": [GOLD], "area": True, "smooth": True,
                   "delay": 0.3}},
    ]}
    end = {"type": "group", "start": 23.0, "end": SECONDS, "enter": {"preset": "fadeIn", "duration": 0.5}, "layers": [
        {"shape": "circle", "x": 960, "y": 470, "width": 160, "height": 160, "fill": "radial-gradient(#FFF1C8, #FFC15E)",
         "glow": {"radius": 50, "color": GOLD}, "enter": {"preset": "popIn"}},
        txt("Sundial", 960, 650, 150, CREAM, font="Inter Display", weight=800, enter={"preset": "fadeUp", "delay": 0.3}),
        txt("Coming this spring", 960, 760, 40, "#ffffffb0", weight=500, enter={"preset": "fadeUp", "delay": 0.6}),
    ]}
    skies = [
        sky("#0D2230", "#E58A5C", 1),
        sky("#2C7FA8", "#CDE8EA", f"=smoothstep(8.2, 10.5, {h}) * (1 - smoothstep(14.5, 17, {h}))"),
        sky("#22456A", "#FFB45F", f"=smoothstep(14.5, 17, {h}) * (1 - smoothstep(18.6, 19.7, {h}))"),
        sky("#121A33", "#D65E48", f"=smoothstep(18.6, 19.7, {h})"),
    ]
    zoom = {"type": "group", "layers": [dial, ui],
            "animate": [{"from": {"scale": 1.0}, "to": {"scale": 1.05}, "at": 2, "duration": 4.5, "ease": "inOutSine"}]}
    return comp(skies + [title, zoom, phone, year, end, {"component": "grain", "props": {"amount": 0.035}}], "#10202A")


# =============================================================================
# 2. Maison Lune: editorial fashion (window 6-10 s)
# =============================================================================

def maison():
    IVORY, OX, INK, SAND = "#F3EDE3", "#6E1423", "#1E1416", "#C9B8A6"
    serif = "Instrument Serif"
    fabric = {"component": "mesh-gradient", "props": {"colors": ["#8C1C2E", "#4A0D18", "#B0485A", "#2B070E", "#7A1A2A"],
                                                       "base": "#3A0A14", "speed": 0.35, "warp": 0.9, "intensity": 0.9,
                                                       "seed": 7, "rate": 30, "smooth": True}}
    layers = [
        # the page
        rect(960, 540, W, H, IVORY),
        # fabric panel: a tall plate on the left that breathes, a second plate slides in for the window
        {"type": "group", "x": 520, "y": 540, "width": 640, "height": 860, "clip": True,
         "animate": [{"from": {"scale": 1.0}, "to": {"scale": 1.02}, "duration": SECONDS, "ease": "linear"}],
         "layers": [{"shader": (HERE / "shaders" / "silk.glsl").read_text(), "width": 640, "height": 860,
                     "uniforms": {"speed": 1.0, "light": "=smoothstep(8.7, 9.0, gt) * (1 - smoothstep(10.0, 10.3, gt))",
                                  "slit": "=0.1 + 0.8 * smoothstep(8.8, 10.1, gt)"}},
                    {"shape": "rect", "x": 320, "y": 430, "width": 640, "height": 860,
                     "fill": "linear-gradient(160deg, #ffffff22, #00000000 40%, #00000055)"}]},
        # per-word treatments of the plate (clipped to it)
        {"type": "group", "x": 520, "y": 540, "width": 640, "height": 860, "clip": True, "layers": [
            # wool: a fine weave
            {"type": "group", "start": 5.8, "end": 7.25, "enter": {"preset": "fadeIn", "duration": 0.2},
             "animate": [{"from": {"scale": 1.08}, "to": {"scale": 1.0}, "duration": 1.45, "ease": "outCubic"}],
             "layers": [rect(320, 4 + 8 * r, 640, 3, "#00000030") for r in range(108)]
                       + [rect(4 + 8 * c, 430, 2, 860, "#ffffff10") for c in range(80)]},
            # silk: one slow gloss pass on the bias
            {"type": "group", "start": 7.25, "end": 8.7, "layers": [
                rect("=-300 + 1300 * (t / 1.45)", 430, 160, 1400,
                     "linear-gradient(90deg, #ffffff00, #ffd9e033, #ffffff00)", skewX=-30, blend="screen")]},
        ]},
        rect(520, 540, 640, 860, "none", stroke={"color": INK, "width": 1}),
        # masthead
        txt("MAISON LUNE", 1160, 140, 26, INK, weight=600, letterSpacing="0.42em", anchor="left"),
        rect(1160, 170, 600, 1.5, INK, anchor="left"),
        txt("AUTUMN / WINTER", 1760, 140, 20, INK, weight=500, letterSpacing="0.28em", anchor="right"),
        # title: huge serif italic, on frame 0 already
        {"type": "group", "end": 5.6, "exit": {"preset": "fadeOut", "duration": 0.6}, "layers": [
            txt("The quiet\nhour.", 1160, 470, 200, OX, font=serif, weight=400, italic=True, lineHeight=0.92,
                anchor="left", align="left"),
            txt("A study in wool, silk and shadow.", 1164, 720, 34, INK, font=serif, weight=400, anchor="left"),
            txt("Look 01 — 24", 1164, 900, 20, INK, letterSpacing="0.2em", weight=500, anchor="left"),
        ]},
        # the window: three words, masked line reveals, one per bar
        *[{"type": "group", "start": s, "end": s + 1.45, "exit": {"preset": "fadeOut", "duration": 0.25}, "layers": [
            txt(word, 1160, 520, 230 if len(word) < 6 else 200, OX, font=serif, weight=400, italic=True, anchor="left", align="left",
                kinetic="mask by line up 0.55s outExpo"),
            txt(sub, 1166, 700, 30, INK, font=serif, weight=400, anchor="left", enter={"preset": "fadeIn", "delay": 0.3}),
            txt(f"Look {n:02d}", 1166, 900, 20, INK, letterSpacing="0.2em", weight=500, anchor="left"),
        ]} for s, word, sub, n in ((5.8, "Wool.", "Double-faced, undyed, heavy.", 7),
                                   (7.25, "Silk.", "Cut on the bias, oxblood.", 12),
                                   (8.7, "Shadow.", "Worn after dark.", 19))],
        # a hairline that sweeps the page each time a word lands
        *[rect(f"=1160 + 600 * smoothstep({s}, {s + 0.6}, gt)", 980, 2, 60, OX) for s in (5.8, 7.25, 8.7)],
        # after the window: credits and the lookbook grid
        {"type": "group", "start": 10.2, "end": SECONDS, "enter": {"preset": "fadeIn", "duration": 0.8}, "layers": [
            txt("Maison Lune", 1160, 470, 150, INK, font=serif, weight=400, italic=True, anchor="left"),
            txt("Autumn / Winter collection.\nIn stores and online.", 1164, 640, 32, INK, font=serif, weight=400,
                anchor="left", align="left", lineHeight=1.35),
        ]},
        txt("01", 1760, 1000, 18, INK, weight=500, letterSpacing="0.2em", anchor="right"),
        {"component": "grain", "props": {"amount": 0.05}},
    ]
    return comp(layers, IVORY)


# =============================================================================
# 3. Where the Rain Goes: Swiss data-viz explainer (window 10-14 s)
# =============================================================================

def rain():
    BLUE, BLACK, WHITE, GREY = "#1F3BFF", "#0B0B0C", "#FAFAF7", "#D7D9DE"
    layers = [rect(960, 540, W, H, WHITE),
              {"shader": (HERE / "shaders" / "rain.glsl").read_text(), "width": W, "height": H, "end": 10.4,
               "uniforms": {"density": "=1.0 - 0.6 * smoothstep(2.4, 3.2, gt)", "ground": 0.9},
               "exit": {"preset": "fadeOut", "duration": 0.4}}]
    # Swiss grid
    for i in range(1, 12):
        layers.append(rect(160 * i, 540, 1, H, "#0000000d"))
    layers += [
        {"type": "group", "end": 2.7, "exit": {"preset": "fadeOut", "duration": 0.35}, "layers": [
            txt("Where the\nRain Goes", 160, 380, 210, BLACK, font="Inter Display", weight=800, lineHeight=0.92,
                letterSpacing="-0.03em", anchor="left", align="left"),
            txt("One year of rain on one city, followed drop by drop.", 168, 660, 38, BLACK, weight=500,
                anchor="left")]},

        txt("01", 160, 980, 20, BLACK, font="JetBrains Mono", weight=500, anchor="left"),
        txt("Source: illustrative", 1760, 980, 18, "#00000080", weight=500, anchor="right"),
    ]
    # 3-10 s: the year as bars filling, month by month
    months = ["J", "F", "M", "A", "M", "J", "J", "A", "S", "O", "N", "D"]
    mm = [78, 52, 61, 49, 55, 45, 44, 50, 49, 69, 84, 76]
    bars = {"type": "group", "start": 2.6, "end": 10.2, "exit": {"preset": "fadeOut", "duration": 0.3}, "layers": []}
    for i, (mo, v) in enumerate(zip(months, mm)):
        x = 820 + i * 80
        bars["layers"] += [
            rect(x, 900, 46, v * 6.5, BLUE, anchor="bottom",
                 animate=[{"from": {"height": 0}, "at": 0.4 + i * 0.12, "duration": 0.8, "ease": "outExpo"}]),
            txt(mo, x, 940, 22, BLACK, font="JetBrains Mono", weight=500),
        ]
    bars["layers"] += [txt("Millimetres of rain, by month", 796, 340, 26, BLACK, weight=600, anchor="left"),
                       rect(1260, 900, 960, 2, BLACK)]
    layers.append(bars)
    # 10-14 s: the window. A flow diagram draws on: rain splits into three paths with counters.
    flow = {"type": "group", "start": 10.0, "end": 17.5, "exit": {"preset": "fadeOut", "duration": 0.4}, "layers": [
        txt("Where it goes", 160, 170, 64, BLACK, font="Inter Display", weight=800, anchor="left",
            kinetic="mask by line up"),
        rect(240, 560, 120, 520, BLUE, anchor="center", animate=[{"from": {"height": 0}, "duration": 0.6, "ease": "outExpo"}]),
        txt("Rain\n100%", 240, 870, 28, BLACK, weight=700, lineHeight=1.1),
    ]}
    paths = [("Evaporates", 45, 330, "#1F3BFF"), ("Runs off", 30, 590, "#5B70FF"), ("Soaks in", 25, 850, "#9AA8FF")]
    y0 = 300
    for i, (name, pct, y_end, col) in enumerate(paths):
        hgt = pct * 5.2
        y_start = y0 + hgt / 2
        y0 += hgt
        d = f"M 300 {y_start:.0f} C 800 {y_start:.0f}, 900 {y_end}, 1380 {y_end}"
        flow["layers"] += [
            {"shape": "path", "d": d, "fill": "none", "x": 840, "y": round((y_start + y_end) / 2, 1), "stroke": {"color": col + "cc", "width": hgt, "cap": "butt"},
             "trimEnd": 1, "animate": [{"from": {"trimEnd": 0}, "at": 0.5 + 0.35 * i, "duration": 1.1,
                                        "ease": "inOutCubic"}]},
            rect(1400, y_end, 18, hgt, col, anchor="left",
                 animate=[{"from": {"height": 0}, "at": 1.5 + 0.35 * i, "duration": 0.4, "ease": "outExpo"}]),
            txt("{value:0}%", 1450, y_end - 6, 96, BLACK, font="Inter Display", weight=800, anchor="left", value=pct,
                animate=[{"from": {"value": 0}, "at": 1.5 + 0.35 * i, "duration": 1.0, "ease": "outCubic"},
                         {"from": {"opacity": 0}, "at": 1.5 + 0.35 * i, "duration": 0.2}]),
            txt(name, 1456, y_end + 58, 28, BLACK, weight=600, anchor="left",
                animate=[{"from": {"opacity": 0}, "at": 1.7 + 0.35 * i, "duration": 0.3}]),
        ]
    layers.append(flow)
    layers.append({"type": "group", "start": 17.0, "end": SECONDS, "enter": {"preset": "fadeIn", "duration": 0.5}, "layers": [
        {"component": "donut-chart", "x": 640, "y": 560, "width": 640, "height": 640,
         "props": {"values": [45, 30, 25], "labels": ["Evaporates", "Runs off", "Soaks in"],
                   "colors": [BLUE, "#5B70FF", "#9AA8FF"]}},
        txt("Design the city\nfor the 25%.", 1100, 520, 96, BLACK, font="Inter Display", weight=800, lineHeight=0.95,
            anchor="left", align="left", kinetic="mask by line up"),
    ]})
    return comp(layers, WHITE)


# =============================================================================
# 4. No Sleep: kinetic-type lyric video, 128 BPM (window 14-18 s)
# =============================================================================

def nosleep():
    ACID, BLACK, RED = "#E8FF2E", "#0A0A0A", "#FF2E2E"
    beat = 60 / 128
    disp = dict(font="Inter Display", weight=900, letterSpacing="-0.04em")
    layers = [rect(960, 540, W, H, BLACK)]
    # outlined marquee rows behind everything, alternating directions
    for r in range(7):
        y = 60 + r * 160
        d = 1 if r % 2 == 0 else -1
        layers.append(txt("NO SLEEP  ·  " * 8, f"={960 + d * 300} + {d} * gt * 140 - {d} * 1400 * floor(({d} * gt * 140 + 700) / 1400)",
                          y, 150, "none", **disp, stroke={"color": ACID + "40", "width": 2}))
    # frame 0: the title, full bleed
    layers.append({"type": "group", "end": 1.88, "layers": [
        txt("NO\nSLEEP", 960, 560, 470, ACID, **disp, lineHeight=0.82, exit={"preset": "zoomOut", "duration": 0.3}),
    ]})
    lyric = ["CITY", "LIGHTS", "DON'T", "BLINK", "SO", "WHY", "SHOULD", "I", "NO", "SLEEP", "NO", "SLEEP",
             "TILL", "THE", "SUN", "COMES", "UP", "RED", "NO", "SLEEP", "TONIGHT", "WE", "RUN", "THE", "STREETS",
             "ELECTRIC", "NO", "SLEEP", "NO", "SLEEP"]
    styles = ["slam", "invert", "stack", "scramble", "slam", "skew"]
    t, i = 1.88, 0
    while t < SECONDS - beat and i < 200:
        word = lyric[i % len(lyric)]
        st = styles[i % len(styles)]
        inv = st == "invert" or (word == "SLEEP" and i % 4 == 1)
        fg, bg = (BLACK, ACID) if inv else (ACID, BLACK)
        if word == "RED":
            fg, bg = BLACK, RED
        size = 520 if len(word) <= 4 else 380 if len(word) <= 6 else 260
        g = {"type": "group", "start": round(t, 4), "end": round(t + beat, 4), "layers": []}
        if bg != BLACK:
            g["layers"].append(rect(960, 540, W, H, bg))
        # outline echo: a bigger hollow copy that blows out behind the word
        g["layers"].append(txt(word, 960, 560, size, "none", **disp, stroke={"color": fg + "90", "width": 3},
                               animate=[{"from": {"scale": 1.0, "opacity": 1}, "to": {"scale": 1.5, "opacity": 0},
                                         "duration": beat, "ease": "outCubic"}]))
        if st == "stack":
            for k, dy in enumerate((-0.92, 0.0, 0.92)):
                filled = dy == 0.0
                g["layers"].append(txt(word, 960, 560 + dy * size, size, fg if filled else "none", **disp,
                                       stroke=None if filled else {"color": fg, "width": 3},
                                       animate=[{"from": {"y": f"{'+' if dy <= 0 else '-'}{abs(dy) * size * 0.4 + 40:.0f}"},
                                                 "duration": beat * 0.6, "ease": "outExpo"}]))
        elif st == "scramble":
            g["layers"].append(txt(word, 960, 560, size, fg, **disp, kinetic="scramble"))
        elif st == "skew":
            g["layers"].append(txt(word, 960, 560, size, fg, **disp, skewX=-14,
                                   animate=[{"from": {"scale": 1.35, "skewX": -30}, "duration": beat * 0.7,
                                             "ease": "outExpo"}]))
        else:
            g["layers"].append(txt(word, 960, 560, size, fg, **disp,
                                   animate=[{"from": {"scale": 1.6, "opacity": 0}, "duration": beat * 0.35,
                                             "ease": "outExpo"},
                                            {"to": {"scale": 0.94}, "at": beat * 0.35, "duration": beat * 0.65,
                                             "ease": "linear"}]))
        if i % 2 == 1:   # a red slash rips across on the off-beats
            g["layers"].append(rect("=-300 + 2520 * smoothstep(0, 0.22, t)", 540, 90, 1500, RED, skewX=-28,
                                    blend="normal", opacity=0.92))
        g["layers"].append(txt(f"{i + 1:03d}", 80, 1010, 22, fg, font="JetBrains Mono", weight=600, anchor="left"))
        g["layers"].append(txt("NO SLEEP — 128", 1840, 1010, 22, fg, font="JetBrains Mono", weight=600, anchor="right"))
        layers.append(g)
        t += beat
        i += 1
    for k in range(int((SECONDS - 1.88) / (beat * 4))):
        tb = 1.88 + k * beat * 4 - 0.06
        layers.append({"shader": "glitch", "filter": True, "start": round(tb, 4), "end": round(tb + 0.2, 4),
                       "uniforms": {"amount": 0.9, "rate": 8.0}})
    layers.append({"component": "grain", "props": {"amount": 0.08, "rate": 24}})
    return comp(layers, BLACK)


# =============================================================================
# 5. Fold: abstract, iridescent (window 18-22 s). Placeholder until the GLSL layer lands.
# =============================================================================

def fold():
    BG = "#03100C"
    glsl = (HERE / "shaders" / "fold.glsl").read_text()
    layers = [
        {"shader": glsl, "width": W, "height": H,
         "uniforms": {"fold": "=0.75 + 0.25 * smoothstep(0, 4, gt) + 0.12 * sin(gt * 0.3)", "spin": 1.0,
                      "film": "=0.6 + 0.4 * smoothstep(16, 20, gt)"}},
        txt("FOLD", 160, 980, 22, "#BFFFEA", font="JetBrains Mono", weight=500, letterSpacing="0.4em", anchor="left"),
        txt("glass, folding through itself", 1760, 980, 20, "#BFFFEA99", font="JetBrains Mono", weight=400,
            anchor="right"),
        {"component": "grain", "props": {"amount": 0.05}},
    ]
    return comp(layers, BG)


# =============================================================================
# 6. Harbour Cup: broadcast sports open (window 22-26 s)
# =============================================================================

def harbour():
    NAVY, RED, WHITE, GOLD = "#0A1A3F", "#FF2E3B", "#FFFFFF", "#FFC83D"
    layers = [
        rect(960, 540, W, H, f"linear-gradient(160deg, #13306E, {NAVY} 55%, #050C20)"),
        # stadium light flares
        *[l for i, x in enumerate((180, 620, 1300, 1740)) for l in (
            {"shape": "circle", "x": x, "y": 70, "width": 360, "height": 360,
             "fill": "#9FC2FF30", "blur": 90, "opacity": f"=0.7 + 0.3 * sin(gt * 1.3 + {i})"},
            {"shape": "rect", "x": x, "y": 70, "width": 120, "height": 34, "radius": 8, "fill": "#F4F8FF",
             "glow": {"radius": 40, "intensity": 1.4, "color": "#BFD6FF"}})],
        rect(960, 1000, W, 300, "linear-gradient(180deg, #05102800, #050C20)"),
    ]
    # light sweeps: skewed bars racing across, every 2 s and densely in the window
    sweeps = []
    for k in range(14):
        s = 0.3 + k * 2.0 if k < 10 else 22.0 + (k - 10) * 0.5
        sweeps.append(rect(f"=-400 + 2800 * smoothstep({s}, {s + 0.7}, gt)", 540, 160, 1400,
                           f"linear-gradient(90deg, {RED}00, {RED}, {RED}00)", skewX=-24, opacity=0.85,
                           blend="screen"))
    layers += sweeps
    # frame 0 lockup
    layers.append({"type": "group", "end": 6.0, "exit": {"preset": "fadeOut", "duration": 0.3}, "layers": [
        {"component": "scene3d", "width": W, "height": H,
         "props": {"text": "HARBOUR", "font": "Inter Display", "weight": 900, "env": "studio", "material": "chrome",
                   "primitives": False, "aperture": 12, "bloom": 0.2, "exposure": 1.25, "quality": "standard",
                   "camera": [{"at": 0.0, "pos": [3.0, 0.2, 10.5], "target": [0.0, 0.62, 0.0], "fov": 30},
                              {"at": 6.0, "pos": [-2.2, 0.5, 9.4], "target": [0.0, 0.62, 0.0], "fov": 29}]}},
        rect(960, 540, W, H, "linear-gradient(180deg, #6C8BE0, #2A4FB0)", blend="multiply"),
        *[l for i, x in enumerate((180, 620, 1300, 1740)) for l in (
            {"shape": "circle", "x": x, "y": 40, "width": 380, "height": 380,
             "fill": "#BFD6FF38", "blur": 90, "opacity": f"=0.7 + 0.3 * sin(gt * 1.3 + {i})"},
            {"shape": "rect", "x": x, "y": 40, "width": 140, "height": 34, "radius": 8, "fill": "#F4F8FF",
             "glow": {"radius": 50, "intensity": 1.6, "color": "#BFD6FF"}})],
        rect(960, 760, 1100, 120, RED, skewX=-18, animate=[{"from": {"scaleX": 0}, "at": 0.3, "duration": 0.4,
                                                             "ease": "outExpo"}]),
        txt("CUP FINAL", 960, 768, 110, WHITE, font="Inter Display", weight=900, skewX=-12, letterSpacing="0.1em",
            animate=[{"from": {"opacity": 0, "x": "-200"}, "at": 0.45, "duration": 0.35, "ease": "outExpo"}]),
        txt("LIVE · SATURDAY 20:00", 960, 880, 34, WHITE, weight=700, letterSpacing="0.3em", skewX=-12),
    ]})
    # the window 22-26: team slams, crests, versus, score bug
    team = {"type": "group", "start": 21.8, "end": SECONDS,
            "animate": [{"from": {"scale": 1.0}, "to": {"scale": 1.12}, "at": 0.0, "duration": 4.5, "ease": "linear"},
                        {"keyframes": {"x": [960, 975, 948, 966, 957, 960], "y": [540, 530, 548, 536, 543, 540]},
                         "at": 1.3, "duration": 0.35, "ease": "linear"}],
            "layers": [
        {"type": "group", "animate": [{"from": {"x": "-900"}, "at": 0.2, "duration": 0.45, "ease": "outExpo"}],
         "layers": [
             {"shape": "polygon", "x": 500, "y": 450, "width": 400, "height": 450, "sides": 6, "fill": WHITE, "rotation": 30,
              "glow": {"radius": 40, "intensity": 0.6, "color": "#9FC2FF"}},
             txt("N", 500, 462, 270, NAVY, font="Inter Display", weight=900, skewX=-10),
             txt("NORTHPORT", 500, 790, 96, WHITE, font="Inter Display", weight=900, skewX=-12)]},
        {"type": "group", "animate": [{"from": {"x": "+900"}, "at": 0.7, "duration": 0.45, "ease": "outExpo"}],
         "layers": [
             {"shape": "polygon", "x": 1420, "y": 450, "width": 400, "height": 450, "sides": 6, "fill": RED, "rotation": 30,
              "glow": {"radius": 40, "intensity": 0.7, "color": RED}},
             txt("S", 1420, 462, 270, WHITE, font="Inter Display", weight=900, skewX=-10),
             txt("SALT BAY", 1420, 790, 96, WHITE, font="Inter Display", weight=900, skewX=-12)]},
        txt("VS", 960, 470, 190, GOLD, font="Inter Display", weight=900, skewX=-12,
            animate=[{"from": {"scale": 2.5, "opacity": 0}, "at": 1.3, "duration": 0.35, "ease": "outExpo"}]),
        # score bug
        {"type": "group", "animate": [{"from": {"y": "+200"}, "at": 2.2, "duration": 0.5, "ease": "outExpo"}],
         "layers": [
             rect(420, 975, 560, 64, "#050C20", skewX=-12),
             rect(162, 975, 44, 64, WHITE, skewX=-12),
             txt("NOR", 260, 975, 30, WHITE, weight=800, letterSpacing="0.06em"),
             txt("0  –  0", 420, 975, 34, WHITE, font="Inter Display", weight=800),
             txt("SAL", 580, 975, 30, WHITE, weight=800, letterSpacing="0.06em"),
             rect(678, 975, 44, 64, RED, skewX=-12),
             txt("LIVE", 790, 975, 26, RED, weight=800, letterSpacing="0.2em")]},
    ]}
    layers.append(team)
    layers.append({"type": "group", "start": 24.6, "end": SECONDS, "layers": [
        rect(960, 540, W, 230, f"linear-gradient(90deg, {RED}, #C8102E)", skewY=-4,
             animate=[{"from": {"scaleX": 0}, "duration": 0.3, "ease": "outExpo"}]),
        txt("KICK-OFF 20:00", 960, 548, 150, WHITE, font="Inter Display", weight=900, skewX=-12,
            animate=[{"from": {"x": "+600", "opacity": 0}, "at": 0.08, "duration": 0.35, "ease": "outExpo"}]),
    ]})
    mid = {"type": "group", "start": 6.0, "end": 22.1, "exit": {"preset": "zoomOut", "duration": 0.3}, "layers": [
        txt("ONE NIGHT.", 960, 480, 220, WHITE, font="Inter Display", weight=900, skewX=-12,
            kinetic="pop by char from center 0.4s 30ms outBack"),
        txt("ONE CUP.", 960, 700, 220, RED, font="Inter Display", weight=900, skewX=-12,
            enter={"preset": "slideUp", "delay": 1.2}),
    ]}
    layers.append(mid)
    layers.append({"component": "grain", "props": {"amount": 0.04}})
    return comp(layers, NAVY)


FILMS = {"sundial": sundial, "maison": maison, "rain": rain, "nosleep": nosleep, "fold": fold, "harbour": harbour}


def preview_spec(fid):
    return {"size": [W, H], "fps": 60, "duration": SECONDS,
            "compositions": {fid: FILMS[fid]()},
            "scenes": [{"duration": SECONDS, "layers": [{"composition": fid, "width": W, "height": H}]}]}


if __name__ == "__main__":
    out = HERE / "films"
    out.mkdir(exist_ok=True)
    for fid in (sys.argv[1:] or FILMS):
        (out / f"{fid}.json").write_text(json.dumps(preview_spec(fid), indent=1))
        print(out / f"{fid}.json")
