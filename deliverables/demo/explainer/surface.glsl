// Noise-cancelling, as one device: a dark liquid surface.
// Sound = rings on it; noise = many rings; the earcup = a brushed-metal ring
// that rises from the liquid; inside it the speaker's mirror wave flattens
// the surface to glass. The type lies under the liquid (iChannel0) and only
// reads when the surface is still.
uniform float camH;    // = 1.3
uniform float camZ;    // = -2.0
uniform float pitch;   // = 0.2
uniform float camX;    // = 0.0
uniform float chaos;   // = 0.0
uniform float ring;    // = 0.0
uniform float mic;     // = 0.0
uniform float anti;    // = 0.0
uniform float cancel;  // = 0.0
uniform float calm;    // = 0.0
uniform float warm;    // = 1.0
uniform float textOn;  // = 1.0

const float PI = 3.14159265;
const vec2 RC = vec2(0.0, 6.2);
const float RR = 2.1;

float hash(float n) { return fract(sin(n * 12.9898 + 4.1414) * 43758.5453); }
float hash2(vec2 p) { return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453); }

// One expanding ring: centre c, age a (s); a wave train behind the front.
// Returns (height, d/dx, d/dz) (the envelope's own slope is small and ignored).
vec3 wave(vec2 p, vec2 c, float a, float amp, float k) {
  if (a <= 0.0) return vec3(0.0);
  vec2 d = p - c;
  float r = max(length(d), 1e-4);
  float front = a * 1.7;
  float behind = front - r;
  if (behind < -0.05) return vec3(0.0);
  float env = smoothstep(-0.05, 0.25, behind) * exp(-0.9 * max(behind, 0.0)) * exp(-0.35 * a) / (1.0 + 0.45 * r);
  float ph = k * behind;
  float g = -amp * env * k * cos(ph) / r;
  return vec3(amp * env * sin(ph), g * d.x, g * d.y);
}

// Noise only (before cancellation): height and gradient.
vec3 noiseG(vec2 p, float t) {
  vec3 h = wave(p, vec2(-1.2, 6.8), t - 0.7, 0.16, 7.0);
  if (chaos > 0.001) {
    for (int i = 0; i < 10; i++) {
      float fi = float(i);
      vec2 c = vec2(hash(fi) * 16.0 - 8.0, hash(fi + 7.0) * 11.0 + 2.5);
      float per = 1.3 + hash(fi + 3.0) * 1.5;
      float a = mod(t - hash(fi + 11.0) * per, per) + 0.05;
      h += chaos * wave(p, c, a, 0.11 + 0.06 * hash(fi + 5.0), 6.0 + 4.0 * hash(fi + 9.0));
    }
  }
  return h;
}

float inside(vec2 p) { return smoothstep(RR - 0.05, RR - 0.35, length(p - RC)); }

// Studio environment: a dark room, a long softbox, a warm key; arc warm -> cool.
vec3 env(vec3 r) {
  vec3 hot = mix(vec3(0.45, 0.75, 0.85), vec3(1.0, 0.5, 0.2), warm);
  // after the collapse the room is lit: a pale, bright studio
  vec3 col = mix(vec3(0.012, 0.013, 0.016) + 0.03 * max(r.y, 0.0) * hot, vec3(0.80, 0.79, 0.76) + 0.12 * max(r.y, 0.0), calm);
  // long horizontal softbox ahead and above
  float band = smoothstep(0.035, 0.012, abs(r.y - 0.30)) * smoothstep(0.9, 0.5, abs(r.x));
  col += band * hot * 3.0;
  // overhead strip light (what the still surface mirrors at the end)
  float over = smoothstep(0.012, 0.003, abs(r.x + 0.15 * r.z)) * smoothstep(0.55, 0.75, r.y) * 0.0;
  col += over * mix(vec3(0.9, 0.95, 1.0), hot, 0.3) * 2.4;
  // a second, colder strip low on the right
  float strip = smoothstep(0.05, 0.0, abs(r.y - 0.08)) * smoothstep(0.2, 0.6, r.x) * smoothstep(1.0, 0.7, r.x);
  col += strip * vec3(0.35, 0.55, 0.9) * 0.9;
  return col;
}

// Torus (the earcup rim) SDF, centred at (RC.x, y0, RC.y).
float sdTorus(vec3 p, float y0) {
  vec3 q = p - vec3(RC.x, y0, RC.y);
  vec2 d = vec2(length(q.xz) - RR, q.y);
  return length(d) - 0.13;
}

void mainImage(out vec4 o, in vec2 f) {
  vec2 uv = (f - 0.5 * iResolution.xy) / iResolution.y;
  float t = iTime;
  // camera
  vec3 ro = vec3(camX, camH, camZ);
  vec3 fw = normalize(vec3(0.0, -sin(pitch), cos(pitch)));
  vec3 rt = normalize(cross(vec3(0.0, 1.0, 0.0), fw));
  vec3 up = cross(fw, rt);
  vec3 rd = normalize(fw * 1.55 + rt * uv.x + up * uv.y);

  vec3 col = env(rd) * 0.25;
  float tPlane = rd.y < -1e-4 ? -ro.y / rd.y : 1e9;

  // the earcup rim (sphere tracing near its bounding box only)
  float tRing = 1e9;
  float y0 = mix(-0.25, 0.05, ring);
  if (ring > 0.001) {
    float s = 0.0;
    for (int i = 0; i < 64; i++) {
      vec3 p = ro + rd * s;
      float d = sdTorus(p, y0);
      if (d < 0.002) { tRing = s; break; }
      s += d;
      if (s > 40.0) break;
    }
  }

  if (tPlane < 1e8 && tPlane < tRing) {
    vec3 P = ro + rd * tPlane;
    vec2 p = P.xz;
    vec3 N = noiseG(p, t);
    float k = (1.0 - cancel * inside(p)) * (1.0 - calm);
    vec3 n = normalize(vec3(-N.y * k, 1.0, -N.z * k));
    vec3 r = reflect(rd, n);
    float fres = 0.03 + 0.97 * pow(1.0 - max(dot(-rd, n), 0.0), 5.0);
    // the type below the surface, refracted by the slope
    vec2 tuv = vec2((p.x + 7.0) / 14.0, (p.y - 2.2) / 9.0);
    tuv += n.xz * 0.16;
    vec3 under = vec3(0.0);
    if (tuv.x > 0.0 && tuv.x < 1.0 && tuv.y > 0.0 && tuv.y < 1.0) {
      under = texture(iChannel0, tuv).rgb * textOn;
    }
    // the last word has its own, sharper channel mapped onto the earcup only
    vec2 tuv2 = vec2((p.x - RC.x + 2.2) / 4.4, (p.y - RC.y) / 2.475 + 0.5) + n.xz * 0.12;
    if (tuv2.x > 0.0 && tuv2.x < 1.0 && tuv2.y > 0.0 && tuv2.y < 1.0) {
      under += texture(iChannel1, tuv2).rgb * textOn;
    }
    float slope = length(n.xz);
    under *= mix(1.0, 0.55, smoothstep(0.1, 0.5, slope));
    // dark ink-black liquid becomes bone-white once still; the type flips from light to ink
    float L = dot(under, vec3(0.333));
    vec3 base = mix(vec3(0.006, 0.007, 0.009) + under * 0.85, vec3(0.83, 0.81, 0.77) * (1.0 - 0.97 * L), calm);
    col = mix(base, env(r), fres);
    col *= 1.0 - 0.85 * L * calm;  // the ink sits on top of the reflection once still
    // the anti-wave inside the cup: the noise's crests glow amber, the speaker's mirror crests (the troughs) cyan
    float inn = inside(p) * anti * (1.0 - cancel);
    float hn = N.x;
    col += inn * (vec3(1.0, 0.55, 0.2) * smoothstep(0.0, 0.08, hn) + vec3(0.25, 0.75, 1.0) * smoothstep(0.0, 0.08, -hn)) * 0.9;
    // the still water inside the cup reads as a perfect mirror: a faint glint ring
    float stillIn = inside(p) * cancel * (1.0 - calm);
    col += stillIn * env(reflect(rd, vec3(0.0, 1.0, 0.0))) * 0.15;
    // distance falloff
    float fog = exp(-0.035 * tPlane);
    col = mix(mix(vec3(0.004, 0.005, 0.007), vec3(0.78, 0.77, 0.74), calm), col, fog);
  } else if (tRing < 1e8) {
    vec3 P = ro + rd * tRing;
    float e = 0.002;
    vec3 n = normalize(vec3(
      sdTorus(P + vec3(e, 0, 0), y0) - sdTorus(P - vec3(e, 0, 0), y0),
      sdTorus(P + vec3(0, e, 0), y0) - sdTorus(P - vec3(0, e, 0), y0),
      sdTorus(P + vec3(0, 0, e), y0) - sdTorus(P - vec3(0, 0, e), y0)));
    // brushed metal: anisotropic streaks around the rim
    float ang = atan(P.z - RC.y, P.x - RC.x);
    float brush = 0.85 + 0.15 * sin(ang * 900.0 + hash2(vec2(floor(ang * 300.0), 1.0)) * 6.0);
    vec3 r = reflect(rd, n);
    float fres = 0.6 + 0.4 * pow(1.0 - max(dot(-rd, n), 0.0), 5.0);
    col = env(r) * fres * brush * vec3(0.95, 0.93, 0.9) + vec3(0.02);
    // the microphone: a small light on the front of the rim
    vec3 mp = vec3(RC.x, y0 + 0.1, RC.y - RR);
    col += mic * vec3(0.3, 0.8, 1.0) * 0.6 * exp(-60.0 * dot(P - mp, P - mp));
  }
  // grain and gentle vignette
  col *= 1.0 - mix(0.35, 0.18, calm) * dot(uv, uv);
  col += (hash2(f + fract(t) * 91.0) - 0.5) * 0.006;
  o = vec4(pow(max(col, 0.0), vec3(1.0 / 2.2)), 1.0);
}
