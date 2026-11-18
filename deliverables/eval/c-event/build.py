#!/usr/bin/env python3
"""Builds spec.json for the Substation line-up film (run: python3 build.py)."""
import json, os

BPM = 128
BEAT = 60 / BPM            # 0.46875 s
BAR = 4 * BEAT             # 1.875 s
NAMES = ["Ancient Methods", "Paula Temple", "DVS1", "Héctor Oaks",
         "Rrose", "Phase Fatale", "Kia", "Ida Engberg"]
STRIKES = [2 * BAR + k * 2 * BEAT for k in range(8)]   # bars 3-6, beats 1 & 3
SURGE = 6 * BAR                                         # bar 7 = 11.25 s
Y0, DY = 575, 130
ROWS = [Y0 + k * DY for k in range(8)]
LOGO_S = BEAT              # sign strikes on beat 2
TICKET_S = SURGE + 0.55

TUBE = "#F3F9FF"
TUBE_GLOW = {"color": "#6FC3FF", "radius": 26, "intensity": 0.75}
DEAD = "#3B434B"
RED = "#FF3B1F"


def strike(s, seed, dur=0.3, surge=True):
    """Opacity expression: a fluorescent tube striking at gt = s."""
    e = (f"(gt < {s:.4f} ? 0 : (gt < {s + dur:.4f} ? "
         f"(rand(floor(gt*26) + {seed}) > 0.42 ? 1 : 0.12) : 1))")
    if surge:
        e += (f" * ((gt > {SURGE:.4f} && gt < {SURGE + 0.42:.4f}) ? "
              f"(rand(floor(gt*30) + {seed * 7 + 3}) > 0.5 ? 1 : 0.25) : 1)")
    return "=" + e


name_font = {"font": "Archivo", "axes": {"wdth": 62, "wght": 820}, "uppercase": True,
             "letterSpacing": "0.005em"}

layers = []

# board-marked concrete wall, lit by the tubes that are on
rows_glsl = ",".join(f"{y:.1f}" for y in ROWS)
str_glsl = ",".join(f"{s:.4f}" for s in STRIKES)
shader = """
float h(vec2 p){ return fract(sin(dot(p, vec2(127.1,311.7)))*43758.5453); }
float vn(vec2 p){ vec2 i=floor(p), f=fract(p); f=f*f*(3.0-2.0*f);
  return mix(mix(h(i),h(i+vec2(1,0)),f.x), mix(h(i+vec2(0,1)),h(i+vec2(1,1)),f.x), f.y); }
float fbm(vec2 p){ float a=0.5, s=0.0; for(int k=0;k<5;k++){ s+=a*vn(p); p*=2.03; a*=0.5; } return s; }
void mainImage(out vec4 o, in vec2 f){
  vec2 q = vec2(f.x, iResolution.y - f.y);           // top-down px
  float rows[8] = float[8](ROWS);
  float st[8]  = float[8](STR);
  // board-marked concrete: horizontal planks, grain along the board
  float plank = floor(q.y / 96.0);
  float seam = smoothstep(2.5, 0.0, abs(mod(q.y, 96.0) - 1.0));
  float grain = fbm(vec2(q.x*0.004 + plank*7.3, q.y*0.045));
  float blot = fbm(q*0.006 + 3.0);
  float pores = step(0.985, h(floor(q/2.0)));
  float tone = 0.55 + 0.35*grain + 0.4*(blot-0.5) + 0.06*h(vec2(plank, 1.0)) - 0.25*seam - 0.2*pores;
  // light from each lit tube row (soft band, left-heavy like the fixtures)
  float L = 0.0;
  for(int k=0;k<8;k++){
    float on = step(st[k], iTime);
    float fl = (iTime < st[k] + 0.3) ? step(0.42, h(vec2(floor(iTime*26.0) + float(k), 9.0))) : 1.0;
    float d = (q.y - rows[k]) / 150.0;
    L += on * fl * exp(-d*d) * (0.75 + 0.25*exp(-q.x/700.0));
  }
  // the sign at the top, and the red lamp at the end
  float sd = (q.y - 250.0)/190.0;
  L += step(SIGN, iTime) * 1.1 * exp(-sd*sd);
  float rd = (q.y - 1650.0)/170.0;
  float red = step(TICK, iTime) * exp(-rd*rd);
  // the surge: whole hall flashes
  float surge = exp(-max(iTime - SURGE, 0.0)*5.0) * step(SURGE, iTime);
  vec3 base = vec3(0.020, 0.022, 0.025);
  vec3 lit = vec3(0.55, 0.68, 0.80) * 0.13 * L * tone;
  vec3 rl = vec3(0.9, 0.18, 0.08) * 0.16 * red * tone;
  vec3 c = base * (0.6 + tone) + lit + rl + vec3(0.6,0.75,0.9)*0.35*surge*tone;
  o = vec4(c, 1.0);
}
""".replace("ROWS", rows_glsl).replace("STR", str_glsl) \
   .replace("SIGN", f"{LOGO_S + 0.35:.4f}").replace("TICK", f"{TICKET_S + 0.3:.4f}") \
   .replace("SURGE", f"{SURGE:.4f}")
layers.append({"id": "wall", "shader": shader})

# the sign
for dead in (True, False):
    l = {"text": "SUBSTATION", **name_font, "axes": {"wdth": 62, "wght": 900},
         "size": 184, "x": "50%", "y": 245, "anchor": "center", "letterSpacing": "0.02em"}
    if dead:
        l.update({"color": "#08090A", "stroke": {"color": DEAD, "width": 2.5}})
    else:
        l.update({"color": TUBE, "glow": {"color": "#78C8FF", "radius": 40, "intensity": 0.9},
                  "opacity": strike(LOGO_S, 11, dur=0.85)})
    layers.append(l)

layers.append({"text": "12—13 JUNE   TWO NIGHTS   THE OLD POWER STATION", "font": "JetBrains Mono",
               "size": 27, "weight": 500, "color": "#C9D3DC", "letterSpacing": "0.08em",
               "x": "50%", "y": 370, "anchor": "center",
               "opacity": strike(BAR, 5, dur=0.2, surge=False)})
layers.append({"shape": "rect", "width": 900, "height": 2, "fill": "#56616B", "x": "50%", "y": 430,
               "anchor": "center", "animate": [{"from": {"scaleX": 0}, "to": {"scaleX": 1},
                                                "at": BAR, "duration": 1.6, "ease": "inOutExpo"}]})

# the line-up: dead tubes, lit tubes, a breaker lamp and a strike flash per row
for k, (name, y, s) in enumerate(zip(NAMES, ROWS, STRIKES)):
    layers.append({"text": name, **name_font, "size": 118, "x": 165, "y": y, "anchor": "left",
                   "color": "#08090A", "stroke": {"color": DEAD, "width": 2}})
    layers.append({"text": name, **name_font, "size": 118, "x": 165, "y": y, "anchor": "left",
                   "color": TUBE, "glow": TUBE_GLOW,
                   "opacity": strike(s, 20 + k)[:] + (
                       f" * (gt > 12.6 && gt < 13.4 ? (rand(floor(gt*22)+{k}) > 0.35 ? 1 : 0.1) : 1)" if name == "Kia"
                       else f" * (0.94 + 0.06*rand(floor(gt*12)+{k*13}))")})
    # breaker lamp
    layers.append({"shape": "circle", "width": 22, "height": 22, "x": 105, "y": y, "anchor": "center",
                   "fill": "#1B1F23", "stroke": {"color": "#3A424A", "width": 2}})
    layers.append({"shape": "circle", "width": 22, "height": 22, "x": 105, "y": y, "anchor": "center",
                   "fill": "#FF4A26", "glow": {"color": "#FF3B1F", "radius": 18, "intensity": 0.9},
                   "opacity": f"=gt < {s:.4f} ? 0 : 1"})
    # arc flash across the row as it strikes
    layers.append({"shape": "rect", "width": 1080, "height": 3, "x": 0, "y": y, "anchor": "left",
                   "fill": "#FFFFFF", "glow": {"color": "#9AD6FF", "radius": 30, "intensity": 1},
                   "start": s - 0.02, "end": s + 0.3,
                   "animate": [{"from": {"scaleX": 0, "opacity": 1}, "to": {"scaleX": 1}, "duration": 0.12,
                                "ease": "outExpo"},
                               {"to": {"opacity": 0, "scaleY": 0.3}, "at": 0.1, "duration": 0.18}]})

# surge flash
layers.append({"shape": "rect", "width": 1080, "height": 1920, "x": "50%", "y": "50%", "anchor": "center",
               "fill": "#E9F5FF", "blend": "screen", "start": SURGE, "end": SURGE + 0.5,
               "animate": [{"from": {"opacity": 0.9}, "to": {"opacity": 0}, "duration": 0.45, "ease": "outCubic"}]})

# tickets: the red lamp
layers.append({"shape": "rect", "width": 900, "height": 2, "fill": "#56616B", "x": "50%", "y": 1560,
               "anchor": "center", "animate": [{"from": {"scaleX": 0}, "to": {"scaleX": 1},
                                                "at": SURGE + 0.1, "duration": 0.9, "ease": "inOutExpo"}]})
for dead in (True, False):
    l = {"text": "TICKETS FRIDAY", **name_font, "axes": {"wdth": 62, "wght": 900}, "size": 150,
         "x": "50%", "y": 1650, "anchor": "center", "letterSpacing": "0.015em"}
    if dead:
        l.update({"color": "#08090A", "stroke": {"color": "#3A2420", "width": 2}, "start": SURGE + 0.1,
                  "enter": {"preset": "fade", "duration": 0.3}})
    else:
        l.update({"color": "#FFD9CF", "glow": {"color": RED, "radius": 40, "intensity": 1.0},
                  "opacity": strike(TICKET_S, 77, dur=0.45, surge=False)})
    layers.append(l)

spec = {
    "size": "9:16",
    "fps": 30,
    "intent": {"pace": "fast"},
    "post": {"bloom": 0.3, "bloomThreshold": 0.6, "halation": 0.25, "grain": 0.045, "vignette": 0.35,
             "aberration": 0.08, "contrast": 1.08},
    "scenes": [{
        "duration": 8 * BAR,
        "background": "#050607",
        "camera": {"animate": [{"from": {"zoom": 1.04}, "to": {"zoom": 1.0}, "duration": SURGE,
                                "ease": "inOutSine"},
                               {"to": {"zoom": 1.025}, "at": SURGE, "duration": 0.08, "ease": "outExpo"},
                               {"to": {"zoom": 1.0}, "at": SURGE + 0.08, "duration": 1.6, "ease": "outExpo"}],
                   "shake": {"amount": 14, "at": SURGE, "duration": 0.5}},
        "layers": layers,
    }],
    "music": {
        "bpm": BPM, "key": "Gm", "length": "8 bars", "end": "ring",
        "master": {"lufs": -14, "ceiling": -1},
        "define": {
            "hum": {"synth": {"osc": [{"wave": "sine", "level": 0.9}, {"wave": "saw", "level": 0.08}],
                              "filter": {"type": "lp", "cutoff": 260, "res": 0.1},
                              "amp": [0.3, 0.1, 1, 1.5], "lfo": {"rate": 6.25, "trem": 0.15}}},
            "rumble": {"synth": {"osc": [{"wave": "saw", "voices": 2, "detune": 8, "level": 0.8}], "sub": 0.5,
                                 "filter": {"type": "ladder", "cutoff": 220, "res": 0.35, "drive": 2.2,
                                            "env": 1.5},
                                 "amp": [0.002, 0.18, 0.0, 0.08], "fenv": [0.001, 0.12, 0.0, 0.05],
                                 "mono": True}},
            "stab": {"synth": {"osc": [{"wave": "square", "voices": 2, "detune": 14, "level": 0.6},
                                       {"wave": "saw", "semis": 12, "level": 0.3}],
                               "filter": {"type": "lp4", "cutoff": 900, "res": 0.5, "env": 3},
                               "amp": [0.001, 0.22, 0.0, 0.25], "fenv": [0.001, 0.1, 0.0, 0.1]}},
        },
        "tracks": [
            {"inst": "hum", "notes": "o1 G1/1 | G1/1 | G1/1 | G1/1 | G1/1 | G1/1 | G1/1 | G1/1",
             "gain": -8, "automate": {"volume": [[0, 0.5], [24, 0.6], [24.1, 0.15], [32, 0.15]]}},
            {"inst": "drums:techno", "at": "@bar2",
             "drums": {"hat": "..x...x...x.x.xx", "rim": "........x......."}, "bars": 1, "gain": -4},
            {"inst": "drums:techno", "at": "@bar3",
             "drums": {"kick": "x...x...x...x...", "hat": "..x...x...x...x.", "rim": "...x......x....."},
             "bars": 2},
            {"inst": "drums:techno", "at": "@bar5",
             "drums": {"kick": "x...x...x...x...", "hat": "x.x.x.x.x.x.x.x.", "clap": "....x.......x...",
                       "ohh": "..x...x...x...x."}, "bars": 2},
            {"inst": "drums:techno", "at": "@bar7",
             "drums": {"kick": "X...x...x...x...", "hat": "xxxxxxxxxxxxxxxx", "clap": "....x.......x...",
                       "ohh": "..x...x...x...x.", "crash": "x..............."}, "bars": 1},
            {"inst": "drums:techno", "at": "@bar8",
             "drums": {"kick": "x...x...x.......", "hat": "x.x.x.x.x.......", "clap": "....x..........."},
             "bars": 1},
            {"inst": "rumble", "at": "@bar3",
             "notes": "o1 (r16 G1/16 G1/16 G1/16)x4 | (r16 G1/16 G1/16 G1/16)x4 | "
                      "(r16 G1/16 G1/16 Bb1/16)x4 | (r16 G1/16 G1/16 G1/16)x4 | "
                      "(r16 G1/16 G1/16 G1/16)x4 | (r16 F1/16 F1/16 F1/16)x2 (r16 G1/16 G1/16 G1/16)x2 | "
                      "(r16 G1/16 G1/16 G1/16)x3 r4",
             "gain": -3},
            {"inst": "stab", "at": "@bar3",
             "notes": "o4 [G3 Bb3 D4]/8! r8 r4 [G3 Bb3 D4]/8 r8 r4 | [F3 A3 C4]/8! r8 r4 [F3 A3 D4]/8 r8 r4 | "
                      "[G3 Bb3 D4]/8! r8 r4 [G3 Bb3 Eb4]/8 r8 r4 | [A3 C4 Eb4]/8! r8 r4 [Bb3 D4 F4]/8! r8 r4 | "
                      "[G3 Bb3 D4 G4]/8! r8 r4 r2 | r1 | [G3 Bb3 D4]/2~ r2",
             "gain": -7, "reverb": 0.35, "delay": 0.2},
        ],
        "sfx": [{"sfx": "riser", "at": "@bar7", "gain": -4},
                {"sfx": "impact", "at": "@bar7", "gain": -2},
                {"sfx": "sub-drop", "at": "@bar7", "gain": -3},
                {"sfx": "zap", "at": round(LOGO_S, 4), "gain": -6},
                {"sfx": "zap", "at": round(TICKET_S, 4), "gain": -6}] +
               [{"sfx": "zap", "at": round(s, 4), "gain": -12} for s in STRIKES],
    },
}

out = os.path.join(os.path.dirname(os.path.abspath(__file__)), "spec.json")
with open(out, "w") as fh:
    json.dump(spec, fh, indent=1, ensure_ascii=False)
print(out)
