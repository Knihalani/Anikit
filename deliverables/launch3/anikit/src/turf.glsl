// Turf in world space: mowing stripes, blade texture, painted lines, afternoon sun
// (grandstand shadow) or stadium lights. The camera uniforms mirror the scene camera
// so the turf moves with the vector layers drawn above it.
uniform float camX;   // = 0.0
uniform float camY;   // = 0.0
uniform float zoom;   // = 1.0
uniform float rot;    // = 0.0   degrees, same as camera rotation
uniform float night;  // = 0.0   0 afternoon sun, 1 stadium lights
uniform float seam;   // = -1.0  screen x (px) of a day|night seam; < 0: none
uniform float seamTilt; // = 0.18
uniform float sun;    // = 1.0   0: overcast (style frame C)

float h21(vec2 p){ p = fract(p*vec2(123.34, 456.21)); p += dot(p, p+45.32); return fract(p.x*p.y); }
float vn(vec2 p){ vec2 i=floor(p), f=fract(p); f=f*f*(3.0-2.0*f);
  return mix(mix(h21(i),h21(i+vec2(1,0)),f.x), mix(h21(i+vec2(0,1)),h21(i+vec2(1,1)),f.x), f.y); }
float fbm(vec2 p){ float a=0.5, s=0.0; for(int k=0;k<4;k++){ s+=a*vn(p); p=p*2.03+vec2(17.1,9.2); a*=0.5; } return s; }

// paint mask: lines every 5 yd, hash marks every yard
float paint(vec2 w){
  float Y = 36.0;
  vec2 y = (w - vec2(960.0, 540.0)) / Y;          // yards from midfield
  float lw = 0.11;                                  // 4 inch lines
  float m = 0.0;
  float d5 = abs(fract(y.x/5.0 + 0.5) - 0.5) * 5.0;  // distance to nearest 5-yd line
  if (abs(y.y) < 26.67) m = max(m, 1.0 - smoothstep(lw*0.5, lw*0.5 + 0.03, d5));
  float d1 = abs(fract(y.x + 0.5) - 0.5);
  float hy = abs(abs(y.y) - 6.6);                   // hash rows
  if (hy < 0.33) m = max(m, 1.0 - smoothstep(lw*0.5, lw*0.5 + 0.03, d1));
  float sl = abs(abs(y.y) - 26.67);                 // sidelines
  m = max(m, 1.0 - smoothstep(0.18, 0.22, sl));
  return m;
}

vec3 turf(vec2 w, float pix){
  vec2 y = (w - vec2(960.0, 540.0)) / 36.0;
  float stripe = step(0.5, fract(y.x / 10.0));                 // mowing pattern, 5 yd bands
  vec3 dark = vec3(0.105, 0.245, 0.085), lite = vec3(0.17, 0.36, 0.12);
  vec3 c = mix(dark, lite, stripe);
  float big = fbm(w / 260.0);
  c *= 0.86 + 0.28 * big;                                     // wear, mottling
  // blades: anisotropic streaks along the mowing direction, fade out when sub-pixel
  float bl = vn(vec2(w.x * 0.9, w.y * 0.22)) * 0.6 + vn(w * 1.7) * 0.4;
  float fine = 1.0 - smoothstep(0.6, 2.2, pix);                 // pix = world px per screen px
  c *= 1.0 + fine * (bl - 0.5) * 0.55;
  float worn = smoothstep(0.55, 0.85, fbm(w / 40.0 + 3.0));
  float pm = paint(w) * (1.0 - 0.45 * worn * fine) ;
  c = mix(c, vec3(0.93, 0.93, 0.88) * (0.92 + 0.12 * bl), pm * 0.92);
  return c;
}

vec3 lightDay(vec3 c, vec2 w){
  // warm low sun from upper left; the grandstand's shadow lies across the upper-left of the field
  vec2 y = (w - vec2(960.0, 540.0)) / 36.0;
  float d = dot(y, normalize(vec2(0.8, 0.6))) + 0.18 * sin(y.x * 0.35) ;
  float inShadow = 1.0 - smoothstep(-22.5, -20.5, d - 0.25 * y.y);
  vec3 sunC = vec3(1.18, 1.02, 0.80);
  vec3 sky = vec3(0.50, 0.60, 0.78);
  vec3 lit = c * sunC;
  vec3 sh  = c * sky * 0.55;
  // slight falloff toward the far (top) side
  float fall = 0.92 + 0.08 * smoothstep(-20.0, 15.0, y.y);
  // the sun pools warm toward the upper left; haze lifts the far side
  float pool = 0.86 + 0.26 * smoothstep(30.0, -25.0, dot(y, normalize(vec2(0.8, 0.6))));
  vec3 day = mix(lit * pool, sh, inShadow) * fall;
  day += vec3(0.05, 0.045, 0.03) * smoothstep(5.0, -22.0, y.y);
  return mix(c * vec3(0.95, 0.98, 1.02) * 0.92, day, sun);
}

vec3 lightNight(vec3 c, vec2 w){
  vec2 y = (w - vec2(960.0, 540.0)) / 36.0;
  vec3 acc = vec3(0.0);
  // four light towers beyond the corners of the field, cool-white
  for (int k = 0; k < 4; k++){
    float sx = (k == 0 || k == 2) ? -1.0 : 1.0;
    float sy = (k < 2) ? -1.0 : 1.0;
    vec2 L = vec2(sx * 30.0, sy * 31.0);
    float r = length(y - L);
    acc += vec3(0.84, 0.91, 1.0) * 560.0 / (r * r + 90.0);
  }
  vec3 amb = vec3(0.015, 0.025, 0.05);
  return c * (acc + amb);
}

void mainImage(out vec4 o, in vec2 f){
  vec2 s = vec2(f.x / iResolution.x * 1920.0, (1.0 - f.y / iResolution.y) * 1080.0);
  float a = radians(rot);
  vec2 q = (s - vec2(960.0, 540.0)) / zoom;
  q = mat2(cos(a), sin(a), -sin(a), cos(a)) * q;
  vec2 w = q + vec2(960.0 + camX, 540.0 + camY);
  float pix = 1.0 / zoom;
  vec3 c = turf(w, pix);
  vec3 day = lightDay(c, w), nit = lightNight(c, w);
  float n = night;
  if (seam >= 0.0) n = step(seam + (s.y - 540.0) * seamTilt, s.x);
  vec3 col = mix(day, nit, n);
  // grade: gentle filmic shoulder
  col = col / (1.0 + 0.35 * col);
  col = pow(col, vec3(0.92));
  o = vec4(col, 1.0);
}
