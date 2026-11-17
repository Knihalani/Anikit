#!/usr/bin/env python3
"""Recreates the design grammar of NASA SVS "Global Temperature Anomalies from 1880 to 2023"
(vertical version, public domain) for a different subject: soil-moisture anomaly, 1950-2024.
The DATA HERE IS ILLUSTRATIVE (procedural noise with a drying trend), not a measurement.
Grammar kept: two orthographic globes stacked on black, one continuous 30 s shot, a diverging
colour field that steps year by year with a slow trend, thin dark linework over the field,
soft limb shading, a slim vertical legend with tiny units at the left, and a hairline timeline
with ticks whose playhead carries the year. Writes soil.json."""
import json
DUR, Y0, Y1 = 30.0, 1950, 2024
GLSL = r"""
float h(vec3 p){ p = fract(p*0.3183099 + 0.1); p *= 17.0; return fract(p.x*p.y*p.z*(p.x+p.y+p.z)); }
float vn(vec3 x){ vec3 i = floor(x); vec3 f = fract(x); f = f*f*(3.0-2.0*f);
  return mix(mix(mix(h(i),h(i+vec3(1,0,0)),f.x), mix(h(i+vec3(0,1,0)),h(i+vec3(1,1,0)),f.x),f.y),
             mix(mix(h(i+vec3(0,0,1)),h(i+vec3(1,0,1)),f.x), mix(h(i+vec3(0,1,1)),h(i+vec3(1,1,1)),f.x),f.y), f.z); }
float fbm(vec3 p){ float a = 0.5, s = 0.0; for (int k = 0; k < 4; k++) { s += a*vn(p); p = p*2.03 + 1.7; a *= 0.5; } return s; }
// Brown-white-teal diverging map (dry = brown, wet = teal).
vec3 cmap(float v){
  v = clamp(v, -1.0, 1.0);
  vec3 c0 = vec3(0.30,0.14,0.01), c1 = vec3(0.70,0.42,0.10), c2 = vec3(0.84,0.74,0.52), c3 = vec3(0.80,0.81,0.80),
       c4 = vec3(0.60,0.80,0.77), c5 = vec3(0.13,0.52,0.49), c6 = vec3(0.0,0.24,0.19);
  float x = (v + 1.0)*3.0;
  if (x < 1.0) return mix(c0,c1,x); if (x < 2.0) return mix(c1,c2,x-1.0); if (x < 3.0) return mix(c2,c3,x-2.0);
  if (x < 4.0) return mix(c3,c4,x-3.0); if (x < 5.0) return mix(c4,c5,x-4.0); return mix(c5,c6,x-5.0);
}
float field(vec3 p, float yr){
  float k = floor(yr); float f = smoothstep(0.75, 1.0, fract(yr));
  float a = fbm(p*1.35 + vec3(k*0.23, k*0.07, 0.0)), b = fbm(p*1.35 + vec3((k+1.0)*0.23, (k+1.0)*0.07, 0.0));
  float n = mix(a, b, f);
  float trend = mix(0.25, -0.55, smoothstep(0.0, 1.0, (yr - 1950.0)/74.0));
  return (n - 0.5)*3.4 + trend + 0.15*sin(p.y*4.0);
}
vec3 globe(vec2 q, float lon0, float lat0, float yr, out float a){
  float r2 = dot(q,q); a = 1.0 - smoothstep(0.995, 1.0, sqrt(r2));
  if (r2 >= 1.0) return vec3(0.0);
  vec3 n = vec3(q.x, q.y, sqrt(1.0 - r2));
  float cl = cos(lat0), sl = sin(lat0);
  vec3 m = vec3(n.x, cl*n.y + sl*n.z, -sl*n.y + cl*n.z);
  float lat = asin(clamp(m.y, -1.0, 1.0)), lon = atan(m.x, m.z) + lon0;
  vec3 p = vec3(cos(lat)*sin(lon), sin(lat), cos(lat)*cos(lon));
  vec3 c = cmap(field(p, yr));
  // Invented geography (not Earth): coastlines and borders as thin dark linework, relief hillshade on land.
  vec3 gp = p*1.6 + vec3(3.1, 0.7, 5.3);
  float land = fbm(gp) + 0.06*fbm(p*14.0 + 4.0) - 0.03;
  float isl = step(0.5, land);
  float coast = 1.0 - smoothstep(0.0012, 0.0035, abs(land - 0.5));
  float rel = fbm(p*7.0 + 2.0), rel2 = fbm(p*7.0 + 2.0 + vec3(0.02, 0.02, 0.0));
  c *= 1.0 + isl*clamp((rel2 - rel)*7.0, -0.07, 0.07);
  float bd = fbm(p*3.3 + 9.0);
  float border = isl*(1.0 - smoothstep(0.001, 0.0028, abs(bd - 0.5)));
  c = mix(c, vec3(0.04), 0.85*coast + 0.4*border);
  c *= mix(0.62, 1.0, pow(n.z, 0.45));
  return c;
}
void mainImage(out vec4 o, in vec2 fc){
  vec2 R = iResolution.xy; vec2 px = vec2(fc.x, R.y - fc.y) / R.x * 1080.0;
  float yr = 1950.0 + 74.0*clamp(iTime/28.5, 0.0, 1.0) + 0.0001;
  float rad = 412.0; vec3 col = vec3(0.0); float a;
  vec3 g1 = globe((px - vec2(540.0, 440.0))/rad*vec2(1.0,-1.0), -1.1, 0.15, yr, a); col = mix(col, g1, a);
  vec3 g2 = globe((px - vec2(540.0, 1320.0))/rad*vec2(1.0,-1.0), 1.75, 0.35, yr, a); col = mix(col, g2, a);
  // Slim vertical legend at the left.
  if (px.x > 74.0 && px.x < 92.0 && px.y > 720.0 && px.y < 1060.0) col = cmap(1.0 - 2.0*(px.y - 720.0)/340.0);
  o = vec4(col, 1.0);
}
"""
G = "#BDBDBD"
lay = [{"shader": GLSL, "width": "100%", "height": "100%"}]
lay += [{"text": t, "font": "Inter", "size": 30, "weight": 600, "color": "#E0E0E0", "x": x, "y": 700, "anchor": "bottomLeft"} for t, x in [("mm", 70)]]
lay += [{"text": t, "font": "Inter", "size": 24, "color": G, "x": 102, "y": y, "anchor": "left"} for t, y in [("+40", 724), ("+20", 805), ("0", 890), ("−20", 975), ("−40", 1056)]]
# Timeline: hairline, ticks every 5 years, played portion brighter, year label riding the playhead.
X0, X1, TY = 84.0, 996.0, 1840.0
lay.append({"shape": "rect", "width": X1 - X0, "height": 12, "fill": "#3C3C3C", "x": X0, "y": TY, "anchor": "left"})
lay.append({"shape": "rect", "height": 12, "fill": "#BDBDBD", "x": X0, "y": TY, "anchor": "left",
            "width": X1 - X0, "scaleX": "=max(0.002, clamp(t / 28.5, 0, 1))"})
for k, yv in enumerate(range(Y0, Y1 + 1, 5)):
    x = X0 + (X1 - X0) * (yv - Y0) / (Y1 - Y0)
    lay.append({"shape": "rect", "width": 2, "height": 10 if yv % 10 == 0 else 6, "fill": "#7A7A7A", "x": round(x, 1), "y": TY + 8, "anchor": "top"})
    if yv % 20 == 10:
        lay.append({"text": str(yv), "font": "Inter", "size": 18, "color": "#8A8A8A", "x": round(x, 1), "y": TY + 22, "anchor": "top"})
lay.append({"text": "=floor(1950 + 74 * clamp(t / 28.5, 0, 1))", "font": "Inter", "size": 34, "weight": 400, "color": "#D6D6D6",
            "x": f"={X0} + {X1 - X0} * clamp(t / 28.5, 0, 1)", "y": TY - 14, "anchor": "bottom"})
lay.append({"text": "Soil moisture · illustrative", "font": "Inter", "size": 18, "color": "#7A7A7A", "x": 70, "y": 1100, "anchor": "topLeft"})
spec = {"size": "9:16", "fps": 30, "background": "#000000", "intent": {"pace": "calm", "genre": "data"},
        "scenes": [{"id": "one-take", "duration": DUR, "notes": "REF: single 30 s take, two stacked globes, year-stepped diverging field, legend left, timeline bottom.", "layers": lay}]}
json.dump(spec, open(__file__.replace("make.py", "soil.json"), "w"), indent=1)
