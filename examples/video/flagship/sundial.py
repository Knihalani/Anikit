"""Sundial: the film inside EVERY FRAME (a fictional calendar app's 60 s launch film).

Every pane in the flagship shows one frame of this film. It's on screen up close at
0-1 s (frame 1, shot 3), 8.4-14.4 s (the projector-gate lock, shot 6) and 0-7 s on
the phone (shot 8), so that's where the craft goes.

Own art direction: golden-hour sky that follows the time of day, cream type, a sun dial
UI. Writes sundial.json next to this file.

    python3 examples/video/flagship/sundial.py
"""

from __future__ import annotations

import json
from pathlib import Path

HERE = Path(__file__).parent
W, H = 1920, 1080

CREAM = "#FFF4E3"
GOLD = "#FFC15E"
SUN = "#FFD98A"
CORAL = "#FF7A59"
INK = "#10202A"

# Hour of the day as a function of the layer's local time t (all layers start at 0).
# Dawn drifts 5:52 -> 6:40 over 8 s, then the day sweeps to 19:58 by 15 s, then dusk.
HOUR = "(5.87 + 0.8 * clamp(gt / 8, 0, 1) + 13.3 * smoothstep(8, 15, gt))"
SUNRISE, SUNSET = 6.2, 19.97

CX, CY, R = 1250, 545, 330        # the dial


def ang(hour_expr):
    """Clockwise angle from 12 o'clock (noon at the top), radians, as an expression."""
    return f"(({hour_expr}) - 12) / 24 * 2 * pi"


def sky(top, bottom, opacity):
    return {"shape": "rect", "width": W, "height": H, "fill": f"linear-gradient(180deg, {top}, {bottom})",
            "opacity": opacity}


def card(i, time, title, sub, hour):
    y = 300 + i * 132
    near = f"(smoothstep({hour - 0.9}, {hour - 0.2}, {HOUR}) * (1 - smoothstep({hour + 0.6}, {hour + 1.3}, {HOUR})))"
    return {
        "type": "group", "x": 140, "y": y, "width": 560, "height": 112, "anchor": "topLeft",
        "enter": {"preset": "fadeUp", "delay": 4.4 + 0.12 * i, "duration": 0.7},
        "layers": [
            {"shape": "rect", "width": 560, "height": 112, "radius": 22, "fill": "#ffffff14",
             "stroke": {"color": "#ffffff26", "width": 1.5}},
            {"shape": "rect", "width": 560, "height": 112, "radius": 22, "fill": "#ffffff2e",
             "stroke": {"color": GOLD, "width": 2.5}, "opacity": f"={near}"},
            {"shape": "rect", "x": 34, "y": 56, "width": 10, "height": 54, "radius": 5,
             "fill": GOLD if i != 3 else CORAL},
            {"text": time, "x": 68, "y": 38, "anchor": "left", "font": "JetBrains Mono", "size": 22,
             "color": "#ffffffb0", "weight": 500},
            {"text": title, "x": 68, "y": 76, "anchor": "left", "font": "Inter Display", "size": 34,
             "weight": 700, "color": CREAM},
            {"text": sub, "x": 530, "y": 56, "anchor": "right", "size": 22, "color": "#ffffff99", "weight": 500},
        ],
    }


def tick(hour, label, color):
    a = (hour - 12) / 24 * 360
    import math
    r_out = R + 46
    x = CX + r_out * math.sin(math.radians(a))
    y = CY - r_out * math.cos(math.radians(a))
    return [
        {"shape": "circle", "x": round(CX + (R + 16) * math.sin(math.radians(a)), 1),
         "y": round(CY - (R + 16) * math.cos(math.radians(a)), 1), "width": 12, "height": 12, "fill": color,
         "enter": {"preset": "popIn", "delay": 5.0 + (hour - 6) * 0.05}},
        {"text": label, "x": round(x, 1), "y": round(y, 1), "size": 20, "font": "JetBrains Mono",
         "color": "#ffffffb8", "enter": {"preset": "fadeIn", "delay": 5.1 + (hour - 6) * 0.05}},
    ]


def digits():
    """hh:mm in the centre of the dial, from HOUR."""
    m = f"(({HOUR}) * 60)"
    hh = f"floor({m} / 60)"
    mm = f"floor({m} - 60 * floor({m} / 60))"
    parts = [
        ("=floor(%s / 10)" % hh, -96), ("=(%s) - 10 * floor(%s / 10)" % (hh, hh), -48),
        ("=floor(%s / 10)" % mm, 48), ("=(%s) - 10 * floor(%s / 10)" % (mm, mm), 96),
    ]
    out = [{"text": "{value:0}", "value": v, "x": CX + dx, "y": CY - 6, "font": "Inter Display", "size": 92,
            "weight": 700, "color": CREAM} for v, dx in parts]
    out.append({"text": ":", "x": CX, "y": CY - 14, "font": "Inter Display", "size": 92, "weight": 700,
                "color": CREAM, "opacity": "=0.55 + 0.45 * step"})
    return out


def build():
    # sky that follows the hour
    h = HOUR
    sky_layers = [
        sky("#0D2230", "#E58A5C", 1),
        sky("#2C7FA8", "#CDE8EA", f"=smoothstep(8.2, 10.5, {h}) * (1 - smoothstep(14.5, 17, {h}))"),
        sky("#22456A", "#FFB45F", f"=smoothstep(14.5, 17, {h}) * (1 - smoothstep(18.6, 19.7, {h}))"),
        sky("#121A33", "#D65E48", f"=smoothstep(18.6, 19.7, {h})"),
    ]
    a = ang(h)
    sun_x = f"={CX} + {R} * sin({a})"
    sun_y = f"={CY} - {R} * cos({a})"
    day0 = (SUNRISE - 12) / 24 * 360
    day_frac = (SUNSET - SUNRISE) / 24

    dial = {
        "type": "group", "start": 3.4, "end": 30.4, "exit": {"preset": "fadeOut", "duration": 0.5},
        "layers": [
            # night ring and daylight arc
            {"shape": "circle", "x": CX, "y": CY, "width": 2 * R, "height": 2 * R, "fill": "none",
             "stroke": {"color": "#ffffff26", "width": 3}, "enter": {"preset": "drawOn", "duration": 1.2}},
            {"shape": "circle", "x": CX, "y": CY, "width": 2 * R, "height": 2 * R, "fill": "none",
             "rotation": day0 - 90,
             "stroke": {"color": GOLD, "width": 10, "cap": "round"}, "trimEnd": day_frac,
             "animate": [{"from": {"trimEnd": 0}, "at": 0.5, "duration": 1.4, "ease": "inOutCubic"}]},
            # elapsed part of the day, brighter
            {"shape": "circle", "x": CX, "y": CY, "width": 2 * R, "height": 2 * R, "fill": "none",
             "rotation": day0 - 90, "stroke": {"color": SUN, "width": 18, "cap": "round"},
             "trimEnd": f"=clamp(({h} - {SUNRISE}) / 24, 0.001, {day_frac})", "glow": {"radius": 18, "color": GOLD},
             "opacity": "=smoothstep(1.8, 2.4, t)"},
            {"shape": "circle", "x": CX, "y": CY, "width": 2 * R - 80, "height": 2 * R - 80, "fill": "#ffffff0d",
             "stroke": {"color": "#ffffff1a", "width": 1.5}, "enter": {"preset": "scaleIn", "delay": 0.3}},
            *[l for hr, lab, col in ((6.67, "6:40", GOLD), (9, "9:00", GOLD), (12.5, "12:30", GOLD),
                                    (17.87, "17:52", CORAL)) for l in tick(hr, lab, col)],
            # the sun
            {"shape": "circle", "x": sun_x, "y": sun_y, "width": 120, "height": 120,
             "fill": "radial-gradient(#FFE6A866, #FFC15E00)", "opacity": "=smoothstep(1.6, 2.2, t)"},
            {"shape": "circle", "x": sun_x, "y": sun_y, "width": 44, "height": 44, "fill": SUN,
             "glow": {"radius": 30, "intensity": 1.2, "color": GOLD}, "opacity": "=smoothstep(1.6, 2.2, t)"},
            *[dict(l, start=0.6, enter={"preset": "fadeIn", "duration": 0.6}) for l in digits()],
            {"text": "SUNSET 19:58", "x": CX, "y": CY + 70, "size": 22, "letterSpacing": "0.14em",
             "font": "Inter", "weight": 600, "color": "#ffffffa8", "enter": {"preset": "fadeIn", "delay": 0.9}},
        ],
    }
    # fix ":" blink expression
    for l in dial["layers"]:
        if l.get("text") == ":":
            l["opacity"] = "=0.6 + 0.4 * (floor(t * 2) - 2 * floor(t))"

    ui = {
        "type": "group", "start": 3.4, "end": 30.4, "exit": {"preset": "fadeOut", "duration": 0.5},
        "layers": [
            {"text": "Sundial", "x": 140, "y": 150, "anchor": "left", "font": "Inter Display", "size": 54,
             "weight": 800, "color": CREAM, "enter": {"preset": "fadeIn", "delay": 0.2}},
            {"shape": "circle", "x": 112, "y": 150, "width": 26, "height": 26, "fill": SUN,
             "glow": {"radius": 14, "color": GOLD}, "enter": {"preset": "popIn", "delay": 0.2}},
            {"text": "TODAY · THURSDAY", "x": 140, "y": 222, "anchor": "left", "size": 22, "weight": 600,
             "letterSpacing": "0.14em", "color": "#ffffff99", "enter": {"preset": "fadeIn", "delay": 0.5}},
            card(0, "06:40", "Run by the river", "sunrise +28m", 6.67),
            card(1, "09:00", "Deep work", "bright, 3h", 9.0),
            card(2, "12:30", "Lunch outside", "sun high", 12.5),
            card(3, "17:52", "Golden hour walk", "45 min", 17.87),
            # toast as golden hour approaches
            {"type": "group", "x": 140, "y": 860, "width": 560, "height": 96, "anchor": "topLeft",
             "start": 8.7, "end": 9.9,
             "enter": {"preset": "slideUp", "duration": 0.6}, "exit": {"preset": "fadeOut", "duration": 0.4},
             "layers": [
                 {"shape": "rect", "width": 560, "height": 96, "radius": 48, "fill": CREAM},
                 {"shape": "circle", "x": 52, "y": 48, "width": 30, "height": 30, "fill": CORAL},
                 {"text": "Golden hour starts in 15 min", "x": 86, "y": 48, "anchor": "left", "size": 30,
                  "weight": 650, "color": INK},
             ]},
        ],
    }

    title = {
        "type": "group", "start": 0, "end": 4.2,
        "layers": [
            # the big sun rising behind the word, then handing off to the dial's sun
            {"shape": "circle", "x": 960, "y": "=860 - 120 * smoothstep(0, 4, t)", "width": 900, "height": 900,
             "fill": "radial-gradient(#FFD98A55, #FFB45F00)", "exit": {"preset": "fadeOut", "duration": 0.8}},
            {"shape": "circle", "x": 960, "y": "=880 - 140 * smoothstep(0, 4, t)", "width": 260, "height": 260,
             "fill": "radial-gradient(#FFF1C8, #FFC15E)", "glow": {"radius": 60, "intensity": 1.3, "color": GOLD},
             "exit": {"preset": "scaleOut", "duration": 0.7}},
            {"shape": "rect", "x": 960, "y": 900, "width": W, "height": 360, "anchor": "top",
             "fill": "linear-gradient(180deg, #2A1E22, #120E14)"},
            {"text": "Sundial", "x": 960, "y": 430, "font": "Inter Display", "size": 230, "weight": 800,
             "letterSpacing": "-0.02em", "color": CREAM, "exit": {"preset": "blurOut", "duration": 0.6}},
            {"text": "Plan your day by the light.", "x": 960, "y": 580, "size": 44, "weight": 500,
             "color": "#FFF4E3cc", "exit": {"preset": "fadeOut", "duration": 0.5}},
        ],
    }

    phone = {
        "type": "group", "start": 30.0, "end": 45.4, "enter": {"preset": "fadeIn", "duration": 0.6},
        "exit": {"preset": "fadeOut", "duration": 0.5},
        "layers": [
            {"component": "model", "x": 1260, "y": 560, "width": 900, "height": 900,
             "props": {"shape": "rounded-box", "size": [0.5, 1.0, 0.06], "radius": 0.07, "color": "#E8B66A",
                       "metallic": 0.85, "roughness": 0.25, "spin": 22, "tilt": 12, "float": 0.03}},
            {"text": "Your day,\nlit right.", "x": 160, "y": 470, "anchor": "left", "align": "left",
             "font": "Inter Display", "size": 120, "weight": 800, "lineHeight": 1.0, "color": CREAM,
             "kinetic": "mask by line up"},
            {"text": "Sundial plans around sunrise, sunset and golden hour.", "x": 160, "y": 660,
             "anchor": "left", "align": "left", "size": 34, "color": "#ffffffb0", "width": 640,
             "enter": {"preset": "fadeUp", "delay": 0.6}},
        ],
    }

    months = ["J", "F", "M", "A", "M", "J", "J", "A", "S", "O", "N", "D"]
    daylight = [8.0, 9.8, 11.9, 13.9, 15.6, 16.6, 16.1, 14.5, 12.5, 10.5, 8.6, 7.7]  # hours, London
    year = {
        "type": "group", "start": 45.0, "end": 53.4, "enter": {"preset": "fadeIn", "duration": 0.5},
        "exit": {"preset": "fadeOut", "duration": 0.5},
        "layers": [
            {"text": "Every season, its own light.", "x": 960, "y": 200, "font": "Inter Display", "size": 72,
             "weight": 800, "color": CREAM, "kinetic": "rise"},
            {"component": "area-chart", "x": 960, "y": 640, "width": 1400, "height": 640,
             "props": {"series": [daylight], "names": ["Hours of daylight"], "labels": months, "colors": [GOLD], "area": True, "smooth": True, "delay": 0.4}},
        ],
    }
    end = {
        "type": "group", "start": 53.0, "end": 60.0, "enter": {"preset": "fadeIn", "duration": 0.6},
        "layers": [
            {"shape": "circle", "x": 960, "y": 470, "width": 160, "height": 160,
             "fill": "radial-gradient(#FFF1C8, #FFC15E)", "glow": {"radius": 50, "color": GOLD},
             "enter": {"preset": "popIn"}},
            {"text": "Sundial", "x": 960, "y": 650, "font": "Inter Display", "size": 150, "weight": 800,
             "color": CREAM, "enter": {"preset": "fadeUp", "delay": 0.3}},
            {"text": "Coming this spring", "x": 960, "y": 760, "size": 40, "weight": 500, "color": "#ffffffb0",
             "enter": {"preset": "fadeUp", "delay": 0.6}},
        ],
    }

    spec = {
        "size": [W, H], "fps": 60, "duration": 60,
        "theme": {"colors": {"primary": GOLD, "accent": CORAL, "bg": INK, "text": CREAM, "muted": "#ffffffa0"},
                  "fonts": {"heading": "Inter Display", "body": "Inter", "mono": "JetBrains Mono"}},
        "scenes": [{
            "id": "sundial", "duration": 60,
            "camera": {"animate": [
                {"from": {"zoom": 1.0}, "to": {"zoom": 1.06}, "at": 8, "duration": 7, "ease": "inOutSine"},
                {"to": {"zoom": 1.0}, "at": 15, "duration": 1.2, "ease": "inOutCubic"}]},
            "layers": sky_layers + [title, ui, dial, phone, year, end] + [
                {"component": "grain", "props": {"amount": 0.035}}],
        }],
    }
    (HERE / "sundial.json").write_text(json.dumps(spec, indent=1))


if __name__ == "__main__":
    build()
