#!/usr/bin/env python3
"""Tern launch film: 20 s, 16:9, 1920x1080.  Run: python3 make.py  ->  tern.json

Idea (see concept.txt): you hear the bird before you see it. The film is drawn by sound: every syllable you
hear is the same syllable you see on Tern's live spectrogram (one event list drives both the synthesised
birdsong and the GLSL spectrogram, so picture and sound are the same data). The motif is the song trace:
it is named on the phone, becomes a dot on the live migration map, and at the end a tern's two-note call
draws the tern itself, which becomes the logo.
"""
import json, math

BPM = 96
BEAT = 60 / BPM          # 0.625 s
BAR = 4 * BEAT           # 2.5 s

INK = "#061114"          # pre-dawn ink
INK2 = "#0B1C20"
TEAL = "#2E6E6A"
SAGE = "#9FB9B1"         # secondary text (contrast vs INK ~ 9:1)
PAPER = "#EEF3EF"        # primary text
AMBER = "#F4A63A"        # the song (accent)
DISPLAY, SANS, MONO = "Inter Display", "Inter", "JetBrains Mono"

# ----------------------------------------------------------------------------------------------
# 1. The birds. One list drives both the audio (synth tracks) and the spectrogram (GLSL).
#    (t0 s, dur s, midi, P semis, T s): pitch = midi + P*exp(-(t-t0)/T)  (the synth's pitch envelope)
# ----------------------------------------------------------------------------------------------
GLIDES = {  # instrument -> (P, T)
    "w": (0, 1), "dn": (9, 0.12), "dn5": (5, 0.08), "up": (-6, 0.07), "upL": (-9, 0.10), "dnL": (11, 0.18), "dn9": (9, 0.10),
}
EV = []      # (t0, dur, midi, inst)
def note(t0, dur, midi, inst="w"):
    EV.append((round(t0, 4), round(dur, 4), midi + 12, inst))

# Wood Thrush: "bud-bud  ee-oh-LAY  trrrrill" (flute-like, 1.5-5 kHz). Two phrases.
def thrush(t, hi=0):
    note(t + 0.00, 0.05, 81); note(t + 0.12, 0.05, 81)                      # bud bud
    note(t + 0.30, 0.17, 86 + hi, "up"); note(t + 0.50, 0.15, 90 + hi)       # ee  oh
    note(t + 0.68, 0.24, 93 + hi, "dn5")                                    # LAY
    for k in range(7):                                                      # the trill
        note(t + 0.96 + k * 0.055, 0.035, (98, 102)[k % 2] + hi)
thrush(0.05)
thrush(2.62, 2)

# American Robin: "cheerily cheer-up cheerio" - caroled two-part syllables, 2-3.3 kHz.
def robin(t):
    for k, (m, g) in enumerate([(98, 0.0), (95, 0.29), (99, 0.58), (96, 0.87), (97, 1.16)]):
        note(t + g, 0.11, m, "up"); note(t + g + 0.12, 0.1, m - 2, "dn5")
robin(4.55)

# Northern Cardinal: "cheer cheer cheer  birdy-birdy" - long down-slurs then quick ones.
def cardinal(t):
    for k in range(3):
        note(t + k * 0.30, 0.24, 92, "dnL")
    for k in range(4):
        note(t + 0.95 + k * 0.13, 0.09, 96, "dn")
cardinal(6.35)

# Song Sparrow: three clear notes, a buzz, a jumble.
def sparrow(t):
    for k in range(3):
        note(t + k * 0.16, 0.08, 94)
    for k in range(9):
        note(t + 0.52 + k * 0.03, 0.022, 103 + (k % 2))
    for k, m in enumerate([99, 96, 101, 93, 97]):
        note(t + 0.85 + k * 0.08, 0.06, m, ("dn", "up")[k % 2])
sparrow(8.05)

# The tern at the end: "kee-arr" - an up-slur straight into a down-slur. On the spectrogram it is a wingbeat: ^.
TERN_T = 15.35
note(TERN_T, 0.24, 97, "upL")
note(TERN_T + 0.24, 0.24, 88, "dn9")

def hz(m):
    return 440 * 2 ** ((m - 69) / 12)

# ----------------------------------------------------------------------------------------------
# 2. Spectrogram shader (composition coordinates: 1920x1080, 0-10 kHz, scrolling left; "now" = NOW).
# ----------------------------------------------------------------------------------------------
NOW, SPEED = 1700.0, 520.0          # px, px/s
Y0, KHZ = 60.0, 106.0                # bottom of 0 kHz (in shader y-up px), px per kHz
syl = []
for (t0, d, m, inst) in EV:
    P, T = GLIDES[inst]
    syl.append(f"  E += syl(tau, fk, {t0:.4f}, {d:.4f}, {hz(m)/1000:.5f}, {P:.1f}, {T:.3f});")
SHADER = """
float h21(vec2 p) { p = fract(p * vec2(123.34, 456.21)); p += dot(p, p + 45.32); return fract(p.x * p.y); }
float vnoise(vec2 p) { vec2 i = floor(p); vec2 f = fract(p); f = f*f*(3.0-2.0*f);
  return mix(mix(h21(i), h21(i+vec2(1.0,0.0)), f.x), mix(h21(i+vec2(0.0,1.0)), h21(i+vec2(1.0,1.0)), f.x), f.y); }
float syl(float tau, float fk, float t0, float d, float f0, float P, float T) {
  float u = tau - t0;
  if (u < 0.0 || u > d) { return 0.0; }
  float f = f0 * exp2(P * exp(-u / T) / 12.0);
  float a = smoothstep(0.0, 0.008, u) * smoothstep(0.0, 0.012, d - u);
  float e = 1.25 * exp(-pow((fk - f) / 0.07, 2.0)) + 0.3 * exp(-pow((fk - 2.0 * f) / 0.1, 2.0));
  return 1.35 * a * e;
}
vec3 ramp(float e) {
  vec3 c0 = vec3(0.020, 0.058, 0.070);
  vec3 c1 = vec3(0.040, 0.190, 0.200);
  vec3 c2 = vec3(0.180, 0.430, 0.400);
  vec3 c3 = vec3(0.960, 0.640, 0.220);
  vec3 c4 = vec3(1.000, 0.960, 0.860);
  vec3 c = mix(c0, c1, smoothstep(0.0, 0.18, e));
  c = mix(c, c2, smoothstep(0.18, 0.42, e));
  c = mix(c, c3, smoothstep(0.42, 0.75, e));
  c = mix(c, c4, smoothstep(0.78, 1.05, e));
  return c;
}
void mainImage(out vec4 o, in vec2 fc0) {
  vec2 fc = fc0 * vec2(1920.0, 1080.0) / iResolution.xy;
  float now = %NOW%;
  float tnow = iTime;
  float tau = tnow - (now - fc.x) / %SPEED%;
  float fk = (fc.y - %Y0%) / %KHZ%;
  float col = floor((fc.x + tnow * %SPEED%) / 3.0);
  float row = floor(fc.y / 3.0);
  // noise floor: wind and distant traffic low down, faint speckle everywhere, a chorus that builds toward dawn
  float wind = exp(-max(fk, 0.0) / 0.55) * (0.20 + 0.12 * vnoise(vec2(col * 0.05, fk * 2.0)));
  float speck = 0.07 * pow(h21(vec2(col, row)), 6.0) + 0.05 * vnoise(vec2(col * 0.2, row * 0.15));
  float chorus = 0.0;
  float dawn = smoothstep(0.0, 14.0, tau);
  for (int k = 0; k < 3; k++) {
    float fk2 = float(k);
    float per = 0.31 + 0.07 * fk2;
    float cell = floor((tau + fk2 * 1.7) / per);
    float u = fract((tau + fk2 * 1.7) / per);
    float hh = h21(vec2(cell, fk2 * 7.0));
    float on = step(0.35, hh) * step(u, 0.38);
    float f = 2.2 + 5.0 * h21(vec2(cell * 1.3, fk2)) + (hh - 0.6) * 4.0 * u;
    chorus += on * smoothstep(0.0, 0.05, u) * smoothstep(0.38, 0.30, u) * exp(-pow((fk - f) / 0.08, 2.0)) * (0.16 + 0.22 * dawn);
  }
  float E = wind + speck + chorus;
%SYL%
  float future = step(now, fc.x);
  E = mix(E, 0.0, future);
  vec3 c = ramp(E);
  // kHz grid
  float g = abs(fract(fk + 0.5) - 0.5) * %KHZ%;
  c += vec3(0.05, 0.11, 0.11) * (1.0 - smoothstep(0.0, 1.2, g)) * step(0.5, fk) * (1.0 - future * 0.5);
  // the now line
  float nl = abs(fc.x - now);
  c += vec3(0.95, 0.75, 0.45) * (exp(-nl / 1.2) * 0.6 + exp(-nl / 30.0) * 0.06);
  c *= 1.0 - 0.35 * future;
  o = vec4(c, 1.0);
}
"""
SHADER = (SHADER.replace("%NOW%", f"{NOW:.1f}").replace("%SPEED%", f"{SPEED:.1f}").replace("%Y0%", f"{Y0:.1f}")
          .replace("%KHZ%", f"{KHZ:.1f}").replace("%SYL%", "\n".join(syl)))

def comp_y(khz):   # composition pixel y (y-down) for a frequency
    return 1080 - (Y0 + khz * KHZ)

def trace_x(tau, tnow):
    return NOW - (tnow - tau) * SPEED

# Species ID moments (global seconds) and the phrase each one boxes on the spectrogram.
IDS = [
    # name, latin, id time, phrase start, phrase end, kHz lo, kHz hi, time heard
    ("Wood Thrush", "Hylocichla mustelina", 3.75, 2.62, 3.95, 1.6, 6.4, "05:12:04"),
    ("American Robin", "Turdus migratorius", 5.95, 4.55, 5.95, 2.7, 5.2, "05:12:06"),
    ("Northern Cardinal", "Cardinalis cardinalis", 7.5, 6.35, 7.80, 3.0, 7.2, "05:12:08"),
    ("Song Sparrow", "Melospiza melodia", 8.95, 8.05, 9.30, 3.4, 8.2, "05:12:09"),
]

spec_layers = [
    {"shader": SHADER, "width": 1920, "height": 1080},
    # frequency labels (UI of the app; big enough to read when the spectrogram is full-frame)
    *[{"text": f"{k} kHz", "font": MONO, "size": 30, "color": "#7FA39C", "x": 40, "y": comp_y(k), "anchor": "left"}
      for k in (1, 2, 3, 4, 5, 6, 7, 8)],
]
# Boxes around each identified phrase, scrolling with the spectrogram (inside the composition).
for (name, latin, tid, a, b, lo, hi, _) in IDS:
    w = (b - a) * SPEED + 40
    hgt = (hi - lo) * KHZ + 30
    x_expr = f"={NOW:.1f} - (t + {tid - 0.05:.3f} - {a:.3f})*{SPEED:.1f} - 20"
    spec_layers.append({"layers": [
        {"shape": "rect", "width": w, "height": hgt, "fill": "none", "stroke": AMBER, "strokeWidth": 5, "radius": 10,
         "x": 0, "y": 0, "anchor": "topLeft"},
        {"shape": "rect", "width": len(name) * 27 + 40, "height": 62, "fill": AMBER, "radius": 8, "x": 0, "y": -74,
         "anchor": "topLeft"},
        {"text": name, "font": SANS, "weight": 600, "size": 40, "color": INK, "x": 20, "y": -43, "anchor": "left"}],
        "x": x_expr, "y": comp_y(hi) - 15, "anchor": "topLeft", "start": tid - 0.05, "end": tid + 3.2,
        "animate": [{"from": {"opacity": 0}, "to": {"opacity": 1}, "duration": 0.15}]})
# the tern's call gets no box: it becomes the bird.

spec_layers.append({"text": "LIVE", "font": MONO, "weight": 600, "size": 30, "color": AMBER, "x": NOW + 24, "y": 46, "anchor": "left",
                    "opacity": "=0.6 + 0.4*sin(t*5)"})
COMPOSITIONS = {"spec": {"size": [1920, 1080], "duration": 20.5, "background": INK, "layers": spec_layers},
                "specbg": {"size": [1920, 1080], "duration": 20.5, "background": INK, "layers": spec_layers[:1]}}

# ----------------------------------------------------------------------------------------------
# 3. The phone app (442 x 932 screen) - Tern's listening screen.
# ----------------------------------------------------------------------------------------------
SW, SH = 442, 932
BAND_Y, BAND_H = 132, SW * 1080 / 1920      # spectrogram band in the screen
app = [
    {"shape": "rect", "width": SW, "height": SH, "fill": "#08171A", "x": 0, "y": 0, "anchor": "topLeft"},
    {"text": "5:12", "font": SANS, "weight": 600, "size": 19, "color": PAPER, "x": 34, "y": 30, "anchor": "left"},
    {"shape": "rect", "width": 26, "height": 12, "radius": 3, "fill": "none", "stroke": PAPER, "strokeWidth": 1.5,
     "x": SW - 60, "y": 24, "anchor": "topLeft"},
    {"shape": "rect", "width": 18, "height": 8, "radius": 2, "fill": PAPER, "x": SW - 57, "y": 26, "anchor": "topLeft"},
    {"text": "Tern", "font": DISPLAY, "weight": 700, "size": 34, "color": PAPER, "x": 26, "y": 84, "anchor": "left"},
    {"shape": "circle", "width": 12, "height": 12, "fill": "#FF6A4A", "x": SW - 136, "y": 84,
     "opacity": "=0.55 + 0.45*sin(t*6)"},
    {"text": "Listening", "font": SANS, "weight": 500, "size": 20, "color": SAGE, "x": SW - 122, "y": 84, "anchor": "left"},
    {"composition": "spec", "width": SW, "height": BAND_H, "x": 0, "y": BAND_Y, "anchor": "topLeft"},
    {"text": "NOW SINGING", "font": MONO, "size": 15, "color": "#6F948D", "letterSpacing": "0.12em",
     "x": 26, "y": BAND_Y + BAND_H + 40, "anchor": "left"},
    {"text": "HEARD THIS MORNING", "font": MONO, "size": 15, "color": "#6F948D", "letterSpacing": "0.12em",
     "x": 26, "y": 600, "anchor": "left"},
    {"shape": "line", "width": SW - 52, "height": 0, "stroke": "#1D3A3D", "strokeWidth": 1.5, "x": 26, "y": 620, "anchor": "topLeft"},
    # bottom bar: Listen | Map | Life list
    {"shape": "rect", "width": SW, "height": 92, "fill": "#0B1F23", "x": 0, "y": SH - 92, "anchor": "topLeft"},
    {"text": "Listen", "font": SANS, "weight": 600, "size": 17, "color": AMBER, "x": 74, "y": SH - 46},
    {"text": "Map", "font": SANS, "weight": 500, "size": 17, "color": SAGE, "x": SW / 2, "y": SH - 46},
    {"text": "Life list", "font": SANS, "weight": 500, "size": 17, "color": SAGE, "x": SW - 74, "y": SH - 46},
    {"shape": "rect", "width": 120, "height": 5, "radius": 3, "fill": "#3A5A5C", "x": SW / 2, "y": SH - 12},
]
# "Now singing" card per species (replaced at each ID) and the growing "heard this morning" list.
for k, (name, latin, tid, a, b, lo, hi, when) in enumerate(IDS):
    end = IDS[k + 1][2] if k + 1 < len(IDS) else 20
    y = BAND_Y + BAND_H + 62
    app.append({"layers": [
        {"text": name, "font": DISPLAY, "weight": 600, "size": 40, "color": PAPER, "x": 0, "y": 0, "anchor": "topLeft"},
        {"text": f"*{latin}*", "font": SANS, "size": 19, "color": SAGE, "x": 0, "y": 54, "anchor": "topLeft"},
        {"shape": "rect", "width": 8, "height": 8, "radius": 4, "fill": AMBER, "x": 0, "y": 98, "anchor": "topLeft"},
        {"text": f"Heard {when} · matched while it sang", "font": SANS, "size": 16, "color": SAGE, "x": 16, "y": 92,
         "anchor": "topLeft"}],
        "x": 26, "y": y, "anchor": "topLeft", "start": tid, "end": end,
        "animate": [{"from": {"opacity": 0, "y": y + 18}, "duration": 0.3, "ease": "outCubic"}]})
for k, (name, latin, tid, *_r) in enumerate(IDS):
    when = IDS[k][7]
    # newest at top: entry k sits at row (number of later IDs at time t)
    rows = [(tid, 0)] + [(IDS[j][2], j - k) for j in range(k + 1, len(IDS))]
    anim = [{"from": {"opacity": 0}, "to": {"opacity": 1}, "duration": 0.3}]
    for (tt, r) in rows[1:]:
        anim.append({"to": {"y": 636 + r * 58}, "at": round(tt - tid, 3), "duration": 0.35, "ease": "outCubic"})
    app.append({"layers": [
        {"shape": "circle", "width": 10, "height": 10, "fill": AMBER, "x": 5, "y": 22},
        {"text": name, "font": SANS, "weight": 500, "size": 22, "color": PAPER, "x": 24, "y": 22, "anchor": "left"},
        {"text": when[:5], "font": MONO, "size": 17, "color": SAGE, "x": SW - 52, "y": 22, "anchor": "right"}],
        "x": 26, "y": 636, "anchor": "topLeft", "start": tid + 0.15, "animate": anim})
COMPOSITIONS["app"] = {"size": [SW, SH], "duration": 10.5, "background": "#08171A", "layers": app}

# ----------------------------------------------------------------------------------------------
# 4. Scene A (0-10 s): full-frame spectrogram -> pull back: it is Tern's screen -> names, live.
# ----------------------------------------------------------------------------------------------
PX, PY = 560, 540                 # phone centre
PW, PH = SW + 28, SH + 28
SX0, SY0 = PX - SW / 2, PY - SH / 2           # screen top-left in frame
BAND_CX, BAND_CY = PX, SY0 + BAND_Y + BAND_H / 2
ZOOM = 1920 / SW                  # band fills the frame

phone = [
    {"shape": "rect", "width": PW + 6, "height": PH + 6, "radius": 74, "fill": "#24302F", "x": PX, "y": PY},
    {"shape": "rect", "width": PW, "height": PH, "radius": 70, "fill": "#05090A", "x": PX, "y": PY},
    {"composition": "app", "width": SW, "height": SH, "x": SX0, "y": SY0, "anchor": "topLeft", "radius": 56},
    {"shape": "rect", "width": 118, "height": 32, "radius": 16, "fill": "#000000", "x": PX, "y": SY0 + 28},
]
ZOOM_AT, ZOOM_DUR = 2.5, 1.0
# camera: x/y are the view centre's offset from the frame centre (in frame px).
camA = {"animate": [
    {"from": {"zoom": ZOOM, "x": BAND_CX - 960, "y": BAND_CY - 540},
     "to": {"zoom": 1, "x": 0, "y": 0}, "at": ZOOM_AT, "duration": ZOOM_DUR, "ease": "inOutQuart"}]}

right = []
RX = 990
right.append({"text": "Tern hears it.", "font": DISPLAY, "weight": 600, "size": 54, "color": SAGE,
              "x": RX, "y": 150, "anchor": "left", "start": ZOOM_AT + 0.5,
              "animate": [{"from": {"opacity": 0, "x": RX + 30}, "duration": 0.4, "ease": "outCubic"}]})
right.append({"text": "And names it while it sings.", "font": DISPLAY, "weight": 600, "size": 54, "color": PAPER,
              "x": RX, "y": 218, "anchor": "left", "start": IDS[0][2] - 0.05,
              "animate": [{"from": {"opacity": 0, "x": RX + 30}, "duration": 0.4, "ease": "outCubic"}]})
right.append({"shape": "line", "width": 820, "height": 0, "stroke": "#1E3B3E", "strokeWidth": 2, "x": RX, "y": 290,
              "anchor": "topLeft", "start": ZOOM_AT + 0.6,
              "animate": [{"from": {"trimEnd": 0}, "to": {"trimEnd": 1}, "duration": 0.6, "ease": "outCubic"}]})
# The species log: newest name enters big at the top, the others step down and shrink.
SLOT0 = 330
for k, (name, latin, tid, *_r) in enumerate(IDS):
    when = IDS[k][7]
    later = [IDS[j][2] for j in range(k + 1, len(IDS))]
    anim = [{"from": {"opacity": 0, "y": SLOT0 + 40}, "to": {"opacity": 1, "y": SLOT0}, "duration": 0.35, "ease": "outExpo"}]
    for r, tt in enumerate(later, start=1):
        y = SLOT0 + 250 + (r - 1) * 92
        anim.append({"to": {"y": y, "scale": 0.5}, "at": round(tt - tid, 3), "duration": 0.45, "ease": "outExpo"})
    big = {"layers": [
        {"text": name, "font": DISPLAY, "weight": 600, "size": 110, "color": PAPER, "letterSpacing": "-0.02em",
         "x": 0, "y": 0, "anchor": "topLeft"},
        {"text": f"*{latin}*", "font": SANS, "size": 52, "color": AMBER, "x": 4, "y": 140, "anchor": "topLeft",
         "opacity": f"={'1' if not later else f'1 - smoothstep({later[0]-tid:.2f}, {later[0]-tid+0.2:.2f}, t)'}"},
        {"text": when, "font": MONO, "size": 52, "color": SAGE, "x": 1850 - RX, "y": 140, "anchor": "topRight",
         "opacity": f"={'1' if not later else f'1 - smoothstep({later[0]-tid:.2f}, {later[0]-tid+0.2:.2f}, t)'}"}],
        "x": RX, "y": SLOT0, "anchor": "topLeft", "start": tid, "animate": anim}
    right.append(big)

# Behind everything: the same live spectrogram, huge and dim - the morning itself.
bgA = [{"shape": "rect", "width": 1920, "height": 1080, "fill": INK, "x": 960, "y": 540},
       {"composition": "specbg", "width": 480, "height": 270, "x": 960, "y": 540, "scale": 4.6, "opacity": 0.16,
        "blur": 2, "depth": 2}]
# A soft shadow under the phone (light from upper left: the dawn)
shadow = [{"shape": "rect", "width": PW, "height": PH, "radius": 70, "fill": "#000000", "x": PX + 26, "y": PY + 30,
           "opacity": 0.55, "blur": 40}]
glowA = [{"shape": "circle", "width": 900, "height": 900, "fill": "radial-gradient(#F4A63A33, #F4A63A00)",
          "x": 200, "y": 120, "opacity": "=0.5 + 0.5*smoothstep(0, 10, t)"}]

overlay = [{"composition": "spec", "width": 1920, "height": 1080, "x": BAND_CX, "y": BAND_CY, "scale": 1 / ZOOM}]
sceneA = {"duration": 10.0, "camera": camA, "layers": bgA + shadow + phone + overlay + right}

# ----------------------------------------------------------------------------------------------
# 5. Scene B (10-15 s): the map. Every recording is a dot; spring arrivals roll north.
# ----------------------------------------------------------------------------------------------
MCX, MCY, K, LON0, LAT0 = 640, 560, 18.6, -95.0, 33.0
COSL = math.cos(math.radians(LAT0))
def P(lon, lat):
    return (MCX + (lon - LON0) * K * COSL, MCY - (lat - LAT0) * K)

NA = [(-64,61),(-61,56),(-56.5,52.3),(-60,50.2),(-64.5,49.2),(-61.2,46),(-65.8,43.6),(-70,43.8),(-70.6,41.7),(-74,40.6),
      (-75.3,38.7),(-76.2,37.1),(-75.5,35.3),(-77.8,34),(-81,31.6),(-80.5,28.5),(-80.1,26.4),(-80.4,25.2),(-81.4,25.6),
      (-82.7,27.8),(-83,29.2),(-84.4,30),(-86.5,30.4),(-89.3,30.3),(-89.2,29.1),(-91.2,29.3),(-94,29.6),(-97.1,27.8),
      (-97.4,25.6),(-97.7,22.3),(-96.3,19.3),(-94.6,18.2),(-91.5,18.6),(-90.4,21),(-87.1,21.5),(-87.5,18.6),(-88.3,16),
      (-86,15.9),(-83.3,15),(-83.6,11.2),(-82,9.1),(-79.6,9.6),(-77.4,8.6),(-76,9.6),(-75.4,10.7),(-74.2,11.2),(-72.2,12),
      (-71.6,11),(-70.1,12),(-68.2,10.6),(-64.2,10.6),(-61.8,10.6),(-60.2,8.5),(-57.2,6),(-53,5.4),(-52,4),
      (-78,4),(-77.4,6.5),(-78.2,7.6),(-79.6,7.4),(-80.6,7.3),(-83,8.3),(-85.8,10.2),(-87.6,13),(-91.5,14),(-94.5,16),
      (-97,15.8),(-101,17.2),(-105.4,20),(-105.7,22.6),(-108.6,25.4),(-110.7,27.6),(-112.3,29.4),(-114.8,31.7),(-113.5,29),
      (-111.6,26.5),(-110.3,24.2),(-109.5,23.1),(-110.3,23.3),(-112.1,24.9),(-114.1,27.6),(-115.7,30.2),(-117.1,32.5),
      (-118.5,34),(-120.6,34.6),(-121.9,36.6),(-122.5,37.8),(-123.8,39.6),(-124.3,40.4),(-124.1,43),(-123.9,46.2),
      (-124.7,48.4),(-123.2,48.3),(-124,49.4),(-125.6,50.3),(-127.8,51.6),(-130.2,54.5),(-132.7,56.8),(-135.6,58.2),
      (-139.6,59.9),(-142,61)]
HUDSON = [(-95,61),(-94.5,59),(-92.6,57),(-88.5,56),(-85.2,55.2),(-82.3,55.2),(-82.2,53),(-80,51.4),(-79,52.4),
          (-78.7,54.5),(-77,58),(-77.8,61)]
ISLANDS = [
    [(-84.9,21.9),(-82,23.1),(-80,23.2),(-77.2,21.8),(-74.2,20.2),(-77.6,19.9),(-80,21.6),(-83,22.1)],
    [(-74.4,19.8),(-72.7,19.9),(-68.4,18.6),(-71,18),(-74.4,18.4)],
    [(-78.3,18.4),(-76.3,18.2),(-76.8,17.9),(-78.3,18.2)],
    [(-59.3,47.6),(-55.5,51.6),(-52.7,47.6),(-53.5,46.7),(-56,47.6)],
    [(-128,50.8),(-125,48.5),(-123.4,48.4),(-125.6,50)],
]
LAKES = [
    [(-92.1,46.7),(-89.5,48),(-86.5,48.7),(-84.6,46.5),(-87,46.5),(-90.3,46.6)],
    [(-87.8,42.3),(-87.5,45.8),(-85,46),(-86.3,44),(-86.2,42.2)],
    [(-84.7,45.9),(-81.5,45.6),(-80,44.6),(-81.7,43.3),(-82.5,43),(-83.5,44),(-82.8,45.4)],
    [(-83.4,41.8),(-79,42.8),(-78.9,42.9),(-81.5,42.3)],
    [(-79.8,43.3),(-76.3,43.5),(-76.2,44.2),(-79.2,43.6)],
]
def d_of(poly):
    pts = [P(*q) for q in poly]
    return "M " + " L ".join(f"{x:.1f} {y:.1f}" for x, y in pts) + " Z"
def inside(pt, poly):
    x, y = pt; c = False
    for i in range(len(poly)):
        x1, y1 = poly[i]; x2, y2 = poly[i - 1]
        if (y1 > y) != (y2 > y) and x < (x2 - x1) * (y - y1) / (y2 - y1) + x1:
            c = not c
    return c
def on_land(lon, lat):
    if inside((lon, lat), HUDSON) or any(inside((lon, lat), L) for L in LAKES):
        return False
    return inside((lon, lat), NA) or any(inside((lon, lat), I) for I in ISLANDS)

LAND, LAKE = "#11292D", INK
map_draw = [{"d": d_of(NA), "fill": LAND, "stroke": "#2D5856", "width": 1.6}]
map_draw += [{"d": d_of(I), "fill": LAND, "stroke": "#2D5856", "width": 1.6} for I in ISLANDS]
map_draw += [{"d": d_of(H), "fill": LAKE, "stroke": "#2D5856", "width": 1.4} for H in [HUDSON] + LAKES]
# graticule
grat = []
for lon in range(-130, -50, 10):
    (x1, y1), (x2, y2) = P(lon, 2), P(lon, 64)
    grat.append({"line": [x1, y1, x2, y2], "stroke": "#0F2226", "width": 1.2})
for lat in range(10, 61, 10):
    (x1, y1), (x2, y2) = P(-140, lat), P(-40, lat)
    grat.append({"line": [x1, y1, x2, y2], "stroke": "#0F2226", "width": 1.2})
# land texture: a fine dot grid (every place someone could be listening)
import random
rng = random.Random(7)
grid = []
step = 0.62
lat = 4.0
row = 0
while lat < 61:
    lon = -140 + (step / 2 if row % 2 else 0)
    while lon < -50:
        if on_land(lon, lat):
            x, y = P(lon, lat)
            grid.append({"circle": [round(x, 1), round(y, 1), 1.5], "fill": "#2A4D4E"})
        lon += step / COSL
    lat += step; row += 1

# Wood Thrush recordings: winter range (S Mexico - Panama), then spring arrivals rolling north through the east.
def thrush_range(lon, lat, phase):
    if phase == "winter":
        return 7.5 <= lat <= 20.5 and -97 <= lon <= -77 and on_land(lon, lat)
    # breeding: eastern North America, roughly south-west edge along -97 at 30N to -90 at 44N
    west = -97 + (lat - 29) * 0.5
    return 29 <= lat <= 47.5 and west <= lon <= -66 and on_land(lon, lat) and not (lat > 45.5 and lon < -84)
winter, spring = [], []
for _ in range(9000):
    lon, lat = rng.uniform(-100, -64), rng.uniform(7, 48)
    if thrush_range(lon, lat, "winter") and rng.random() < 0.55:
        winter.append((lon, lat))
    elif thrush_range(lon, lat, "spring"):
        spring.append((lon, lat))
spring = spring[:1500]

TB = 10.0                       # scene B start (global)
FRONT0, FRONT1 = 11.2, 14.4     # global seconds the arrival front takes to cross 29N -> 47N
def arrive(lat):
    return FRONT0 + (FRONT1 - FRONT0) * (lat - 29) / (47.5 - 29)

dots_layers = []
# winter dots: present at the start, thin out as the birds leave
dots_layers.append({"draw": [{"circle": [round(P(*q)[0], 1), round(P(*q)[1], 1), 2.6], "fill": AMBER} for q in winter],
                    "opacity": f"=0.95 - 0.6*smoothstep({FRONT0 - TB:.2f}, {FRONT1 - TB:.2f}, t)"})
NB = 14
for b in range(NB):
    la0 = 29 + (47.5 - 29) * b / NB
    la1 = 29 + (47.5 - 29) * (b + 1) / NB
    pts = [q for q in spring if la0 <= q[1] < la1]
    st = arrive(la0) - TB
    dots_layers.append({"draw": [{"circle": [round(P(*q)[0], 1), round(P(*q)[1], 1), 2.6], "fill": "#FFD08A"} for q in pts],
                        "start": round(st, 3),
                        "opacity": f"=smoothstep(0, 0.25, t) * (0.75 + 0.25*smoothstep(0.6, 0.0, t))"})
    # new arrivals flash bigger, then settle
    dots_layers.append({"draw": [{"circle": [round(P(*q)[0], 1), round(P(*q)[1], 1), 6], "fill": "#FFE7BF"} for q in pts[::3]],
                        "start": round(st, 3), "end": round(st + 0.6, 3),
                        "opacity": "=0.55*(1 - smoothstep(0, 0.6, t))"})

# Flyways: comets across the Gulf of Mexico and up the Mississippi and the Atlantic coast.
fly_src = [(-89.5, 17.5), (-88.5, 15.0), (-90.5, 16.0), (-87.5, 20.0), (-91.5, 18.0), (-85.5, 14.0), (-84.0, 11.0)]
fly_dst = [(-90, 36), (-86, 39), (-83, 40.5), (-79, 38), (-77, 41), (-73, 42.5), (-88, 42), (-84, 34), (-81, 35.5)]
comets = []
crng = random.Random(3)
for c in range(26):
    s = fly_src[c % len(fly_src)]; e = fly_dst[(c * 4) % len(fly_dst)]
    (x0, y0), (x1, y1) = P(*s), P(*e)
    mx, my = (x0 + x1) / 2 + crng.uniform(-60, 60), (y0 + y1) / 2 + crng.uniform(-40, 40)
    pts = [((1-u)**2*x0 + 2*(1-u)*u*mx + u*u*x1, (1-u)**2*y0 + 2*(1-u)*u*my + u*u*y1) for u in [k/30 for k in range(31)]]
    bx, by = min(p[0] for p in pts), min(p[1] for p in pts)
    d = "M " + " L ".join(f"{x-bx:.1f} {y-by:.1f}" for x, y in pts)
    period = crng.uniform(1.6, 2.4)
    off = crng.uniform(0, period)
    start = 1.0 + crng.uniform(0, 1.2)
    head = f"clamp(fract((t + {off:.2f}) / {period:.2f}) * 1.25, 0, 1)"
    comets.append({"shape": "path", "d": d, "stroke": "#FFC879", "strokeWidth": 2.6, "fill": "none",
                   "x": round(bx, 1), "y": round(by, 1), "anchor": "topLeft",
                   "start": round(start, 2), "trimEnd": f"={head}", "trimStart": f"=max({head} - 0.18, 0)",
                   "opacity": "=smoothstep(0, 0.4, t) * 0.9"})

# You: the four IDs from this morning, at the listener's spot (south-west Ohio).
YOU = P(-84.2, 39.6)
you = [{"shape": "circle", "width": 22, "height": 22, "fill": AMBER, "x": YOU[0], "y": YOU[1]},
       {"shape": "circle", "width": 22, "height": 22, "fill": "none", "stroke": AMBER, "strokeWidth": 2.5,
        "x": YOU[0], "y": YOU[1], "scale": "=1 + 3*fract(t*0.8)", "opacity": "=1 - fract(t*0.8)"}]
you_label = [{"layers": [
    {"text": "You, 05:12", "font": MONO, "size": 13, "color": PAPER, "x": 0, "y": -7, "anchor": "left"},
    {"text": "4 species", "font": MONO, "size": 13, "color": AMBER, "x": 0, "y": 11, "anchor": "left"}],
    "x": YOU[0] + 18, "y": YOU[1], "anchor": "left", "end": 1.5,
    "opacity": "=1 - smoothstep(0.9, 1.3, t)"}]

camB = {"animate": [{"from": {"zoom": 2.4, "x": YOU[0] - 960, "y": YOU[1] - 540},
                     "to": {"zoom": 1.0, "x": 0, "y": 0}, "at": 0.15, "duration": 1.35, "ease": "inOutQuart"}]}

# Right-hand panel (map UI), outside the map's ocean area
months = []
d0 = (31 + 28 + 31 + 1)                     # Apr 1 day-of-year
names = ["JAN", "FEB", "MAR", "APR", "MAY", "JUN"]
cum = [0, 31, 59, 90, 120, 151, 181]
def label(doy):
    for mi in range(6):
        if cum[mi] < doy <= cum[mi + 1]:
            return f"{names[mi]} {doy - cum[mi]:02d}"
steps = 16
for s in range(steps):
    doy = 91 + round(52 * s / (steps - 1))   # Apr 1 -> May 23
    t0 = (FRONT0 - TB) + (FRONT1 - FRONT0) * s / steps - 0.6
    t1 = (FRONT0 - TB) + (FRONT1 - FRONT0) * (s + 1) / steps - 0.6
    if s == 0:
        t0 = 0.0
    if s == steps - 1:
        t1 = 5.5
    months.append({"text": label(doy), "font": MONO, "weight": 500, "size": 104, "color": PAPER,
                   "x": 1340, "y": 470, "anchor": "left", "start": round(t0, 3), "end": round(t1, 3)})
panel_bg = [{"shape": "rect", "width": 600, "height": 1080, "fill": "linear-gradient(90deg, #06111400, #061114F2 30%)",
             "x": 1620, "y": 540}]
panel = [
    {"text": "Every recording", "font": DISPLAY, "weight": 600, "size": 66, "color": PAPER, "x": 1340, "y": 180,
     "anchor": "left", "start": 1.3, "animate": [{"from": {"opacity": 0, "x": 1370}, "duration": 0.4, "ease": "outCubic"}]},
    {"text": "joins a live", "font": DISPLAY, "weight": 600, "size": 66, "color": PAPER, "x": 1340, "y": 256,
     "anchor": "left", "start": 1.4, "animate": [{"from": {"opacity": 0, "x": 1370}, "duration": 0.4, "ease": "outCubic"}]},
    {"text": "migration map.", "font": DISPLAY, "weight": 600, "size": 66, "color": AMBER, "x": 1340, "y": 332,
     "anchor": "left", "start": 1.5, "animate": [{"from": {"opacity": 0, "x": 1370}, "duration": 0.4, "ease": "outCubic"}]},
    {"shape": "line", "width": 440, "height": 0, "stroke": "#2A4A4A", "strokeWidth": 2, "x": 1340, "y": 390, "anchor": "topLeft"},
    {"text": "Wood Thrush · spring arrivals", "font": SANS, "weight": 500, "size": 34, "color": SAGE, "x": 1340, "y": 580,
     "anchor": "left"},
    {"shape": "circle", "width": 16, "height": 16, "fill": "#FFD08A", "x": 1348, "y": 660},
    {"text": "a recording", "font": SANS, "size": 36, "color": SAGE, "x": 1370, "y": 660, "anchor": "left"},
    {"shape": "line", "width": 34, "height": 0, "stroke": "#FFC879", "strokeWidth": 3, "x": 1332, "y": 720, "anchor": "topLeft"},
    {"text": "moving north", "font": SANS, "size": 36, "color": SAGE, "x": 1380, "y": 720, "anchor": "left"},
]
sceneB = {"duration": 5.0, "camera": camB, "layers": [
    {"shape": "rect", "width": 1920, "height": 1080, "fill": INK, "x": 960, "y": 540},
    {"draw": grat}, {"draw": map_draw}, {"draw": grid},
    *dots_layers, *comets, *you, *you_label, *panel_bg, *panel, *months]}

# ----------------------------------------------------------------------------------------------
# 6. Scene C (15-20 s): a tern calls; its call draws the tern; the tern is the name.
# ----------------------------------------------------------------------------------------------
TC = 15.0
# the call's trace in frame coordinates at the moment it freezes (global FREEZE)
FREEZE = TERN_T + 0.62
def trace_pts(t0, d, m, inst, tnow):
    P_, T_ = GLIDES[inst]
    pts = []
    for k in range(25):
        u = d * k / 24
        f = hz(m) * 2 ** (P_ * math.exp(-u / T_) / 12) / 1000
        pts.append((trace_x(t0 + u, tnow), comp_y(f)))
    return pts
tern_ev = [e for e in EV if e[0] >= TERN_T - 0.01]
wing = trace_pts(*tern_ev[0], FREEZE) + trace_pts(*tern_ev[1], FREEZE)
wx = [p[0] for p in wing]; wy = [p[1] for p in wing]
WCX, WCY = (min(wx) + max(wx)) / 2, (min(wy) + max(wy)) / 2
wing_d = "M " + " L ".join(f"{x - WCX:.1f} {y - WCY:.1f}" for x, y in wing)
WW = max(wx) - min(wx)

# The mark: the call's arch, made into a tern - sharp wings, a forked tail, a beak. Local coords centred on the arch.
hw = WW / 2
mark_d = (f"M {-hw:.1f} {wing[0][1] - WCY + 6:.1f} "
          f"Q {-hw * 0.45:.1f} {-60:.1f} {0:.1f} {-6:.1f} "
          f"Q {hw * 0.45:.1f} {-60:.1f} {hw:.1f} {wing[-1][1] - WCY + 6:.1f} "
          f"Q {hw * 0.42:.1f} {-26:.1f} {16:.1f} {14:.1f} "
          f"L {44:.1f} {58:.1f} L {14:.1f} {30:.1f} L {0:.1f} {66:.1f} L {-8:.1f} {26:.1f} "
          f"L {-34:.1f} {22:.1f} L {-14:.1f} {10:.1f} "
          f"Q {-hw * 0.42:.1f} {-26:.1f} {-hw:.1f} {wing[0][1] - WCY + 6:.1f} Z")

LOGO_X, LOGO_Y, LOGO_S = 680, 470, 1.3
call_trace = {"layers": [{"shape": "path", "d": wing_d, "stroke": "#FFF3DC", "strokeWidth": 7, "fill": "none",
                          "x": 0, "y": 0, "glow": AMBER}],
              # follows the scrolling spectrogram until it freezes, then flies to the logo position
              "x": f"={WCX:.1f} - {SPEED:.1f}*(min(t, {FREEZE - TC:.3f}) - {FREEZE - TC:.3f})", "y": WCY,
              "start": round(FREEZE - TC - 0.02, 3), "end": 1.85,
              "animate": [{"to": {"x": LOGO_X, "y": LOGO_Y, "scale": LOGO_S}, "at": 0.25 + 0.02, "duration": 0.9,
                           "ease": "inOutQuart"},
                          {"to": {"opacity": 0}, "at": 1.15, "duration": 0.2}]}
mark = {"shape": "path", "d": mark_d, "fill": PAPER, "stroke": "none", "x": LOGO_X, "y": LOGO_Y, "scale": LOGO_S,
        "start": 1.0, "animate": [{"from": {"opacity": 0, "scale": LOGO_S * 0.96}, "to": {"opacity": 1, "scale": LOGO_S},
                                   "duration": 0.45, "ease": "outCubic"}]}
specC = {"composition": "spec", "width": 1920, "height": 1080, "x": 960, "y": 540, "trimStart": TC,
         "opacity": "=1 - 0.82*smoothstep(1.0, 1.9, t)"}
wordmark = {"text": "Tern", "font": DISPLAY, "weight": 700, "size": 300, "color": PAPER, "letterSpacing": "-0.035em",
            "x": 915, "y": 470, "anchor": "left", "start": 1.25,
            "animate": [{"from": {"opacity": 0, "x": 955}, "duration": 0.55, "ease": "outExpo"}]}
tag1 = {"text": "Names birds by their song, as they sing.", "font": DISPLAY, "weight": 500, "size": 56, "color": PAPER,
        "x": 470, "y": 720, "anchor": "left", "start": 1.9,
        "animate": [{"from": {"opacity": 0, "y": 740}, "duration": 0.45, "ease": "outCubic"}]}
tag2 = {"text": "Maps their migration from every recording.", "font": DISPLAY, "weight": 500, "size": 56, "color": AMBER,
        "x": 470, "y": 800, "anchor": "left", "start": 2.2,
        "animate": [{"from": {"opacity": 0, "y": 820}, "duration": 0.45, "ease": "outCubic"}]}
sceneC = {"duration": 5.0, "layers": [
    {"shape": "rect", "width": 1920, "height": 1080, "fill": INK, "x": 960, "y": 540},
    specC,
    call_trace, mark, wordmark, tag1, tag2]}

# ----------------------------------------------------------------------------------------------
# 7. Music: acoustic. Strings and harp in D; birds are the melody. Hit at the map (bar 5) and the call (bar 7).
# ----------------------------------------------------------------------------------------------
def bird_tracks():
    by = {}
    for (t0, d, m, inst) in EV:
        by.setdefault(inst, []).append((t0, d, m))
    tracks = []
    for inst, evs in by.items():
        evs.sort()
        cur = 0.0
        toks = ["t+24"]
        for (t0, d, m) in evs:
            gap = (t0 - cur) / BEAT
            if gap > 0.001:
                toks.append(f"r:{gap:.4f}")
            nm = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"][(m - 24) % 12] + str((m - 24) // 12 - 1)
            toks.append(f"{nm}:{d / BEAT:.4f}")
            cur = t0 + d
        tracks.append({"inst": "bird_" + inst, "notes": " ".join(toks), "gain": -13, "reverb": 0.35,
                       "pan": -0.15 if inst in ("w", "up") else 0.15})
    return tracks
define = {}
for inst, (P_, T_) in GLIDES.items():
    syn = {"osc": [{"wave": "sine", "level": 0.9}], "amp": [0.006, 0.02, 0.85, 0.02]}
    if P_:
        syn["pitch"] = [P_, T_]
    define["bird_" + inst] = {"synth": syn}

music = {
    "bpm": BPM, "key": "D", "seed": 4, "end": "ring", "hits": ["@bar5", "@bar7"],
    "define": define,
    "tracks": [
        {"inst": "slow-strings", "chords": "D Bm G A D Bm G D", "play": "pad", "gain": -11,
         "automate": {"volume": [[0, -14], ["@bar3", -6], ["@bar5", 0], ["@bar7", -2]]}},
        {"inst": "harp", "notes": "@2 l8 (B3 D4 F#4 B4 F#4 D4 B3 F#3) (G3 B3 D4 G4 D4 B3 G3 D3) (A3 C#4 E4 A4 E4 C#4 A3 E3) (D4 F#4 A4 D5 A4 F#4 D4 A3) (B3 D4 F#4 B4 F#4 D4 B3 F#3) (G3 B3 D4 G4 A3 C#4 E4 A4) D4:4",
         "gain": -6, "reverb": 0.4},
        {"inst": "nylon-guitar", "notes": "@5 l8 (D3 A3 D4 F#4 A3 D4 F#4 A4) (B2 F#3 B3 D4 F#3 B3 D4 F#4) (G2 D3 G3 B3 A2 E3 A3 C#4) D3:4",
         "gain": -9, "pan": -0.3},
        {"inst": "piano", "notes": "@5 [D3 A3 F#4]:4 [B2 F#3 D4]:4 [G2 D3 B3]:2 [A2 E3 C#4]:2 [D2 A2 F#3 D4]:4",
         "gain": -8},
        {"inst": "cello", "notes": "@3 G2:4 A2:4 D2:4 B1:4 G2:2 A2:2 D2:4", "gain": -9},
        {"inst": "drums:brush", "drums": {"kick": "x.......x.......", "snare": "....o.......o...", "hat": "..o...o...o...o."},
         "notes": None, "bars": 2, "at": "@bar5", "gain": -10},
        *bird_tracks(),
    ],
    "sfx": [
        {"sfx": "whoosh", "at": ZOOM_AT + 0.45, "gain": -16},
        *[{"sfx": "click", "at": i[2], "gain": -12} for i in IDS],
        {"sfx": "riser", "at": TB, "gain": -26},
        {"sfx": "chime", "at": TC + 1.25, "gain": -8},
    ],
}
music["tracks"] = [{k: v for k, v in tr.items() if v is not None} for tr in music["tracks"]]

spec = {
    "size": "16:9", "fps": 30, "background": INK, "transition": "cut",
    "intent": {"pace": "steady", "genre": "launch"},
    "compositions": COMPOSITIONS,
    "music": music,
    "post": {"bloom": 0.18, "bloomThreshold": 0.82, "vignette": 0.22, "grain": 0.0, "dither": True},
    "scenes": [sceneA, sceneB, sceneC],
}
with open("tern.json", "w") as f:
    json.dump(spec, f, indent=1)
print("events", len(EV), "spring dots", len(spring), "winter", len(winter), "grid", len(grid))
