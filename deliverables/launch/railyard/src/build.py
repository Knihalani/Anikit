"""Builds spec.json for the Railyard launch film (carbon-copy device).
Only does geometry the spec keys can't express compactly: the pen silhouette at
two scales and the shared timing expressions. Run: python3 src/build.py"""
import json, math, os

BAR = 240 / 128            # 1.875 s
FELT, BOND, CANARY, INK, CARBON = "#2A2926", "#F3EEE2", "#F2D86A", "#1B1A18", "#22309C"
HAND = "Homemade Apple"

def pen(k, x, y):
    """Fountain pen seen from above, leaning to the upper right; (x, y) = nib tip (expressions)."""
    a = math.radians(28)
    def rot(p): return (p[0]*math.cos(a) - p[1]*math.sin(a), p[0]*math.sin(a) + p[1]*math.cos(a))
    body = [(0,0),(46,0),(46,600),(36,700),(28,770),(23,800),(18,770),(10,700),(0,600)]
    hi = [(9,14),(17,14),(17,585),(9,585)]
    nib = [(10,700),(36,700),(28,770),(23,800),(18,770)]
    cap = [(0,0),(46,0),(46,250),(0,250)]
    ref = [rot(p) for p in body]
    mx, my = min(p[0] for p in ref), min(p[1] for p in ref)
    def path(pts, fill, extra=None):
        r = [((q[0]-mx)*k, (q[1]-my)*k) for q in map(rot, pts)]
        ox, oy = min(p[0] for p in r), min(p[1] for p in r)
        d = "M" + " L".join(f"{px-ox:.1f} {py-oy:.1f}" for px, py in r) + " Z"
        L = {"shape": "path", "d": d, "fill": fill, "x": round(ox, 1), "y": round(oy, 1), "anchor": "topLeft"}
        if extra: L.update(extra)
        return L
    tip = rot((23, 800)); tx, ty = (tip[0]-mx)*k, (tip[1]-my)*k
    return {"id": "pen", "width": round(400*k), "height": round(720*k), "anchor": "topLeft",
            "x": f"=({x}) - {tx:.1f}", "y": f"=({y}) - {ty:.1f}",
            "layers": [
                path(body, "#1E1C1A", {"shadow": {"x": round(46*k), "y": round(60*k), "blur": round(28*k), "color": "#00000066"}}),
                path(cap, "#2B2724"),
                path(hi, "#8A8177", {"opacity": 0.55}),
                path(nib, "#C9AE72")]}

LAMP = {"shape": "rect", "width": "140%", "height": "140%", "x": "50%", "y": "50%", "blend": "multiply",
        "fill": "linear-gradient(130deg, #FFFAF0, #F0E4D2 25%, #A89683 62%, #4A3D33)"}

def lamp(drift=False):
    L = dict(LAMP)
    if drift: L["x"] = "=960 + 60*sin(t*0.9)"
    return L

def written(text, size, x, y, color, t0, dur, mx0, mx1, mid, rot=0, opacity=1):
    """Hand-written word revealed left→right by a matte whose edge is at mx0 + (mx1-mx0)*s(t)."""
    return [
        {"id": mid, "shape": "rect", "x": mx0, "y": y - 40, "anchor": "left", "height": size*2.2,
         "width": f"=({mx1 - mx0})*smoothstep({t0},{t0 + dur},t)", "fill": "#ffffff", "rotation": rot},
        {"text": text, "font": HAND, "size": size, "color": color, "x": x, "y": y, "anchor": "left",
         "matte": mid, "rotation": rot, "opacity": opacity}]

def edge(t0, dur, mx0, mx1): return f"{mx0} + {mx1 - mx0}*smoothstep({t0},{t0 + dur},t)"

scenes = []

# 1 · Hook: pen already writing "Sent." on white bond (macro)
s1w = written("Sent.", 360, 420, 560, INK, -0.25, 1.45, 230, 1480, "m1")
scenes.append({"duration": BAR, "background": BOND,
  "camera": {"animate": [{"from": {"zoom": 1.16, "x": -40}, "to": {"zoom": 1.04, "x": 0}, "duration": BAR, "ease": "linear"}]},
  "layers": [{"component": "paper", "props": {"stock": "paper", "color": BOND, "tooth": 0.65, "seed": 3}}] + s1w + [
    pen(1.0, edge(-0.25, 1.45, 230, 1480) + " + 40*smoothstep(1.3,1.8,t)", "610 + 34*sin(t*21) - 90*smoothstep(1.3,1.8,t)"),
    lamp()]})

# 3 · Distance: two sheets apart on the felt; "Paid." lands on both at once
P0, PD = 0.15, 1.2
def sheet3(x, y, rot, stock_color, texture, ink, mid, extra=()):
    w = written("Paid.", 210, 90, 450, ink, P0, PD, 50, 660, mid)
    return {"width": 720, "height": 940, "x": x, "y": y, "rotation": rot,
            "layers": [{"component": "cutout", "props": dict({"color": stock_color, "enter": "none", "shadow": 0.75, "wobble": 0.05}, **({"texture": texture} if texture else {})),
                        "x": 360, "y": 470, "width": 720, "height": 940}] + w + list(extra)}
def uline(color):
    return {"component": "brush", "x": 90, "y": 560, "anchor": "topLeft", "width": 520, "height": 40,
            "props": {"d": "M0 28 C 170 4, 350 36, 515 10", "color": color, "size": 12, "texture": "ink", "pressure": "taper", "draw": 0.45, "delay": 1.65, "ease": "linear"}}
pe = edge(P0, PD, 50, 660)
scenes.append({"duration": 1.5 * BAR, "background": FELT,
  "camera": {"animate": [{"from": {"x": -40, "zoom": 1.07}, "to": {"x": 30, "zoom": 1.0}, "duration": 1.5 * BAR, "ease": "linear"}]},
  "layers": [
    {"component": "paper", "props": {"stock": "canvas", "color": "#2C2B28", "tooth": 0.75, "vignette": 0.15, "seed": 7}, "width": "130%", "height": "130%"},
    sheet3(470, 560, -2, BOND, "paper", INK, "ma", [uline(INK)]),
    sheet3(1470, 520, 3, CANARY, None, CARBON, "mb", [uline(CARBON)]),
    # left sheet local (0,0) = (110, 90); nib follows the matte edge, then the underline
    pen(0.6, f"110 + ({pe})*(1-smoothstep(1.35,1.65,t)) + (90 + 515*clamp((t-1.65)/0.45,0,1))*smoothstep(1.35,1.65,t) + 60*smoothstep(2.15,2.6,t)",
             f"90 + 450 - 0.035*({pe}) + 16*sin(t*23)*(1-smoothstep(1.3,1.5,t)) + 110*smoothstep(1.35,1.65,t) - 70*smoothstep(2.15,2.6,t) - 40*(1-smoothstep(0,0.15,t))"),
    lamp()]})

# 4 · Scale: one original among 44 copies; tick, pull out, underline — all at once
TICK = {"d": "M5 40 L35 70 L95 8", "x": 35, "y": 60, "width": 100, "height": 80, "delay": 0.30, "draw": 0.35}
LINE = {"d": "M2 18 C 40 6, 80 24, 118 10", "x": 25, "y": 160, "width": 120, "height": 30, "delay": 2.5, "draw": 0.4}
def marks(color):
    out = []
    for m in (TICK, LINE):
        out.append({"component": "brush", "x": m["x"], "y": m["y"], "anchor": "topLeft", "width": m["width"], "height": m["height"],
                    "props": {"d": m["d"], "color": color, "size": 9, "texture": "ink", "pressure": "taper", "draw": m["draw"], "delay": m["delay"], "ease": "linear"}})
    return out
slip = {"width": 170, "height": 220, "layers": [
    {"component": "cutout", "props": {"color": CANARY, "enter": "none", "shadow": 0.55, "wobble": 0.1, "seed": "$seed"}, "x": 85, "y": 110, "width": 170, "height": 220}] + marks(CARBON)}
orig = {"width": 170, "height": 220, "x": 960, "y": 540, "layers": [
    {"component": "cutout", "props": {"color": BOND, "texture": "paper", "enter": "none", "shadow": 0.7, "wobble": 0.05}, "x": 85, "y": 110, "width": 170, "height": 220}] + marks(INK)}
s1, s2, m, s3 = "clamp((t-0.3)/0.116,0,1)", "clamp((t-0.416)/0.234,0,1)", "smoothstep(0.9,2.3,t)", "clamp((t-2.5)/0.4,0,1)"
px = f"915 + 30*{s1} + 60*{s2} - 103*{m} + 116*{s3} + 40*smoothstep(2.95,3.3,t)"
py = (f"530 + 30*{s1} - 62*{s2} + 110*{m} - 8*{s3} - 26*(1-smoothstep(0.05,0.3,t))"
      f" - 26*(smoothstep(0.65,0.85,t) - smoothstep(2.25,2.5,t)) - 40*smoothstep(2.95,3.3,t)")
scenes.append({"duration": 2 * BAR, "background": FELT,
  "defs_local": None,
  "camera": {"animate": [{"from": {"zoom": 2.3}, "to": {"zoom": 1.0}, "at": 0.8, "duration": 1.5, "ease": "inOutCubic"},
                         {"to": {"zoom": 0.96}, "at": 2.3, "duration": 1.45, "ease": "linear"}]},
  "layers": [
    {"component": "paper", "props": {"stock": "canvas", "color": "#2C2B28", "tooth": 0.75, "vignette": 0.1, "seed": 9}, "width": "130%", "height": "130%"},
    {"use": "slip", "repeat": 45, "with": {"seed": "=$i+11"},
     "x": "=140 + 205*($i - 9*floor($i/9))", "y": "=30 + 255*floor($i/9)",
     "rotation": "=4*sin($i*12.9898)"},
    orig,
    pen(0.36, px, py),
    lamp()]})

# 5 · Hard stop: macro, tilted; the pen signs the name in silence
R0, RD = -0.65, 2.3
s5w = written("Railyard", 270, 300, 600, INK, R0, RD, 160, 1640, "m5")
scenes.append({"duration": 1.5 * BAR, "background": BOND,
  "camera": {"rotation": -5, "animate": [{"from": {"zoom": 1.12, "rotation": -5}, "to": {"zoom": 1.2, "rotation": -5}, "duration": 1.5 * BAR, "ease": "linear"}]},
  "layers": [{"component": "paper", "props": {"stock": "paper", "color": BOND, "tooth": 0.65, "seed": 4}, "width": "130%", "height": "130%"}] + s5w + [
    pen(0.9, edge(R0, RD, 160, 1640) + " + 60*smoothstep(1.85,2.5,t)", "650 + 40*sin(t*17) - 160*smoothstep(1.85,2.5,t)"),
    lamp()]})

# 6 · Payoff: tear the original away on the hit; the copy already bears the name
COPY = {"width": 1240, "height": 820}
def sheet6(color, texture, ink, extra):
    L = [{"component": "cutout", "props": dict({"color": color, "enter": "none", "shadow": 0.75, "wobble": 0.05}, **({"texture": texture} if texture else {})),
          "x": 620, "y": 410, "width": 1240, "height": 820},
         {"text": "Railyard", "font": HAND, "size": 190, "color": ink, "x": 600, "y": 330, "rotation": -4}]
    return L + extra
underline = {"component": "brush", "x": 260, "y": 470, "anchor": "topLeft", "width": 820, "height": 60,
             "props": {"d": "M0 40 C 260 6, 540 58, 815 18", "color": CARBON, "size": 10, "texture": "ink", "pressure": "taper"}}
cta_l = written("One API call. Instant settlement.", 72, 110, 650, CARBON, 0.7, 1.0, 80, 1180, "mc")
cta_l[1]["font"] = "Caveat"; cta_l[1]["weight"] = 600
cta = {"layers": cta_l, "width": 1240, "height": 820, "x": 620, "y": 410}
scenes.append({"duration": 2 * BAR, "background": FELT,
  "camera": {"animate": [{"from": {"zoom": 1.0, "x": 0}, "to": {"zoom": 1.12, "x": 60}, "duration": 2 * BAR, "ease": "inOutSine"}]},
  "layers": [
    {"component": "paper", "props": {"stock": "canvas", "color": "#2C2B28", "tooth": 0.75, "vignette": 0.15, "seed": 7}, "width": "130%", "height": "130%"},
    {"component": "cutout", "props": {"color": BOND, "texture": "paper", "torn": 0.85, "enter": "none", "shadow": 0.9}, "x": "9%", "y": "50%", "width": 470, "height": 1250, "rotation": 7},
    dict(COPY, x=1130, y=560, rotation=3, layers=sheet6(CANARY, None, CARBON, [underline, cta])),
    dict(COPY, x=1130, y=560, rotation=3, sfx={"in": "page"},
         animate=[{"to": {"x": -1100, "y": 420, "rotation": -14}, "at": 0.02, "duration": 0.6, "ease": "inOutQuad"}],
         layers=sheet6(BOND, "paper", INK, [{"component": "brush", "x": 260, "y": 470, "anchor": "topLeft", "width": 820, "height": 60,
             "props": {"d": "M0 40 C 260 6, 540 58, 815 18", "color": INK, "size": 10, "texture": "ink", "pressure": "taper"}}])),
    lamp(drift=True)]})

for s in scenes: s.pop("defs_local", None)

t5 = 3 * BAR + 1.5 * BAR      # start of shot 5
spec = {
  "size": "16:9", "fps": 30,
  "intent": {"pace": "medium"},
  "defs": {"slip": slip},
  "music": {"bpm": 128, "key": "F", "style": "acoustic", "mood": "bright warm", "length": "8 bars",
            "instruments": {"lead": "marimba"},
            "sections": [{"name": "intro", "bars": 2, "energy": 0.35}, {"name": "build", "bars": 4, "energy": 0.7},
                         {"name": "payoff", "bars": 2, "energy": 0.85, "stop": 2}],
            "hits": ["@bar7"], "end": "ring", "followCuts": False, "seed": 4},
  "sound": {"palette": "organic", "ambience": "room", "density": 0.35, "cues": [
     {"sfx": "paper", "at": 0.0, "duration": 1.3, "gain": -4},
          {"sfx": "paper", "at": BAR + 0.15, "duration": 1.2, "x": -0.5, "gain": -5},
     {"sfx": "paper", "at": BAR + 1.65, "duration": 0.45, "x": -0.5, "gain": -6},
     {"sfx": "paper", "at": 2.5 * BAR + 0.3, "duration": 0.35, "gain": -6},
     {"sfx": "paper", "at": 2.5 * BAR + 2.5, "duration": 0.4, "gain": -5},
     {"sfx": "paper", "at": t5, "duration": 1.75, "gain": -2},
     {"sfx": "page", "at": 6 * BAR + 0.05, "gain": 0, "intensity": 1},
     {"sfx": "thump", "at": 6 * BAR, "gain": -4},
     {"sfx": "paper", "at": 6 * BAR + 0.75, "duration": 1.0, "gain": -9}]},
  "scenes": scenes}
here = os.path.dirname(os.path.abspath(__file__))
json.dump(spec, open(os.path.join(here, "..", "spec.json"), "w"), indent=1)
print("total", sum(s["duration"] for s in scenes))
