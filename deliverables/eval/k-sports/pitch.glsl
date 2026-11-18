// Overhead football pitch at night, lit by four corner floodlight banks.
uniform float pz; // = 1.0
uniform float l1; // = 0.0
uniform float l2; // = 0.0
uniform float l3; // = 0.0
uniform float l4; // = 0.0
uniform float tint; // = 0.0
uniform float dim; // = 0.0
uniform float boards; // = 0.0

float hash(vec2 p) { return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453); }
float vnoise(vec2 p) {
  vec2 i = floor(p); vec2 f = fract(p);
  vec2 u = f * f * (3.0 - 2.0 * f);
  float a = hash(i), b = hash(i + vec2(1.0, 0.0)), c = hash(i + vec2(0.0, 1.0)), d = hash(i + vec2(1.0, 1.0));
  return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}
float sdBox(vec2 p, vec2 b) { vec2 d = abs(p) - b; return length(max(d, 0.0)) + min(max(d.x, d.y), 0.0); }

void mainImage(out vec4 o, in vec2 f) {
  float k = 1080.0 / iResolution.y;
  vec2 c = (f - 0.5 * iResolution.xy) * k / pz;   // pitch px, centred
  float HX = 810.0, HY = 497.0;
  vec2 a = abs(c);
  // markings
  float d = abs(sdBox(c, vec2(HX, HY)));
  d = min(d, max(a.x, a.y - HY));
  d = min(d, abs(length(c) - 141.0));
  d = min(d, length(c) - 4.0);
  d = min(d, abs(sdBox(vec2(a.x - (HX - 127.5), c.y), vec2(127.5, 311.0))));
  d = min(d, abs(sdBox(vec2(a.x - (HX - 42.5), c.y), vec2(42.5, 141.0))));
  vec2 sp = vec2(a.x - (HX - 170.0), c.y);
  d = min(d, length(sp) - 3.5);
  d = min(d, max(abs(length(sp) - 141.0), a.x - (HX - 255.0)));
  d = min(d, max(abs(length(vec2(a.x - HX, a.y - HY)) - 15.0), max(a.x - HX, a.y - HY)));
  float lw = 2.2;
  float px = k / pz;
  float line = 1.0 - smoothstep(lw - px, lw + px, d);
  // worn chalk
  line *= 0.75 + 0.25 * vnoise(c * 0.35);

  float inside = 1.0 - smoothstep(HX + 60.0, HX + 64.0, a.x);
  inside *= 1.0 - smoothstep(HY + 50.0, HY + 54.0, a.y);

  // grass: mown stripes, clumps, blades
  float stripe = mod(floor((c.x + HX) / (2.0 * HX / 14.0)), 2.0);
  float stripe2 = mod(floor((c.y + HY) / (2.0 * HY / 9.0)), 2.0);
  float mow = 0.80 + 0.13 * stripe + 0.05 * stripe2;
  float clump = 0.85 + 0.3 * vnoise(c * 0.02) + 0.12 * vnoise(c * 0.09);
  float blade = 0.8 + 0.4 * hash(floor(c * 1.3));
  vec3 grass = vec3(0.055, 0.19, 0.075) * mow * clump * blade;
  // worn centre and goalmouths
  float wear = exp(-dot(c, c) / (90.0 * 90.0)) * 0.35 + exp(-((a.x - HX + 40.0) * (a.x - HX + 40.0) + c.y * c.y) / (60.0 * 60.0)) * 0.5;
  grass = mix(grass, vec3(0.22, 0.20, 0.12) * blade, wear * vnoise(c * 0.15));
  vec3 surround = vec3(0.035, 0.04, 0.045) * (0.7 + 0.6 * hash(floor(c * 0.7)));
  vec3 base = mix(surround, grass, inside);
  base = mix(base, vec3(0.92, 0.94, 0.90), line * inside);

  // floodlights: four banks beyond the corners
  vec2 L1 = vec2(-HX - 140.0, HY + 90.0), L2 = vec2(HX + 140.0, -HY - 90.0);
  vec2 L3 = vec2(HX + 140.0, HY + 90.0), L4 = vec2(-HX - 140.0, -HY - 90.0);
  float s = 780.0;
  float li = l1 * exp(-dot(c - L1, c - L1) / (s * s)) + l2 * exp(-dot(c - L2, c - L2) / (s * s))
           + l3 * exp(-dot(c - L3, c - L3) / (s * s)) + l4 * exp(-dot(c - L4, c - L4) / (s * s));
  li *= 2.1;
  float amb = 0.02;
  vec3 light = vec3(1.0, 0.97, 0.90) * li + amb;
  vec3 col = base * light;

  // team halves wash
  vec3 cob = vec3(0.12, 0.25, 1.0), org = vec3(1.0, 0.36, 0.12);
  float side = smoothstep(-2.0, 2.0, c.x);
  col = mix(col, col * mix(cob, org, side) * 1.6, tint * inside);

  // LED advertising boards along the touchlines and behind the goals
  float bt = step(HY + 24.0, a.y) * step(a.y, HY + 32.0) * step(a.x, HX - 20.0);
  float bg = step(HX + 30.0, a.x) * step(a.x, HX + 38.0) * step(a.y, HY - 40.0);
  float seg = step(0.08, fract(c.x / 96.0)) * step(0.08, fract(c.y / 96.0));
  vec3 boardCol = mix(vec3(0.9, 0.92, 1.0), mix(cob, org, side) * 1.4, tint > 0.01 ? 1.0 : 0.0);
  col += boardCol * (bt + bg) * seg * boards * 0.9;
  // vignette & dim
  vec2 uv = f / iResolution.xy - 0.5;
  col *= 1.0 - 0.55 * dot(uv, uv) * 2.0;
  col *= 1.0 - dim;
  o = vec4(col, 1.0);
}
