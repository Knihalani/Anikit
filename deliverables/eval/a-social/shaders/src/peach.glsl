// Shot 3: a ripe peach halved, the stone still in it.
const vec2 C = vec2(540.0, 1100.0);
const float R = 410.0;
void mainImage(out vec4 o, in vec2 f){
  float t = iTime;
  vec2 es = enterSpin(t, 1.0);
  vec2 p = (f - C) / R;
  vec3 col = field(f, hexc(92.0, 22.0, 52.0));
  float sh = smoothstep(1.25, 0.85, length(p - vec2(0.09, -0.12)));
  col *= 1.0 - 0.55 * sh;
  vec2 q = rot(es.x - 0.03 * t) * p / es.y;
  q.x *= 0.97;
  float r = length(q);
  float a = atan(q.y, q.x);
  float aa = 1.6 / R;
  float edgeR = 1.0 + 0.03 * sin(a * 2.0 + 0.7) + 0.012 * sin(a * 5.0 + 2.0);
  float inside = smoothstep(edgeR + aa, edgeR - aa, r);
  float rn = r / edgeR;
  // stone cavity
  vec2 s = rot(0.35) * (q - vec2(0.02, 0.03));
  float sd = length(s / vec2(0.46, 0.34)) + 0.06 * fbm(s * 6.0) - 1.0;
  float bleed = smoothstep(0.75, 0.0, sd + 0.25 * fbm(q * 5.0 + 3.0));
  float fib = noise(vec2(a * 46.0, rn * 3.0)) * 0.5 + noise(vec2(a * 120.0, rn * 6.0)) * 0.5;
  vec3 flesh = mix(hexc(255.0, 186.0, 74.0), hexc(252.0, 150.0, 60.0), smoothstep(0.2, 0.9, rn));
  flesh = mix(flesh, hexc(232.0, 76.0, 46.0), bleed * 0.9);
  flesh *= 0.92 + 0.12 * fib;
  // skin rim
  vec3 skin = mix(hexc(200.0, 40.0, 40.0), hexc(240.0, 120.0, 50.0), noise(vec2(a * 4.0, 1.0)));
  flesh = mix(flesh, skin, smoothstep(0.955, 0.985, rn));
  // stone: wrinkled, deep red-brown
  float w = fbm(s * 7.0);
  float ridges = abs(sin(w * 14.0 + s.y * 6.0));
  vec3 stone = mix(hexc(96.0, 30.0, 18.0), hexc(166.0, 70.0, 40.0), smoothstep(0.2, 0.9, ridges));
  stone *= 0.75 + 0.35 * smoothstep(0.3, -0.6, sd);
  float inStone = smoothstep(0.012, -0.012, sd);
  // the flesh dips into a red, glistening lip around the stone
  float lip = smoothstep(0.12, 0.0, sd) * (1.0 - inStone);
  flesh = mix(flesh, hexc(196.0, 46.0, 34.0), lip * 0.8);
  vec3 c = mix(flesh, stone, inStone);
  // wet sheen
  vec2 L = normalize(vec2(-0.55, 0.83));
  float sheen = smoothstep(0.55, 1.0, fbm(q * 4.0 + 7.0)) * smoothstep(-0.2, 0.8, dot(q, L)) * (1.0 - inStone);
  c += sheen * 0.22;
  c *= 0.9 + 0.12 * dot(q, L);
  col = mix(col, c, inside);
  o = vec4(col, 1.0);
}
