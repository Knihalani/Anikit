// The darkroom tray: liquid over everything below, lit by one safelight.
uniform float room;    // = 0.0   0 safelight, 1 white room light
uniform float lights;  // = 1.0   master light level (0 = blackout)
uniform float lx;      // = 0.18  key light position (uv, 0..1, y up)
uniform float ly;      // = 0.95
uniform float slosh;   // = 1.0   ripple amplitude
uniform float wave;    // = -1.0  position of a developer wave front sweeping left->right (-1 = none)
uniform float seed;    // = 0.0

float hsh(vec2 p){ return fract(sin(dot(p, vec2(127.1,311.7))) * 43758.5453); }
float height(vec2 p, float t){
  float h = 0.0;
  h += 0.50 * sin(dot(p, vec2( 7.0,  2.0)) - t*1.30 + seed);
  h += 0.35 * sin(dot(p, vec2(-3.0,  8.5)) - t*1.75 + 1.7);
  h += 0.22 * sin(dot(p, vec2(13.0, -5.0)) - t*2.40 + 4.1);
  h += 0.12 * sin(dot(p, vec2(-19.0,-14.0)) - t*3.10 + 2.3);
  h *= 0.6 * slosh;
  if (wave > -0.5) {           // a travelling bore of developer
    float d = p.x - (wave * 2.2 - 0.3);
    h += 1.6 * exp(-d*d*28.0) + 0.5 * exp(-(d+0.18)*(d+0.18)*60.0) * sin(p.y*20.0 + t*3.0);
  }
  return h;
}
void mainImage(out vec4 o, in vec2 f){
  vec2 R = iResolution.xy;
  vec2 uv = f / R;
  vec2 p = vec2(uv.x * R.x / R.y, uv.y);
  float t = iTime;
  float e = 0.004;
  float h  = height(p, t);
  float hx = height(p + vec2(e, 0.0), t) - h;
  float hy = height(p + vec2(0.0, e), t) - h;
  vec3 n = normalize(vec3(-hx / e * 0.012, -hy / e * 0.012, 1.0));
  // refraction of the paper under the liquid
  vec2 ruv = clamp(uv + n.xy * 0.010, 0.0, 1.0);
  vec3 c = texture(iChannel0, ruv).rgb;
  float L = dot(c, vec3(0.299, 0.587, 0.114));
  // key light: position, inverse-square falloff
  vec2 lp = vec2(lx * R.x / R.y, ly);
  vec2 dl = p - lp;
  float dist2 = dot(dl, dl);
  float fall = 1.0 / (1.0 + dist2 * 2.6);
  vec3 safe = vec3(1.0, 0.24, 0.07);
  vec3 safeHi = vec3(1.0, 0.55, 0.25);
  vec3 lit = mix(safe * pow(L, 1.15), safeHi * L, smoothstep(0.55, 1.0, L));
  vec3 white = c * vec3(1.0, 0.97, 0.92);
  vec3 col = mix(lit * 1.25, white, room) * mix(0.07 + fall * 1.25, 0.4 + 0.65 * fall, room);
  // the safelight's own reflection in the developer: a small hard rectangle, broken up by the ripples
  vec2 rp = p + n.xy * 0.9 - vec2(0.0, 0.0);
  vec2 lq = (rp - (lp + vec2(0.05, -0.06))) / vec2(0.075, 0.03);
  float lamp = (1.0 - smoothstep(0.85, 1.0, abs(lq.x))) * (1.0 - smoothstep(0.8, 1.0, abs(lq.y)));
  col += lamp * 1.4 * mix(safeHi, vec3(1.0, 0.97, 0.9), room) * (1.0 - L * 0.6);
  // small glints of the key on ripple crests
  vec3 l3 = normalize(vec3(lp - p, 0.9));
  vec3 hv = normalize(l3 + vec3(0.0, 0.0, 1.0));
  float spec = pow(max(dot(n, hv), 0.0), 900.0) * 0.9 * fall;
  col += spec * mix(safeHi, vec3(1.0, 0.98, 0.94), room);
  // grain
  float g = hsh(f + fract(t * 23.0) * 91.0) - 0.5;
  col += g * 0.035 * (0.4 + L);
  col *= lights;
  col = max(col - 0.004, 0.0);
  o = vec4(col, 1.0);
}
