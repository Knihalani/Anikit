// Atelier Vauclair Calibre 9: a macro of the guilloche dial, shaded as metal under one moving key.
uniform float zoom;   // = 1.0  camera magnification
uniform float cx;     // = 0.0  camera centre, dial units (dial radius ~1)
uniform float cy;     // = 0.0
uniform float rot;    // = 0.0  camera roll (radians)
uniform float light;  // = 0.8  key azimuth (radians)
uniform float lightz; // = 0.45 key elevation (z of light vector)
uniform float hrs;    // = 10.1667 time on the hands (hours)
uniform float reserve;// = 1.0  power reserve 0..1 (72 h)
uniform float secs;   // = 0.0  small-seconds hand (seconds)
uniform float ox;     // = 0.0  screen-space offset of the dial (fraction of height)
uniform float cut;    // = 99.0 rose-engine progress in rings (fractional part = angle of the cutter)
uniform float shade;  // = 0.0  darken the left of frame for type

const float PI = 3.14159265;

vec2 rot2(vec2 p, float a) { float c = cos(a), s = sin(a); return vec2(c*p.x - s*p.y, s*p.x + c*p.y); }

// studio environment seen in polished steel: dark room, one long softbox following the key, a warm bounce card
vec3 env(vec3 R) {
  float az = atan(R.y, R.x);
  float d = az - light; d = d - 2.0*PI*floor((d + PI)/(2.0*PI));
  float el = R.z;
  float box = exp(-d*d*9.0) * smoothstep(0.05, 0.35, el) * smoothstep(0.98, 0.75, el);
  float d2 = az - light - 2.6; d2 = d2 - 2.0*PI*floor((d2 + PI)/(2.0*PI));
  float card = exp(-d2*d2*4.0) * smoothstep(0.0, 0.4, el) * 0.18;
  vec3 c = vec3(0.004, 0.0045, 0.006) + vec3(0.05, 0.052, 0.058) * pow(max(el, 0.0), 3.0);
  // ceiling bands: give polished steel something to show
  c += vec3(0.06) * smoothstep(0.02, 0.0, abs(fract(az*1.6/PI) - 0.5) - 0.18) * smoothstep(0.2, 0.6, el) * smoothstep(0.95, 0.7, el);
  c += vec3(1.0, 0.97, 0.92) * box * 4.0;
  c += vec3(1.0, 0.72, 0.45) * card;
  return c;
}

float sdSeg(vec2 p, vec2 a, vec2 b) { vec2 pa = p - a, ba = b - a; float h = clamp(dot(pa, ba)/dot(ba, ba), 0.0, 1.0); return length(pa - ba*h); }

void mainImage(out vec4 o, in vec2 f) {
  vec2 uv = (f - 0.5*iResolution.xy) / iResolution.y;
  uv.x -= ox;
  float span = 2.45 / zoom;                 // dial units per screen height
  float px = span / iResolution.y;          // one pixel in dial units
  vec2 p = rot2(uv * span, rot) + vec2(cx, cy);
  float r = length(p), a = atan(p.y, p.x);
  vec3 P3 = vec3(p, 0.0);
  vec3 L = normalize(vec3(2.4*cos(light), 2.4*sin(light), 2.4*lightz/sqrt(max(1.0 - lightz*lightz, 0.05))) - P3);
  vec3 V = normalize(vec3(0.0, 0.0, 3.2) - P3);
  vec3 H = normalize(L + V);
  vec3 col;

  // ---------- background: dark slate cloth with a pool of key light ----------
  float pool = exp(-dot(p - 1.4*vec2(cos(light), sin(light)), p - 1.4*vec2(cos(light), sin(light))) * 0.25);
  vec3 bg = vec3(0.004, 0.0042, 0.005) + vec3(0.03, 0.028, 0.026) * pool;
  // case shadow
  bg *= mix(0.25, 1.0, smoothstep(1.0, 1.35, length(p + 0.08*vec2(cos(light), sin(light)))));
  col = bg;

  // ---------- crown at 3 o'clock (knurled) ----------
  {
    vec2 q = p - vec2(1.13, 0.0);
    float inside = step(abs(q.x), 0.075) * step(abs(q.y), 0.13);
    float stem = step(abs(q.x + 0.09), 0.03) * step(abs(q.y), 0.06);
    if (inside + stem > 0.0) {
      float k = sin(q.y * 190.0);
      float bend = clamp(q.y / 0.13, -1.0, 1.0);
      vec3 n = normalize(vec3(0.0, bend*1.2 + 0.35*k*inside, 1.0));
      n.x += 0.6*q.x/0.075*inside;
      n = normalize(n);
      col = env(reflect(-V, n)) * vec3(0.78, 0.79, 0.8) + 0.02;
      col *= mix(1.0, 0.55, step(0.9, abs(k))*inside);
    }
  }

  // ---------- case + bezel: polished steel, convex ----------
  if (r < 1.06) {
    float t = clamp((r - 0.9) / 0.16, 0.0, 1.0);        // across the bezel
    float tilt = mix(0.15, 1.35, t*t);                   // curves away at the edge
    vec2 rh = p / max(r, 1e-4);
    vec3 n = normalize(vec3(rh * tilt, 1.0));
    vec3 steel = env(reflect(-V, n)) * vec3(0.8, 0.81, 0.83);
    steel += vec3(0.02);
    // a fine brushed band on the outer flank
    float brushed = smoothstep(0.995, 1.0, r);
    steel = mix(steel, steel*0.6 + 0.03*vec3(1.0), brushed);
    col = steel;
    col *= smoothstep(1.06, 1.06 - 1.5*px, r) * 0.0 + 1.0;
    float edge = smoothstep(1.06, 1.06 - 2.0*px, r);
    col = mix(bg, col, edge);
  }

  // ---------- inner flange (rehaut) with minute track ----------
  if (r < 0.905) {
    vec3 n = normalize(vec3(-(p/r)*0.5, 1.0));
    col = vec3(0.012, 0.016, 0.03) + env(reflect(-V, n)) * 0.05;
    // printed minute ticks on the flange
    float m = a / (2.0*PI) * 60.0;
    float dm = abs(fract(m + 0.5) - 0.5) * (2.0*PI/60.0) * r;
    float five = step(abs(fract(m/5.0 + 0.5) - 0.5)*5.0, 0.5);
    float tick = smoothstep(px*1.2 + mix(0.0028, 0.0055, five), mix(0.0028, 0.0055, five), dm) * step(0.865, r) * step(r, 0.895);
    col = mix(col, vec3(0.86, 0.87, 0.9), tick*0.92);
  }

  // ---------- dial: rose-engine guilloche, blue lacquered over silver ----------
  if (r < 0.858) {
    float N = 64.0;
    float wav = smoothstep(0.0, 0.3, r) * r * (0.024 * sin(18.0*a + 9.0*r) + 0.005 * sin(54.0*a));
    float phi = N * (r + wav);
    // gradient of phi
    vec2 rh = p / max(r, 1e-4), ah = vec2(-rh.y, rh.x);
    vec2 g = N * (rh * (1.0 + 0.024*sin(18.0*a + 9.0*r) + 0.005*sin(54.0*a) + r*0.024*9.0*cos(18.0*a + 9.0*r)) + ah * (0.024*18.0*cos(18.0*a + 9.0*r) + 0.005*54.0*cos(54.0*a)));
    float gl = length(g);
    vec2 gn = g / max(gl, 1e-4);
    float fp = gl * px;                                  // grooves per pixel
    float amp = 0.55 * smoothstep(0.6, 0.18, fp);         // fade sub-pixel grooves (stays a sheen)
    // the rose engine: rings are cut outward, one revolution each; the cutter sits at angle fract(cut)
    float ring = floor(phi);
    float ca = fract(cut) * 2.0*PI;                        // cutter angle, clockwise from 12
    float pa = atan(p.x, p.y); pa = pa < 0.0 ? pa + 2.0*PI : pa;
    float done = step(ring + 1.0, floor(cut) + 0.5) + step(abs(ring - floor(cut)), 0.5) * step(pa, ca);
    float cutMask = clamp(done, 0.0, 1.0);
    amp *= cutMask;
    float blank = 1.0 - cutMask;
    float s = sin(2.0*PI*phi);
    vec3 n = normalize(vec3(-amp * s * gn, 1.0));
    // the sheen that fine grooves leave: anisotropic, perpendicular to grooves
    float diff = max(dot(n, L), 0.0);
    float ndh = max(dot(n, H), 0.0);
    float aniso = 1.0 - abs(dot(normalize(H.xy + 1e-4), gn));
    float spec = pow(ndh, 120.0) * 3.0 + pow(ndh, 18.0) * 0.25;
    spec += (1.0 - amp/0.55) * pow(aniso, 10.0) * 0.18;
    vec3 base = vec3(0.010, 0.026, 0.085);
    vec3 dial = base * (0.15 + 1.0*diff*diff) + vec3(0.40, 0.58, 1.0) * spec;
    // the cut floors catch less light: subtle ink in the valleys
    dial *= 0.82 + 0.18 * (0.5 + 0.5*s);
    // lacquer clearcoat: a faint broad reflection of the softbox
    dial += env(reflect(-V, vec3(0.0, 0.0, 1.0) + vec3(p*0.08, 0.0))) * 0.035;
    // uncut blank: lapped flat, a softer broad sheen
    vec3 blankc = vec3(0.016, 0.034, 0.10) * (0.2 + 0.9*max(L.z, 0.0)) + vec3(0.35, 0.5, 0.9) * pow(max(H.z, 0.0), 400.0) * 1.5;
    dial = mix(dial, blankc, blank * step(ring, 60.0));
    // the graver's hot point and the fresh, bright groove just behind it
    if (cut < 90.0) {
      vec2 tp = (floor(cut) + 0.5) / N * vec2(sin(ca), cos(ca));
      float dt = length(p - tp);
      float behind = ca - pa; behind = behind < 0.0 ? behind + 2.0*PI : behind;
      float fresh = step(abs(ring - floor(cut)), 0.5) * exp(-behind * 3.0) * (0.5 + 0.5*s);
      dial += vec3(0.75, 0.85, 1.0) * fresh * 0.9;
      dial += vec3(1.0, 0.92, 0.8) * (exp(-dt*dt / (px*px*30.0)) * 2.5 + exp(-dt * 60.0) * 0.25);
    }
    col = dial;
    col *= smoothstep(0.858, 0.858 - 1.5*px, r) * 0.5 + 0.5;

    // ---------- sub-dials: azurage (fine concentric snailing), sunk ----------
    for (int k = 0; k < 2; k++) {
      vec2 c = (k == 0) ? vec2(0.0, 0.43) : vec2(0.0, -0.43);
      float rad = (k == 0) ? 0.215 : 0.17;
      vec2 q = p - c; float rq = length(q);
      if (rq < rad) {
        float phi2 = rq * 240.0;
        vec2 qn = q / max(rq, 1e-4);
        float fp2 = 240.0 * px;
        float amp2 = 0.45 * smoothstep(0.6, 0.18, fp2);
        float s2 = sin(2.0*PI*phi2);
        vec3 n2 = normalize(vec3(-amp2*s2*qn, 1.0));
        float an2 = 1.0 - abs(dot(normalize(H.xy + 1e-4), qn));
        float sp2 = pow(max(dot(n2, H), 0.0), 70.0) * 1.6 + (1.0 - amp2/0.45) * pow(an2, 8.0) * 0.2;
        vec3 sub = vec3(0.016, 0.034, 0.095) * (0.4 + max(dot(n2, L), 0.0)) + vec3(0.42, 0.56, 0.9) * sp2;
        // bevelled rim of the recess
        float rim = smoothstep(rad - 0.012, rad, rq);
        sub = mix(sub, env(reflect(-V, normalize(vec3(-qn*0.9, 1.0)))) * 0.5, rim);
        col = mix(col, sub, smoothstep(rad, rad - 1.5*px, rq));
        float qa = atan(q.x, q.y);                       // 0 at 12, clockwise positive
        if (cut > 90.0) {
        if (k == 0) {
          // power reserve: arc from -60deg (0 h) to +60deg (72 h), 6 h ticks, 72 heavier
          float u = qa / (PI/3.0);                       // -1..1 across the scale
          float h = (u + 1.0) * 36.0;                    // hours 0..72
          float dh = abs(fract(h/6.0 + 0.5) - 0.5) * 6.0 * (PI/3.0/36.0) * rq;
          float major = step(abs(fract(h/24.0 + 0.5) - 0.5)*24.0, 0.5);
          float tk = smoothstep(px*1.2 + mix(0.0022, 0.0045, major), mix(0.0022, 0.0045, major), dh)
                     * step(abs(u), 1.003) * step(rad - 0.065 - 0.02*major, rq) * step(rq, rad - 0.02);
          float arc = smoothstep(px + 0.0018, 0.0018, abs(rq - (rad - 0.022))) * step(abs(u), 1.0);
          col = mix(col, vec3(0.86, 0.87, 0.9), max(tk, arc) * 0.92);
          // hand
          float ha = (reserve*2.0 - 1.0) * (PI/3.0);
          vec2 hd = vec2(sin(ha), cos(ha));
          float dh2 = sdSeg(q, -hd*0.03, hd*(rad - 0.03));
          float hw = mix(0.006, 0.0025, clamp(dot(q, hd)/(rad), 0.0, 1.0));
          vec3 hn = normalize(vec3(vec2(hd.y, -hd.x) * sign(dot(q, vec2(hd.y, -hd.x))) * 0.8, 1.0));
          vec3 hc = env(reflect(-V, hn)) * 0.85 + 0.03;
          col = mix(col, hc, smoothstep(hw + px, hw, dh2));
          col = mix(col, env(reflect(-V, normalize(vec3(q*12.0, 1.0)))) * 0.8 + 0.04, smoothstep(0.011 + px, 0.011, rq));
        } else {
          float sec = qa / (2.0*PI) * 60.0;
          float ds = abs(fract(sec/5.0 + 0.5) - 0.5) * 5.0 * (2.0*PI/60.0) * rq;
          float tk = smoothstep(px*1.2 + 0.0024, 0.0024, ds) * step(rad - 0.04, rq) * step(rq, rad - 0.016);
          col = mix(col, vec3(0.86, 0.87, 0.9), tk * 0.9);
          float sa = secs / 60.0 * 2.0*PI;
          vec2 sd = vec2(sin(sa), cos(sa));
          float d3 = sdSeg(q, -sd*0.04, sd*(rad - 0.02));
          vec3 hc = env(reflect(-V, normalize(vec3(vec2(sd.y, -sd.x)*sign(dot(q, vec2(sd.y, -sd.x)))*0.8, 1.0)))) * 0.85 + 0.03;
          col = mix(col, hc, smoothstep(0.0022 + px, 0.0022, d3));
          col = mix(col, env(reflect(-V, normalize(vec3(q*12.0, 1.0)))) * 0.8 + 0.04, smoothstep(0.009 + px, 0.009, rq));
        }
        }
      }
    }

    // ---------- applied indices: polished steel batons with a central ridge ----------
    float ha2 = atan(p.x, p.y);                          // 0 at 12, clockwise
    float hidx = floor(ha2 / (2.0*PI) * 12.0 + 0.5);
    float aidx = hidx / 12.0 * 2.0*PI;
    vec2 dir = vec2(sin(aidx), cos(aidx));
    vec2 side = vec2(dir.y, -dir.x);
    bool skip = (abs(hidx) < 0.5) || (abs(abs(hidx) - 6.0) < 0.5);
    float along = dot(p, dir), across = dot(p, side);
    float w = (abs(hidx) < 0.5) ? 0.0 : 0.016;
    float inIdx = 0.0;
    float acr = across;
    if (abs(hidx) < 0.5) { acr = abs(across) - 0.026; }   // double baton at 12
    float r0 = 0.68, r1 = 0.83;
    if (abs(abs(hidx) - 6.0) < 0.5) { r0 = 0.66; }
    float wb = 0.017;
    float dIdx = max(abs(acr) - wb, max(r0 - along, along - r1));
    if (cut > 90.0 && dIdx < px) {
      float sgn = sign(acr);
      vec3 n3 = normalize(vec3(side * sgn * 0.45 + dir * (along - 0.755) * 3.0, 1.0));
      vec3 ic = env(reflect(-V, n3)) * vec3(0.82, 0.83, 0.85) + 0.025;
      // drop shadow edge
      col = mix(col, ic, smoothstep(px, 0.0, dIdx));
    } else if (cut > 90.0) {
      // shadow cast by the baton onto the dial
      vec2 sh = p + vec2(cos(light), sin(light)) * 0.012;
      float al2 = dot(sh, dir), ac2 = dot(sh, side);
      if (abs(hidx) < 0.5) { ac2 = abs(ac2) - 0.026; }
      float dS = max(abs(ac2) - wb, max(r0 - al2, al2 - r1));
      col *= mix(0.45, 1.0, smoothstep(-0.004, 0.008, dS));
    }
    float unused = w + float(skip);

    // ---------- dauphine hands: two polished facets each ----------
    for (int k = 0; k < 2; k++) {
      if (cut < 90.0) break;
      float ang = (k == 0) ? (hrs / 12.0 * 2.0*PI) : (fract(hrs) * 2.0*PI);
      float len = (k == 0) ? 0.5 : 0.78;
      float wid = (k == 0) ? 0.05 : 0.04;
      vec2 hd = vec2(sin(ang), cos(ang));
      vec2 hs = vec2(hd.y, -hd.x);
      // shadow first
      vec2 sp = p + vec2(cos(light), sin(light)) * (0.02 + 0.01*float(k));
      float sl = dot(sp, hd), sx = dot(sp, hs);
      float shw = (sl < len*0.18) ? wid * clamp((sl + 0.06)/(len*0.18 + 0.06), 0.0, 1.0) : wid * (len - sl) / (len*0.82);
      float shd = max(abs(sx) - shw, max(-0.06 - sl, sl - len));
      col *= mix(0.5, 1.0, smoothstep(-0.006, 0.01, shd));
      float l = dot(p, hd), x = dot(p, hs);
      float hw = (l < len*0.18) ? wid * clamp((l + 0.06)/(len*0.18 + 0.06), 0.0, 1.0) : wid * (len - l) / (len*0.82);
      float dH = max(abs(x) - hw, max(-0.06 - l, l - len));
      if (dH < px) {
        vec3 n4 = normalize(vec3(hs * sign(x) * 0.4, 1.0));
        vec3 hc = env(reflect(-V, n4)) * vec3(0.84, 0.85, 0.87) + 0.02;
        hc *= mix(0.75, 1.0, smoothstep(0.0, 0.004, abs(x)));   // the ridge line
        col = mix(col, hc, smoothstep(px, 0.0, dH));
      }
    }
    // centre cannon pinion
    if (cut > 90.0) col = mix(col, env(reflect(-V, normalize(vec3(p*14.0, 1.0)))) * 0.85 + 0.04, smoothstep(0.024 + px, 0.024, r));
    col = mix(col, vec3(0.02), smoothstep(0.007 + px, 0.007, r));
  }

  // sapphire: one broad soft reflection of the key, sliding with it
  vec2 lp = vec2(cos(light), sin(light));
  float glare = exp(-pow(dot(p, vec2(-lp.y, lp.x)) - 0.25, 2.0) * 6.0) * smoothstep(0.92, 0.80, r);
  col += vec3(0.9, 0.95, 1.0) * glare * 0.025;

  col *= mix(1.0, 0.35, shade * smoothstep(0.15, -0.6, uv.x));
  // filmic shoulder + gamma
  col = col / (1.0 + col*0.35);
  col = pow(max(col - 0.0015, 0.0), vec3(1.0/2.2));
  // grain
  float gr = fract(sin(dot(f + fract(iTime*7.0)*vec2(91.7, 37.3), vec2(12.9898, 78.233))) * 43758.5453) - 0.5;
  col += gr * 0.012;
  o = vec4(col, 1.0);
}
