// Filter: everything above the horizon (sky, lamps, the words) reflects into the
// tidal channels of a wet mudflat. Grain and a soft vignette for the whole frame.
uniform float horizon; // = 0.40
uniform float zoom;    // = 1.0
uniform float camx;    // = 0.0
uniform float tide;    // = 0.0   0 = low water, 1 = channels brim
uniform float chop;    // = 1.0
uniform float boat;    // = 1.0   1 = show the beached boat
uniform float boatx;   // = 0.22
uniform float chan;    // = 0.0   gully offset

float h2(vec2 p){ return fract(sin(dot(p, vec2(127.1, 311.7)))*43758.5453); }
float vn(vec2 p){ vec2 i = floor(p); vec2 f = fract(p); f = f*f*(3.0-2.0*f);
  return mix(mix(h2(i), h2(i+vec2(1.0,0.0)), f.x), mix(h2(i+vec2(0.0,1.0)), h2(i+vec2(1.0,1.0)), f.x), f.y); }
float fbm(vec2 p){ float s = 0.0; float a = 0.5; for (int i = 0; i < 4; i++){ s += a*vn(p); p = p*2.07 + vec2(3.1, 1.3); a *= 0.5; } return s; }

vec3 tap(vec2 p){ return texture(iChannel0, clamp(p, vec2(0.001), vec2(0.999))).rgb; }

void mainImage(out vec4 o, in vec2 f){
  vec2 uv = f/iResolution.xy;
  float asp = iResolution.x/iResolution.y;
  vec3 col = tap(uv);
  float s = horizon - uv.y;
  if (s > 0.0) {
    // ground plane coordinates (perspective)
    float z = 0.10/(s + 0.004);
    float gx = (uv.x - 0.5)*asp/zoom*z*0.9 + camx*4.0;
    vec2 g = vec2(gx, z);
    // tidal channels winding toward the quay; more water far away and as the tide comes in
    float n = fbm(vec2(g.x*2.2 + 0.7*sin(g.y*1.1), g.y*1.4));
    float th = mix(0.50, 0.26, tide) - 0.25*smoothstep(3.0, 9.0, z);
    float edge = 0.012 + 0.004*z;
    float water = smoothstep(th - edge, th + edge, n);
    // the main gully, meandering out toward the harbour mouth
    float xc = chan + 0.22*sin(z*0.9 + 1.0) + 0.08*sin(z*2.3);
    float wdt = mix(0.075, 0.16, tide) + 0.03*vn(vec2(z*3.0, 1.0));
    float gully = smoothstep(wdt, wdt*0.75, abs(g.x - xc));
    water = max(water, gully);
    // ripples: stronger close to us, wind-blown
    float t = iTime;
    float rip = sin(g.y*9.0 - t*1.6 + 2.0*vn(g*vec2(1.5, 0.8)))*0.6 + (vn(vec2(g.x*3.0, g.y*6.0 - t*0.8)) - 0.5)*1.4;
    float amp = (0.0016 + 0.010*s)*chop*(1.0 + tide*1.5);
    float ry = horizon + s;
    vec2 rp = vec2(uv.x + rip*amp*0.6, ry + rip*amp*0.5);
    // long-exposure smear: lamps stretch into streaks
    vec3 refl = vec3(0.0);
    float spread = 0.003 + s*0.07;
    for (int i = 0; i < 8; i++){
      float fi = float(i) - 3.5;
      refl += tap(rp + vec2(0.0, fi*spread));
    }
    refl /= 8.0;
    vec3 sharp = tap(rp);
    vec3 wref = mix(sharp, refl, 0.55);
    float fres = mix(0.85, 0.45, smoothstep(0.0, 0.35, s));
    // the mud: dark silt with ripple marks catching the moon, a wet rough sheen
    float marks = sin(g.y*26.0 + 6.0*fbm(g*vec2(0.6, 1.2)) + g.x*2.0);
    float silt = fbm(g*vec2(2.5, 3.0));
    vec3 mud = vec3(0.036, 0.033, 0.028)*(0.45 + 1.0*silt);
    mud += vec3(0.05, 0.065, 0.07)*smoothstep(0.75, 1.0, marks)*smoothstep(0.02, 0.2, s)*0.5;
    vec3 sheen = vec3(0.0);
    float sp2 = 0.012 + s*0.18;
    for (int i = 0; i < 5; i++){
      float fi = float(i) - 2.0;
      sheen += tap(vec2(uv.x + (silt - 0.5)*0.01, ry + fi*sp2 + 0.01));
    }
    sheen *= 0.2*0.30*(0.4 + silt)*(0.6 + 0.8*smoothstep(0.6, 1.0, marks));
    vec3 ground = mud + sheen;
    vec3 wat = wref*fres*vec3(0.92, 0.97, 1.0) + vec3(0.016, 0.034, 0.042)*(1.0 - smoothstep(0.0, 0.4, s)*0.5);
    // wet glistening rim along every channel edge
    float rim = smoothstep(edge*1.6, 0.0, abs(n - th))*0.5*(1.0 - gully);
    col = mix(ground, wat, water) + refl*rim*0.25;
    // haze where the flat meets the harbour water
    col = mix(col, vec3(0.03, 0.05, 0.06), exp(-s*40.0)*0.25);
  }
  // a boat lying over on the mud, mast leaning into the sky
  if (boat > 0.5) {
    vec2 bp = vec2((uv.x - boatx)*asp, uv.y - (horizon - 0.29));
    float ca = cos(0.21); float sa = sin(0.21);
    vec2 r = vec2(ca*bp.x - sa*bp.y, sa*bp.x + ca*bp.y);
    float hull = step(r.y, 0.0)*step(dot(r/vec2(0.21, 0.085), r/vec2(0.21, 0.085)), 1.0);
    float mast = smoothstep(0.0045, 0.0025, abs(r.x + 0.02))*step(0.0, r.y)*step(r.y, 0.55);
    float stay = smoothstep(0.0016, 0.0006, abs(r.x + 0.02 - (0.55 - r.y)*0.30))*step(0.0, r.y)*step(r.y, 0.55)*step(r.x, 0.15);
    float sh = clamp(hull + mast + stay*0.8, 0.0, 1.0);
    vec3 dark = vec3(0.008, 0.009, 0.010);
    // rim light on the gunwale from the quay
    float rim = smoothstep(0.012, 0.0, abs(r.y))*step(abs(r.x), 0.21);
    float ny = r.y + 0.085;
    col = mix(col, dark + vec3(0.03, 0.035, 0.04)*smoothstep(0.0, 0.07, ny), sh);
    col += vec3(1.0, 0.55, 0.25)*smoothstep(0.010, 0.0, abs(r.y + 0.004))*step(abs(r.x), 0.21)*0.35;
    col += vec3(0.6, 0.7, 0.75)*0.04*mast;
    // shadow / wet patch under the hull
    float pud = exp(-pow((bp.x)/0.30, 2.0) - pow((bp.y + 0.06)/0.02, 2.0));
    col *= 1.0 - 0.5*pud*(1.0 - sh);
  }
  // grain, boiled every 2 frames; soft vignette
  float gr = h2(f + vec2(floor(float(iFrame)*0.5)*17.0, 3.0)) - 0.5;
  col += gr*0.022;
  vec2 vc = uv - 0.5;
  col *= 1.0 - 0.35*dot(vc*vec2(1.0, 1.3), vc*vec2(1.0, 1.3));
  o = vec4(max(col, vec3(0.0)), 1.0);
}
