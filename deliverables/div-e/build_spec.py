#!/usr/bin/env python3
"""Generate arctic.json (Anikit Video spec) from NSIDC Sea Ice Index v4 September extent.

Data: NSIDC Sea Ice Index, Version 4 (G02135), N_09_extent_v4.0.csv
https://noaadata.apps.nsidc.org/NOAA/G02135/north/monthly/data/N_09_extent_v4.0.csv
"""
import json, os

HERE = os.path.dirname(os.path.abspath(__file__))
rows = [l.split(',') for l in open(os.path.join(HERE, 'N_09_extent_v4.0.csv')).read().strip().split('\n')[1:]]
years = [int(r[0]) for r in rows]
ext = [float(r[4]) for r in rows]
n = len(years)

# Least-squares trend
mx, me = sum(years) / n, sum(ext) / n
slope = sum((y - mx) * (e - me) for y, e in zip(years, ext)) / sum((y - mx) ** 2 for y in years)
fit = lambda y: me + slope * (y - mx)
base = [e for y, e in zip(years, ext) if 1981 <= y <= 2010]
pct_decade = slope * 10 / (sum(base) / len(base)) * 100
loss = fit(years[0]) - fit(years[-1])
print(f"trend {slope*10:.3f} Mkm2/decade ({pct_decade:.1f}%/decade vs 1981-2010), fitted loss {loss:.2f} Mkm2")

BG = "#04080E"
ICE = "#E9F1F6"
DIM = "#8FA3B3"
ACC = "#F0693C"
SERIF = "Instrument Serif"
SANS = "Inter"
SOURCE = "Source: NSIDC Sea Ice Index v4 (G02135) · September monthly average extent"

FLOES = r"""
uniform float cover;  // = 0.95
uniform float bright; // = 0.62
uniform float side;   // = 0.35
vec2 h2(vec2 p) { p = vec2(dot(p, vec2(127.1, 311.7)), dot(p, vec2(269.5, 183.3))); return fract(sin(p) * 43758.5453); }
void mainImage(out vec4 o, in vec2 f) {
  vec2 uv = f / iResolution.y * 6.5 + vec2(iTime * 0.025, iTime * 0.008);
  vec2 g = floor(uv), fr = fract(uv);
  float d1 = 8.0, d2 = 8.0; vec2 id = g; vec2 p1 = vec2(0.0);
  for (int y = -1; y <= 1; y++) for (int x = -1; x <= 1; x++) {
    vec2 c = vec2(float(x), float(y));
    vec2 r = h2(g + c);
    vec2 p = c + 0.5 + 0.38 * sin(iTime * 0.12 + 6.2831 * r) - fr;
    float d = dot(p, p);
    if (d < d1) { d2 = d1; d1 = d; id = g + c; p1 = p; } else if (d < d2) { d2 = d; }
  }
  float edge = sqrt(d2) - sqrt(d1);
  vec2 hh = h2(id + 17.0);
  float melt = clamp((hh.x - cover) * 6.0, 0.0, 1.0);
  float thr = 0.035 + melt * 0.55;
  float ice = smoothstep(thr, thr + 0.025, edge) * (1.0 - step(0.999, melt));
  float grain = fract(sin(dot(floor(f * 0.5), vec2(12.9898, 78.233))) * 43758.5453);
  vec3 water = vec3(0.012, 0.026, 0.045) + 0.012 * sin(uv.x * 3.0 + iTime * 0.4);
  vec3 icec = mix(vec3(0.50, 0.63, 0.74), vec3(0.86, 0.93, 0.98), hh.y) * (0.92 + 0.08 * grain);
  icec *= 0.85 + 0.15 * smoothstep(0.0, 0.25, edge);
  vec3 col = mix(water, icec * bright, ice);
  vec2 s = f / iResolution.xy;
  float fade = smoothstep(side - 0.12, side + 0.22, s.x);
  col = mix(vec3(0.016, 0.031, 0.055), col, fade);
  col *= 1.0 - 0.35 * pow(length(s - vec2(0.6, 0.5)), 2.0);
  o = vec4(col, 1.0);
}
"""

def T(text, **kw):
    d = {"text": text, "font": SANS, "color": ICE}
    d.update(kw)
    return d

scenes = []

# ---- S1: title over a field of floes -------------------------------------------------
scenes.append({"duration": 4.5, "layers": [
    {"shader": FLOES, "uniforms": {"cover": 0.97, "bright": 0.6, "side": 0.5},
     "animate": [{"from": {"cover": 0.97}, "to": {"cover": 0.82}, "duration": 4.5, "ease": "linear"}]},
    T("ARCTIC SEA ICE · 1979–2026", size=30, weight=600, letterSpacing="0.18em", color=DIM,
      x="7%", y="34%", anchor="left", enter="rise"),
    {"text": "Every September, the ice\nreaches its yearly low.", "font": SERIF, "size": "8.4vh", "color": ICE,
     "lineHeight": 1.05, "x": "7%", "y": "50%", "anchor": "left", "enter": "mask by line up",
     "animate": [{"from": {"opacity": 1}, "at": 0}]},
    T("That low is shrinking.", font=SERIF, size="5.2vh", color=ACC, italic=True,
      x="7%", y="69%", anchor="left", start=2.2, enter="rise"),
]})

# ---- S2: area-proportional discs, 1979 vs 2012 ----------------------------------------
R79 = 330.0
R12 = R79 * (ext[years.index(2012)] / ext[0]) ** 0.5
CX, CY = 1300, 540
scenes.append({"duration": 6.3, "transition": {"type": "fadeBlack", "duration": 0.6}, "layers": [
    {"shape": "rect", "width": 1920, "height": 1080, "fill": "#07101A"},
    {"shape": "circle", "width": 2 * R79, "height": 2 * R79, "x": CX, "y": CY,
     "fill": "#22364A",
     "animate": [{"from": {"width": 2 * R79 * 0.6, "height": 2 * R79 * 0.6, "opacity": 0}, "to": {"width": 2 * R79, "height": 2 * R79, "opacity": 1}, "duration": 0.9, "ease": "outCubic"}]},
    {"shape": "circle", "width": 2 * R79, "height": 2 * R79, "x": CX, "y": CY, "fill": "none",
     "stroke": "#C9D7E2", "strokeWidth": 3, "opacity": 0.85,
     "animate": [{"from": {"trimEnd": 0}, "to": {"trimEnd": 1}, "at": 1.6, "duration": 1.0, "ease": "inOutCubic"}]},
    {"shape": "circle", "width": 2 * R12, "height": 2 * R12, "x": CX, "y": CY,
     "fill": "#E4EDF3", "start": 0.3,
     "animate": [{"from": {"width": 2 * R79 * 0.6, "height": 2 * R79 * 0.6, "opacity": 0}, "to": {"width": 2 * R79, "height": 2 * R79, "opacity": 1}, "duration": 0.9, "ease": "outCubic"},
                 {"to": {"width": 2 * R12, "height": 2 * R12}, "at": 1.9, "duration": 1.3, "ease": "inOutCubic"}]},
    T("1979", font=SERIF, size="9vh", x="7%", y="26%", anchor="left", enter="rise"),
    T("7.05 million km²", size=44, weight=500, color=DIM, x="7%", y="35%", anchor="left", enter="rise"),
    T("2012 · record low", font=SERIF, size="9vh", color=ACC, x="7%", y="52%", anchor="left", start=2.0, enter="rise"),
    T("3.57 million km²", size=44, weight=500, color=ICE, x="7%", y="61%", anchor="left", start=2.2, enter="rise"),
    T("About half the ice of 1979.", font=SERIF, size="5vh", italic=True, color=ICE, x="7%", y="75%",
      anchor="left", start=3.0, enter="rise"),
    T("Disc areas to scale", size=26, color=DIM, x=CX, y=CY + R79 + 40, start=1.0, enter="rise"),
    T(SOURCE, size=24, color="#8A9CAC", x="7%", y="90.5%", anchor="left"),
]})

# ---- S3: the record, year by year ------------------------------------------------------
L, Rx, BASE, PX = 230, 1700, 900, 82.0   # px per million km²
step = (Rx - L) / n
bw = step * 0.62
t0, dt = 0.9, 0.15
layers = [{"shape": "rect", "width": 1920, "height": 1080, "fill": f"linear-gradient(180deg, #08121D, {BG})"}]
for v in range(0, 9, 2):
    y = BASE - v * PX
    layers.append({"shape": "line", "x": L - 20, "y": y, "anchor": "left", "width": Rx - L + 40, "height": 0,
                   "stroke": "#1E2D3B" if v else "#5C7080", "strokeWidth": 1.5 if v else 2.5,
                   "animate": [{"from": {"trimEnd": 0}, "to": {"trimEnd": 1}, "duration": 0.7, "ease": "outCubic"}]})
    layers.append(T(str(v), size=26, color=DIM, x=L - 34, y=y, anchor="right", enter="fade"))
layers.append(T("million km²", size=24, color=DIM, x=L - 20, y=BASE - 8 * PX - 44, anchor="left", enter="fade"))
for yr in (1980, 1990, 2000, 2010, 2020):
    i = years.index(yr)
    layers.append(T(str(yr), size=26, color=DIM, x=L + step * (i + 0.5), y=BASE + 36, enter="fade"))
for i, (yr, e) in enumerate(zip(years, ext)):
    at = t0 + i * dt
    col = ACC if yr == 2012 else ("#D7E3EC" if e >= 6 else ("#B7CAD8" if e >= 5 else "#93AEC2"))
    layers.append({"shape": "rect", "x": L + step * i + (step - bw) / 2, "y": BASE, "anchor": "bottomLeft",
                   "width": bw, "height": e * PX, "fill": col, "radius": 2,
                   "animate": [{"from": {"height": 0}, "at": at, "duration": 0.45, "ease": "outCubic"}]})
    # running readout
    end = t0 + (i + 1) * dt if i < n - 1 else None
    rd = {"start": at, **({"end": end} if end else {})}
    layers.append(T(str(yr), font=SERIF, size="11vh", x=1700, y=128, anchor="right", **rd))
    layers.append(T(f"{e:.2f} million km²", size=36, weight=500, color=DIM, x=1700, y=204, anchor="right", **rd))
for yr, label, lvl, side in ((2007, "2007 · 4.27", 6.7, "right"), (2012, "2012 · 3.57 · record low", 7.2, "left")):
    i = years.index(yr)
    xc = L + step * (i + 0.5)
    yt = BASE - ext[i] * PX - 8
    yl = BASE - lvl * PX
    st = t0 + i * dt + 0.3
    layers.append({"shape": "rect", "x": xc - 1, "y": yl - 14, "anchor": "topLeft", "width": 2, "height": yt - yl + 14,
                   "fill": ACC if yr == 2012 else DIM, "start": st,
                   "animate": [{"from": {"height": 0}, "duration": 0.4, "ease": "outCubic"}]})
    layers.append(T(label, size=28, weight=600, color=ACC if yr == 2012 else ICE,
                    x=xc + (12 if side == "left" else -12), y=yl + 4, anchor=side, start=st, enter="rise"))
# trend line
tl = t0 + n * dt + 0.3
x1, x2 = L + step * 0.5, L + step * (n - 0.5)
y1, y2 = BASE - fit(years[0]) * PX, BASE - fit(years[-1]) * PX
layers.append({"shape": "path", "d": f"M0 0 L {x2 - x1:.1f} {y2 - y1:.1f}", "x": x1, "y": min(y1, y2), "anchor": "topLeft",
               "stroke": ACC, "strokeWidth": 5, "fill": "none",
               "animate": [{"from": {"trimEnd": 0}, "to": {"trimEnd": 1}, "at": tl, "duration": 1.3, "ease": "inOutCubic"}]})
layers.append(T("Trend: −12% per decade", font=SERIF, size="6.6vh", color=ACC, x=L + 10, y=165, anchor="left",
                start=tl + 1.0, enter="rise"))
layers.append(T(f"−{abs(slope)*10:.2f} million km² per decade, relative to the 1981–2010 average",
                size=28, color=ICE, x=L + 12, y=235, anchor="left", start=tl + 1.3, enter="rise"))
layers.append(T("Arctic sea ice extent, September", size=30, weight=600, letterSpacing="0.06em", color=DIM,
                x=L + 10, y=112, anchor="left", end=tl + 0.9, enter="fade"))
layers.append(T(SOURCE, size=24, color="#8A9CAC", x="7%", y="96%", anchor="left"))
scenes.append({"duration": 12.9, "transition": {"type": "fadeBlack", "duration": 0.5}, "layers": layers})

# ---- S4: the takeaway, floes thinning ---------------------------------------------------
scenes.append({"duration": 5.0, "transition": {"type": "fadeBlack", "duration": 0.6}, "layers": [
    {"shader": FLOES, "uniforms": {"cover": 0.8, "bright": 0.55, "side": 0.72},
     "animate": [{"from": {"cover": 0.8}, "to": {"cover": 0.4}, "duration": 5.0, "ease": "inOutSine"}]},
    T("Since 1979, the long-term trend is down", font=SERIF, size="6vh", x="7%", y="34%", anchor="left", enter="mask by line up"),
    T("3.5 million", font=SERIF, size="15vh", color=ICE, x="7%", y="50%", anchor="left", start=0.5,
      enter="mask by line up"),
    T("square kilometres of September ice: more than the area of India.", font=SERIF, size="5vh", italic=True, color=DIM,
      x="7%", y="65%", anchor="left", start=1.4, enter="rise"),
]})

# ---- S5: source card -------------------------------------------------------------------
scenes.append({"duration": 3.5, "transition": {"type": "fadeBlack", "duration": 0.5}, "layers": [
    {"shape": "rect", "width": 1920, "height": 1080, "fill": BG},
    {"shape": "line", "x": "7%", "y": "40%", "anchor": "left", "width": 120, "height": 0, "stroke": ACC, "strokeWidth": 4},
    T("Source", size=28, weight=600, letterSpacing="0.16em", uppercase=True, color=DIM, x="7%", y="46%", anchor="left"),
    T("National Snow and Ice Data Center (NSIDC), Sea Ice Index, Version 4 (G02135)", size=38, color=ICE,
      x="7%", y="53%", anchor="left"),
    T("Arctic September monthly average sea ice extent, 1979–2026 · nsidc.org/data/g02135", size=30, color=DIM,
      x="7%", y="59.5%", anchor="left"),
    T("Trend: least-squares fit, 1979–2026. India: 3.29 million km².", size=26, color="#8A9CAC",
      x="7%", y="65%", anchor="left"),
]})

spec = {
    "size": "16:9",
    "fps": 30,
    "background": BG,
    "intent": {"pace": "steady", "genre": "explainer"},
    "music": {"style": "piano", "bpm": 66, "key": "Dm", "mood": "dark sombre", "instrumentation": "orchestral",
              "end": "ring"},
    "post": {"grain": 0.02, "vignette": 0.25},
    "scenes": scenes,
}
json.dump(spec, open(os.path.join(HERE, 'arctic.json'), 'w'), indent=1, ensure_ascii=False)
print("total", sum(s["duration"] for s in scenes), "s")
