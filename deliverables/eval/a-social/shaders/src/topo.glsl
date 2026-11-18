// Shot 5: the highlands of Guji as contour lines. A ring draws itself around the farms.
uniform float ring; // = 0.0
const vec2 C = vec2(540.0, 1100.0);
const float R = 360.0;
float height(vec2 u){ return fbm(u * 1.6 + vec2(3.0, 1.0)) + 0.45 * fbm(u * 3.7 - 2.0); }
void mainImage(out vec4 o, in vec2 f){
  float t = iTime;
  vec2 u = (f - C) / R;
  vec2 drift = vec2(0.0, -0.06 * t);
  vec2 su = u * (0.75 - 0.03 * t) + drift;
  float h = height(su);
  float e = 0.004;
  float hx = height(su + vec2(e, 0.0)) - h, hy = height(su + vec2(0.0, e)) - h;
  float g = length(vec2(hx, hy)) / e;
  float k = 13.0;
  float v = h * k;
  float dl = abs(fract(v) - 0.5);
  float px = R;                                    // pixels per unit
  float wpx = g * k / px;                          // contour spacing derivative per pixel
  float line = smoothstep(1.8 * wpx, 0.6 * wpx, 0.5 - dl);
  float major = step(mod(floor(v + 0.5), 5.0), 0.5);
  vec3 bg = field(f, hexc(30.0, 16.0, 11.0));
  float r = length(u);
  float insideR = smoothstep(1.02, 0.98, r);
  vec3 lc = mix(hexc(170.0, 92.0, 66.0), hexc(255.0, 176.0, 128.0), insideR);
  float alpha = (0.45 + 0.5 * major) * (0.6 + 0.4 * insideR);
  vec3 col = mix(bg, lc, line * alpha * smoothstep(0.0, 0.35, t + 0.15));
  // the ring: drawn on clockwise from 12 o'clock
  float ang = atan(u.x, u.y); ang = ang < 0.0 ? ang + 6.28318 : ang;
  float drawn = step(ang / 6.28318, ring);
  float rl = smoothstep(4.0 / R, 1.5 / R, abs(r - 1.0)) * drawn;
  col = mix(col, hexc(255.0, 214.0, 170.0), rl);
  // farm dots inside: washing stations
  vec2 dp = u * 2.4;
  float dots = 0.0;
  for (int i = 0; i < 7; i++) {
    float fi = float(i);
    vec2 c = vec2(hash(vec2(fi, 2.0)) - 0.5, hash(vec2(fi, 7.0)) - 0.5) * 3.0;
    float app = smoothstep(0.6 + fi * 0.12, 0.7 + fi * 0.12, t);
    dots += smoothstep(0.07, 0.05, length(dp - c)) * app;
  }
  
  o = vec4(col, 1.0);
}
